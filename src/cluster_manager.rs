use std::{
    collections::HashMap,
    fmt::Debug,
    hash::{BuildHasher, Hash},
};

use crate::{ConsistentRing, RingConfig, RingError, TokenRangeMigration};

/// Owns physical node membership while the ring handles key placement.
///
/// The registry maps each unique identifier to its node value and replica
/// multiplier. Membership changes go through this type so the registry and
/// ring stay in sync.
///
/// # Example
///
/// ```
/// use std::hash::{BuildHasherDefault, DefaultHasher};
/// use ringlet::{ClusterManager, RingConfig};
///
/// let mut cluster = ClusterManager::try_new(RingConfig {
///     virtual_node_base_count: 50,
///     build_hasher: BuildHasherDefault::<DefaultHasher>::default(),
/// })?;
///
/// cluster.add_node("server-a", "a", 1)?;
/// cluster.add_node("server-b", "b", 2)?;
///
/// let moves = cluster.calculate_migration_on_remove("a")?;
/// assert!(!moves.is_empty());
///
/// cluster.remove_node("a")?;
/// assert_eq!(cluster.get_node(&"customer-42"), Some(&"server-b"));
///
/// Ok::<(), ringlet::RingError>(())
/// ```
#[derive(Clone)]
pub struct ClusterManager<T, BH: BuildHasher> {
    nodes: HashMap<String, (T, usize)>,
    ring: ConsistentRing<T, BH>,
}

impl<T: Clone + PartialEq + Debug, BH: BuildHasher + Clone> ClusterManager<T, BH> {
    /// Creates a manager with an empty ring.
    ///
    /// Returns [`RingError::EmptyRing`] if the base virtual node count is zero.
    pub fn try_new(config: RingConfig<BH>) -> Result<Self, RingError> {
        Ok(Self {
            nodes: HashMap::new(),
            ring: ConsistentRing::try_new(config)?,
        })
    }

    /// Returns the physical node registry, keyed by identifier.
    pub fn nodes(&self) -> &HashMap<String, (T, usize)> {
        &self.nodes
    }

    /// Returns the ring for read-only lookups and token inspection.
    pub fn ring(&self) -> &ConsistentRing<T, BH> {
        &self.ring
    }

    /// Adds a physical node with the given identifier and replica multiplier.
    ///
    /// Returns [`RingError::NodeAlreadyExists`] for a duplicate identifier or
    /// [`RingError::ZeroWeight`] for a zero multiplier.
    pub fn add_node(
        &mut self,
        node: T,
        identifier: &str,
        replica_multiplier: usize,
    ) -> Result<(), RingError> {
        if self.nodes.contains_key(identifier) {
            return Err(RingError::NodeAlreadyExists(identifier.to_owned()));
        }

        self.ring
            .add_node(node.clone(), identifier, replica_multiplier)?;
        self.nodes
            .insert(identifier.to_owned(), (node, replica_multiplier));
        Ok(())
    }

    /// Removes a physical node and all its virtual nodes.
    ///
    /// Returns [`RingError::NodeNotFound`] for an unknown identifier.
    pub fn remove_node(&mut self, identifier: &str) -> Result<(), RingError> {
        if !self.nodes.contains_key(identifier) {
            return Err(RingError::NodeNotFound(identifier.to_owned()));
        }

        self.ring.remove_node(identifier);
        self.nodes.remove(identifier);
        Ok(())
    }

    /// Previews migration ranges for adding a node without changing membership.
    ///
    /// Returns [`RingError::NodeAlreadyExists`] for a duplicate identifier or
    /// [`RingError::ZeroWeight`] for a zero multiplier.
    pub fn calculate_migration_on_add(
        &self,
        new_node: T,
        identifier: &str,
        replica_multiplier: usize,
    ) -> Result<Vec<TokenRangeMigration<T>>, RingError> {
        if self.nodes.contains_key(identifier) {
            return Err(RingError::NodeAlreadyExists(identifier.to_owned()));
        }

        self.ring
            .calculate_migration_on_add(new_node, identifier, replica_multiplier)
    }

    /// Previews migration ranges for removing a node without changing membership.
    ///
    /// Returns [`RingError::NodeNotFound`] for an unknown identifier. Removing
    /// the last node has no migration destination and returns an empty list.
    pub fn calculate_migration_on_remove(
        &self,
        identifier: &str,
    ) -> Result<Vec<TokenRangeMigration<T>>, RingError> {
        if !self.nodes.contains_key(identifier) {
            return Err(RingError::NodeNotFound(identifier.to_owned()));
        }

        Ok(self.ring.calculate_migration_on_remove(identifier))
    }

    /// Returns the primary owner of `key`, or `None` for an empty cluster.
    pub fn get_node<K: Hash>(&self, key: &K) -> Option<&T> {
        self.ring.get_node(key)
    }

    /// Returns up to `count` distinct physical owners for `key`.
    pub fn get_nodes<K: Hash>(&self, key: &K, count: usize) -> Vec<&T> {
        self.ring.get_nodes(key, count)
    }

    /// Returns the number of registered physical nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns whether no physical nodes are registered.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
