use std::{
    collections::HashSet,
    hash::{BuildHasherDefault, DefaultHasher, Hash, Hasher},
};

use ringlet_hash::{ConsistentRing, RingConfig};

fn ring() -> ConsistentRing<&'static str, BuildHasherDefault<DefaultHasher>> {
    ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 50,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap()
}

fn populated_ring() -> ConsistentRing<&'static str, BuildHasherDefault<DefaultHasher>> {
    let mut ring = ring();
    ring.add_node("alpha", "a", 1).unwrap();
    ring.add_node("beta", "b", 1).unwrap();
    ring.add_node("gamma", "c", 1).unwrap();
    ring
}

fn keys() -> impl Iterator<Item = u64> {
    0..2_000
}

fn hash<K: Hash + ?Sized>(key: &K) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

fn in_range(hash: u64, start: u64, end: u64) -> bool {
    if start < end {
        hash > start && hash <= end
    } else {
        hash > start || hash <= end
    }
}

#[test]
fn a_new_ring_is_empty() {
    let ring = ring();
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
}

#[test]
fn an_empty_ring_has_no_owner_for_any_key() {
    let ring = ring();
    for key in [0, 1, u64::MAX] {
        assert_eq!(ring.get_node(&key), None);
    }
}

#[test]
fn an_empty_ring_returns_no_replicas() {
    let ring = ring();
    assert!(ring.get_nodes(&"key", 3).is_empty());
}

#[test]
fn adding_a_node_makes_the_ring_nonempty() {
    let mut ring = ring();
    ring.add_node("alpha", "a", 1).unwrap();
    assert!(!ring.is_empty());
    assert!(ring.len() > 0);
}

#[test]
fn one_node_owns_every_key() {
    let mut ring = ring();
    ring.add_node("alpha", "a", 1).unwrap();
    for key in keys() {
        assert_eq!(ring.get_node(&key), Some(&"alpha"));
    }
}

#[test]
fn different_hashable_key_types_are_supported() {
    let mut ring = ring();
    ring.add_node("alpha", "a", 1).unwrap();
    assert_eq!(ring.get_node(&"hello"), Some(&"alpha"));
    assert_eq!(ring.get_node(&42_u128), Some(&"alpha"));
    assert_eq!(ring.get_node(&(7_u8, "tuple")), Some(&"alpha"));
}

#[test]
fn zero_weight_is_rejected_without_adding_a_node() {
    let mut ring = ring();
    assert!(ring.add_node("alpha", "a", 0).is_err());
    assert!(ring.is_empty());
    assert_eq!(ring.get_node(&0), None);
}

#[test]
fn valid_weight_is_accepted() {
    let mut ring = ring();
    assert!(ring.add_node("alpha", "a", 3).is_ok());
    assert_eq!(ring.get_node(&0), Some(&"alpha"));
}

#[test]
fn node_weights_set_the_number_of_virtual_nodes() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 10,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    assert_eq!(ring.add_node("weak", "node_1", 1).unwrap(), "node_1");
    assert_eq!(ring.len(), 10);
    assert_eq!(ring.add_node("medium", "node_2", 2).unwrap(), "node_2");
    assert_eq!(ring.len(), 30);
    assert_eq!(ring.add_node("strong", "node_3", 5).unwrap(), "node_3");
    assert_eq!(ring.len(), 80);
}

#[test]
fn repeated_lookup_is_deterministic() {
    let ring = populated_ring();
    for key in keys() {
        let owner = ring.get_node(&key);
        assert_eq!(ring.get_node(&key), owner);
    }
}

#[test]
fn insertion_order_does_not_change_ownership() {
    let forward = populated_ring();
    let mut reverse = ring();
    reverse.add_node("gamma", "c", 1).unwrap();
    reverse.add_node("beta", "b", 1).unwrap();
    reverse.add_node("alpha", "a", 1).unwrap();

    for key in keys() {
        assert_eq!(forward.get_node(&key), reverse.get_node(&key), "key {key}");
    }
}

#[test]
fn adding_a_node_only_moves_keys_to_the_new_node() {
    let mut ring = populated_ring();
    let before: Vec<_> = keys().map(|key| *ring.get_node(&key).unwrap()).collect();
    ring.add_node("delta", "d", 1).unwrap();

    for (key, old_owner) in keys().zip(before) {
        let new_owner = *ring.get_node(&key).unwrap();
        assert!(new_owner == old_owner || new_owner == "delta", "key {key}");
    }
}

#[test]
fn removing_a_node_only_moves_keys_it_owned() {
    let mut ring = populated_ring();
    let before: Vec<_> = keys().map(|key| *ring.get_node(&key).unwrap()).collect();
    ring.remove_node("b");

    for (key, old_owner) in keys().zip(before) {
        let new_owner = *ring.get_node(&key).unwrap();
        assert_ne!(new_owner, "beta", "key {key}");
        if old_owner != "beta" {
            assert_eq!(new_owner, old_owner, "key {key}");
        }
    }
}

#[test]
fn removing_an_unknown_identifier_preserves_ownership() {
    let mut ring = populated_ring();
    let before: Vec<_> = keys().map(|key| *ring.get_node(&key).unwrap()).collect();
    ring.remove_node("missing");
    for (key, old_owner) in keys().zip(before) {
        assert_eq!(ring.get_node(&key), Some(&old_owner), "key {key}");
    }
}

#[test]
fn removing_the_last_node_empties_the_ring() {
    let mut ring = ring();
    ring.add_node("alpha", "a", 1).unwrap();
    ring.remove_node("a");
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
    assert_eq!(ring.get_node(&0), None);
}

#[test]
fn removing_one_of_two_nodes_leaves_only_the_other_owner() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 10,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    ring.add_node("alpha", "a", 1).unwrap();
    ring.add_node("beta", "b", 1).unwrap();
    assert_eq!(ring.len(), 20);
    ring.remove_node("a");
    assert_eq!(ring.len(), 10);
    for key in keys().take(100) {
        assert_eq!(ring.get_node(&key), Some(&"beta"));
    }
}

#[test]
fn removing_all_nodes_empties_the_ring() {
    let mut ring = populated_ring();
    for id in ["a", "b", "c"] {
        ring.remove_node(id);
    }
    assert!(ring.is_empty());
    assert_eq!(ring.get_node(&0), None);
}

#[test]
fn removing_and_readding_a_node_restores_ownership() {
    let mut ring = populated_ring();
    let before: Vec<_> = keys().map(|key| *ring.get_node(&key).unwrap()).collect();
    ring.remove_node("b");
    ring.add_node("beta", "b", 1).unwrap();
    for (key, owner) in keys().zip(before) {
        assert_eq!(ring.get_node(&key), Some(&owner), "key {key}");
    }
}

#[test]
fn requesting_zero_nodes_returns_an_empty_list() {
    let ring = populated_ring();
    assert!(ring.get_nodes(&42, 0).is_empty());
}

#[test]
fn requesting_one_node_matches_primary_owner() {
    let ring = populated_ring();
    for key in keys() {
        assert_eq!(ring.get_nodes(&key, 1), vec![ring.get_node(&key).unwrap()]);
    }
}

#[test]
fn requesting_several_nodes_returns_distinct_physical_nodes() {
    let ring = populated_ring();
    for key in keys().take(100) {
        let nodes = ring.get_nodes(&key, 3);
        assert_eq!(nodes.len(), 3, "key {key}");
        assert_eq!(nodes.iter().copied().collect::<HashSet<_>>().len(), 3);
        assert_eq!(nodes[0], ring.get_node(&key).unwrap());
    }
}

#[test]
fn requesting_more_nodes_than_exist_returns_each_once() {
    let ring = populated_ring();
    let nodes = ring.get_nodes(&42, 100);
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes.iter().copied().collect::<HashSet<_>>().len(), 3);
}

#[test]
fn cloning_a_ring_preserves_ownership() {
    let ring = populated_ring();
    let cloned = ring.clone();
    for key in keys() {
        assert_eq!(ring.get_node(&key), cloned.get_node(&key), "key {key}");
    }
}

#[test]
fn changing_a_clone_does_not_change_the_original() {
    let ring = populated_ring();
    let before: Vec<_> = keys().map(|key| *ring.get_node(&key).unwrap()).collect();
    let mut cloned = ring.clone();
    cloned.remove_node("b");
    for (key, owner) in keys().zip(before) {
        assert_eq!(ring.get_node(&key), Some(&owner), "key {key}");
    }
}

#[test]
fn migration_preview_does_not_change_the_ring() {
    let ring = populated_ring();
    let before: Vec<_> = keys().map(|key| *ring.get_node(&key).unwrap()).collect();
    let _ = ring.calculate_migration_on_add("delta", "d", 1);
    for (key, owner) in keys().zip(before) {
        assert_eq!(ring.get_node(&key), Some(&owner), "key {key}");
    }
}

#[test]
fn migration_preview_only_transfers_to_the_added_node() {
    let ring = populated_ring();
    let migrations = ring.calculate_migration_on_add("delta", "d", 1).unwrap();
    assert!(!migrations.is_empty());
    for migration in migrations {
        assert_ne!(migration.from_node, "delta");
        assert_eq!(migration.to_node, "delta");
    }
}

#[test]
fn migration_preview_keeps_existing_ring_size_and_addition_grows_it() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 5,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    ring.add_node("alpha", "a", 1).unwrap();
    ring.add_node("beta", "b", 1).unwrap();
    assert_eq!(ring.len(), 10);

    let migrations = ring.calculate_migration_on_add("gamma", "c", 1).unwrap();
    assert!(!migrations.is_empty());
    assert!(migrations.iter().all(|migration| {
        migration.from_node != migration.to_node && migration.to_node == "gamma"
    }));
    assert_eq!(ring.len(), 10);

    ring.add_node("gamma", "c", 1).unwrap();
    assert_eq!(ring.len(), 15);
    assert!(ring.get_node(&"some_test_key_123").is_some());
}

#[test]
fn find_start_index_accepts_an_exact_virtual_node_hash() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 1,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    ring.add_node("alpha", "a", 1).unwrap();
    let virtual_hash = hash("a#virtual-node-0");
    assert_eq!(ring.find_start_index(virtual_hash), 0);
}

#[test]
fn find_start_index_wraps_after_the_last_virtual_node() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 1,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    ring.add_node("alpha", "a", 1).unwrap();
    let virtual_hash = hash("a#virtual-node-0");
    if virtual_hash < u64::MAX {
        assert_eq!(ring.find_start_index(u64::MAX), 0);
    }
}

#[test]
fn migration_range_wraps_from_the_previous_virtual_node() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 1,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    ring.add_node("alpha", "a", 1).unwrap();
    let old_hash = hash("a#virtual-node-0");

    let new_identifier = (0..10_000)
        .map(|i| format!("candidate_{i}"))
        .find(|identifier| hash(&format!("{identifier}#virtual-node-0")) < old_hash)
        .expect("find a virtual node before the existing one");
    let new_hash = hash(&format!("{new_identifier}#virtual-node-0"));

    let migrations = ring
        .calculate_migration_on_add("beta", &new_identifier, 1)
        .unwrap();
    assert_eq!(migrations.len(), 1);
    assert_eq!(migrations[0].start_hash, old_hash);
    assert_eq!(migrations[0].end_hash, new_hash);
    assert_eq!(migrations[0].from_node, "alpha");
    assert_eq!(migrations[0].to_node, "beta");
}

#[test]
fn migration_preview_from_one_node_names_that_node_as_source() {
    let mut ring = ring();
    ring.add_node("alpha", "a", 1).unwrap();
    let migrations = ring.calculate_migration_on_add("beta", "b", 1).unwrap();
    assert!(!migrations.is_empty());
    for migration in migrations {
        assert_eq!(migration.from_node, "alpha");
        assert_eq!(migration.to_node, "beta");
    }
}

#[test]
fn migration_preview_of_an_empty_ring_has_no_source() {
    let ring = ring();
    assert!(
        ring.calculate_migration_on_add("alpha", "a", 1)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn migration_preview_for_zero_weight_is_empty() {
    let ring = populated_ring();
    assert!(ring.calculate_migration_on_add("delta", "d", 0).is_err());
}

#[test]
fn remove_preview_on_an_empty_ring_has_no_ranges() {
    assert!(ring().calculate_migration_on_remove("a").is_empty());
}

#[test]
fn remove_preview_for_an_unknown_identifier_has_no_ranges() {
    assert!(
        populated_ring()
            .calculate_migration_on_remove("missing")
            .is_empty()
    );
}

#[test]
fn remove_preview_of_the_last_node_has_no_destination() {
    let mut ring = ring();
    ring.add_node("alpha", "a", 2).unwrap();
    assert!(ring.calculate_migration_on_remove("a").is_empty());
    assert_eq!(ring.len(), 100);
}

#[test]
fn remove_preview_describes_the_keys_that_change_owner() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 5,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    ring.add_node("alpha", "a", 2).unwrap();
    ring.add_node("beta", "b", 1).unwrap();
    ring.add_node("gamma", "c", 1).unwrap();

    let preview = ring.calculate_migration_on_remove("a");
    assert_eq!(preview.len(), 10);
    assert_eq!(ring.len(), 20);
    let mut future_ring = ring.clone();
    future_ring.remove_node("a");
    assert_eq!(future_ring.len(), 10);

    for key in 0..2_000_u64 {
        let old_owner = *ring.get_node(&key).unwrap();
        let new_owner = *future_ring.get_node(&key).unwrap();
        let matching: Vec<_> = preview
            .iter()
            .filter(|range| in_range(hash(&key), range.start_hash, range.end_hash))
            .collect();

        if old_owner == "alpha" {
            assert_eq!(matching.len(), 1, "key {key}");
            assert_eq!(matching[0].from_node, old_owner, "key {key}");
            assert_eq!(matching[0].to_node, new_owner, "key {key}");
        } else {
            assert!(matching.is_empty(), "key {key}");
            assert_eq!(new_owner, old_owner, "key {key}");
        }
    }
}

#[test]
fn remove_preview_uses_the_previous_token_across_the_ring_boundary() {
    let mut ring = ConsistentRing::try_new(RingConfig {
        virtual_node_base_count: 1,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    })
    .unwrap();
    ring.add_node("alpha", "a", 1).unwrap();
    ring.add_node("beta", "b", 1).unwrap();

    let ranges = ring.calculate_migration_on_remove("a");
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].start_hash, hash("b#virtual-node-0"));
    assert_eq!(ranges[0].end_hash, hash("a#virtual-node-0"));
    assert_eq!(ranges[0].from_node, "alpha");
    assert_eq!(ranges[0].to_node, "beta");
}
