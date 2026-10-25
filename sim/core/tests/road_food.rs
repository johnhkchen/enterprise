//! Road food (`road.rs`, `rules/road.rs`), pinned as the mod's caravan tests pin it: the test network's hub and far end
//! four blocks apart (a point of food a leg), bread at six an emerald for 5 points, paper at 24 an emerald.

use enterprise_core::cash_book::Account;
use enterprise_core::road::{self, Stay};
use enterprise_core::state::Ledger;
use enterprise_core::{Content, Good, Output, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const HUB: usize = 0;
const DEST: usize = 1;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0)], DAY)
}

/// The mod's test network (`CaravanTests.network`): a hub with `paper`, `bread` and `emeralds`; a far end four blocks
/// away holding `dest_spurs` (16 an emerald) and two bread, whose lockers take paper up to a stack (its par).
fn network_with(paper: f64, bread: f64, emeralds: f64, dest_spurs: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", emeralds);
    let dest = s.add_town("Dest", dest_spurs / 16.0);
    // Its librarian's locker takes paper: a par of one stack (5 days of 12.8), its people using next to none.
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    if paper > 0.0 {
        s.deliver(hub, PAPER, paper);
    }
    if bread > 0.0 {
        s.deliver(hub, BREAD, bread);
    }
    s.deliver(dest, BREAD, 2.0);
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

fn network(bread: f64, emeralds: f64, dest_spurs: f64) -> Society {
    network_with(128.0, bread, emeralds, dest_spurs)
}

fn purse(s: &Society) -> enterprise_core::Fixed {
    s.balance(Account::Wagon(0)).unwrap_or_default()
}

#[test]
fn the_network_is_a_point_of_food_a_leg() {
    let s = network(2.0, 5.0, 192.0);
    let food = s.road_food(0);
    assert_eq!((food.blocks, food.food_a_leg), (4, 1.0), "4 blocks: one point a leg");
    assert_eq!(s.route_road(0).0, 80, "80 ticks on the road, 20 a block");
    assert_eq!(s.food_points(HUB), 10.0, "two bread, 5 points each");
}

// parity: CaravanTests.caravanSellsItsCargoAndBuysItsFood
#[test]
fn a_wagon_sells_its_cargo_and_buys_its_food_at_the_far_end() {
    let mut s = network(2.0, 5.0, 192.0);
    let hub_money = s.town(HUB).money;
    s.advance(LOOK);
    assert!(s.route(0).on_the_road && s.road_food(0).home_at.is_none(), "it leaves");
    assert_eq!(s.route(0).carried, 64.0, "with the 64 paper the far end will take (its par, one stack)");
    let out = purse(&s);
    assert!(out == 0.05, "and a purse for the road home's food, a point at 0.05: {out}");
    let fodder = 2.0 * 0.05; // the team's, a point each way, bought on the road (the hub has no hay or wheat)
    assert!(hub_money - s.town(HUB).money - out == fodder,
        "out of the hub's money, the purse and the team's fodder: the way out's food is its bread");
    assert_eq!(s.town(HUB).stock[PAPER], 64.0, "the rest of the paper stays home");
    assert_eq!(s.food_points(HUB), 5.0, "one bread eaten");

    let dest_money = s.town(DEST).money;
    let outs = s.advance(2 * LOOK);
    let takings = outs.iter().find_map(|o| match o { Output::WagonSold { takings, .. } => Some(*takings), _ => None })
        .expect("it sold at the far end");
    assert!((s.town(DEST).stock[PAPER] - 64.0).abs() < 1e-6, "the far end bought paper up to its par of a stack");
    assert_eq!(s.route(0).sold, 64.0);
    let food = s.road_food(0);
    assert!(food.home_at.is_some(), "heads home");
    // A bread eaten for the road home's point: a sixth of an emerald, 2.67 spurs, paid in 3.
    assert_eq!(s.town(DEST).stock[BREAD], 1.0, "one bread of the far end's eaten");
    assert!(food.food_paid == enterprise_core::Fixed::ceil(3.0 / 16.0), "paid for at its worth, in whole spurs: {}", food.food_paid);
    assert!(takings > 0.0 && food.food_paid > 0.0, "a sale and a food purchase");
    let dest_spent = dest_money - s.town(DEST).money;
    assert!(dest_spent == takings - food.food_paid, "the far end's money moved by sales less food: {dest_spent}");
    let back = purse(&s);
    assert!(back == (out + takings - food.food_paid), "the wagon carries its purse plus sales less food");
    let book = s.cash_book(DEST, 1);
    assert!(book.iter().any(|(_, why, v)| why == "road food" && *v == food.food_paid),
        "the far end's income, in its cash book: {book:?}");

    let before = s.town(HUB).money;
    s.advance(3 * LOOK);
    assert!(!s.route(0).on_the_road, "the wagon is home");
    assert!(s.town(HUB).money - before == back, "the hub banks what it carried");
    assert_eq!((s.route(0).trips, s.route(0).sold), (1, 64.0), "one trip, 64 sold");
    assert_eq!(s.town(HUB).stock[PAPER], 64.0, "the paper it didn't send is still at the hub");
    assert_eq!(s.road_food(0).food_eaten, 2.0, "a point each way (`foodEaten`)");
}

// parity: CaravanTests.aTownsWagonWaitsForALoadWorthItsRoadFood
#[test]
fn a_towns_wagon_waits_for_a_load_worth_its_road_food() {
    // A sheet of paper (a 24th of an emerald) isn't worth both legs' food (two points at 0.05): it waits.
    let mut s = network_with(1.0, 8.0, 5.0, 192.0);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "it waits");
    assert_eq!(s.road_food(0).stayed, Some(Stay::WaitingForALoad));
    s.deliver(HUB, PAPER, 127.0);
    // It tries again a quarter day on (`Caravans.dispatch` 438: `recheckAt`, `RECHECK_TICKS`), not at every look; the
    // mod's test polls again a day on.
    s.advance(LOOK + DAY / 4 - LOOK);
    assert!(!s.route(0).on_the_road, "waiting out its recheck");
    s.advance(LOOK + DAY / 4);
    assert!(s.route(0).on_the_road, "two stacks, worth the trip: it goes");
    assert_eq!(s.route(0).carried, 64.0, "a stack, the far end's par");
    assert_eq!(s.road_food(0).stayed, None);
}

#[test]
fn a_wagon_waiting_for_a_load_tries_again_a_quarter_day_on() {
    // `Caravans.dispatch` 433–441: a wagon waiting for a fuller load sets `recheckAt` a quarter day on
    // (`RECHECK_TICKS`), and `poll` 173 doesn't try it till then; each try that sends none is a "no trip" (182–184).
    let mut s = network_with(1.0, 8.0, 5.0, 192.0);
    s.advance(DAY);
    let tries = s.whys(HUB).into_iter().find(|w| w.line == "Dest | - | no trip: waiting for a fuller load").map(|w| w.times);
    assert_eq!(tries, Some(4), "four tries in the day, not twenty: {:?}", s.whys(HUB));
    assert_eq!(s.road_food(0).stayed, Some(Stay::WaitingForALoad), "between tries it says why it's home");
}

// parity: CaravanTests.aHubThatCantFundThePurseStaysHome
#[test]
fn a_hub_that_cant_fund_the_purse_stays_home() {
    let mut s = network(2.0, 0.0, 192.0);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "no purse, no wagon");
    assert_eq!(s.road_food(0).stayed, Some(Stay::CantFund), "the hub can't fund it");
}

// parity: CaravanTests.noFoodNoMoneyNoCaravan
#[test]
fn no_food_and_no_money_means_no_wagon() {
    let mut s = network(0.0, 0.0, 192.0);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "without food or money for it, nobody leaves");
    assert_eq!(s.town(HUB).stock[PAPER], 128.0, "the goods stay at the hub");
    assert_eq!(s.road_food(0).stayed, Some(Stay::CantFund), "it can't fund the trip");
}

// parity: CaravanTests.withoutBreadAWagonBuysItsRoadFood
#[test]
fn without_bread_a_wagon_buys_its_road_food() {
    let mut s = network(0.0, 20.0, 192.0);
    let before = s.town(HUB).money;
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "no bread at home, but money: it goes");
    let spent = before - s.town(HUB).money;
    assert!(spent > purse(&s), "more than its purse went: its road food bought on the way, {spent}");
    assert!(spent == 0.2,
        "a point bought (0.05), the team's two of fodder (0.1) and the purse for the road home (0.05): {spent}");
    let book = s.cash_book(HUB, 1);
    assert!(book.iter().any(|(_, why, v)| why == "road food" && *v == -0.15), "{book:?}");
}

// parity: CaravanTests.foodCapsALegAt3200Blocks
#[test]
fn food_caps_a_leg_at_3200_blocks() {
    let mut s = network(2.0, 5.0, 192.0);
    let c = s.content().clone();
    assert!(road::in_range(&c, 3200), "3,200 blocks takes a full stack of bread");
    assert!(!road::in_range(&c, 3210), "a little further is out of range");
    assert_eq!(road::travel_ticks(1200), 24_000, "1,200 blocks is a day on the road");
    assert_eq!((road::food_needed(&c, 3200), road::food_needed(&c, 3201), road::food_needed(&c, 1)), (320.0, 321.0, 1.0));
    // A route: refused beyond the range; a day's is 24,000 ticks.
    assert!(s.try_add_route(HUB, DEST, 3200.0 / 1200.0, 0.0, 4.0).is_ok());
    assert!(s.try_add_route(HUB, DEST, 3210.0 / 1200.0, 0.0, 4.0).is_err());
    let day = s.add_route(HUB, DEST, 1.0, 0.0, 4.0);
    assert_eq!((s.route_road(day).0, s.road_food(day).blocks), (24_000, 1200));
}

#[test]
fn a_route_beyond_the_range_never_sends_its_wagon() {
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 500.0);
    let dest = s.add_town("Dest", 100.0);
    s.record_capacity(dest, 1, &[(PAPER, 12.8)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 640.0);
    s.deliver(hub, BREAD, 640.0);
    let far = s.add_route(hub, dest, 3.0, 0.0, 1.0); // 3,600 blocks: 360 points a leg
    s.advance(10 * DAY);
    assert_eq!(s.route(far).trips, 0);
    assert!(!s.route(far).on_the_road);
    assert_eq!(s.road_food(far).stayed, Some(Stay::OutOfRange));
    // Without food in the content, its road is money, and it goes.
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64)], DAY));
    let hub = s.add_town("Hub", 500.0);
    let dest = s.add_town("Dest", 100.0);
    s.set_trade(dest, PAPER, 0.0, 12.8);
    s.deliver(hub, PAPER, 640.0);
    assert!(s.try_add_route(hub, dest, 3.0, 0.5, 1.0).is_ok());
    s.advance(10 * DAY);
    assert!(s.route(0).trips > 0);
}

#[test]
fn a_road_too_long_for_a_legs_food_isnt_built() {
    let mut s = Society::new(content());
    let (a, b, c) = (s.add_town("A", 0.0), s.add_town("B", 0.0), s.add_town("C", 0.0));
    s.place_town(a, 0.0, 0.0);
    s.place_town(b, 2000.0, 0.0);
    s.place_town(c, 4000.0, 0.0);
    let near = s.add_road(a, b, 1.0).expect("2,000 blocks: in range");
    assert!(s.road_food(near).blocks <= 3200);
    assert!(s.add_road(a, c, 1.0).is_none(), "4,000 blocks: out of range");
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64)], DAY));
    let (a, c) = (s.add_town("A", 0.0), s.add_town("C", 0.0));
    s.place_town(a, 0.0, 0.0);
    s.place_town(c, 4000.0, 0.0);
    assert!(s.add_road(a, c, 1.0).is_some(), "no food in the content: no range");
}

// parity-partial: CaravanTests.aCaravanGoesHomeOnShortRationsRatherThanWaitForDays (the far end's bread eaten after
// the wagon leaves: the mod's has none from the start, its unknown stocktake counting as plenty, `foodKnownAt`)
#[test]
fn a_wagon_goes_home_on_short_rations_rather_than_wait_for_days() {
    // The far end's two bread go a bread a look to its people: it had food when the wagon left (so the wagon packed
    // none), and has none when it gets there. (The mod's far end has none from the start: its unknown stocktake
    // counts as plenty, `foodKnownAt`; the core reads the far end's warehouse.) The hub's people eat a little bread
    // too, so it keeps its own and sends the far end none.
    let mut s = network(8.0, 5.0, 192.0);
    s.set_trade(DEST, BREAD, 0.0, 20.0);
    s.set_trade(HUB, BREAD, 0.0, 2.0);
    s.advance(LOOK);
    assert!(s.route(0).on_the_road && s.road_food(0).provisions == 0.0, "it left, fed there as far as it knew");
    s.advance(2 * LOOK);
    let waiting = s.road_food(0);
    assert_eq!(s.town(DEST).stock[BREAD], 0.0, "the far end has no food to sell");
    assert!(waiting.waiting_since == Some(2 * LOOK) && waiting.home_at.is_none(), "waits a while: {waiting:?}");
    let sold = s.route(0).sold;
    s.advance(2 * LOOK + 12_000 - LOOK);
    assert!(s.road_food(0).waiting_since.is_some() && s.road_food(0).short_rations == 0, "still waiting");
    assert_eq!(s.route(0).sold, sold, "its business there done once");
    s.advance(2 * LOOK + 12_000);
    let hungry = s.road_food(0);
    assert_eq!(hungry.short_rations, 1, "then goes home hungry: {hungry:?}");
    assert_eq!(hungry.home_at, Some(2 * LOOK + 12_000 + 160), "at half pace: twice its 80 ticks");
    assert_eq!((hungry.food_paid, hungry.food_eaten), (enterprise_core::Fixed::ZERO, 1.0), "only the way out's eaten: none for the road home");
    s.advance(DAY);
    assert_eq!(s.route(0).trips, 1, "home");
}

#[test]
fn a_far_end_short_of_food_gets_wagons_that_pack_their_own() {
    // The far end has no bread: the wagon packs the road home's point, eats it there, and pays the far end nothing.
    let mut bare = Society::new(content());
    bare.content_mut().dials.standing_orders = false;
    let hub = bare.add_town("Hub", 5.0);
    let dest = bare.add_town("Dest", 12.0);
    bare.record_capacity(dest, 1, &[(PAPER, 12.8)]);
    bare.set_trade(dest, PAPER, 0.0, 1e-9);
    bare.deliver(hub, PAPER, 128.0);
    bare.deliver(hub, BREAD, 8.0);
    bare.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    bare.advance(LOOK);
    let out = bare.road_food(0);
    assert_eq!(out.provisions, 1.0, "the road home's point, packed");
    assert_eq!(purse(&bare), 0.0, "and no purse to buy it there");
    assert_eq!(bare.food_points(hub), 35.0, "a bread for both legs' points");
    bare.advance(2 * LOOK);
    let there = bare.road_food(0);
    assert!(there.home_at.is_some() && there.waiting_since.is_none(), "it ate its own and went home: {there:?}");
    assert_eq!((there.food_paid, there.provisions, there.food_eaten), (enterprise_core::Fixed::ZERO, 0.0, 2.0));
}

#[test]
fn wagons_bound_for_a_town_count_what_the_others_will_eat_there() {
    // Two towns send wagons 40 blocks (4 points a leg) to a far end holding one bread (5 points): the first is fed
    // there; the second, counting what the first will eat, packs its own.
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let a = s.add_town("A", 5.0);
    let b = s.add_town("B", 5.0);
    let dest = s.add_town("Dest", 12.0);
    s.record_capacity(dest, 1, &[(PAPER, 12.8)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(dest, BREAD, 1.0);
    for t in [a, b] {
        s.deliver(t, PAPER, 32.0);
        s.deliver(t, BREAD, 8.0);
        s.add_route(t, dest, 40.0 / 1200.0, 0.0, 4.0);
    }
    s.advance(LOOK);
    assert_eq!((s.road_food(0).food_a_leg, s.road_food(0).provisions), (4.0, 0.0), "the first: fed there");
    assert_eq!(s.road_food(1).provisions, 4.0, "the second packs the road home's food");
    assert!(purse(&s) == 0.2, "the first carries 4 points' purse");
}

#[test]
fn a_poor_town_rich_in_bread_packs_its_own_road_food() {
    // No money, 66 bread (330 points: 10 beyond its 320 kept): it packs the road home's food and needs no purse. No
    // team: the mod's would eat home's hay or wheat (`fodderOnHand`), which the core doesn't feed it yet.
    let mut s = network(66.0, 0.0, 192.0);
    s.content_mut().dials.wagon_team = 0;
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "it goes, though it holds no money");
    assert_eq!((s.road_food(0).provisions, purse(&s)), (1.0, enterprise_core::Fixed::ZERO));
    assert_eq!(s.town(HUB).stock[BREAD], 65.0, "a bread for both legs");
    s.advance(2 * LOOK);
    assert_eq!(s.town(DEST).stock[BREAD], 2.0, "the far end's bread untouched");
    assert_eq!(s.road_food(0).food_paid, 0.0);
    // With money, it would rather buy there: the far end's road-food income.
    let mut rich = network(66.0, 5.0, 192.0);
    rich.advance(LOOK);
    assert_eq!(rich.road_food(0).provisions, 0.0);
}

// parity: MoodTests.treatsAreNotRoadFood
#[test]
fn treats_are_not_road_food() {
    let treats = 0;
    let content = Content::new(vec![
        Good::new("cookie", 1.0 / 18.0, 64).comfort(treats).food(2.0),
        Good::new("pumpkin_pie", 0.25, 64).comfort(treats).food(8.0),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0),
    ], DAY);
    let mut ledger = Ledger { used: vec![0.0; 3], ..Default::default() };
    let mut stock = vec![64.0, 16.0, 0.0];
    assert_eq!(road::held(&content, &stock), 0.0, "no road food here");
    assert!(road::eat(&content, &mut ledger, &mut stock, 10.0).is_empty(), "wagons can't eat the treats");
    assert_eq!(stock, [64.0, 16.0, 0.0]);
    stock[2] += 4.0;
    assert_eq!(road::held(&content, &stock), 20.0, "bread they can");
    // Treats alone aren't road food: such content keeps the road's cost in money.
    let only_treats = Content::new(vec![Good::new("cookie", 1.0 / 18.0, 64).comfort(treats).food(2.0)], DAY);
    assert!(!road::road_food(&only_treats) && road::road_food(&content));
}

#[test]
fn eating_takes_whole_items_the_cheapest_a_point_first() {
    let content = Content::new(vec![
        Good::new("steak", 0.5, 64).food(8.0),  // 0.0625 a point
        Good::new("bread", 1.0 / 6.0, 64).food(5.0), // 0.033 a point
    ], DAY);
    let mut ledger = Ledger { used: vec![0.0; 2], ..Default::default() };
    let mut stock = vec![3.0, 2.5];
    let eaten = road::eat(&content, &mut ledger, &mut stock, 14.0);
    assert_eq!(eaten, [(1, 2.0), (0, 1.0)], "both whole bread (10 points), then a steak");
    assert_eq!(stock, [2.0, 0.5]);
    assert_eq!(ledger.used, [1.0, 2.0], "gone from the world through the ledger");
    assert!(road::eat(&content, &mut ledger, &mut stock, 17.0).is_empty(), "16 points held: nothing eaten");
}

#[test]
fn road_food_keeps_the_books_every_day() {
    // The hub makes paper and bakes bread; the far end uses both; wagons eat and buy bread both ways for 60 days.
    let mut s = network(8.0, 20.0, 192.0);
    s.set_trade(HUB, PAPER, 64.0, 0.0);
    s.set_trade(HUB, BREAD, 4.0, 0.0);
    s.set_trade(DEST, BREAD, 0.0, 1.0);
    s.set_trade(DEST, PAPER, 0.0, 12.0);
    let start = s.totals();
    for day in 1..=60 {
        s.advance(day * DAY);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    let food = s.road_food(0);
    assert!(s.route(0).trips > 10 && food.food_eaten > 10.0 && food.food_paid > 0.0, "{food:?}");
}

#[test]
fn without_food_in_the_content_the_road_costs_its_money() {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64)], DAY));
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", 12.0);
    s.set_trade(dest, PAPER, 0.0, 12.8);
    s.deliver(hub, PAPER, 128.0);
    s.add_route(hub, dest, 1.0, 0.5, 4.0);
    s.advance(LOOK);
    assert!(s.route(0).on_the_road);
    assert_eq!(purse(&s), 0.0, "no purse for food");
    assert!(enterprise_core::Fixed::emeralds(5) - s.town(HUB).money == 0.5, "its road_cost paid");
    s.advance(DAY);
    assert_eq!(s.road_food(0).food_eaten, 0.0);
}

#[test]
fn an_advance_leaves_room_for_the_road_food() {
    // The buyer (3 emeralds, no bread) owes an advance of 8/3 it can't wholly cover, on a 40-block road: its purse
    // takes what's left after the road's food, the team's fodder and the purse for the road home (`spare` less
    // `roadPurse`), so the wagon goes; at the far end it pays the advance keeping its road food, the rest by a bill.
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0)], DAY));
    let (mill, scribes) = (s.add_town("Wheatfield", 5.0), s.add_town("Hayford", 3.0));
    s.set_trade(mill, 0, 128.0, 0.0);
    s.set_trade(scribes, 0, 0.0, 0.0);
    s.deliver(mill, BREAD, 20.0);
    s.add_route(scribes, mill, 40.0 / 1200.0, 0.0, 1.0);
    s.add_route(mill, scribes, 40.0 / 1200.0, 0.0, 1.0);
    let id = s.draft_advanced(0, 1, 0, 2.0, 1.0, 3 * DAY, DAY, 4, enterprise_core::Incoterm::Exw, false, false, true);
    s.advance(DAY);
    assert!(s.route(0).trips > 0, "the buyer's wagon went: {:?}", s.road_food(0).stayed);
    let c = s.contract(id);
    assert!(c.advanced == enterprise_core::Fixed::ceil(8.0 / 3.0), "the advance paid, coin and a bill: {}", c.advanced);
    assert!(s.road_food(0).food_paid > 0.0, "and the purse kept the road home's food: {:?}", s.road_food(0));
}

#[test]
fn the_team_eats_home_s_hay_or_spare_wheat_else_its_fodder_is_bought() {
    // The test network with wheat and hay: the team's two points (one a leg) come from home's wheat, not its money; a
    // bale of hay (9 points) is too much for two; a town whose people use wheat keeps it and buys the fodder.
    let goods = |content: &mut Content| {
        content.goods.push(Good::new("wheat", 0.05, 64));
        content.goods.push(Good::new("hay_block", 0.45, 64));
    };
    for keeps in [false, true] {
        let mut c = content();
        goods(&mut c);
        let mut s = Society::new(c);
        s.content_mut().dials.standing_orders = false;
        let hub = s.add_town("Hub", 5.0);
        let dest = s.add_town("Dest", 12.0);
        s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
        s.set_trade(dest, PAPER, 0.0, 1e-9);
        s.deliver(hub, PAPER, 128.0);
        s.deliver(hub, BREAD, 2.0);
        s.deliver(dest, BREAD, 2.0);
        s.deliver(hub, 2, 10.0);
        s.deliver(hub, 3, 3.0);
        if keeps {
            s.set_trade(hub, 2, 0.0, 1e-9);
        }
        s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
        let before = s.town(hub).money;
        s.advance(LOOK);
        assert!(s.route(0).on_the_road);
        let spent = before - s.town(hub).money - purse(&s);
        let (wheat, hay) = (s.town(hub).stock[2], s.town(hub).stock[3]);
        if keeps {
            // (its people's use of a wheat in a billion a day aside)
            assert!((wheat - 10.0).abs() < 1e-6 && hay == 3.0 && spent == 0.1, "bought: {wheat} {hay} {spent}");
        } else {
            assert!((wheat, hay, spent.abs() < 1e-12) == (8.0, 3.0, true), "home's wheat: {wheat} {hay} {spent}");
        }
    }
}

/// A town whose wagon would go but can't pay for the road (`Stay::CantFund`, a load worth the road aboard) saves up for
/// it: a home for its homeless waits (`road_reserve`, kept back with its trading capital). With nothing worth taking it
/// keeps nothing back, and builds. (A survival world's new town built two homes on the money its first trip wanted; its
/// wagon had nothing to take, and the reserve that would have kept its homes from it would have kept them for nothing.)
#[test]
fn a_town_saves_up_for_a_loaded_wagon_it_cant_send_but_not_for_an_empty_one() {
    let homeless = |paper: f64| {
        let mut s = Society::new(content());
        s.content_mut().dials.standing_orders = false;
        let hub = s.add_town("Hub", 20.0);
        let dest = s.add_town("Dest", 1e5);
        s.record_capacity(dest, 1, &[(PAPER, 2048.0 / 5.0)]); // a par of 32 stacks
        s.set_trade(dest, PAPER, 0.0, 1e-9);
        if paper > 0.0 {
            s.deliver(hub, PAPER, paper);
        }
        s.add_route(hub, dest, 1.0, 0.0, 4.0); // a day each way: about 30 emeralds of road food and purse
        s.set_people(hub, 5.0, 0.0, 40.0);
        s
    };
    let mut loaded = homeless(2048.0);
    loaded.advance(5 * DAY);
    assert_eq!(loaded.road_food(0).stayed, Some(Stay::CantFund), "the wagon can't be funded");
    assert_eq!(loaded.town(HUB).people.homes_built, 0, "a home would spend the trip");
    let mut empty = homeless(0.0);
    empty.advance(5 * DAY);
    assert_eq!(empty.town(HUB).people.homes_built, 1, "nothing for the road to save: a home");
}
