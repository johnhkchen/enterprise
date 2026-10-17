//! Property-based: random worlds (towns, goods, trades, people, routes, contracts) over hundreds of seeds, every day
//! of each checked for the books balancing and nobody in debt, with a few sellers selling emeralds to the mint each
//! day, and each world saved and loaded halfway. A small generator of our own, so no dependency.

mod common;

use common::{world, Rng, DAY};
use enterprise_core::{Output, Society};

/// The emeralds each of a world's sellers sells to the mint each day: a few towns, a stack or two each.
fn sellers(seed: u64) -> Vec<u64> {
    let mut r = Rng(seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);
    (0..r.below(4)).map(|_| (1 + r.below(4)) * 64 + r.below(64)).collect()
}

/// Advances `s` to day `day`, with each seller's emeralds sold to the mint at the day's start. Returns what came out.
fn run_day(s: &mut Society, daily: &[u64], day: u64) -> Vec<Output> {
    for &emeralds in daily {
        s.sell_emeralds(emeralds);
    }
    s.advance(day * DAY)
}

#[test]
fn the_books_balance_and_nobody_goes_into_debt_in_any_world() {
    for seed in 0..300 {
        let mut s = world(seed);
        let daily = sellers(seed);
        let start = s.totals();
        for day in 1..=120 {
            run_day(&mut s, &daily, day);
            if day == 60 {
                // Saved and loaded mid-run: the checks still hold after, and the loaded world saves the same blob.
                let blob = s.save();
                s = Society::load(&blob).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
                assert!(s.save() == blob, "seed {seed}: a loaded world saves as it was loaded");
            }
            if let Err(e) = s.conserved(&start).and_then(|_| s.solvent()) {
                panic!("seed {seed}, day {day}: {e}");
            }
            for t in 0..s.town_count() {
                let (roster, people) = (s.roster(t), s.town(t).people);
                assert_eq!(roster.residents() as f64, people.villagers, "seed {seed}, day {day}: town {t} follows its roster");
                assert!(roster.hands.iter().all(|h| (1..=5).contains(&h.level)), "seed {seed}, day {day}: levels 1 to 5");
            }
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_world() {
    for seed in [3, 77, 1234] {
        let (mut a, mut b) = (world(seed), world(seed));
        let daily = sellers(seed);
        let (mut oa, mut ob) = (Vec::new(), Vec::new());
        for day in 1..=90 {
            oa.extend(run_day(&mut a, &daily, day));
            ob.extend(run_day(&mut b, &daily, day));
        }
        assert_eq!(oa, ob, "seed {seed}: the same wagons");
        for t in 0..a.town_count() {
            assert_eq!(a.town(t).money.to_bits(), b.town(t).money.to_bits(), "seed {seed}, town {t}");
        }
        assert_eq!(a.mint_rate().to_bits(), b.mint_rate().to_bits(), "seed {seed}: the same mint rate");
    }
}
