//! The random worlds' fingerprint: one hash of everything a host sees of 300 base worlds (no slice's daily doings),
//! every day for 120 days. Not a gate: a refactor that mustn't change behaviour compares it before and after
//! (`just core-fingerprint` on main, then in the tree). The worlds run across the cores (`common::each_seed`), each
//! writing what it hashes to a [`Tape`]; the tapes are hashed in the seeds' order, a batch at a time, so the value is
//! the one a single loop over the seeds gives.

use crate::common;

use common::{world, Rng, DAY};
use std::hash::{DefaultHasher, Hash, Hasher};

/// The bytes a world writes to the hasher, in order: SipHash reads its input as one stream, so writing them after is
/// writing them then (`str`'s hash is its bytes and 0xff, a `u64`'s its native bytes, through `write` here as there).
#[derive(Default)]
struct Tape(Vec<u8>);

impl Hasher for Tape {
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
    fn finish(&self) -> u64 {
        unreachable!("a tape is replayed, not finished")
    }
}

#[test]
#[ignore]
fn fingerprint() {
    let mut h = DefaultHasher::new();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()) as u64;
    for batch in (0..300u64).collect::<Vec<_>>().chunks(4 * threads as usize) {
        for tape in common::each_seed(batch.iter().copied(), tape) {
            h.write(&tape.0);
        }
    }
    println!("fingerprint {:016x}", h.finish());
}

fn tape(seed: u64) -> Tape {
    let mut h = Tape::default();
    {
        let mut s = world(seed);
        let mut r = Rng(seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);
        let daily: Vec<u64> = (0..r.below(4)).map(|_| (1 + r.below(4)) * 64 + r.below(64)).collect();
        for day in 1..=120 {
            for &e in &daily {
                s.sell_emeralds(e);
            }
            format!("{:?}", s.advance(day * DAY)).hash(&mut h);
            for t in 0..s.town_count() {
                format!("{:?}", s.town(t)).hash(&mut h);
            }
            s.mint_rate().to_bits().hash(&mut h);
        }
        for route in 0..s.route_count() {
            format!("{:?}", s.route(route)).hash(&mut h);
        }
    }
    h
}
