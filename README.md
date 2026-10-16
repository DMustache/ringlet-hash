# Ringlet

<!-- coverage-badges:start -->
![Test line coverage: 100%](https://img.shields.io/badge/Test%20line%20coverage-100%25-brightgreen)
![Public API docs: 100%](https://img.shields.io/badge/Public%20API%20docs-100%25-brightgreen)
<!-- coverage-badges:end -->

Ringlet is a consistent hashing library for assigning keys to physical nodes. It uses virtual nodes to distribute ownership and supports weighted nodes, multiple distinct owners per key, and migration previews when cluster membership changes.

## Quick start

The crates.io package is `ringlet-hash`; Rust imports use `ringlet_hash`.

```sh
cargo add ringlet-hash
```

```rust
use std::hash::{BuildHasherDefault, DefaultHasher};
use ringlet_hash::{ClusterManager, RingConfig};

fn main() -> Result<(), ringlet_hash::RingError> {
    let mut cluster = ClusterManager::try_new(RingConfig {
        virtual_node_base_count: 50,
        build_hasher: BuildHasherDefault::<DefaultHasher>::default(),
    })?;

    cluster.add_node("server-a", "a", 1)?;
    cluster.add_node("server-b", "b", 2)?;

    let owner = cluster.get_node(&"customer-42");
    let replicas = cluster.get_nodes(&"customer-42", 2);
    let migration = cluster.calculate_migration_on_remove("a")?;

    assert!(owner.is_some());
    assert_eq!(replicas.len(), 2);
    assert!(!migration.is_empty());
    Ok(())
}
```

`ConsistentRing` stores each physical node and its weight once; its virtual positions refer to those nodes by index. `ClusterManager` provides checked membership changes over a ring. `cluster.nodes()` returns physical nodes in insertion order, with `identifier()`, `value()`, and `weight()` accessors. Adding and looking up nodes no longer requires `T: Clone`; migration previews still require it because they return owned values. Use the same hash builder, identifiers, weights, and key types across processes that share a ring.

## Coverage

The [coverage workflow](.github/workflows/coverage-badges.yml) refreshes these badges on the default branch after successful checks. The current values are:

<!-- coverage-details:start -->
| Metric | Result | How it is measured |
| --- | ---: | --- |
| Test line coverage | 100% (257/257 lines) | `cargo llvm-cov` on library source; tests and benchmarks excluded |
| Public API documentation | 100% (33/33 items) | Rustdoc `--show-coverage` documented-item count |
<!-- coverage-details:end -->

Recheck them with:

```sh
cargo test
cargo llvm-cov --summary-only --ignore-filename-regex 'tests/|benches/'
RUSTDOCFLAGS='-D missing_docs -D rustdoc::broken_intra_doc_links' cargo doc --no-deps
RUSTDOCFLAGS='-Z unstable-options --show-coverage --output-format json' cargo +nightly doc --no-deps
```

The coverage command requires `cargo-llvm-cov` and the `llvm-tools-preview` component for the selected Rust toolchain. The documentation percentage is Rustdoc's documented-item count; it does not score the detail or accuracy of the prose. Rustdoc currently calculates that percentage with a nightly-only option.
