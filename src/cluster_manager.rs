use std::hash::{BuildHasher, Hash};

use crate::{ConsistentRing, PhysicalRingNode, RingConfig, RingError, TokenRangeMigration};

/// Manages physical node membership through a [`ConsistentRing`].
///
/// The ring borrows each physical value and stores its weight once. Callers
/// must keep values alive while the manager uses them. This type provides
/// checked membership changes and delegates placement to the ring.
///
/// # Example
///
/// ```
/// use xxhash_rust::xxh3::Xxh3Builder;
/// use ringlet_hash::{ClusterManager, RingConfig};
///
/// let mut cluster = ClusterManager::try_new(RingConfig {
///     virtual_node_base_count: 50,
///     build_hasher: Xxh3Builder::new().with_seed(0),
/// })?;
///
/// let server_a = String::from("server-a");
/// let server_b = String::from("server-b");
/// cluster.add_node(&server_a, "a", 1)?;
/// cluster.add_node(&server_b, "b", 2)?;
///
/// let moves = cluster.calculate_migration_on_remove("a")?;
/// assert!(!moves.is_empty());
///
/// cluster.remove_node("a")?;
/// assert_eq!(cluster.get_node(&"customer-42"), Some(&server_b));
///
/// Ok::<(), ringlet_hash::RingError>(())
/// ```
#[derive(Clone)]
pub struct ClusterManager<'a, T, BH: BuildHasher> {
    ring: ConsistentRing<'a, T, BH>,
}

impl<'a, T, BH: BuildHasher> ClusterManager<'a, T, BH> {
    /// Creates a manager with an empty ring.
    ///
    /// Returns [`RingError::EmptyRing`] if the base virtual node count is zero.
    pub fn try_new(config: RingConfig<BH>) -> Result<Self, RingError> {
        Ok(Self {
            ring: ConsistentRing::try_new(config)?,
        })
    }

    /// Returns physical nodes and weights in insertion order.
    pub fn nodes(&self) -> &[PhysicalRingNode<'a, T>] {
        self.ring.physical_nodes()
    }

    /// Returns the ring for read-only lookups and token inspection.
    pub fn ring(&self) -> &ConsistentRing<'a, T, BH> {
        &self.ring
    }

    /// Adds a physical node with the given identifier and replica multiplier.
    ///
    /// The multiplier is a relative placement weight, not a replica count.
    /// The caller must choose a weight that reflects the node's capacity.
    /// Returns [`RingError::NodeAlreadyExists`] for a duplicate identifier or
    /// [`RingError::ZeroWeight`] for a zero multiplier.
    pub fn add_node(
        &mut self,
        node: &'a T,
        identifier: &str,
        replica_multiplier: usize,
    ) -> Result<(), RingError> {
        self.ring.add_node(node, identifier, replica_multiplier)?;
        Ok(())
    }

    /// Removes a physical node and all its virtual nodes.
    ///
    /// Returns [`RingError::NodeNotFound`] for an unknown identifier.
    pub fn remove_node(&mut self, identifier: &str) -> Result<(), RingError> {
        if !self.ring.contains_identifier(identifier) {
            return Err(RingError::NodeNotFound(identifier.to_owned()));
        }

        self.ring.remove_node(identifier);
        Ok(())
    }

    /// Previews migration ranges for adding a node without changing membership.
    ///
    /// Returns [`RingError::NodeAlreadyExists`] for a duplicate identifier or
    /// [`RingError::ZeroWeight`] for a zero multiplier.
    pub fn calculate_migration_on_add(
        &self,
        new_node: &'a T,
        identifier: &str,
        replica_multiplier: usize,
    ) -> Result<Vec<TokenRangeMigration<T>>, RingError>
    where
        T: Clone,
        BH: Clone,
    {
        if self.ring.contains_identifier(identifier) {
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
    ) -> Result<Vec<TokenRangeMigration<T>>, RingError>
    where
        T: Clone,
        BH: Clone,
    {
        if !self.ring.contains_identifier(identifier) {
            return Err(RingError::NodeNotFound(identifier.to_owned()));
        }

        Ok(self.ring.calculate_migration_on_remove(identifier))
    }

    /// Returns the primary owner of `key`, or `None` for an empty cluster.
    pub fn get_node<K: Hash>(&self, key: &K) -> Option<&'a T> {
        self.ring.get_node(key)
    }

    /// Returns up to `count` distinct physical owners for `key`.
    /// The caller must check the returned length and store any desired copies.
    pub fn get_nodes<K: Hash>(&self, key: &K, count: usize) -> Vec<&'a T> {
        self.ring.get_nodes(key, count)
    }

    /// Returns the number of registered physical nodes.
    pub fn len(&self) -> usize {
        self.ring.physical_nodes().len()
    }

    /// Returns whether no physical nodes are registered.
    pub fn is_empty(&self) -> bool {
        self.ring.physical_nodes().is_empty()
    }
}
