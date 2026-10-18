//! The proof's requests, as postcard frames: what both the native framing test and the WebAssembly test send.

#![allow(dead_code)]

use enterprise_core::wire::*;

pub const DAY: u64 = 24_000;

fn desk(x: i32, z: i32) -> Point {
    Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x, y: -60, z }
}

fn pack() -> ContentPack {
    let mut wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    wheat.flags = vec![GoodFlag::Crop];
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    bread.saturation = 6.0;
    ContentPack { day_ticks: DAY, goods: vec![wheat, bread] }
}

/// A world made, two towns stocked and run 30 days, asked, saved, and a few requests the core must refuse: the same
/// conversation a host has, as requests. Each is a [`Request`] as postcard bytes.
pub fn requests() -> Vec<Vec<u8>> {
    let counts = |w: f64, b: f64| vec![Count { good: 0, count: w }, Count { good: 1, count: b }];
    let commands = vec![
        Command::Ask { queries: vec![Query::Points] }, // refused: no society yet
        Command::Create { pack: pack(), seed: 7 },
        Command::Apply { inputs: vec![
            Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
            Input::AddTown { name: "Bly".into(), desk: desk(600, 40) },
            Input::Pay { town: 0, money: enterprise_core::Fixed::emeralds(40) },
            Input::Stocktake { point: desk(27, 27), counts: counts(64.0, 12.0) },
            Input::Advance { to: 3 * DAY },
        ] },
        Command::Ask { queries: vec![Query::Stock { town: 0 }, Query::Stock { town: 1 }, Query::Points] },
        Command::Apply { inputs: vec![Input::Advance { to: 30 * DAY }] },
        Command::Ask { queries: vec![Query::Stock { town: 0 }, Query::Stock { town: 1 }, Query::Points] },
        Command::Apply { inputs: vec![Input::Deliver { town: 5, good: 0, count: 1.0 }] }, // refused: no such town
        Command::Save,
    ];
    let mut out: Vec<Vec<u8>> = commands.into_iter().map(|c| postcard::to_allocvec(&Request::new(c)).unwrap()).collect();
    out.push(postcard::to_allocvec(&Request { abi: ABI + 1, schema: SCHEMA, command: Command::Save }).unwrap());
    out.push(postcard::to_allocvec(&Request { abi: ABI, schema: SCHEMA + 1, command: Command::Save }).unwrap());
    out.push(vec![0xff, 0xff, 0xff]); // not a request at all
    out
}
