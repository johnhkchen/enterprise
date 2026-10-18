//! The fast host: the core with no game. Builds the network study's world (`dev/experiments/network_study.py`: the
//! hub with its warehouse and squad, settlements S1-S4 at 300, 600, 1,200 and 4,000 blocks with their farms and money,
//! toll town T on the S3 road, each flag's additions as far as the core has them), runs it DAYS days (default 10,000),
//! checking the books every day (goods and money conserved, nobody in debt), and reports what it did: per town, per
//! route, why wagons went or stayed, and each town's cash book. `--csv OUT` writes a row per town and per route a day.
//!
//!     cargo run --release -p enterprise-sim -- 365 --csv /tmp/base.csv
//!     cargo run --release -p enterprise-sim -- 120 --river --services --metal --trades --seeds 5
//!     cargo run --release -p enterprise-sim -- 365 --people-use   # the settlements' lockers' wants as their people's uses
//!
//! With `--sweep N [DAYS]` it runs the random worlds instead (seeds 0 to N-1, DAYS days each, default 120: the core
//! tests' generator, every slice on by seed, each day's checks) across the cores and prints what they did.
//!
//!     cargo run --release -p enterprise-sim -- --sweep 300
//!
//! The world is built through the core's host calls; the only thing read off the world directly is each route's state
//! (`RouteState`: stacks home, items bought, food eaten), which no host call returns.

mod flags;
mod goods;
mod report;
mod world;

use std::time::Instant;

use enterprise_core::sweep::Sweep;
use flags::Flags;
use goods::DAY;
use report::Tracker;
use world::Study;

/// The random worlds' generator, as the core's tests build them.
#[path = "../../core/tests/common/mod.rs"]
mod common;

/// What one seed's run came to, for the sweep's summary.
struct Outcome {
    seed: u64,
    days: u64,
    money: Vec<(String, f64)>,
    mood: Vec<(String, f64)>,
    trips: u32,
    stacks_out: f64,
    stacks_sold: f64,
    seconds: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--sweep") {
        let n = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(300);
        let days = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(120);
        return random_worlds(n, days);
    }
    let flags = match Flags::parse(&args) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let mut outcomes = Vec::new();
    for seed in 1..=flags.seeds {
        outcomes.push(run(&flags, seed, flags.seeds == 1 || seed == 1));
    }
    if outcomes.len() > 1 {
        sweep(&outcomes);
    }
}

/// One seed's run: built, run day by day with the books checked, reported in full if `full`.
fn run(flags: &Flags, seed: u64, full: bool) -> Outcome {
    let mut st = Study::build(flags.clone(), seed);
    for (name, v) in &flags.dials {
        if let Err(e) = st.s.set_dial(name, *v) {
            eprintln!("--dial {name}: {e}");
            std::process::exit(2);
        }
    }
    if full {
        println!("seed {seed}: {} towns, {} routes, {} goods", st.s.town_count(), st.s.route_count(), st.s.content().goods.len());
        for l in st.setup_log.drain(..) {
            println!("{l}");
        }
        for m in &st.missing {
            println!("missing: {m}");
        }
    }
    st.setup_log.clear();
    let csv = if seed == 1 { flags.csv.clone() } else { flags.csv.as_ref().map(|c| format!("{c}.seed{seed}")) };
    let mut tracker = Tracker::new(csv.as_deref()).unwrap_or_else(|e| {
        eprintln!("--csv: {e}");
        std::process::exit(2);
    });
    let every = if flags.every > 0 { flags.every } else { (flags.days / 12).max(1) };
    let started = Instant::now();
    let mut start = st.s.totals();
    tracker.fit(&st.s, true);
    let mut ran = 0;
    for day in 1..=flags.days {
        let towns_before = st.s.town_count();
        st.morning(day);
        if st.s.town_count() != towns_before {
            // A town opened today (the spring): its money came in paid, its goods delivered; count afresh from here.
            start = st.s.totals();
        }
        if full {
            for l in st.setup_log.drain(..) {
                println!("{l}");
            }
        }
        st.setup_log.clear();
        let out = st.s.advance(day * DAY);
        if let Err(e) = st.s.conserved(&start).and_then(|_| st.s.solvent()) {
            eprintln!("day {day}: {e}");
            std::process::exit(1);
        }
        tracker.day(&mut st, day, &out);
        ran = day;
        if full && (day % every == 0 || day == flags.days) {
            println!("{}", report::day_line(&st, &tracker, day));
        }
        let met = stop_met(&st, &flags.stop_when);
        if !met.is_empty() {
            println!("stopped on day {day}: {}", met.iter().map(|(why, _)| why.as_str()).collect::<Vec<_>>().join("; "));
            for (_, t) in met {
                println!("\n{}'s cash book, the last ten days:", st.s.town(t).name);
                for (d, label, v) in st.s.cash_book(t, 10) {
                    println!("  day {d:>4}  {v:>+9.2}  {label}");
                }
            }
            break;
        }
    }
    tracker.read_books(&st.s, ran);
    tracker.finish();
    let took = started.elapsed();
    let routes = st.s.route_count();
    println!("seed {seed}: {ran} days, {} towns, {routes} routes, {} events in {:.2?} ({:.1} µs a day); books balanced every day",
        st.s.town_count(), tracker.events, took, took.as_secs_f64() * 1e6 / ran.max(1) as f64);
    if full {
        print!("{}", report::end_report(&mut st, &tracker, ran));
        if let Some(c) = &csv {
            println!("\nwrote {} rows to {c}", tracker.rows);
        }
    }
    let s = &st.s;
    let left: u32 = tracker.routes.iter().map(|t| t.left).sum();
    let arrived: u32 = tracker.routes.iter().map(|t| t.arrived).sum();
    Outcome {
        seed,
        days: ran,
        money: st.named.iter().map(|(n, t)| (n.clone(), s.town(*t).money.to_f64())).collect(),
        mood: st.named.iter().map(|(n, t)| (n.clone(), s.town(*t).people.mood)).collect(),
        trips: (0..routes).map(|r| s.route(r).trips).sum(),
        stacks_out: tracker.routes.iter().map(|t| t.stacks_out).sum::<u64>() as f64 / left.max(1) as f64,
        stacks_sold: tracker.routes.iter().map(|t| t.sold_stacks).sum::<f64>() / arrived.max(1) as f64,
        seconds: took.as_secs_f64(),
    }
}

/// One random world for `days` days, as the core's `tests/random_worlds.rs` runs it: what it did, or the day it failed
/// a check.
fn random_world(seed: u64, days: u64) -> Result<Sweep, String> {
    let mut s = common::world(seed);
    let daily = common::sellers(seed);
    let start = s.totals();
    let mut sweep = Sweep::new();
    for day in 1..=days {
        for &emeralds in &daily {
            s.sell_emeralds(emeralds);
        }
        s.advance(day * DAY);
        common::slices::daily(&mut s, seed, day);
        s.conserved(&start).and_then(|_| s.solvent()).and_then(|_| common::slices::check(&s, day))
            .map_err(|e| format!("seed {seed}, day {day}: {e}"))?;
        sweep.day(&s);
    }
    sweep.end(&s);
    Ok(sweep)
}

/// `--sweep N [DAYS]`: N random worlds, and what they did.
fn random_worlds(n: u64, days: u64) {
    let started = Instant::now();
    let mut all = Sweep::new();
    let mut failed = Vec::new();
    for r in common::each_seed(0..n, |seed| random_world(seed, days)) {
        match r {
            Ok(s) => all.merge(s),
            Err(e) => failed.push(e),
        }
    }
    print!("{}", all.report());
    println!("in {:.2?}; {} worlds failed a check", started.elapsed(), failed.len());
    for e in &failed {
        println!("  {e}");
    }
    if !failed.is_empty() {
        std::process::exit(1);
    }
}

/// The towns meeting a `--stop-when` condition today, and why.
fn stop_met(st: &Study, conds: &[(String, String, String, f64)]) -> Vec<(String, usize)> {
    let mut met = Vec::new();
    for (who, field, op, value) in conds {
        for (name, t) in &st.named {
            if who != "any" && who != name {
                continue;
            }
            let v = st.s.town(*t);
            let x = match field.as_str() {
                "treasury" => v.money.to_f64(),
                "mood" => v.people.mood,
                "villagers" => v.people.villagers,
                "purse" => v.people.purse.to_f64(),
                "hexes" => st.s.holding(*t).hexes.len() as f64,
                _ => continue,
            };
            let hit = match op.as_str() { "<" => x < *value, "<=" => x <= *value, ">" => x > *value, _ => x >= *value };
            if hit {
                met.push((format!("{name} {field} {x:.2} {op} {value}"), *t));
            }
        }
    }
    met
}

/// The sweep's summary: each seed's line, then each town's money and mood across the seeds.
fn sweep(outcomes: &[Outcome]) {
    println!("\n== Sweep over {} seeds (the core's dice: lockers' XP rolls, the jobs draw)", outcomes.len());
    for o in outcomes {
        let money: Vec<String> = o.money.iter().map(|(n, m)| format!("{n} {m:.0}")).collect();
        println!("seed {:>3}: {} days in {:.2}s, {} trips, {:.2} stacks out / {:.2} sold a trip | {}", o.seed, o.days,
            o.seconds, o.trips, o.stacks_out, o.stacks_sold, money.join(" "));
    }
    let names: Vec<String> = outcomes[0].money.iter().map(|(n, _)| n.clone()).collect();
    for name in names {
        let pick = |f: &dyn Fn(&Outcome) -> Option<f64>| -> (f64, f64, f64) {
            let v: Vec<f64> = outcomes.iter().filter_map(f).collect();
            let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
            (mean, v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
        };
        let m = pick(&|o| o.money.iter().find(|(n, _)| *n == name).map(|x| x.1));
        let d = pick(&|o| o.mood.iter().find(|(n, _)| *n == name).map(|x| x.1));
        println!("{name:>8}: money {:.1} (from {:.1} to {:.1}), mood {:+.2} (from {:+.2} to {:+.2})", m.0, m.1, m.2, d.0, d.1, d.2);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn month(args: &str) -> (Study, Tracker) {
        let argv: Vec<String> = format!("30 {args}").split_whitespace().map(String::from).collect();
        let mut st = Study::build(Flags::parse(&argv).unwrap(), 1);
        let mut start = st.s.totals();
        let mut tracker = Tracker::new(None).unwrap();
        tracker.fit(&st.s, true);
        for day in 1..=30 {
            let towns = st.s.town_count();
            st.morning(day);
            if st.s.town_count() != towns {
                start = st.s.totals();
            }
            let out = st.s.advance(day * DAY);
            st.s.conserved(&start).and_then(|_| st.s.solvent()).unwrap_or_else(|e| panic!("{args}, day {day}: {e}"));
            tracker.day(&mut st, day, &out);
        }
        (st, tracker)
    }

    #[test]
    fn every_flag_builds_a_world_that_runs_a_month_with_its_books_balanced() {
        for args in ["", "--river", "--river-bridge", "--services --metal --trades", "--comforts --fleet --master",
                "--war 5,10,20", "--freight", "--ridge --realms", "--people-use", "--people-use --river --comforts"] {
            let (st, _) = month(args);
            assert!(st.s.town_count() >= 6, "{args}: {} towns", st.s.town_count());
        }
    }

    #[test]
    fn the_hubs_wagons_carry_paper_to_the_settlements_lockers() {
        // The faithful world: no one in the settlements uses paper, but their Librarians' and Cartographers' trades,
        // switched on, take it (`Caravans.wantsAt`), so the hub's wagons bring it and the lockers turn it into money.
        let (st, _) = month("");
        let paper = st.s.content().good("paper").unwrap();
        let s1 = st.town("S1").unwrap();
        let hub_s1 = (0..st.s.route_count()).find(|&r| st.s.route_ends(r) == (st.hub, s1)).unwrap();
        assert!(st.s.route(hub_s1).trips > 0 && st.s.route(hub_s1).sold > 0.0, "the hub's wagons sell at S1");
        assert!(st.s.town(st.hub).stock[paper] < 3.0 * 1728.0, "the hub's paper went out");
        assert!(st.s.town(s1).money > 64.0, "S1's lockers earn from it: {}", st.s.town(s1).money);
    }

    #[test]
    fn the_hubs_wagons_carry_paper_where_the_settlements_people_use_it() {
        let (mut st, tracker) = month("--people-use");
        let left: u32 = (0..st.s.route_count()).map(|r| st.s.route(r).trips).sum();
        assert!(left > 0, "no wagon came home");
        let paper = st.s.content().good("paper").unwrap();
        assert!(st.s.town(st.hub).stock[paper] < 3.0 * 1728.0, "the hub kept all its paper");
        assert!(report::end_report(&mut st, &tracker, 30).contains("hub->S1"));
    }
}
