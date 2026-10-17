//! Property-based: random worlds (towns, goods, trades, people, routes, contracts) over hundreds of seeds, every day
//! of each checked for the books balancing and nobody in debt, with a few sellers selling emeralds to the mint each
//! day, and each world saved and loaded halfway. A small generator of our own, so no dependency.

mod common;

use enterprise_core::cash_book::{Account, Reason};

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

/// A day's spending at towns' own choosing, through the Accountant and the cash book: some ask approval for a work
/// out of the world, some take what the budget allows for their households, the rest spend nothing.
fn spend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        match r.below(4) {
            0 => {
                let cost = r.range(0.0, 30.0);
                if s.approve(t, "works", cost) {
                    let cost = cost.min(s.town(t).money.max(0.0));
                    s.post(Account::Treasury(t), Account::Outside, cost, Reason::Other("works".into()));
                }
            }
            1 => {
                let gift = s.allow(t, "a feast", r.range(0.0, 10.0)).min(s.town(t).money.max(0.0));
                s.post(Account::Treasury(t), Account::Households(t), gift, Reason::Other("a feast".into()));
            }
            _ => {}
        }
    }
}

#[test]
fn the_books_balance_and_nobody_goes_into_debt_in_any_world() {
    let (mut roads, mut detours, mut road_trips) = (0, 0, 0);
    for seed in 0..300 {
        let mut s = world(seed);
        let daily = sellers(seed);
        let start = s.totals();
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut at_90 = Vec::new();
        for day in 1..=120 {
            if day == 90 {
                // The last look of day 89 (a look is every 1,200 ticks, so this adds none).
                s.advance(day * DAY - 1200);
                at_90 = (0..s.town_count()).map(|t| s.town(t).money).collect();
            }
            run_day(&mut s, &daily, day);
            spend(&mut s, &mut r);
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
        for route in 0..s.route_count() {
            if let Some(path) = s.route_path(route) {
                roads += 1;
                detours += (path.len() as i32 > path[0].distance(*path.last().unwrap()) + 1) as u32;
                road_trips += s.route(route).trips;
            }
        }
        // Each town's book explains its treasury: days 90 to 120's entries are what its money moved since day 89.
        for t in 0..s.town_count() {
            let booked: f64 = s.cash_book(t, 31).iter().map(|(_, _, v)| v).sum();
            let moved = s.town(t).money - at_90[t];
            assert!((booked - moved).abs() < 1e-6 * moved.abs().max(1.0), "seed {seed}, town {t}: {booked} {moved}");
        }
    }
    // The map is exercised: roads are built, some go round the water and hills, and wagons run on them.
    assert!(roads > 300 && detours > 30 && road_trips > 1000, "{roads} roads, {detours} detours, {road_trips} trips");
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
