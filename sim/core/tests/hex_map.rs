//! Hexes, ground and paths: the mod's HexTests and TollTests fixtures as numbers. The TollTests fixture: a road from
//! (0, 0) to (12, 0), road food 8.5 a hex both ways, 1,000 coins carried home, a bridge at (6, 0).

use std::collections::{BTreeMap, BTreeSet};

use enterprise_core::map::{self, Ground, Post, Posts};
use enterprise_core::{Content, Good, Hex, Map, Society};

const DAY: u64 = 24_000;
const START: Hex = Hex::new(0, 0);
const GOAL: Hex = Hex::new(12, 0);
const PER_HEX: f64 = 8.5;
const VALUE: f64 = 1000.0;
const BRIDGE: Hex = Hex::new(6, 0);

fn none() -> BTreeSet<Hex> {
    BTreeSet::new()
}

fn nobody() -> BTreeSet<usize> {
    BTreeSet::new()
}

/// A river along q = 6, crossable only at the bridge: every path from START to GOAL crosses q = 6.
fn river(bridged: bool) -> Map {
    let mut m = Map::default();
    for r in -20..=20 {
        m.set_ground(Hex::new(6, r), 1.0, 0.0);
    }
    if bridged {
        m.bridge(BRIDGE);
    }
    m
}

/// Town 7 holding the bridge hex and two rings around it, a post on every hex, at `toll`.
fn town_across_the_road(toll: f64) -> Posts {
    let mut posts = BTreeMap::new();
    for dq in -2..=2 {
        for dr in -2..=2 {
            let hex = Hex::new(BRIDGE.q + dq, BRIDGE.r + dr);
            if hex.distance(BRIDGE) <= 2 {
                posts.insert(hex, Post { owner: 7, rate: toll, fee: 0.0 });
            }
        }
    }
    posts
}

// parity: HexTests.hexCentresMapBackToTheirHex
#[test]
fn hex_centres_map_back_to_their_hex() {
    for q in -5..=5 {
        for r in -5..=5 {
            let hex = Hex::new(q, r);
            assert_eq!(Hex::at(hex.center_x(), hex.center_z()), hex, "centre of {hex} maps back");
        }
    }
}

// parity: HexTests.hexesAreEightyBlocksAcross
#[test]
fn hexes_are_eighty_blocks_across() {
    let origin = Hex::new(0, 0);
    let east = origin.neighbors()[0];
    assert_eq!(east, Hex::new(1, 0), "east first");
    let (dx, dz) = (east.center_x() - origin.center_x(), east.center_z() - origin.center_z());
    assert!(((dx * dx + dz * dz).sqrt() - 80.0).abs() < 1e-6, "neighbouring centres are 80 apart");
    assert!(origin.neighbors().iter().all(|n| n.distance(origin) == 1), "neighbours are 1 away");
    for n in origin.neighbors() {
        let (dx, dz) = (n.center_x(), n.center_z());
        assert!(((dx * dx + dz * dz).sqrt() - 80.0).abs() < 1e-6, "every neighbour 80 away: {n}");
    }
}

// parity: HexTests.lineRunsHexByHex
#[test]
fn line_runs_hex_by_hex() {
    let (a, b) = (Hex::new(-3, 1), Hex::new(4, -2));
    let line = Hex::line(a, b);
    assert_eq!(line.len() as i32, a.distance(b) + 1, "a line has distance + 1 hexes");
    assert_eq!((line[0], *line.last().unwrap()), (a, b), "it runs end to end");
    assert!(line.windows(2).all(|w| w[0].distance(w[1]) == 1), "each step is to a neighbour");
    assert_eq!(Hex::line(a, a), vec![a], "a line of one");
}

#[test]
fn points_round_into_their_hex_as_the_mod_rounds() {
    assert_eq!(Hex::at(0.0, 0.0), Hex::new(0, 0));
    assert_eq!(Hex::at(39.9, 0.0), Hex::new(0, 0), "a hex reaches 40 blocks east of its centre");
    assert_eq!(Hex::at(40.1, 0.0), Hex::new(1, 0));
    assert_eq!(Hex::at(-40.1, 0.0), Hex::new(-1, 0));
    assert_eq!(Hex::at(-39.9, 0.0), Hex::new(0, 0));
    // Pointy-top: south-east is (0, 1), 80 blocks away at 60 degrees.
    assert_eq!(Hex::at(40.0, 69.28), Hex::new(0, 1));
}

// parity: TollTests.withNoTollsThePathIsStraight
#[test]
fn with_no_tolls_the_path_is_straight() {
    let path = Map::default().plan(START, GOAL, PER_HEX, VALUE, &Posts::new(), &nobody(), false, &none()).unwrap();
    assert_eq!(path.hexes.len() as i32, START.distance(GOAL) + 1, "straight: {:?}", path.hexes);
    assert!((path.cost - 12.0 * PER_HEX).abs() < 1e-9, "12 hexes of road food: {}", path.cost);
    assert_eq!(path.blocks(), 960.0);
}

// parity: TollTests.aPriceyTownIsGoneAround
#[test]
fn a_pricey_town_is_gone_around() {
    let posts = town_across_the_road(0.25); // 250 a crossing
    let path = Map::default().plan(START, GOAL, PER_HEX, VALUE, &posts, &nobody(), false, &none()).unwrap();
    assert!(path.hexes.iter().all(|h| !posts.contains_key(h)), "the path skirts the town: {:?}", path.hexes);
    assert!(path.hexes.len() as i32 > START.distance(GOAL) + 1, "taking a few more hexes");
    assert!(path.cost < 12.0 * PER_HEX + 250.0, "cheaper than the toll: {}", path.cost);
}

// parity: TollTests.aCheapTollIsPaid
#[test]
fn a_cheap_toll_is_paid() {
    let posts = town_across_the_road(0.01); // 10 a crossing, less than any detour
    let path = Map::default().plan(START, GOAL, PER_HEX, VALUE, &posts, &nobody(), false, &none()).unwrap();
    assert_eq!(path.hexes.len() as i32, START.distance(GOAL) + 1, "straight through: {:?}", path.hexes);
    assert!((path.cost - (12.0 * PER_HEX + 10.0)).abs() < 1e-9, "the toll once, not at each post: {}", path.cost);
}

#[test]
fn a_route_s_own_towns_charge_it_nothing_and_a_post_charges_its_fee() {
    let posts = town_across_the_road(0.25);
    let free: BTreeSet<usize> = [7].into_iter().collect();
    let path = Map::default().plan(START, GOAL, PER_HEX, VALUE, &posts, &free, false, &none()).unwrap();
    assert_eq!(path.hexes.len(), 13, "free: straight through");
    assert!((path.cost - 12.0 * PER_HEX).abs() < 1e-9);
    // One post with a fee and no rate: the fee alone, once.
    let one: Posts = [(Hex::new(1, 0), Post { owner: 3, rate: 0.0, fee: 2.0 })].into_iter().collect();
    let path = Map::default().plan(Hex::new(0, 0), Hex::new(2, 0), 100.0, VALUE, &one, &nobody(), false, &none()).unwrap();
    assert_eq!(path.hexes, vec![Hex::new(0, 0), Hex::new(1, 0), Hex::new(2, 0)], "2 is cheaper than going round");
    assert!((path.cost - 202.0).abs() < 1e-9, "{}", path.cost);
    // The mod's rule: charged on stepping onto a town's post from somewhere that isn't its post.
    let a = Hex::new(1, 0);
    assert_eq!(map::post_toll(&one, Hex::new(0, 0), a, &nobody(), VALUE), 2.0);
    assert_eq!(map::post_toll(&one, a, Hex::new(2, 0), &nobody(), VALUE), 0.0, "off a post: nothing");
}

// parity: TollTests.waterCantBeCrossed
#[test]
fn water_cant_be_crossed() {
    let m = river(true);
    let path = m.plan(START, GOAL, PER_HEX, VALUE, &Posts::new(), &nobody(), false, &none()).unwrap();
    assert!(path.hexes.contains(&BRIDGE), "the way over is the bridge: {:?}", path.hexes);
    let avoid: BTreeSet<Hex> = [BRIDGE].into_iter().collect();
    assert!(m.plan(START, GOAL, PER_HEX, VALUE, &Posts::new(), &nobody(), false, &avoid).is_none(), "no way without it");
    assert!(river(false).plan(START, GOAL, PER_HEX, VALUE, &Posts::new(), &nobody(), false, &none()).is_none(),
        "no bridge, no way");
}

// parity: TollTests.steepGroundIsHardGoingAndACliffStopsACaravan
#[test]
fn steep_ground_is_hard_going_and_a_cliff_stops_a_caravan() {
    let mut m = Map::default();
    let hills = Hex::new(6000, 6000);
    m.set_ground(hills, 0.0, 0.0);
    assert_eq!(m.crossing_factor(hills), 1.0, "flat: open ground");
    m.set_ground(hills, 0.0, 16.0); // a hill of 16
    assert_eq!(m.crossing_factor(hills), 3.0, "a hill: hard going");
    m.set_ground(hills, 0.0, 31.0);
    assert!(m.crossing_factor(hills).is_finite(), "31 is steep, not a cliff");
    m.set_ground(hills, 0.0, 40.0); // a mountain's flank, 40 up
    assert!(m.crossing_factor(hills).is_infinite(), "a cliff: impassable");
    m.set_ground(hills, 0.0, 32.0);
    assert!(m.crossing_factor(hills).is_infinite(), "32 is a cliff");
    // Water and relief multiply.
    m.set_ground(hills, 0.25, 8.0);
    assert_eq!(m.crossing_factor(hills), 1.5 * 2.0);
}

// parity: TollTests.aRouteFindsThePassThroughAMountainRange
#[test]
fn a_route_finds_the_pass_through_a_mountain_range() {
    let (start, goal, pass) = (Hex::new(0, 0), Hex::new(8, 0), Hex::new(4, -3));
    let mut m = Map::default();
    for r in -30..=30 {
        if Hex::new(4, r) != pass {
            m.set_ground(Hex::new(4, r), 0.0, 40.0);
        }
    }
    let path = m.plan(start, goal, 1.0, 0.0, &Posts::new(), &nobody(), false, &none()).unwrap();
    assert!(path.hexes.contains(&pass), "through the pass: {:?}", path.hexes);
}

// parity-partial: TollTests.aFloodedHexIsImpassable
#[test]
fn a_flooded_hex_is_impassable() {
    let mut m = Map::default();
    let lake = Hex::new(5000, 5000);
    assert_eq!(m.crossing_factor(lake), 1.0, "dry land by default");
    m.set_ground(lake, 1.0, 0.0);
    assert!(m.crossing_factor(lake).is_infinite(), "flooded: impassable");
    m.set_ground(lake, 0.5, 0.0);
    assert_eq!(m.crossing_factor(lake), 2.0, "half water: twice the road food, still crossable");
    m.set_ground(lake, 0.51, 0.0);
    assert!(m.crossing_factor(lake).is_infinite(), "more than half: impassable");
}

// parity-partial: TollTests.aBridgedHexIsOpenGround
#[test]
fn a_bridged_hex_is_open_ground() {
    let mut m = Map::default();
    m.set_ground(BRIDGE, 1.0, 0.0);
    assert!(m.crossing_factor(BRIDGE).is_infinite(), "not until it's bridged");
    m.bridge(BRIDGE);
    assert_eq!(m.crossing_factor(BRIDGE), 1.0, "bridged: caravans cross");
    m.set_ground(BRIDGE, 0.9, 5.0);
    assert_eq!(m.crossing_factor(BRIDGE), 1.0, "new ground keeps its bridge");
    // A bridge carries caravans over water, not over a cliff, and does nothing for a hex they can walk.
    let cliff = Hex::new(9, 9);
    m.set_ground(cliff, 0.0, 40.0);
    m.bridge(cliff);
    assert!(m.crossing_factor(cliff).is_infinite());
    let marsh = Hex::new(9, 10);
    m.set_ground(marsh, 0.25, 0.0);
    m.bridge(marsh);
    assert_eq!(m.crossing_factor(marsh), 1.5);
}

// parity-partial: TollTests.aFishermanSailsWhereCaravansCantWalk
#[test]
fn a_fisherman_sails_where_caravans_cant_walk() {
    let m = river(false);
    assert!(m.plan(START, GOAL, PER_HEX, VALUE, &Posts::new(), &nobody(), false, &none()).is_none(), "no way by land");
    let by_boat = m.plan(START, GOAL, PER_HEX, VALUE, &Posts::new(), &nobody(), true, &none()).unwrap();
    assert!(by_boat.hexes.iter().any(|h| h.q == 6), "a Fisherman sails across: {:?}", by_boat.hexes);
    assert!((by_boat.cost - PER_HEX * (11.0 + map::BOAT_FACTOR)).abs() < 1e-9, "the water hex at 0.8: {}", by_boat.cost);
    assert_eq!(Ground { water_share: 1.0, ..Default::default() }.factor(true), map::BOAT_FACTOR);
    assert_eq!(Ground { water_share: 0.4, ..Default::default() }.factor(true), 1.8, "walkable water is walked");
}

/// The cheapest cost by Dijkstra (no heuristic), over the same corridor and ground, no tolls.
fn dijkstra(m: &Map, start: Hex, goal: Hex, per_hex: f64, boat: bool) -> Option<f64> {
    let direct = start.distance(goal);
    let mut best: BTreeMap<Hex, f64> = [(start, 0.0)].into_iter().collect();
    let mut done = BTreeSet::new();
    loop {
        let (&hex, &g) = best.iter().filter(|(h, _)| !done.contains(*h)).min_by(|a, b| a.1.total_cmp(b.1))?;
        if hex == goal {
            return Some(g);
        }
        done.insert(hex);
        for next in hex.neighbors() {
            if next.distance(start) + next.distance(goal) > direct + 2 * map::MARGIN {
                continue;
            }
            let hard = if next == goal || next == start { 1.0 } else { m.ground(next).factor(boat) };
            if hard.is_finite() && g + per_hex * hard < best.get(&next).copied().unwrap_or(f64::MAX) {
                best.insert(next, g + per_hex * hard);
            }
        }
    }
}

#[test]
fn the_plan_is_the_cheapest_way_by_land_and_by_boat() {
    // Boats make the plain distance guess overestimate; the planner's scaled guess keeps it exact. Random lakes and
    // hills over 200 maps, every plan's cost checked against Dijkstra's.
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..200 {
        let mut m = Map::default();
        for q in -6..=14 {
            for r in -10..=10 {
                let w = next();
                let water = if w < 0.35 { 1.0 } else if w < 0.5 { next() * 0.5 } else { 0.0 };
                m.set_ground(Hex::new(q, r), water, if next() < 0.3 { next() * 40.0 } else { 0.0 });
            }
        }
        let goal = Hex::new(8, (next() * 6.0) as i32 - 3);
        for boat in [false, true] {
            let plan = m.plan(START, goal, 1.0, 0.0, &Posts::new(), &nobody(), boat, &none());
            let want = dijkstra(&m, START, goal, 1.0, boat);
            assert_eq!(plan.is_some(), want.is_some(), "boat {boat}: found the same");
            if let (Some(p), Some(w)) = (plan, want) {
                assert!((p.cost - w).abs() < 1e-9, "boat {boat}: {} against {w}", p.cost);
                let walked: f64 = p.hexes.windows(2).map(|s| {
                    if s[1] == goal { 1.0 } else { m.ground(s[1]).factor(boat) }
                }).sum();
                assert!((walked - p.cost).abs() < 1e-9, "the path costs what it says");
            }
        }
    }
}

/// A farm and a mill on the TollTests road, river and bridge between them; bread worth 0.2 for 5 food points.
fn farm_and_mill_by_road(bridged: bool) -> (Society, Option<usize>) {
    let goods = vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).food(5.0)];
    let mut s = Society::new(Content::new(goods, DAY));
    s.content_mut().dials.standing_orders = false;
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 10.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.place_town(farm, START.center_x() + 3.0, START.center_z() - 2.0);
    s.place_town(mill, GOAL.center_x(), GOAL.center_z());
    for r in -20..=20 {
        s.set_ground(Hex::new(6, r), 1.0, 0.0);
    }
    if bridged {
        s.bridge(BRIDGE);
    }
    let road = s.add_road(farm, mill, 2.0);
    (s, road)
}

#[test]
fn a_road_between_placed_towns_runs_the_trade_loop() {
    let (mut s, road) = farm_and_mill_by_road(true);
    assert_eq!((s.town_hex(0), s.town_hex(1)), (Some(START), Some(GOAL)), "placed in their hexes");
    let road = road.expect("a way over the bridge");
    let path = s.route_path(road).unwrap();
    assert!(path.contains(&BRIDGE) && path.len() == 13, "{path:?}");
    // 16 food points a hex both ways at 0.04 a point: 0.64 a hex, 12 hexes.
    assert!((s.per_hex() - 0.64).abs() < 1e-12);
    let (ticks, cost) = s.route_road(road);
    assert_eq!(ticks, 19_200, "960 blocks at 1,200 a day: 0.8 of a day");
    assert!((cost - 12.0 * 0.64).abs() < 1e-9, "{cost}");
    let start = s.totals();
    for day in 1..=60 {
        s.advance(day * DAY);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    let r = s.route(road);
    assert!(r.sold > 0.0 && r.trips >= 2, "wagons ran and sold: {r:?}");
}

#[test]
fn a_road_goes_around_and_costs_more_or_there_is_none() {
    let (s, road) = farm_and_mill_by_road(false);
    assert!(road.is_none(), "no bridge: no road");
    assert_eq!(s.route_count(), 0);
    // Hills on the straight way: the road skirts them, and the detour costs time and food.
    let goods = vec![Good::new("wheat", 0.05, 64)];
    let mut s = Society::new(Content::new(goods, DAY));
    let (a, b) = (s.add_town("A", 0.0), s.add_town("B", 0.0));
    s.place_town(a, 0.0, 0.0);
    s.place_town(b, GOAL.center_x(), 0.0);
    for q in 1..12 {
        s.set_ground(Hex::new(q, 0), 0.0, 24.0); // four times as hard
    }
    let road = s.add_road(a, b, 1.0).unwrap();
    let path = s.route_path(road).unwrap();
    assert!(path.len() > 13 && path[1..path.len() - 1].iter().all(|h| h.r != 0), "skirts the hills: {path:?}");
    let (ticks, cost) = s.route_road(road);
    assert_eq!(ticks, ((path.len() - 1) as f64 * 80.0 / 1200.0 * DAY as f64).round() as u64);
    assert!((cost - (path.len() - 1) as f64 * 16.0 / 30.0).abs() < 1e-9, "no bread: 1/30 a point: {cost}");
    // An unplaced town has no road.
    let c = s.add_town("C", 0.0);
    assert!(s.add_road(a, c, 1.0).is_none());
}
