use std::{
    hash::{BuildHasherDefault, Hasher},
    io::Cursor,
};

use ringlet_hash::{ConsistentRing, RingConfig};

#[derive(Clone)]
pub struct Murmur3Adapter {
    buffer: Vec<u8>,
    seed: u32,
}

impl Murmur3Adapter {
    pub fn new(seed: u32) -> Self {
        Self {
            buffer: Vec::with_capacity(64),
            seed,
        }
    }
}

impl Default for Murmur3Adapter {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Hasher for Murmur3Adapter {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    fn finish(&self) -> u64 {
        let mut cursor = Cursor::new(&self.buffer);
        match murmur3::murmur3_x64_128(&mut cursor, self.seed) {
            Ok(full_hash) => full_hash as u64,
            Err(_) => 0,
        }
    }
}

pub type Murmur3BuildHasher = BuildHasherDefault<Murmur3Adapter>;

#[allow(unused)]
#[derive(Clone, Debug, PartialEq)]
struct ServerNode {
    addr: &'static str,
}

fn main() {
    let config = RingConfig {
        virtual_node_base_count: 128,
        build_hasher: Murmur3BuildHasher::default(),
    };
    let mut ring = ConsistentRing::try_new(config).unwrap();

    ring.add_node("server_A", &1.to_string(), 1).unwrap();
    ring.add_node("server_B", &2.to_string(), 1).unwrap();

    let mut hits_a = 0;
    let mut hits_b = 0;
    let total_keys = 100_000;

    for i in 0..total_keys {
        let key = format!("user_key_{}", i);
        match ring.get_node(&key) {
            Some(server) if *server == "server_A" => hits_a += 1,
            Some(server) if *server == "server_B" => hits_b += 1,
            _ => panic!("Unknown"),
        }
    }

    let ratio_b = hits_b as f64 / hits_a as f64;
    println!("Server A hits: {}, Server B hits: {}", hits_a, hits_b);
    println!("Ratio B/A: {:.2} (expected ~2.0)", ratio_b);
}
