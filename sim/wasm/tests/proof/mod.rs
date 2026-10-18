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
    ContentPack::new(DAY, vec![wheat, bread])
}

fn frame(command: Command) -> Vec<u8> {
    postcard::to_allocvec(&Request::new(command)).unwrap()
}

/// A world made, two towns stocked and run 30 days, asked, saved; another world made and the save loaded over it, then
/// carried on with trade (a shop of the town's own, a player's order, a delivery) and saved again; and a few requests
/// the core must refuse: the same conversation a host has, as requests. Each is a [`Request`] as postcard bytes. The
/// save the `Load` carries is the one the native core gives at that point, so the module loads what it saved itself.
pub fn requests() -> Vec<Vec<u8>> {
    let counts = |w: f64, b: f64| vec![Count { good: 0, count: w }, Count { good: 1, count: b }];
    let ask = || Command::Ask { queries: vec![Query::Stock { town: 0 }, Query::Stock { town: 1 }, Query::Points] };
    let mut out: Vec<Vec<u8>> = vec![
        Command::Ask { queries: vec![Query::Points] }, // refused: no society yet
        Command::Create { pack: pack(), seed: 7 },
        Command::Apply { inputs: vec![
            Input::AddTown { name: "Ashford".into(), desk: desk(27, 27) },
            Input::AddTown { name: "Bly".into(), desk: desk(600, 40) },
            Input::Pay { town: 0, money: enterprise_core::Fixed::emeralds(40) },
            Input::Stocktake { point: desk(27, 27), counts: counts(64.0, 12.0), free: 25 },
            Input::Advance { to: 3 * DAY },
        ] },
        ask(),
        Command::Apply { inputs: vec![Input::Advance { to: 30 * DAY }] },
        ask(),
        Command::Apply { inputs: vec![Input::Deliver { town: 5, good: 0, count: 1.0 }] }, // refused: no such town
        Command::Save,
    ]
    .into_iter()
    .map(frame)
    .collect();

    // The save so far, as the native core gives it.
    let mut society = None;
    let saved = out.iter().map(|r| handle_bytes(&mut society, r)).last().unwrap();
    let Response::Saved { save } = postcard::from_bytes(&saved).unwrap() else { panic!("the proof's save: {saved:?}") };

    out.extend(
        [
            Command::Create { pack: pack(), seed: 99 }, // another world, which the load replaces
            Command::Load { save },
            Command::Apply { inputs: vec![
                Input::OwnShop { town: 0 },
                Input::Order { town: 0, good: 0, lot: 16.0, price: 1.0, purse: 20.0 },
                Input::Deliver { town: 1, good: 0, count: 32.0 },
                Input::Pay { town: 1, money: enterprise_core::Fixed::emeralds(25) },
                Input::Advance { to: 40 * DAY },
            ] },
            ask(),
            Command::Save,
        ]
        .into_iter()
        .map(frame),
    );
    out.push(postcard::to_allocvec(&Request { abi: ABI + 1, schema: SCHEMA, command: Command::Save }).unwrap());
    out.push(postcard::to_allocvec(&Request { abi: ABI, schema: SCHEMA + 1, command: Command::Save }).unwrap());
    out.push(vec![0xff, 0xff, 0xff]); // not a request at all
    out
}
