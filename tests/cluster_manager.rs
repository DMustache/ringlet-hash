use std::hash::{BuildHasherDefault, DefaultHasher};

use ringlet_hash::{ClusterManager, RingConfig, RingError};

type TestManager = ClusterManager<'static, &'static str, BuildHasherDefault<DefaultHasher>>;

fn manager() -> TestManager {
    TestManager::try_new(RingConfig {
        virtual_node_base_count: 5,
        build_hasher: BuildHasherDefault::default(),
    })
    .unwrap()
}

#[test]
fn manager_starts_with_an_empty_registry_and_ring() {
    let manager = manager();
    assert!(manager.is_empty());
    assert_eq!(manager.len(), 0);
    assert!(manager.nodes().is_empty());
    assert!(manager.ring().is_empty());
    assert_eq!(manager.get_node(&"key"), None);
}

#[test]
fn manager_rejects_zero_base_virtual_nodes() {
    assert!(matches!(
        TestManager::try_new(RingConfig {
            virtual_node_base_count: 0,
            build_hasher: BuildHasherDefault::default(),
        }),
        Err(RingError::EmptyRing(_))
    ));
}

#[test]
fn adding_a_node_registers_its_value_weight_and_tokens() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 3).unwrap();

    assert_eq!(manager.len(), 1);
    assert_eq!(manager.nodes()[0].identifier(), "a");
    assert_eq!(manager.nodes()[0].value(), &"alpha");
    assert_eq!(manager.nodes()[0].weight(), 3);
    assert_eq!(manager.ring().len(), 15);
    assert_eq!(manager.get_node(&"key"), Some(&"alpha"));
}

#[test]
fn duplicate_identifier_is_rejected_without_changing_membership() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 2).unwrap();

    assert!(matches!(
        manager.add_node(&"replacement", "a", 1),
        Err(RingError::NodeAlreadyExists(id)) if id == "a"
    ));
    assert_eq!(manager.nodes()[0].identifier(), "a");
    assert_eq!(manager.nodes()[0].value(), &"alpha");
    assert_eq!(manager.nodes()[0].weight(), 2);
    assert_eq!(manager.ring().len(), 10);
    assert_eq!(manager.get_node(&"key"), Some(&"alpha"));
}

#[test]
fn zero_weight_is_rejected_without_registering_a_node() {
    let mut manager = manager();
    assert!(matches!(
        manager.add_node(&"alpha", "a", 0),
        Err(RingError::ZeroWeight(_))
    ));
    assert!(manager.is_empty());
    assert!(manager.ring().is_empty());
}

#[test]
fn removing_a_weighted_node_removes_every_virtual_token() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 3).unwrap();
    manager.add_node(&"beta", "b", 1).unwrap();

    manager.remove_node("a").unwrap();
    assert_eq!(manager.len(), 1);
    assert!(!manager.ring().contains_identifier("a"));
    assert_eq!(manager.ring().len(), 5);
    for key in 0..100 {
        assert_eq!(manager.get_node(&key), Some(&"beta"));
    }
}

#[test]
fn removing_the_last_node_empties_both_registry_and_ring() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 1).unwrap();
    manager.remove_node("a").unwrap();
    assert!(manager.is_empty());
    assert!(manager.ring().is_empty());
    assert_eq!(manager.get_node(&"key"), None);
}

#[test]
fn removing_an_unknown_identifier_leaves_membership_unchanged() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 1).unwrap();
    assert!(matches!(
        manager.remove_node("missing"),
        Err(RingError::NodeNotFound(id)) if id == "missing"
    ));
    assert_eq!(manager.len(), 1);
    assert_eq!(manager.ring().len(), 5);
}

#[test]
fn lookups_delegate_to_the_ring() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 1).unwrap();
    manager.add_node(&"beta", "b", 1).unwrap();
    for key in 0..100 {
        assert_eq!(manager.get_node(&key), manager.ring().get_node(&key));
        assert_eq!(
            manager.get_nodes(&key, 2),
            manager.ring().get_nodes(&key, 2)
        );
    }
}

#[test]
fn add_preview_does_not_change_registry_or_ring() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 1).unwrap();
    let migrations = manager.calculate_migration_on_add(&"beta", "b", 2).unwrap();
    assert!(!migrations.is_empty());
    assert!(migrations.iter().all(|range| range.to_node == "beta"));
    assert_eq!(manager.len(), 1);
    assert_eq!(manager.ring().len(), 5);
    assert!(!manager.ring().contains_identifier("b"));
}

#[test]
fn add_preview_rejects_duplicate_and_zero_weight() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 1).unwrap();
    assert!(matches!(
        manager.calculate_migration_on_add(&"replacement", "a", 1),
        Err(RingError::NodeAlreadyExists(id)) if id == "a"
    ));
    assert!(matches!(
        manager.calculate_migration_on_add(&"beta", "b", 0),
        Err(RingError::ZeroWeight(_))
    ));
    assert_eq!(manager.len(), 1);
}

#[test]
fn remove_preview_does_not_change_registry_or_ring() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 2).unwrap();
    manager.add_node(&"beta", "b", 1).unwrap();
    let migrations = manager.calculate_migration_on_remove("a").unwrap();
    assert_eq!(migrations.len(), 10);
    assert!(
        migrations
            .iter()
            .all(|range| range.from_node == "alpha" && range.to_node == "beta")
    );
    assert_eq!(manager.len(), 2);
    assert_eq!(manager.ring().len(), 15);
}

#[test]
fn remove_preview_of_last_node_has_no_destination() {
    let mut manager = manager();
    manager.add_node(&"alpha", "a", 2).unwrap();
    assert!(
        manager
            .calculate_migration_on_remove("a")
            .unwrap()
            .is_empty()
    );
    assert_eq!(manager.len(), 1);
}

#[test]
fn remove_preview_rejects_an_unknown_identifier() {
    let manager = manager();
    assert!(matches!(
        manager.calculate_migration_on_remove("missing"),
        Err(RingError::NodeNotFound(id)) if id == "missing"
    ));
}
