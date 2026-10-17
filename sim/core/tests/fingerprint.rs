//! The random worlds' fingerprint: one hash of everything a host sees of 300 base worlds (no slice's daily doings),
//! every day for 120 days. Not a gate: a refactor that mustn't change behaviour compares it before and after
//! (`just core-fingerprint` on main, then in the tree).

mod common;

use common::{world, Rng, DAY};
use std::hash::{DefaultHasher, Hash, Hasher};

#[test]
#[ignore]
fn fingerprint() {
    let mut h = DefaultHasher::new();
    for seed in 0..300u64 {
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
    println!("fingerprint {:016x}", h.finish());
}
