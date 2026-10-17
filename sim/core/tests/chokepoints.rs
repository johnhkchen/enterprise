//! Chokepoints: a town sees what guarding its land would cost the wagons crossing it, and every three days picks the
//! posts and toll that pay best, at most 80% of the detour and 25% (more with armed and armoured guards), with a
//! passage fee where there's no way round; and what a bridge would bring it. On the mod's `TollTests` fixture: a road
//! from (0, 0) to (12, 0), 8.5 spurs of road food a hex, wagons carrying 1,000 spurs, a town holding two rings around
//! the hex at (6, 0), where a river along q = 6 may be crossed.

use std::collections::BTreeSet;

use enterprise_core::chokepoints::{best_plan, guard_cost_per_day, Crossing, PostOption};
use enterprise_core::{Content, Dials, Good, Hex, Path, Society};

const DAY: u64 = 24_000;
const START: Hex = Hex::new(0, 0);
const GOAL: Hex = Hex::new(12, 0);
const PER_HEX: f64 = 8.5; // road food both ways at bread prices, in spurs
const VALUE: f64 = 1000.0; // coins a wagon carries home, in spurs
const BRIDGE: Hex = Hex::new(6, 0);
const SECOND: Hex = Hex::new(6, 1);
const FORD: Hex = Hex::new(6, -6);

/// The river along q = 6: every hex of it water but those given.
fn river(s: &mut Society, crossings: &[Hex]) {
    for r in -30..=30 {
        let hex = Hex::new(6, r);
        let water = if crossings.contains(&hex) { 0.0 } else { 1.0 };
        s.set_ground(hex, water, 0.0);
    }
}

/// The mod's fixture (`TollTests.townAcrossTheRoad`): a town whose bell stands far off (999,000, 999,000), holding the
/// 19 hexes within two of the bridge hex, with bread and emeralds for its guards, its toll `toll`, no guards; and a
/// bare route from START's centre to GOAL's, between two towns with no money (so no wagon leaves on it).
fn fixture(toll: f64) -> (Society, usize, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64).food(5.0)], DAY));
    let town = s.add_town("Bridgeford", 100.0);
    s.place_town(town, 999_000.5, 999_000.5);
    s.deliver(town, 0, 640.0);
    for dq in -2..=2 {
        for dr in -2..=2 {
            let hex = Hex::new(BRIDGE.q + dq, BRIDGE.r + dr);
            if hex.distance(BRIDGE) <= 2 {
                assert!(s.claim_hex(town, hex));
            }
        }
    }
    assert_eq!(s.holding(town).hexes.len(), 19);
    s.set_toll(town, toll);
    s.set_guards(town, 19); // a post on every hex
    s.set_guards(town, 0);
    let from = s.add_town("Start", 0.0);
    let to = s.add_town("Goal", 0.0);
    s.place_town(from, START.center_x(), START.center_z());
    s.place_town(to, GOAL.center_x(), GOAL.center_z());
    let route = s.add_route(from, to, 1.0, 1.0, 1.0);
    (s, town, route)
}

/// The cheapest way from START to GOAL past the towns' posts, nobody free (`TollTests.plan`).
fn plan(s: &Society) -> Option<Path> {
    s.map().plan(START, GOAL, PER_HEX, VALUE, &s.toll_posts(), &BTreeSet::new(), false, &BTreeSet::new())
}

/// Three days of a wagon a day carrying 1,000 spurs past the town, after its toll clock starts.
fn three_wagons(s: &mut Society, route: usize) {
    for _ in 0..3 {
        s.pass(route, VALUE, PER_HEX);
    }
}

fn paid(s: &mut Society, route: usize) -> f64 {
    s.pass(route, VALUE, PER_HEX).iter().map(|(_, t)| t).sum()
}

// parity: TollTests.onlyPostsCollect
#[test]
fn only_posts_collect() {
    let (mut s, town, route) = fixture(0.25);
    assert_eq!(plan(&s).unwrap().hexes.len() as i32, START.distance(GOAL) + 1, "no posts: straight through");
    assert_eq!(paid(&mut s, route), 0.0, "and nothing paid");
    s.set_posts(town, &[BRIDGE]);
    let path = plan(&s).unwrap().hexes;
    assert!(!path.contains(&BRIDGE), "one post on open ground is stepped around: {path:?}");
    assert!(path.len() as i32 <= START.distance(GOAL) + 2, "for a hex at most: {}", path.len());
}

// parity: TollTests.aTownOnOpenGroundPostsNoGuard
#[test]
fn a_town_on_open_ground_posts_no_guard() {
    let (mut s, town, route) = fixture(0.05);
    assert!(!s.review_tolls(town), "the first review starts the clock");
    three_wagons(&mut s, route);
    let seen = s.crossings(town);
    assert_eq!(seen.len(), 1);
    assert!(seen[0].options[0].detour <= PER_HEX + 1e-6, "one post can be gone around for a hex's food at most: {:?}",
        seen[0].options);
    s.advance(3 * DAY);
    assert_eq!(s.guards(town), 0, "no chokepoint, so no guard: {:?}", s.holding(town).posts);
    // At most 0.8 x 8.5 = 6.8 spurs a day, against a guard's 11.2 (0.5 + 6/30 emeralds at 16 spurs).
    assert_eq!(s.toll_plan(town).posts, [], "no plan");
    assert_eq!(s.toll_rate(town), 0.05, "it keeps its toll for later");
    assert!((guard_cost_per_day(&Dials::default(), 16.0) - 11.2).abs() < 1e-9);
    assert_eq!(s.route(route).trips, 0, "no wagon of the route's own ran");
}

// parity: TollTests.aTownHoldingTheOnlyCrossingTollsIt
#[test]
fn a_town_holding_the_only_crossing_tolls_it() {
    let (mut s, town, route) = fixture(0.05);
    river(&mut s, &[BRIDGE]);
    s.review_tolls(town);
    three_wagons(&mut s, route);
    let options = &s.crossings(town)[0].options;
    assert_eq!(options.len(), 1, "a post on the bridge closes the way: {options:?}");
    assert_eq!(options[0].posts, [BRIDGE]);
    assert!(options[0].detour.is_infinite());
    s.advance(3 * DAY);
    assert_eq!(s.holding(town).posts, [BRIDGE], "a guard on the bridge");
    assert_eq!(s.toll_rate(town), 0.25, "no way around, so the most it may ask");
    let fee = s.passage_fee(town);
    assert_eq!(fee, 48.0, "and the handling fee where there's no way round: 3 emeralds at 16 spurs");
    assert_eq!(paid(&mut s, route), (250.0 + fee).floor(), "the next wagon pays 25% and the fee");
}

// parity: TollTests.aTownTollsUpToTheDetour
#[test]
fn a_town_tolls_up_to_the_detour() {
    let (mut s, town, route) = fixture(0.05);
    // The river has a second, far crossing, 6 hexes off the road: going around the bridge costs a long detour.
    river(&mut s, &[BRIDGE, FORD]);
    s.review_tolls(town);
    three_wagons(&mut s, route);
    let option = s.crossings(town)[0].options[0].clone();
    assert_eq!(option.posts, [BRIDGE], "first try: a post on the bridge");
    let detour = option.detour;
    assert!((detour - 6.0 * PER_HEX).abs() < 1e-9, "by the ford, six hexes further: {detour}");
    s.advance(3 * DAY);
    assert_eq!(s.holding(town).posts, [BRIDGE], "the bridge is posted");
    let expected = 0.8 * detour / 1000.0;
    assert!((s.toll_rate(town) - expected).abs() < 1e-9 && expected < 0.25, "toll at 80% of the detour: {}",
        s.toll_rate(town));
    assert_eq!(s.passage_fee(town), 0.0, "the toll takes all it may ask");
    let path = plan(&s).unwrap().hexes;
    assert!(path.contains(&BRIDGE), "so wagons still take the bridge: {path:?}");
}

// parity: TollTests.armedAndArmouredGuardsAskMore
#[test]
fn armed_and_armoured_guards_ask_more() {
    let (mut s, town, route) = fixture(0.05);
    river(&mut s, &[BRIDGE, FORD]);
    s.review_tolls(town);
    three_wagons(&mut s, route);
    let detour = s.crossings(town)[0].options[0].detour;
    s.set_guard_kit(town, 1.0, 0.0);
    s.advance(3 * DAY);
    let expected = (0.25f64).min(0.95 * detour / 1000.0);
    assert!((s.toll_rate(town) - expected).abs() < 1e-9, "armed guards ask 95% of the detour: {}, expected {expected}",
        s.toll_rate(town));
    s.set_guard_kit(town, 1.0, 1.0);
    s.set_ground(FORD, 1.0, 0.0); // the ford washed out: no way around
    three_wagons(&mut s, route);
    s.advance(6 * DAY);
    assert!(s.toll_rate(town) > 0.25, "armoured guards on the only crossing ask above 25%: {}", s.toll_rate(town));
    assert!((s.toll_rate(town) - 0.35).abs() < 1e-12, "up to 35%: {}", s.toll_rate(town));
}

// parity: TollTests.twoBridgesTakeTwoPosts
#[test]
fn two_bridges_take_two_posts() {
    let (mut s, town, route) = fixture(0.05);
    river(&mut s, &[BRIDGE, SECOND]); // two crossings side by side, both the town's
    s.review_tolls(town);
    three_wagons(&mut s, route);
    let options = s.crossings(town)[0].options.clone();
    assert!(options.len() == 2 && options[0].detour <= PER_HEX + 1e-6 && options[1].detour.is_infinite(),
        "one bridge is gone around, both aren't: {options:?}");
    assert_eq!(options[0].posts, [BRIDGE]);
    assert_eq!(options[1].posts, [BRIDGE, SECOND]);
    s.advance(3 * DAY);
    assert_eq!(s.holding(town).posts, [BRIDGE], "a guard hired per review");
    assert_eq!(s.toll_plan(town).posts, [BRIDGE, SECOND], "of the two it plans");
    three_wagons(&mut s, route);
    s.advance(6 * DAY);
    assert_eq!(s.holding(town).posts, [BRIDGE, SECOND], "both posted");
    assert_eq!(s.toll_rate(town), 0.25, "at the most it may ask");
    assert_eq!(s.deserted(town), 0, "fed and paid");
}

// parity: TollTests.aBridgeIsWorthMostWhereNothingCrosses
#[test]
fn a_bridge_is_worth_most_where_nothing_crosses() {
    let (mut s, town, _) = fixture(0.05);
    s.set_guards(town, 19);
    river(&mut s, &[]);
    let value = s.bridge_value(town, BRIDGE);
    // No way at all: the bridge saves the whole trip; the town may ask the most, 25% of the 300 spurs expected.
    assert!((value.tolls_per_day - 75.0).abs() < 1e-6, "75 spurs a day: {}", value.tolls_per_day);
    assert!(value.from.is_some_and(|h| h.q == 5) && value.to.is_some_and(|h| h.q == 7),
        "the deck runs across the river: {:?} to {:?}", value.from, value.to);
    let (s, town, _) = fixture(0.05);
    assert_eq!(s.bridge_value(town, SECOND).tolls_per_day, 0.0, "a bridge on dry land is worth nothing");
}

// parity: TollTests.aSecondBridgeBesideTheFirstIsWorthLittle
#[test]
fn a_second_bridge_beside_the_first_is_worth_little() {
    let (mut s, town, _) = fixture(0.25);
    s.set_posts(town, &[BRIDGE]); // the first bridge, posted at 25%
    river(&mut s, &[BRIDGE]); // bridged already at (6, 0); (6, 1) still water
    assert_eq!(s.bridge_value(town, SECOND).tolls_per_day, 0.0, "dodging its own post gains the town nothing");
}

#[test]
fn a_bridge_on_the_towns_own_route_is_worth_nothing_to_its_tolls() {
    let (mut s, town, _) = fixture(0.05);
    river(&mut s, &[]);
    let other = s.add_town("Far", 0.0);
    s.place_town(other, GOAL.center_x(), GOAL.center_z() + 400.0);
    s.add_route(town, other, 1.0, 1.0, 1.0);
    assert!((s.bridge_value(town, BRIDGE).tolls_per_day - 75.0).abs() < 1e-6, "only the route it isn't an end of");
}

/// One crossing seen with these options, a trip a day, carrying `value`.
fn crossing(value: f64, options: Vec<(Vec<Hex>, f64)>) -> Crossing {
    Crossing {
        route: 0,
        options: options.into_iter().map(|(posts, detour)| PostOption { posts, detour }).collect(),
        value,
        trips_per_day: 1.0,
        ..Default::default()
    }
}

#[test]
fn the_best_plan_pays_its_guards_or_posts_none() {
    // Open ground: a hex's detour (8.5) bears 0.0068, 6.8 spurs a day, less than a guard's 11.2.
    let open = crossing(1000.0, vec![(vec![BRIDGE], 8.5)]);
    assert_eq!(best_plan(&[open], 11.2, 0.8, 0.25, 48.0).posts, []);
    // The only crossing: 25% and the fee cap, 298 a day, less the guard.
    let only = crossing(1000.0, vec![(vec![BRIDGE], f64::INFINITY)]);
    let p = best_plan(std::slice::from_ref(&only), 11.2, 0.8, 0.25, 48.0);
    assert_eq!((p.rate, p.posts.clone(), p.fee, p.income), (0.25, vec![BRIDGE], 48.0, 298.0));
    // A poor route through the same post, five wagons a day, that could go round for 30: asking it 24% brings in
    // 240 + 5 x 24 = 360, more than 25% from the rich one alone (298); and the fee is what every wagon through will
    // still bear, none.
    let poor = Crossing { trips_per_day: 5.0, ..crossing(100.0, vec![(vec![BRIDGE], 30.0)]) };
    let p = best_plan(&[only.clone(), poor.clone()], 11.2, 0.8, 0.25, 48.0);
    assert_eq!(p.posts, [BRIDGE]);
    assert!((p.rate - 0.24).abs() < 1e-12, "0.8 x 30 / 100: both pay: {}", p.rate);
    assert_eq!(p.fee, 0.0, "the poor route bears no fee beyond its toll");
    assert!((p.income - 360.0).abs() < 1e-9, "{}", p.income);
    // A wagon a day on it and the rich route's 25% and fee win: 298 against 264.
    let p = best_plan(&[only, Crossing { trips_per_day: 1.0, ..poor }], 11.2, 0.8, 0.25, 48.0);
    assert_eq!((p.rate, p.fee, p.income), (0.25, 48.0, 298.0));
    // Tolls below half a percent aren't worth a post.
    let thin = crossing(1e6, vec![(vec![BRIDGE], 4000.0)]);
    assert_eq!(best_plan(&[thin], 0.0, 0.8, 0.25, 0.0).posts, [], "0.0032 is below the least toll");
}

#[test]
fn a_route_unseen_for_four_reviews_is_forgotten() {
    let (mut s, town, route) = fixture(0.05);
    river(&mut s, &[BRIDGE]);
    s.review_tolls(town);
    three_wagons(&mut s, route);
    s.advance(3 * DAY);
    assert_eq!(s.crossings(town)[0].trips_per_day, 1.0);
    assert_eq!(s.holding(town).posts, [BRIDGE]);
    for n in 1..=3 {
        s.advance((3 + 3 * n) * DAY);
        assert_eq!(s.crossings(town)[0].quiet_reviews, n as u32, "quiet for {n}");
        assert!(s.holding(town).posts.is_empty(), "a quiet route pays nothing: its guard goes at once");
    }
    s.advance(15 * DAY);
    assert!(s.crossings(town).is_empty(), "forgotten at the fourth review unseen");
}

#[test]
fn a_wagon_home_shows_the_towns_its_road_crosses_what_its_posts_would_cost() {
    // A farm sends wheat east over open ground, past a town's land, then bridged water between two lakes.
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).food(5.0)], DAY));
    s.content_mut().dials.standing_orders = false;
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 500.0);
    let keeper = s.add_town("Tollgate", 100.0);
    s.place_town(farm, 0.0, 0.0);
    s.place_town(mill, 1000.0, 0.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    let road = s.add_road(farm, mill, 2.0).expect("a road");
    let path = s.route_path(road).unwrap();
    for x in [path[4], path[5]] {
        assert!(s.claim_hex(keeper, x));
    }
    s.deliver(keeper, 1, 1000.0);
    let start = s.totals();
    for day in 1..=30 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    assert!(s.route(road).trips > 3, "wagons came home");
    let seen = s.crossings(keeper);
    assert_eq!(seen.len(), 1, "the keeper saw the road");
    assert!(seen[0].value > 0.0 && !seen[0].options.is_empty());
    assert!(seen[0].options[0].detour <= s.per_hex() * s.mint_rate() + 1e-6, "open ground: a hex's food at most");
    assert!(s.expected_value(road) > 0.0 && s.expected_value(road) != 300.0, "the route's takings a trip");
    assert_eq!(s.guards(keeper), 0, "no chokepoint, no guard");
    // The ends see nothing of their own road.
    assert!(s.crossings(farm).is_empty() && s.crossings(mill).is_empty());
}

#[test]
fn a_toll_plan_survives_the_save() {
    let (mut s, town, route) = fixture(0.05);
    river(&mut s, &[BRIDGE]);
    s.review_tolls(town);
    three_wagons(&mut s, route);
    s.advance(3 * DAY);
    let before = s.holding(town);
    assert!(!before.toll_plan.posts.is_empty());
    let loaded = Society::load(&s.save()).unwrap();
    assert_eq!(loaded.holding(town), before);
    assert_eq!(loaded.unsaved(), Vec::<String>::new());
}
