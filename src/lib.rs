use std::{
    collections::HashSet,
    fmt::Debug,
    hash::{BuildHasher, Hash},
};

#[derive(Debug, Clone)]
pub struct RingNode<T> {
    hash: u64,
    node: T,
    identifier: String,
}

#[derive(Debug, Clone)]
pub struct TokenRangeMigration<T> {
    pub start_hash: u64,
    pub end_hash: u64,
    pub from_node: T,
    pub to_node: T,
}

#[derive(Clone)]
pub struct RingConfig<BH: BuildHasher> {
    pub virtual_node_base_count: usize,
    pub build_hasher: BH,
}

#[derive(Clone)]
pub struct ConsistentRing<T, BH: BuildHasher> {
    nodes: Vec<RingNode<T>>,
    virtual_node_base_count: usize,
    build_hasher: BH,
}

#[derive(Debug, Clone)]
pub enum RingError {
    EmptyRing(&'static str),
    ZeroWeight(&'static str),
}

impl<T: Clone + PartialEq + Debug, BH: BuildHasher + Clone> ConsistentRing<T, BH> {
    pub fn try_new(config: RingConfig<BH>) -> Result<Self, RingError> {
        if config.virtual_node_base_count == 0 {
            return Err(RingError::EmptyRing("Replics count must be > 0"));
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

    pub fn remove_node(&mut self, identifier: &str) {
        let hashes_to_remove: Vec<u64> = (0..self.virtual_node_base_count)
            .map(|i| {
                let virtual_node_key = self.format_virtual_node_key(identifier, i);
                self.hash_key(&virtual_node_key)
            })
            .collect();

        self.nodes
            .retain(|node| !hashes_to_remove.contains(&node.hash));
    }

    fn get_node_by_hash(&self, hash: u64) -> Option<&T> {
        if self.nodes.is_empty() {
            return None;
        }

        let start_index = self.find_start_index(hash);

        Some(&self.nodes[start_index].node)
    }

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

    pub fn get_node<K: Hash>(&self, key: &K) -> Option<&T> {
        self.get_nodes(key, 1).into_iter().next()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
