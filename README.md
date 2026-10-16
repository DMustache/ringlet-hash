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

For rings shared across processes or kept across deployments, do not use [`DefaultHasher`](https://doc.rust-lang.org/std/collections/hash_map/struct.DefaultHasher.html): its algorithm and hashes may change between Rust releases. Use an explicitly named algorithm and the same seed on every member. This example uses XXH3 with seed `0`; add its crate alongside Ringlet:

```sh
cargo add xxhash-rust@=0.8.16 --features xxh3
```

```rust
use xxhash_rust::xxh3::Xxh3Builder;
use ringlet_hash::{ClusterManager, RingConfig};

fn main() -> Result<(), ringlet_hash::RingError> {
    let server_a = String::from("server-a");
    let server_b = String::from("server-b");
    let mut cluster = ClusterManager::try_new(RingConfig {
        virtual_node_base_count: 50,
        build_hasher: Xxh3Builder::new().with_seed(0),
    })?;

    cluster.add_node(&server_a, "a", 1)?;
    cluster.add_node(&server_b, "b", 2)?;

    let owner = cluster.get_node(&"customer-42");
    let replicas = cluster.get_nodes(&"customer-42", 2);
    let migration = cluster.calculate_migration_on_remove("a")?;

    assert!(owner.is_some());
    assert_eq!(replicas.len(), 2);
    assert!(!migration.is_empty());
    Ok(())
}
```

`ConsistentRing` borrows each physical node and stores its identifier and weight once; its virtual positions refer to those entries by index. Keep node values alive while the ring uses them. `ClusterManager` provides checked membership changes over a ring. `cluster.nodes()` returns physical nodes in insertion order, with `identifier()`, `value()`, and `weight()` accessors. Adding and looking up nodes does not require `T: Clone`; migration previews still require it because they return owned values. Use the same hash builder, identifiers, weights, and key types across processes that share a ring.

The named hasher alone does not make placement portable. Rust's [`Hash` trait](https://doc.rust-lang.org/std/hash/trait.Hash.html#portability) does not promise the same bytes across compiler versions or platforms, and Ringlet hashes both keys and virtual node identifier strings through that trait. For a shared ring, use the same Rust version, target architecture, key types, hasher implementation version, and seed on every member. If placement must survive those changes, define and test a canonical byte encoding and hashing scheme before relying on it. XXH3 is noncryptographic; use a specified keyed hash if untrusted clients can choose keys to skew placement.

## How the ring works

Ringlet hashes a key to a position on a 64-bit circle and assigns it to the first virtual node at or after that position, wrapping around at the end. A physical node has `virtual_node_base_count * replica_multiplier` virtual positions, so the multiplier influences its expected share of keys; it does not guarantee an exact share. `get_nodes` walks clockwise and skips repeat physical nodes. Migration previews report proposed token intervals; the caller is responsible for moving data.

## Choosing counts and weights

These three settings serve different purposes:

| Setting | Starting point | What to check |
| --- | --- | --- |
| `virtual_node_base_count` | Start with `50` as in the example; compare with `100` if placement is too uneven. | Larger values usually smooth out placement but create more ring entries and work during membership changes. There is no universal best count. |
| `replica_multiplier` in `add_node` | Use `1` for equal-capacity nodes; try `2` for a node intended to receive about twice the key share. | This is a relative weight, not a data replica count. For weights `1`, `2`, and `1`, the intended shares are roughly 25%, 50%, and 25%, not guaranteed quotas. |
| `count` in `get_nodes` | Use `1` for a primary owner; request `2` or `3` only if the application will store copies on that many distinct physical nodes. | Have at least `count` physical nodes if the application requires exactly that many owners. |

The library rejects zero values for `virtual_node_base_count` and `replica_multiplier`, and rejects duplicate identifiers. It does not verify that weights match actual capacity, that the resulting key distribution is balanced, or that enough physical nodes exist for a requested `get_nodes` count. If fewer distinct nodes exist, `get_nodes` returns the available nodes without an error. It selects owners; it does not copy data, enforce durability, or check failure domains. The application must check its replica policy and measure placement with representative keys before relying on it. Keep `virtual_node_base_count * replica_multiplier` within `usize` and a practical memory budget; the library does not cap or check that product.

## Further reading

- [Consistent hashing on Wikipedia](https://en.wikipedia.org/wiki/Consistent_hashing) is a short introduction to ring placement and minimal remapping.
- [Karger et al., *Consistent Hashing and Random Trees*](https://people.csail.mit.edu/karger/Papers/web.pdf) introduces the consistent hashing idea and its behavior as nodes change.
- [DeCandia et al., *Dynamo: Amazon's Highly Available Key-value Store*](https://www.amazon.science/publications/dynamo-amazons-highly-available-key-value-store), sections 4.2–4.3, discusses virtual nodes, weighted capacity, and distinct replica owners. Dynamo is a larger storage system; Ringlet implements placement only.
- [RFC 4981, section 3.1.2](https://datatracker.ietf.org/doc/html/rfc4981#section-3.1.2) surveys consistent hashing in peer-to-peer systems. It is background reading, not a wire format or compatibility specification for Ringlet.

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
