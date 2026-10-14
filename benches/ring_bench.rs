use criterion::{Criterion, criterion_group, criterion_main};
use ringlet_hash::{ConsistentRing, RingConfig};
use std::{
    hash::{BuildHasherDefault, DefaultHasher},
    hint::black_box,
};

fn bench_ring_lookup(c: &mut Criterion) {
    let config = RingConfig {
        virtual_node_base_count: 1_000,
        build_hasher: BuildHasherDefault::<DefaultHasher>::new(),
    };
    let mut ring = ConsistentRing::try_new(config).unwrap();

    ring.add_node("Server_A", "node_a", 2).unwrap();
    ring.add_node("Server_B", "node_b", 1).unwrap();
    ring.add_node("Server_C", "node_c", 3).unwrap();
    ring.add_node("Server_D", "node_d", 1).unwrap();
    ring.add_node("Server_E", "node_e", 2).unwrap();

    c.bench_function("consistent_ring_get_node", |b| {
        let mut counter = 0usize;
        b.iter(|| {
            counter = counter.wrapping_add(1);
            let key = format!("benchmark_key_{}", counter);
            black_box(ring.get_node(&black_box(&key)));
        });
    });
}

criterion_group!(benches, bench_ring_lookup);
criterion_main!(benches);
