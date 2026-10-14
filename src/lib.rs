//! Consistent hashing for assigning hashable keys to nodes on a ring.
//!
//! Each physical node is represented by `virtual_node_base_count * replica_multiplier`
//! virtual nodes. Adding or removing a physical node generally changes the owner of
//! only part of the key space. Use [`ConsistentRing::get_nodes`] when a key needs
//! several distinct physical nodes.
//!
//! All processes that share a ring must use the same node identifiers, weights,
//! [`BuildHasher`] implementation, and [`Hash`] representation for keys. Otherwise
//! they may disagree about ownership.
//!
//! [`ClusterManager`] keeps a registry of physical nodes and their weights in
//! sync with a [`ConsistentRing`]. Use it when managing cluster membership.
//!
//! # Example
//!
//! ```
//! extern crate ringlet_hash;
//! use ringlet_hash::{ConsistentRing, RingError, RingConfig};
//! use std::hash::{BuildHasherDefault, DefaultHasher};
//!
//! let config = RingConfig {
//!     virtual_node_base_count: 50,
//!     build_hasher: BuildHasherDefault::<DefaultHasher>::default(),
//! };
//! let mut ring = ConsistentRing::try_new(config)?;
//! ring.add_node("server-a", "a", 1)?;
//! ring.add_node("server-b", "b", 1)?;
//!
//! let owner = ring.get_node(&"customer-42").unwrap();
//! assert!(["server-a", "server-b"].contains(owner));
//! assert_eq!(ring.get_nodes(&"customer-42", 2).len(), 2);
//! # Ok::<(), RingError>(())
//! ```

use std::{
    collections::HashSet,
    fmt::Debug,
    hash::{BuildHasher, Hash},
};

mod cluster_manager;

pub use cluster_manager::ClusterManager;

/// A virtual node placed at a hash position on the ring.
///
/// Ring nodes are managed by [`ConsistentRing`]; their fields are private.
#[derive(Debug, Clone)]
pub struct RingNode<T> {
    hash: u64,
    node: T,
    identifier: String,
}

/// A proposed token interval to move when adding or removing a physical node.
///
/// Hash positions wrap around at [`u64::MAX`], so `start_hash` can be greater
/// than `end_hash`.
#[derive(Debug, Clone)]
pub struct TokenRangeMigration<T> {
    /// Hash at the start of the interval.
    pub start_hash: u64,
    /// Hash at the end of the interval.
    pub end_hash: u64,
    /// Owner of the interval before the membership change.
    pub from_node: T,
    /// Owner of the interval after the membership change.
    pub to_node: T,
}

/// Settings used to construct a [`ConsistentRing`].
#[derive(Clone)]
pub struct RingConfig<BH: BuildHasher> {
    /// Number of virtual nodes created for a physical node with weight 1.
    /// Must be greater than zero.
    pub virtual_node_base_count: usize,
    /// Hash builder used for both keys and virtual node identifiers.
    pub build_hasher: BH,
}

/// A consistent hashing ring of physical nodes of type `T`.
///
/// The ring stores cloned values of `T` at its virtual node positions. Its
/// [`len`](Self::len) counts virtual nodes, not physical nodes.
#[derive(Clone)]
pub struct ConsistentRing<T, BH: BuildHasher> {
    nodes: Vec<RingNode<T>>,
    virtual_node_base_count: usize,
    build_hasher: BH,
}

/// Error returned for an invalid ring configuration or node weight.
#[derive(Debug, Clone)]
pub enum RingError {
    /// The configured base virtual node count is zero.
    EmptyRing(&'static str),
    /// A node was added or previewed with a zero replica multiplier.
    ZeroWeight(&'static str),
    /// A physical node with this identifier is already registered.
    NodeAlreadyExists(String),
    /// No physical node has this identifier.
    NodeNotFound(String),
}

impl<T: Clone + PartialEq + Debug, BH: BuildHasher + Clone> ConsistentRing<T, BH> {
    /// Creates an empty ring with the supplied configuration.
    ///
    /// Returns [`RingError::EmptyRing`] if `virtual_node_base_count` is zero.
    pub fn try_new(config: RingConfig<BH>) -> Result<Self, RingError> {
        if config.virtual_node_base_count == 0 {
            return Err(RingError::EmptyRing("Virtual nodes count must be > 0"));
        }
        Ok(Self {
            nodes: Vec::new(),
            virtual_node_base_count: config.virtual_node_base_count,
            build_hasher: config.build_hasher,
        })
    }

    fn hash_key<K: Hash>(&self, key: &K) -> u64 {
        self.build_hasher.hash_one(key)
    }

    fn format_virtual_node_key(&self, identifier: &str, i: usize) -> String {
        format!("{identifier}#virtual-node-{i}")
    }

    /// Adds a physical node and returns its identifier.
    ///
    /// The node gets `virtual_node_base_count * replica_multiplier` positions.
    /// Give each physical node a unique identifier; this method does not reject
    /// duplicate identifiers. Returns [`RingError::ZeroWeight`] for a zero
    /// multiplier.
    pub fn add_node(
        &mut self,
        node: T,
        identifier: &str,
        replica_multiplier: usize,
    ) -> Result<String, RingError> {
        if replica_multiplier == 0 {
            return Err(RingError::ZeroWeight("Replica multiplier must be > 0"));
        }

        let virtual_nodes_count = self.virtual_node_base_count * replica_multiplier;

        for i in 0..virtual_nodes_count {
            let virtual_node_key = self.format_virtual_node_key(identifier, i);
            let hash = self.hash_key(&virtual_node_key);

            self.nodes.push(RingNode {
                hash,
                node: node.clone(),
                identifier: identifier.to_string().clone(),
            });
        }

        self.nodes.sort_unstable_by_key(|ring_node| ring_node.hash);

        Ok(identifier.to_string())
    }

    /// Removes all virtual nodes for `identifier`; an unknown identifier has no effect.
    pub fn remove_node(&mut self, identifier: &str) {
        self.nodes.retain(|node| node.identifier != identifier);
    }

    fn get_node_by_hash(&self, hash: u64) -> Option<&T> {
        if self.nodes.is_empty() {
            return None;
        }

        let start_index = self.find_start_index(hash);

        Some(&self.nodes[start_index].node)
    }

    /// Returns the index of the first virtual node at or after `target_hash`.
    ///
    /// The search wraps to index 0 beyond the last token. An empty ring also
    /// returns 0; check [`is_empty`](Self::is_empty) before indexing.
    pub fn find_start_index(&self, target_hash: u64) -> usize {
        match self
            .nodes
            .binary_search_by(|ring_node| ring_node.hash.cmp(&target_hash))
        {
            Ok(exact_match) => exact_match,
            Err(some_position) => {
                if some_position == self.nodes.len() {
                    0
                } else {
                    some_position
                }
            }
        }
    }

    /// Returns up to `count` distinct physical nodes for a key, in ring order.
    ///
    /// The first result is the primary owner returned by [`get_node`](Self::get_node).
    /// The result is empty for an empty ring or a zero count, and may contain
    /// fewer than `count` nodes when the ring has fewer distinct identifiers.
    pub fn get_nodes<K: Hash>(&self, key: &K, count: usize) -> Vec<&T> {
        if self.is_empty() || count == 0 {
            return Vec::new();
        }

        let target_hash = self.hash_key(key);

        let start_index = self.find_start_index(target_hash);

        let mut result = Vec::new();
        let mut seen_indentifiers = HashSet::new();

        for i in 0..self.len() {
            let current_index = (start_index + i) % self.len();
            let ring_node = &self.nodes[current_index];

            if seen_indentifiers.insert(&ring_node.identifier) {
                result.push(&ring_node.node);
                if result.len() == count {
                    break;
                }
            }
        }

        result
    }

    /// Previews token ranges associated with adding a node without changing this ring.
    ///
    /// Each returned entry identifies an existing owner at a new virtual token
    /// and the proposed destination. An empty ring has no source to migrate
    /// from, so it returns an empty list. A zero multiplier returns
    /// [`RingError::ZeroWeight`].
    pub fn calculate_migration_on_add(
        &self,
        new_node: T,
        new_identifier: &str,
        new_replica_multiplier: usize,
    ) -> Result<Vec<TokenRangeMigration<T>>, RingError> {
        let mut future_ring = self.clone();

        future_ring.add_node(new_node.clone(), new_identifier, new_replica_multiplier)?;

        let mut migrations = Vec::new();
        for i in 0..future_ring.len() {
            let current_virtual_node = &future_ring.nodes[i];

            if current_virtual_node.identifier == new_identifier {
                let old_node = self.get_node_by_hash(current_virtual_node.hash);

                if let Some(from_node) = old_node {
                    let previous_virtual_node = if i == 0 { future_ring.len() - 1 } else { i - 1 };

                    let start_hash = future_ring.nodes[previous_virtual_node].hash;
                    let end_hash = current_virtual_node.hash;

                    migrations.push(TokenRangeMigration {
                        start_hash,
                        end_hash,
                        from_node: from_node.clone(),
                        to_node: new_node.clone(),
                    });
                }
            }
        }

        Ok(migrations)
    }

    /// Previews token ranges that would leave `identifier` on removal.
    ///
    /// Each range is `(start_hash, end_hash]` on the hash ring, wrapping at
    /// [`u64::MAX`]. The ring is unchanged. Returns an empty list when the
    /// identifier is absent or when removal would leave no destination node.
    pub fn calculate_migration_on_remove(&self, identifier: &str) -> Vec<TokenRangeMigration<T>> {
        if self.nodes.is_empty() {
            return Vec::new();
        }

        let mut future_ring = self.clone();
        future_ring.remove_node(identifier);
        if future_ring.is_empty() {
            return Vec::new();
        }

        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.identifier == identifier)
            .map(|(index, node)| {
                let previous_index = if index == 0 {
                    self.nodes.len() - 1
                } else {
                    index - 1
                };

                TokenRangeMigration {
                    start_hash: self.nodes[previous_index].hash,
                    end_hash: node.hash,
                    from_node: node.node.clone(),
                    to_node: future_ring
                        .get_node_by_hash(node.hash)
                        .expect("a destination exists after removal")
                        .clone(),
                }
            })
            .collect()
    }

    /// Returns the primary owner of `key`, or `None` for an empty ring.
    pub fn get_node<K: Hash>(&self, key: &K) -> Option<&T> {
        self.get_nodes(key, 1).into_iter().next()
    }

    /// Returns the number of virtual nodes in the ring.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns whether the ring has no virtual nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
