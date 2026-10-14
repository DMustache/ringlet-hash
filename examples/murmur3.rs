use murmur3;
use ringlet_hash::{ConsistentRing, RingConfig};
use std::hash::{BuildHasherDefault, Hasher};
use std::io::Cursor;

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

    ring.add_node(ServerNode { addr: "Foo" }, &1.to_string(), 1)
        .unwrap();
    ring.add_node(ServerNode { addr: "Bar" }, &2.to_string(), 1)
        .unwrap();

    let keys = vec!["bar_session", "baz_session"];
    for key in keys {
        if let Some(server) = ring.get_node(&key) {
            println!("Key '{}' routed to server: {}", key, server.addr);
        }
    }
}
