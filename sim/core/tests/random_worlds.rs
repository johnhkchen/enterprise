//! Property-based: random worlds (towns, goods, trades, people, routes, contracts) over hundreds of seeds, every day
//! of each checked for the books balancing and nobody in debt, with a few sellers selling emeralds to the mint each
//! day, each slice's doings and checks (`common/slices.rs`, the sweep's invariants among them), and each world saved
//! and loaded every 30 days, the loaded one running on to the bit. The seeds run across the machine's cores
//! (`common::each_seed`); what the worlds did is printed (`--nocapture`). A small generator of our own, so no
//! dependency.

use crate::common;

use common::{sellers, world, DAY};
use enterprise_core::sweep::Sweep;
use enterprise_core::{Output, Society};

/// Advances `s` to day `day`, with each seller's emeralds sold to the mint at the day's start. Returns what came out.
fn run_day(s: &mut Society, daily: &[u64], day: u64) -> Vec<Output> {
    for &emeralds in daily {
        s.sell_emeralds(emeralds);
    }
    s.advance(day * DAY)
}

/// What one world did, for the totals.
#[derive(Default)]
struct Ran {
    roads: u32,
    detours: u32,
    road_trips: u32,
    sweep: Sweep,
}

/// One world, 120 days: each day the sellers sell, the day runs, each slice's doings, then the checks (conserved,
/// solvent, every slice's invariants). Every 30 days it's saved and loaded: the loaded world saves the same blob, runs
/// the next day alongside the one never saved to the same blob, and carries on in its place.
fn run(seed: u64) -> Ran {
    let mut s = world(seed);
    let daily = sellers(seed);
    let start = s.totals();
    let mut ran = Ran { sweep: Sweep::new(), ..Default::default() };
    let mut twin: Option<Society> = None;
    for day in 1..=120 {
        let out = run_day(&mut s, &daily, day);
        common::slices::daily(&mut s, seed, day);
        if let Some(mut loaded) = twin.take() {
            let twin_out = run_day(&mut loaded, &daily, day);
            common::slices::daily(&mut loaded, seed, day);
            assert_eq!(twin_out, out, "seed {seed}, day {day}: a world loaded the day before sends the same wagons");
            let blob = s.save();
            assert!(loaded.save() == blob, "seed {seed}, day {day}: a world loaded the day before runs on to the bit");
            s = loaded;
        }
        if let Err(e) = s.conserved(&start).and_then(|_| s.solvent()).and_then(|_| common::slices::check(&s, day)) {
            panic!("seed {seed}, day {day}: {e}");
        }
        ran.sweep.day(&s);
        if day % 30 == 0 {
            let blob = s.save();
            let mut loaded = Society::load(&blob).unwrap_or_else(|e| panic!("seed {seed}, day {day}: {e}"));
            assert!(loaded.save() == blob, "seed {seed}, day {day}: a loaded world saves as it was loaded");
            twin = Some(loaded);
        }
    }
    ran.sweep.end(&s);
    for route in 0..s.route_count() {
        if let Some(path) = s.route_path(route) {
            ran.roads += 1;
            ran.detours += (path.len() as i32 > path[0].distance(*path.last().unwrap()) + 1) as u32;
            ran.road_trips += s.route(route).trips;
        }
    }
    ran
}

#[test]
fn the_books_balance_and_nobody_goes_into_debt_in_any_world() {
    let (mut roads, mut detours, mut road_trips) = (0, 0, 0);
    let mut sweep = Sweep::new();
    for ran in common::each_seed(common::seeds(300), run) {
        roads += ran.roads;
        detours += ran.detours;
        road_trips += ran.road_trips;
        sweep.merge(ran.sweep);
    }
    println!("{}", sweep.report());
    // The map is exercised: roads are built, some go round the water and hills, and wagons run on them.
    assert!(roads > 300 && detours > 30 && road_trips > 1000, "{roads} roads, {detours} detours, {road_trips} trips");
    // Every mechanism is on somewhere: contracts complete and break, farms pay wages, banks, hubs, a war.
    assert!(sweep.contracts[2] > 0 && sweep.contracts[3] > 0 && sweep.farms > 0 && sweep.wages_paid > 0.0
        && sweep.banks > 0 && sweep.hubs > 0 && sweep.war_days > 0 && sweep.harvests > 0 && sweep.levies > 0,
        "{}", sweep.report());
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
            assert_eq!(a.town(t).money, b.town(t).money, "seed {seed}, town {t}");
        }
        assert_eq!(a.mint_rate().to_bits(), b.mint_rate().to_bits(), "seed {seed}: the same mint rate");
    }
}
