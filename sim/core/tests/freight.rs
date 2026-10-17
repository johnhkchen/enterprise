//! Freight hubs (`freight.rs`, `rules/freight.rs`), pinned as the mod's caravan tests pin them: the test network's town
//! and hub, a buyer or carrier 200 blocks beyond the hub (20 food points a leg), bread at six an emerald for 5 points,
//! paper at 24 an emerald; and on maps of hexes, the hubs the roads make, the stops on the way and the routes that
//! reorganise round them.

use enterprise_core::cargo::Shipment;
use enterprise_core::freight::{self, Network, Run};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const CLOTH: usize = 2;
const TOOLS: usize = 3;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("cloth", 0.5, 64), Good::new("tools", 1.0, 64)], DAY)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A crate of 64 paper from `shipper`, made out to `buyer` (`Papers.billOfLading(0, shipper, buyer, "64 paper", ...)`).
fn crate_of_paper(shipper: usize, buyer: usize) -> Shipment {
    Shipment { good: PAPER, count: 64.0, shipper, consignee: Some(buyer), contract: None, settled: true, copy: false }
}

/// The mod's test network with a hub (`CaravanTests.network(helper, 2)` and a storehouse at its far end): a town and
/// the hub two blocks apart, and a third town 200 blocks beyond the hub; `onward` a route from the hub to it (else one
/// from it to the hub: a carrier whose wagon calls).
fn network(onward: bool, hub_emeralds: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let town = s.add_town("Town", 5.0);
    let hub = s.add_town("Hub", hub_emeralds);
    let beyond = s.add_town("Beyond", 5.0);
    s.add_route(town, hub, 2.0 / 1200.0, 0.0, 4.0);
    if onward {
        s.add_route(hub, beyond, 200.0 / 1200.0, 0.0, 4.0);
    } else {
        s.add_route(beyond, hub, 200.0 / 1200.0, 0.0, 4.0);
    }
    s.make_hub(hub);
    s
}

/// A stack's freight on a 200-block run with one mouth and a wagon's 27 stacks: 20 points both ways at 0.05 over 27
/// stacks, and the carriers' quarter.
fn stack_price_200() -> f64 {
    20.0 * 2.0 * 1.0 * (1.5 / 30.0) / 27.0 * 1.25
}

// parity: CaravanTests.aForwarderTakesCratesBoundOnAndTheirRunLoadsThemFirst
#[test]
fn a_forwarder_takes_crates_bound_on_and_their_run_loads_them_first() {
    let mut s = network(true, 5.0);
    let (shipper, hub, buyer) = (0, 1, 2);
    let onward = 1;
    let mut cargo = vec![crate_of_paper(shipper, buyer)];
    let mut purse = 1000.0;
    assert_eq!(s.hand_over(hub, &mut cargo, &mut purse), 0, "no draymen to cart it in");
    assert!(cargo.len() == 1 && purse == 1000.0, "it stays aboard, nothing paid");
    let leg = s.leg(onward, true);
    s.book(hub, cargo[0].clone(), leg, 1);
    let mut h = s.hub(hub);
    let load = h.load_booked(s.content(), &leg, 27);
    assert_eq!(load.len(), 1, "booked, it loads for its run");
    assert_eq!(load[0].booked.map(|b| (b.leg, b.stacks)), Some((leg, 1)), "one stack booked on the run out");
    assert!(load[0].shipment.consigned_to(buyer), "and still made out to its buyer");
}

// parity: CaravanTests.theForwarderBuysABlockOfSpaceItFillsAndPaysForEachRun
#[test]
fn the_forwarder_buys_a_block_of_space_it_fills_and_pays_for_each_run() {
    let mut s = network(false, 20.0);
    let (hub, carrier) = (1, 2);
    let calling = 1;
    let home = s.leg(calling, false); // the carrier's run home from the hub
    assert!(!home.passing() && home.from() == hub && home.to() == carrier && home.carrier() == carrier);
    s.forward_shipment(hub, crate_of_paper(0, carrier), true);
    assert_eq!(s.forward(hub), 0, "no shop yet: it waits");
    assert!(!s.has_block(hub, &home), "no block yet");
    assert_eq!(s.hub(hub).wanted_on.get(&home.key()), Some(&1.0), "and it remembers what it wanted: a stack");
    s.buy_blocks(hub);
    assert!(s.has_block(hub, &home), "a block on the carrier's run home");
    let block = s.hub(hub).blocks[&home.key()].clone();
    assert_eq!((block.stacks, block.runs), (1, 5), "min(27 x 0.5, 1) stacks, for 5 runs");
    let leg_price = stack_price_200() * 0.25; // home: a backhaul
    assert!(close(s.leg_price(&home), leg_price), "{} against {leg_price}", s.leg_price(&home));
    assert!(close(block.price, 0.8 * leg_price), "at 0.8 of the leg's price: {}", block.price);
    assert_eq!(s.forward(hub), 1, "booked on its block, no shop needed");
    assert!(s.hub(hub).held.iter().any(|h| h.booked.is_some_and(|b| b.leg == home && b.stacks == 1 && b.block)));
    let before = s.town(hub).money;
    let paid = s.run_block(hub, &home);
    assert!(paid > 0.0 && close(paid, 0.8 * leg_price), "the run loads: the forwarder pays for its block, {paid}");
    assert!(close(before - s.town(hub).money, paid), "out of the hub's treasury");
    assert_eq!(s.hub(hub).blocks[&home.key()].runs, 4, "a run counted down");
}

#[test]
fn space_is_priced_by_the_road_food_over_what_the_wagon_carries() {
    let s = network(true, 5.0);
    let out = s.leg(1, true);
    assert!(close(s.stack_price(1), stack_price_200()), "{}", s.stack_price(1));
    assert!(close(s.leg_price(&out), stack_price_200()), "out from the hub: the whole price");
    assert!(close(s.forwarding_price(&out), stack_price_200() * 1.2), "the forwarder's fifth on top");
    let c = s.content();
    assert!(close(freight::cart_price(c), 0.5 / 16.0 * 4.0 * 1.25), "a cart: four stacks of a drayman's day, and the margin");
    assert_eq!(freight::carts_per_day(c, 0), 4, "one drayman: 16 stacks a day, four carts");
    assert_eq!(freight::carts_per_day(c, 3), 16, "three idle hands and one more");
    assert_eq!(freight::carts_for(1), 1, "a stack in and out: two stacks, a cart");
    assert_eq!(freight::carts_for(9), 5, "nine stacks: eighteen, five carts");
    assert!(close(freight::run_cost(c, 200), 20.0 * 2.0 * 2.0 * 0.05), "a usual squad's food both ways");
    assert!(close(freight::stop_fee(c, 0.0, 16.0), 0.25), "a landing a quarter emerald at least");
    assert!(close(freight::stop_fee(c, 100.0, 16.0), 25.0 / 16.0), "or a quarter of passing the posts: 25 spurs");
}

#[test]
fn with_draymen_free_the_forwarder_takes_the_crate_for_its_fee() {
    let mut s = network(true, 5.0);
    let (shipper, hub, buyer) = (0, 1, 2);
    s.restock_hub(hub);
    assert_eq!(s.hub(hub).carts, 4, "a hub without a roster: one drayman, four carts");
    let start = s.totals();
    let mut cargo = vec![crate_of_paper(shipper, buyer)];
    let mut purse = 10.0;
    let money = s.town(hub).money;
    assert_eq!(s.hand_over(hub, &mut cargo, &mut purse), 1, "a stack handed over");
    assert!(cargo.is_empty(), "off the wagon");
    // The cheapest leg's forwarding a stack and a cart in and out (3 spurs, at 16) with the fifth, in whole spurs.
    let fee = (stack_price_200() * 1.2 * 16.0 + 3.0 * 1.2).ceil() / 16.0;
    assert!(close(10.0 - purse, fee) && close(s.town(hub).money - money, fee), "paid {} into the hub, {fee}", 10.0 - purse);
    let h = s.hub(hub);
    assert_eq!((h.carts, h.book.forwarded, h.book.carts), (3, 1, 1), "a cart of the day's four used");
    assert!(h.held.len() == 1 && h.held[0].carted && h.held[0].booked.is_none(), "in the forwarder's hands, carted in");
    assert!(close(h.book.drayage, 3.0 / 16.0), "the draymen's 3 spurs");
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
    let book = s.cash_book(hub, 1);
    assert!(book.iter().any(|(_, what, v)| what == "forwarding" && *v > 0.0)
        && book.iter().any(|(_, what, v)| what == "drayage" && close(*v, 3.0 / 16.0)), "{book:?}");
}

#[test]
fn the_draymen_are_the_idle_hands_and_one_more() {
    let mut s = network(true, 5.0);
    let hub = 1;
    s.set_roster(hub, &[("merchant", 2), ("none", 1), ("nitwit", 1), ("none", 1), ("farmer", 1)]);
    s.restock_hub(hub);
    assert_eq!(s.hub(hub).carts, 16, "three idle hands and one more, four carts each");
    let out = s.leg(1, true);
    assert_eq!(s.hub(hub).tickets.get(&out.key()), Some(&13), "half a wagon's space for sale on its run");
}

#[test]
fn a_forwarder_books_tickets_on_the_soonest_run_and_the_carrier_is_paid_as_it_loads() {
    // A carrier from beyond calls at the hub's far end; the forwarder holds a crate for it and the shops are stocked.
    let mut s = network(false, 20.0);
    let (hub, carrier) = (1, 2);
    let home = s.leg(1, false);
    s.forward_shipment(hub, crate_of_paper(0, carrier), true);
    s.restock_hub(hub);
    let money = s.town(hub).money;
    assert_eq!(s.forward(hub), 1, "a ticket bought at the carrier's shop");
    let h = s.hub(hub);
    let booking = h.held[0].booked.expect("booked");
    let fare = (stack_price_200() * 0.25 * 16.0).ceil().max(1.0) / 16.0; // the shop's price in whole spurs
    assert!(booking.leg == home && !booking.block && close(booking.fare, fare), "{booking:?} against {fare}");
    assert_eq!(h.tickets[&home.key()], 12, "one of its 13 tickets sold");
    assert_eq!(s.town(hub).money, money, "the fare is paid as the run loads, not before");
}

#[test]
fn a_crate_stays_aboard_where_no_carrier_runs_its_way() {
    let mut s = network(true, 5.0);
    let (shipper, hub) = (0, 1);
    let elsewhere = s.add_town("Elsewhere", 0.0);
    s.restock_hub(hub);
    let mut cargo = vec![crate_of_paper(shipper, elsewhere), crate_of_paper(shipper, hub)];
    let mut purse = 10.0;
    assert_eq!(s.hand_over(hub, &mut cargo, &mut purse), 0, "no run to Elsewhere, and one made out to the hub itself");
    assert!(cargo.len() == 2 && purse == 10.0, "both stay aboard, nothing paid");
}

/// A straight road east, 10 hexes, from A to B, with T half way on it, and a road back: T sees both pass.
fn pass_map(t_money: f64, t_bread: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let a = s.add_town("A", 400.0);
    let t = s.add_town("T", t_money);
    let b = s.add_town("B", 400.0);
    for (town, q) in [(a, 0), (t, 5), (b, 10)] {
        let hex = Hex::new(q, 0);
        s.place_town(town, hex.center_x(), hex.center_z());
    }
    // A ridge across the land, cliffs from north to south, the pass at T's own hex.
    for r in -20..=20 {
        if r != 0 {
            s.set_ground(Hex::new(5, r), 0.0, 40.0);
        }
    }
    // A makes tools B uses; B makes cloth A uses; both have bread for the road.
    s.set_trade(a, TOOLS, 40.0, 0.0);
    s.set_trade(b, TOOLS, 0.0, 20.0);
    s.set_trade(b, CLOTH, 60.0, 0.0);
    s.set_trade(a, CLOTH, 0.0, 15.0);
    s.deliver(a, TOOLS, 100.0);
    s.deliver(b, CLOTH, 100.0);
    for town in [a, b] {
        s.deliver(town, BREAD, 600.0);
    }
    s.deliver(t, BREAD, t_bread);
    s
}

#[test]
fn a_town_the_roads_pass_becomes_a_hub() {
    let mut s = pass_map(20.0, 100.0);
    let (a, t, b) = (0, 1, 2);
    let ab = s.add_road(a, b, 1.0).expect("a road through the pass");
    let path = s.route_path(ab).unwrap();
    assert!(path.contains(&Hex::new(5, 0)), "through T's hex: {path:?}");
    s.add_road(b, a, 1.0).expect("and one back");
    let money = s.town(t).money;
    s.advance(LOOK);
    assert!(s.is_hub(t), "two routes of others pass: a hub");
    assert!(close(money - s.town(t).money, 4.0), "its storehouse cost 4 emeralds");
    assert!(s.cash_book(t, 1).iter().any(|(_, what, v)| what == "storehouse" && close(*v, -4.0)));
    assert!(s.whys(t).iter().any(|w| w.line.contains("a freight hub now: 2 routes of others pass through, 0 call")),
        "{:?}", s.whys(t));
    assert_eq!(s.stop_on(ab), Some((t, 4)), "A's road calls at T: the first hex of its path within a hex of T's");
}

#[test]
fn a_town_needs_the_routes_the_food_the_money_and_its_merchant_to_become_a_hub() {
    let (a, t, b) = (0, 1, 2);
    // One route passing, none calling: not enough.
    let mut s = pass_map(20.0, 100.0);
    s.add_road(a, b, 1.0).unwrap();
    s.advance(LOOK);
    assert!(!s.is_hub(t), "one route passing");
    // Two passing, but too little food to feed callers (320 points).
    let mut s = pass_map(20.0, 60.0);
    s.add_road(a, b, 1.0).unwrap();
    s.add_road(b, a, 1.0).unwrap();
    s.advance(LOOK);
    assert!(!s.is_hub(t), "60 bread is 300 points: short of the 320 to feed callers");
    // Too poor for the storehouse.
    let mut s = pass_map(3.5, 100.0);
    s.add_road(a, b, 1.0).unwrap();
    s.add_road(b, a, 1.0).unwrap();
    s.advance(LOOK);
    assert!(!s.is_hub(t), "3.5 emeralds: short of the storehouse's 4");
    // A roster with no Merchant at its desk.
    let mut s = pass_map(20.0, 100.0);
    s.set_roster(t, &[("farmer", 1), ("none", 1)]);
    s.add_road(a, b, 1.0).unwrap();
    s.add_road(b, a, 1.0).unwrap();
    s.advance(LOOK);
    assert!(!s.is_hub(t), "no Merchant: no hub");
    // One passing and two calling: three routes on its land.
    let mut s = pass_map(20.0, 100.0);
    s.add_road(a, b, 1.0).unwrap();
    s.add_road(t, a, 1.0).unwrap();
    s.add_road(b, t, 1.0).unwrap();
    s.advance(LOOK);
    assert!(s.is_hub(t), "one passing and two calling");
}

#[test]
fn a_route_calls_at_the_hub_on_its_way_out_and_home() {
    let mut s = pass_map(20.0, 100.0);
    let (a, t, b) = (0, 1, 2);
    let ab = s.add_road(a, b, 1.0).unwrap();
    s.make_hub(t);
    let start = s.totals();
    s.advance(LOOK);
    assert!(s.route(ab).on_the_road, "A's wagon left");
    let stop = s.wagon_freight(ab).and_then(|f| f.stop).expect("a stop on its way");
    assert_eq!(stop.hub, t);
    let (travel, _) = s.route_road(ab);
    assert_eq!(travel, 800 * 20, "10 hexes of 80 blocks, 20 ticks a block");
    assert_eq!(stop.remaining, travel - travel * 4 / 10, "four tenths along: the rest kept for after the stop");
    let t_money = s.town(t).money;
    s.advance(LOOK + travel * 4 / 10 + LOOK);
    assert!(s.wagon_freight(ab).and_then(|f| f.stop).is_some_and(|x| x.called), "it called at T");
    assert_eq!(s.route_freight(ab).stops, 1);
    assert!(close(s.town(t).money - t_money, 0.25), "and paid its landing, a quarter emerald");
    // On to B and home again, calling at T on the way back.
    for day in 1..=4 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    assert!(s.route(ab).trips >= 1, "a trip made");
    assert!(s.route_freight(ab).stops >= 2, "called out and home: {}", s.route_freight(ab).stops);
    assert!(s.known(a).contains(&t), "A knows the hub now");
}

#[test]
fn routes_crossing_a_pass_town_make_it_a_hub_and_it_opens_its_own_runs() {
    let mut s = pass_map(30.0, 400.0);
    let (a, t, b) = (0, 1, 2);
    s.add_road(a, b, 1.0).unwrap();
    s.add_road(b, a, 1.0).unwrap();
    let start = s.totals();
    let routes = s.route_count();
    let mut day = 0;
    while s.route_count() == routes && day < 80 {
        day += 1;
        s.advance(day * DAY);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    assert!(s.is_hub(t), "the pass town became a hub");
    assert!(s.route_count() > routes, "and opened a run of its own by day 80");
    let opened = s.route_count() - 1;
    let (from, to) = s.route_ends(opened);
    assert!(from == t && (to == a || to == b), "its own run, to a far end of a route calling there: {from} to {to}");
    assert!(s.whys(t).iter().any(|w| w.line.contains("opened a run of its own")), "{:?}", s.whys(t));
    assert!(s.route(0).trips >= 5 || s.route(1).trips >= 5, "once a route calling there had made five trips: {} and {}",
        s.route(0).trips, s.route(1).trips);
    println!("T opened a run to {to} on day {day}");
}

/// A long run A→B calling at the hub H, which runs on to B: weighed against a short run to H and freight from there.
fn reorganising(long_blocks: u32, short_blocks: u32, trips: u32, onward_trips: u32, carried: f64) -> Network {
    let (a, h, b) = (0, 1, 2);
    let run = |route, from, to, blocks: u32, trips, path: Vec<Hex>| Run {
        route, from, to, blocks, travel_ticks: blocks as u64 * 20, road_cost: 0.0, every_ticks: DAY, last_departure: None,
        path, trips, carried, crew: 2.0, capacity: 27, wagon: None, stop: None,
    };
    let hexes = [(a, Hex::new(0, 0)), (h, Hex::new((short_blocks / 80) as i32, 0)), (b, Hex::new((long_blocks / 80) as i32, 0))];
    let path = Hex::line(hexes[0].1, hexes[2].1);
    let onward = Hex::line(hexes[1].1, hexes[2].1);
    Network::new(vec![run(0, a, b, long_blocks, trips, path), run(1, h, b, long_blocks - short_blocks, onward_trips, onward)],
        [h].into_iter().collect(), hexes.into_iter().collect())
}

#[test]
fn the_owner_of_a_long_run_winds_it_down_when_the_hub_costs_three_quarters_or_less() {
    use enterprise_core::rules::freight::{reorganise, Reorganise};
    let c = content();
    let territory = Default::default();
    let takings = Default::default();
    let net = reorganising(800, 400, 6, 3, 600.0);
    assert_eq!(net.stop_on(0), Some((1, 4)), "A's run calls at H, the hex before it");
    // H runs to B already and to A not: it opens a run to A first.
    match reorganise(&c, &net, &territory, &takings, 1, 16.0, &[], &[]) {
        Some(Reorganise::Open { end, caller }) => assert_eq!((end, caller), (0, 0), "a run of its own to A"),
        other => panic!("{other:?}"),
    }
    // With that run open (pending), the owner weighs its long run: 16 + 0.25 a trip direct, against 8 + 0.25 to the
    // hub and two stacks' freight on (a usual load of 100 items a trip).
    let Some(Reorganise::WindDown { route, owner, to, by_hub, direct, open }) =
        reorganise(&c, &net, &territory, &takings, 1, 16.0, &[(1, 0)], &[]) else { panic!("it winds down") };
    assert_eq!((route, owner, to, open), (0, 0, 2, true), "A's run to B, a short run to H opened");
    assert!(close(direct, 80.0 * 2.0 * 2.0 * 0.05 + 0.25), "{direct}");
    let fwd = 40.0 * 2.0 * 2.0 * 0.05 / 27.0 * 1.25 * 1.2;
    let cart = 0.5 / 16.0 * 4.0 * 1.25 * 2.0 / 4.0 * 1.2;
    assert!(close(by_hub, 40.0 * 2.0 * 2.0 * 0.05 + 0.25 + 2.0 * (fwd + cart)), "{by_hub}");
    assert!(by_hub <= 0.75 * direct);
    // Not while the hub's run on hasn't made three trips, nor the long run five.
    assert!(!matches!(reorganise(&c, &reorganising(800, 400, 6, 2, 600.0), &territory, &takings, 1, 16.0, &[(1, 0)], &[]),
        Some(Reorganise::WindDown { .. })), "the hub doesn't run there reliably yet");
    // Nor when the hub sits so near B that the short run costs as much as the long.
    assert!(!matches!(reorganise(&c, &reorganising(800, 720, 6, 3, 600.0), &territory, &takings, 1, 16.0, &[(1, 0)], &[]),
        Some(Reorganise::WindDown { .. })), "by the hub dearer than three quarters");
}

// The min-cost-flow yardstick (`docs/core-math.md`, "Min-cost network flow": still a maybe): three towns round a hub,
// each shipping two stacks a trip to each of the others. The optimum is a tiny network design solved whole: each pair
// of towns by its own road (a long run carries both ways) or by the hub (the two towns' runs to the hub, shared with
// the pair the other way round, and the carting there); every choice tried. The hub's routing as the core prices it
// (each town's own run to the hub, then the forwarder's fee a stack: the backhaul on the other town's run home, the
// carting and the fifth on top) should sit within the forwarder's margins of that optimum, and at most three quarters
// of the direct roads (the reorganising's switch), which pay the long runs' extra length.
#[test]
fn a_hubs_routing_of_three_towns_shipments_against_the_direct_roads() {
    let c = content();
    let spoke = 800u32; // each town 10 hexes from the hub, 120 degrees apart: the towns 800 x sqrt(3) apart
    let across = (spoke as f64 * 3f64.sqrt()).round() as u32;
    let stacks = 2.0; // a town's freight to each other town, a trip
    let carting = freight::cart_price(&c) * 2.0 / freight::CART as f64 * 1.2; // a stack carted in and out, the fifth
    let pairs = [(0, 1), (0, 2), (1, 2)];
    // Every choice of direct road or hub for each pair: the roads it runs, the spokes its hub pairs need, the carting.
    let mut optimum = f64::INFINITY;
    let mut best = 0u32;
    for choice in 0..8u32 {
        let mut cost = 0.0;
        let mut spokes = [false; 3];
        for (i, &(x, y)) in pairs.iter().enumerate() {
            if choice & (1 << i) != 0 {
                spokes[x] = true;
                spokes[y] = true;
                cost += 2.0 * stacks * carting;
            } else {
                cost += freight::run_cost(&c, across);
            }
        }
        cost += spokes.iter().filter(|s| **s).count() as f64 * freight::run_cost(&c, spoke);
        if cost < optimum {
            optimum = cost;
            best = choice;
        }
    }
    let direct = 3.0 * freight::run_cost(&c, across);
    let run = Run { route: 0, from: 0, to: 3, blocks: spoke, crew: 2.0, capacity: 27, ..Default::default() };
    let backhaul = freight::stack_price(&c, &run) * c.dials.backhaul_share * (1.0 + c.dials.forward_margin);
    let priced = 3.0 * freight::run_cost(&c, spoke) + 6.0 * stacks * (backhaul + carting);
    println!("direct {direct:.2}, by the hub as priced {priced:.2}, the optimum {optimum:.2} (choice {best:03b})");
    assert_eq!(best, 0b111, "the optimum sends every pair by the hub");
    assert!(close(direct / (3.0 * freight::run_cost(&c, spoke)), 139.0 / 80.0), "the long runs' extra length: 139 points to 80");
    assert!(priced >= optimum && priced <= optimum * 1.06, "within the forwarder's margins of the optimum: {priced} to {optimum}");
    assert!(priced <= 0.75 * direct, "and at most three quarters of the direct roads: {priced} against {direct}");
}

#[test]
fn a_hubs_own_run_loads_the_crates_booked_on_it_first() {
    let mut s = network(true, 10.0);
    let (hub, buyer) = (1, 2);
    let onward = 1;
    // The buyer uses tools; the hub has some to sell, and bread for the road.
    s.set_trade(buyer, TOOLS, 0.0, 30.0);
    s.deliver(hub, TOOLS, 64.0);
    s.deliver(hub, BREAD, 64.0);
    s.deliver(buyer, BREAD, 64.0);
    let leg = s.leg(onward, true);
    let cloth = Shipment { good: CLOTH, count: 64.0, shipper: 0, consignee: Some(buyer), contract: None, settled: true,
        copy: false };
    s.book(hub, cloth.clone(), leg, 1);
    let start = s.totals();
    s.advance(LOOK);
    assert!(s.route(onward).on_the_road, "the hub's run left: {:?} {:?}", s.road_food(onward).stayed, s.town(hub));
    let (aboard, _) = s.wagon_shipments(onward);
    assert!(aboard.contains(&cloth), "with the booked crate aboard: {aboard:?}");
    assert!(s.wagon_freight(onward).unwrap().booked.contains(&cloth), "on its ticket");
    assert!(s.hub(hub).held.is_empty() && s.hub(hub).book.carried == 1, "out of the forwarder's hands");
    let cloth_before = s.town(buyer).stock[CLOTH];
    s.advance(DAY);
    assert_eq!(s.town(buyer).stock[CLOTH] - cloth_before, 64.0, "received by its consignee");
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_wagon_to_a_hub_sends_on_what_its_town_owes_markets_beyond_and_the_forwarder_buys_its_bill() {
    let mut s = network(true, 30.0);
    let (shipper, hub, buyer) = (0, 1, 2);
    s.set_trade(buyer, PAPER, 0.0, 30.0);
    for t in [shipper, hub, buyer] {
        s.deliver(t, BREAD, 128.0);
    }
    s.deliver(shipper, PAPER, 256.0);
    s.deliver(hub, PAPER, 128.0);
    // The shipper owes the buyer 64 paper a week, DAP, both holding copies; it has no road there, the hub has.
    let id = s.draft(shipper, buyer, PAPER, 64.0, 1.0 / 24.0, 0, 7 * DAY, 4, enterprise_core::Incoterm::Dap, true);
    let start = s.totals();
    for look in 1..=40 {
        s.advance(look * LOOK);
        s.conserved(&start).unwrap_or_else(|e| panic!("look {look}: {e}"));
    }
    let h = s.hub(hub);
    assert!(h.book.forwarded >= 1, "the forwarder took the crate: {:?}", h.book);
    assert!(h.book.bills >= 1, "and bought the shipper's bill on the buyer: {:?}", h.book);
    let c = s.contract(id);
    assert!(c.delivered >= 64.0 || c.shipped >= 64.0, "under way or delivered: {c:?}");
    for day in 2..=10 {
        s.advance(day * DAY);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    let c = s.contract(id);
    assert!(c.delivered >= 64.0, "delivered by the hub's run: {c:?}");
}

#[test]
fn the_network_reorganises_round_the_pass_and_a_long_run_is_wound_down() {
    let mut s = pass_map(30.0, 400.0);
    let (a, t, b) = (0, 1, 2);
    // The pass town makes tools for B and cloth for A, so its own runs have loads.
    s.set_trade(t, TOOLS, 30.0, 0.0);
    s.set_trade(t, CLOTH, 30.0, 0.0);
    let ab = s.add_road(a, b, 1.0).unwrap();
    let ba = s.add_road(b, a, 1.0).unwrap();
    let start = s.totals();
    let mut day = 0;
    while s.route_freight(ab).wound_down.is_none() && s.route_freight(ba).wound_down.is_none() && day < 120 {
        day += 1;
        s.advance(day * DAY);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    let routes: Vec<(usize, usize)> = (0..s.route_count()).map(|r| s.route_ends(r)).collect();
    println!("day {day}: routes {routes:?}, trips {:?}", (0..s.route_count()).map(|r| s.route(r).trips).collect::<Vec<_>>());
    assert!(routes.contains(&(t, a)) && routes.contains(&(t, b)), "the pass town runs to both ends");
    let (long, owner) = if s.route_freight(ab).wound_down.is_some() { (ab, a) } else { (ba, b) };
    assert!(s.route_freight(long).wound_down.is_some(), "a long run through the pass wound down by day 120");
    assert!(routes.contains(&(owner, t)), "and its owner runs to the pass town instead");
    assert!(s.whys(owner).iter().any(|w| w.line.contains("wound down its run to") && w.line.contains("by T it costs")),
        "the reason in its books: {:?}", s.whys(owner));
    // It plans no more wagons on the long run.
    let trips = s.route(long).trips;
    for d in day + 1..=day + 10 {
        s.advance(d * DAY);
    }
    assert!(s.route(long).trips <= trips + 1 && !s.route(long).on_the_road, "the long run stays home once its wagon is in");
}

#[test]
fn going_out_a_wagon_buys_its_road_home_food_at_the_stop_where_the_far_end_has_none() {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let a = s.add_town("A", 400.0);
    let t = s.add_town("T", 10.0);
    let b = s.add_town("B", 400.0);
    for (town, q) in [(a, 0), (t, 5), (b, 10)] {
        let hex = Hex::new(q, 0);
        s.place_town(town, hex.center_x(), hex.center_z());
    }
    s.set_trade(a, TOOLS, 40.0, 0.0);
    s.set_trade(b, TOOLS, 0.0, 20.0);
    s.deliver(a, TOOLS, 100.0);
    // A has the way out's food (80 points) and half again, not two legs' worth; B has none; T has plenty.
    s.deliver(a, BREAD, 24.0);
    s.deliver(t, BREAD, 200.0);
    let ab = s.add_road(a, b, 1.0).unwrap();
    s.make_hub(t);
    let start = s.totals();
    s.advance(LOOK);
    assert!(s.route(ab).on_the_road && s.road_food(ab).provisions == 0.0, "it left with no food for the road home");
    let (bread, money) = (s.town(t).stock[BREAD], s.town(t).money);
    s.advance(LOOK + 16_000 * 4 / 10 + LOOK);
    assert_eq!(s.road_food(ab).provisions, 80.0, "it bought the road home's 80 points at T");
    assert_eq!(bread - s.town(t).stock[BREAD], 16.0, "16 bread, out of T's warehouse");
    // The landing (a quarter emerald) and the bread at its worth, in whole spurs: 16 x 1/6 at 16 is 42.7, 43 spurs.
    assert!(close(s.town(t).money - money, 0.25 + 43.0 / 16.0), "{}", s.town(t).money - money);
    s.advance(2 * DAY);
    assert!(s.road_food(ab).short_rations == 0 && s.road_food(ab).waiting_since.is_none(), "home without waiting at B");
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_wagon_calling_at_the_hub_carries_its_contract_copies_on_to_where_its_bound() {
    let mut s = pass_map(20.0, 100.0);
    let (a, t, b) = (0, 1, 2);
    let ab = s.add_road(a, b, 1.0).unwrap();
    s.make_hub(t);
    // A contract drafted at the hub with B: only the hub holds a copy.
    let id = s.draft(t, b, CLOTH, 16.0, 0.5, 10 * DAY, 2 * DAY, 3, enterprise_core::Incoterm::Ddp, false);
    assert_eq!(s.contract(id).state, enterprise_core::State::Drafted);
    s.advance(LOOK);
    assert!(s.route(ab).on_the_road);
    s.advance(LOOK + 16_000 * 4 / 10 + LOOK);
    assert_eq!(s.route_freight(ab).stops, 1, "it called at the hub");
    assert_eq!(s.contract(id).state, enterprise_core::State::Active, "B holds its copy now: in force");
}
