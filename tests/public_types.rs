use std::hash::{BuildHasherDefault, DefaultHasher};

use ringlet_hash::{ConsistentRing, RingConfig, RingError, TokenRangeMigration};

#[test]
fn migration_exposes_its_range_and_endpoints() {
    let migration = TokenRangeMigration {
        start_hash: 10,
        end_hash: 20,
        from_node: "old",
        to_node: "new",
    };

    assert_eq!(migration.start_hash, 10);
    assert_eq!(migration.end_hash, 20);
    assert_eq!(migration.from_node, "old");
    assert_eq!(migration.to_node, "new");
}

#[test]
fn cloning_a_migration_preserves_its_values() {
    let original = TokenRangeMigration {
        start_hash: u64::MAX,
        end_hash: 0,
        from_node: String::from("old"),
        to_node: String::from("new"),
    };

    let cloned = original.clone();
    assert_eq!(cloned.start_hash, original.start_hash);
    assert_eq!(cloned.end_hash, original.end_hash);
    assert_eq!(cloned.from_node, original.from_node);
    assert_eq!(cloned.to_node, original.to_node);
}

#[test]
fn zero_replicas_are_rejected_with_an_empty_ring_error() {
    assert!(matches!(
        ConsistentRing::<bool, BuildHasherDefault<DefaultHasher>>::try_new(RingConfig {
            virtual_node_base_count: 0,
            build_hasher: BuildHasherDefault::<DefaultHasher>::new()
        }),
        Err(RingError::EmptyRing(_))
    ));
}
