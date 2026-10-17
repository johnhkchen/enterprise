//! The fast host: the core with no game. Builds the study's map (the capital, a freight hub and seven towns, as in
//! `dev/experiments/network_study.py`), runs it for DAYS (default 10,000), checking the books every day, and says how
//! long it took.
//!
//!     cargo run --release -p enterprise-sim -- 10000

use std::time::Instant;

use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

/// Towns of the study's map, at their desks' positions: name, x, z, money.
const TOWNS: [(&str, f64, f64, f64); 9] = [
    ("Hayford", 1002.0, 1002.0, 200.0),
    ("Haygate", 1400.0, 1403.0, 100.0),
    ("Meadowworth", 1302.0, 1002.0, 50.0),
    ("Wheatfield", 1002.0, 1602.0, 50.0),
    ("Hollowwick", 1851.0, 1851.0, 50.0),
    ("Greenwick", 5002.0, 1002.0, 50.0),
    ("Barleyby", 1657.0, 1357.0, 50.0),
    ("Cloverby", 1307.0, 1757.0, 50.0),
    ("Lindenfield", 2057.0, 1307.0, 50.0),
];

fn main() {
    let days: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(10_000);
    let goods = ["wheat", "bread", "paper", "book", "wool", "leather", "iron_ingot", "cookie"];
    let content = Content::new(
        goods.iter().zip([0.05, 0.2, 0.04, 0.6, 0.1, 0.25, 1.0, 0.055])
            .map(|(name, value)| Good { name: name.to_string(), value, stack: 64 })
            .collect(),
        DAY,
    );
    let mut s = Society::new(content);
    let ids: Vec<_> = TOWNS.iter().map(|(name, _, _, money)| s.add_town(name, *money)).collect();
    // Each town makes something and its people use others: farms grow wheat, mills bake, scribes write.
    let trades: [(usize, &[(usize, f64, f64)]); 9] = [
        (0, &[(1, 0.0, 48.0), (3, 0.0, 6.0), (6, 0.0, 4.0), (7, 0.0, 30.0)]),
        (1, &[(1, 0.0, 24.0), (2, 0.0, 20.0), (4, 0.0, 10.0)]),
        (2, &[(0, 160.0, 0.0), (2, 0.0, 16.0), (5, 0.0, 6.0)]),
        (3, &[(0, 192.0, 0.0), (1, 48.0, 0.0), (3, 0.0, 4.0)]),
        (4, &[(2, 96.0, 0.0), (3, 12.0, 0.0), (1, 0.0, 16.0)]),
        (5, &[(4, 40.0, 0.0), (0, 0.0, 30.0)]),
        (6, &[(5, 24.0, 0.0), (1, 0.0, 12.0), (7, 0.0, 10.0)]),
        (7, &[(7, 60.0, 0.0), (0, 0.0, 40.0)]),
        (8, &[(6, 10.0, 0.0), (1, 0.0, 12.0), (4, 0.0, 8.0)]),
    ];
    for (town, list) in trades {
        for &(good, makes, uses) in list {
            s.set_trade(ids[town], good, makes, uses);
        }
    }
    // A route each way between towns within 1,500 blocks: a day on the road for 1,200 blocks, a third of an emerald
    // of road food a hundred blocks there and back, every four days.
    for (a, (_, ax, az, _)) in TOWNS.iter().enumerate() {
        for (b, (_, bx, bz, _)) in TOWNS.iter().enumerate() {
            let blocks = ((ax - bx).powi(2) + (az - bz).powi(2)).sqrt();
            if a != b && blocks <= 1500.0 {
                s.add_route(ids[a], ids[b], blocks / 1200.0, blocks / 300.0, 4.0);
            }
        }
    }
    let started = Instant::now();
    let start = s.totals();
    let mut events = 0usize;
    for day in 1..=days {
        events += s.advance(day * DAY).len();
        if let Err(e) = s.conserved(&start).and_then(|_| s.solvent()) {
            eprintln!("day {day}: {e}");
            std::process::exit(1);
        }
    }
    let took = started.elapsed();
    println!("{days} days, {} towns, {} routes, {events} wagon events in {:.2?} ({:.1} µs a day); books balanced every day",
        s.town_count(), s.route_count(), took, took.as_secs_f64() * 1e6 / days as f64);
    for t in 0..s.town_count() {
        let v = s.town(t);
        println!("  {:12} {:>10.1} emeralds", v.name, v.money);
    }
    let (mut trips, mut sold, mut carried) = (0u64, 0.0, 0.0);
    for r in 0..s.route_count() {
        let v = s.route(r);
        trips += v.trips as u64;
        sold += v.sold;
        carried += v.carried;
    }
    println!("  {trips} trips: {:.1} stacks carried and {:.1} sold a trip", carried / 64.0 / trips.max(1) as f64,
        sold / 64.0 / trips.max(1) as f64);
}
