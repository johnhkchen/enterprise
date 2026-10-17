#![allow(dead_code)]
//! Realms (`realms.rs`, `rules/realms.rs`), pinned as the mod's caravan tests pin them: markets off the map, and the
//! Coast and the March beyond the border, with the test network's hub and far end four blocks apart (a point of food a
//! leg), bread at six an emerald for 5 points, paper at 24 an emerald.

use enterprise_core::cash_book::Account;
use enterprise_core::state::Ledger;
use enterprise_core::bargain;
use enterprise_core::{Content, Good, Incoterm, Output, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const LEATHER: usize = 2;
const WHEAT: usize = 3;
const BOOK: usize = 4;
const WOOL: usize = 5;
const CARROT: usize = 6;
const IRON: usize = 7;
const PIPE: usize = 8;
const LEAF: usize = 9;
const COCOA: usize = 10;
const HUB: usize = 0;
const DEST: usize = 1;


fn content() -> Content {
    let reading = Content::COMFORTS.iter().position(|k| *k == "reading").unwrap();
    Content::new(vec![
        Good::new("paper", 1.0 / 24.0, 64),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("leather", 1.0 / 6.0, 64),
        Good::new("wheat", 1.0 / 12.0, 64).food(2.0),
        Good::new("book", 2.0, 16).comfort(reading),
        Good::new("white_wool", 1.0 / 8.0, 64),
        Good::new("carrot", 1.0 / 12.0, 64).food(3.0),
        Good::new("iron_ingot", 0.5, 64),
        Good::new("pipe_tobacco", 0.25, 64),
        Good::new("tobacco_leaf", 0.1, 64),
        Good::new("cocoa_beans", 0.1, 64),
    ], DAY)
}

/// The mod's test network (`CaravanTests.network`): a hub with 128 paper, `bread` and `emeralds`; a far end four blocks
/// away holding `dest_spurs` (16 an emerald) and two bread, whose lockers take paper up to a stack (its par).
fn network(bread: f64, emeralds: f64, dest_spurs: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", emeralds);
    let dest = s.add_town("Dest", dest_spurs / 16.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 128.0);
    s.deliver(hub, BREAD, bread);
    s.deliver(dest, BREAD, 2.0);
    let _ = (hub, dest);
    s
}

fn bread_at(s: &Society, town: usize) -> f64 {
    s.town(town).stock[BREAD]
}

// parity: CaravanTests.aCaravanDeliversToAMarketOffTheMap
#[test]
fn a_caravan_delivers_to_a_market_off_the_map() {
    let mut s = network(8.0, 5.0, 192.0);
    // The front: 64 blocks off, no one there, 1,000 spurs in its account, no food to sell, 20 bread for the road home.
    let front = s.add_off_map("the front", 64.0, 0.0, 1000.0 / 16.0);
    s.deliver(front, BREAD, 20.0);
    assert!(s.is_off_map(front) && !s.is_realm(front) && !s.is_off_map(HUB));
    let route = s.add_route(HUB, front, 64.0 / 1200.0, 0.0, 4.0);
    let contract = s.draft(HUB, front, PAPER, 32.0, 0.1, 0, DAY, 1, Incoterm::Dap, true);
    let money = s.town(front).money;
    let mut fed_by_the_wagon = false;
    let mut t = 0;
    while t < 4_000 {
        t += LOOK;
        s.advance(t);
        // It packs the road home's food: the front sells none (`OffMap.sells`), however much bread it holds.
        fed_by_the_wagon |= s.route(route).on_the_road && s.road_food(route).provisions > 0.0;
    }
    assert_eq!(s.contract(contract).delivered, 32.0, "delivered: {:?}", s.contract(contract));
    assert_eq!(s.town(front).stock[PAPER], 32.0, "into its warehouse on the ledger");
    assert!(s.town(front).money < money, "and paid for from its account: {} of {money}", s.town(front).money);
    assert!(fed_by_the_wagon, "the wagon carried its own food both ways");
    assert_eq!(bread_at(&s, front), 20.0, "and never touched the front's bread");
    assert!(s.route(route).trips >= 1 || s.route(route).on_the_road, "it ran");
}

#[test]
fn an_off_map_market_that_sells_food_feeds_the_wagon_home() {
    let mut s = network(8.0, 5.0, 192.0);
    let front = s.add_off_map("the front", 64.0, 0.0, 1000.0 / 16.0);
    s.deliver(front, BREAD, 20.0);
    s.set_off_map_sells(front, true);
    let route = s.add_route(HUB, front, 64.0 / 1200.0, 0.0, 4.0);
    s.draft(HUB, front, PAPER, 32.0, 0.1, 0, DAY, 1, Incoterm::Dap, true);
    let mut packed = false;
    for t in 1..=6 {
        s.advance(t * LOOK);
        packed |= s.route(route).on_the_road && s.road_food(route).provisions > 0.0;
    }
    assert!(!packed, "it packs nothing: the front feeds it");
    assert!(bread_at(&s, front) < 20.0, "it ate the front's bread: {}", bread_at(&s, front));
}

/// The March's trade at a town of ours that wants leather (a Librarian's locker takes a stack), 640 spurs and a
/// capital if one is named. Returns the world, the town, the March and its route to the town.
fn march(capital: bool, bread: f64) -> (Society, usize, usize, usize, Option<usize>) {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let dest = s.add_town("Dest", 640.0 / 16.0);
    s.record_capacity(dest, 1, &[(LEATHER, 64.0 / 5.0)]);
    s.set_trade(dest, LEATHER, 0.0, 1e-9);
    s.deliver(dest, BREAD, bread);
    let march = s.add_march(64.0, 0.0, 2000.0 / 16.0);
    let crown = capital.then(|| {
        let c = s.add_town("Capital", 10.0);
        s.set_capital(Some(c));
        c
    });
    let route = s.add_route(march, dest, 64.0 / 1200.0, 0.0, 4.0);
    (s, dest, march, route, crown)
}

fn sold(outs: &[Output]) -> f64 {
    outs.iter().filter_map(|o| match o { Output::WagonSold { takings, .. } => Some(*takings), _ => None }).sum()
}

// parity: CaravanTests.aForeignRealmSendsItsOwnTraders
#[test]
fn a_foreign_realm_sends_its_own_traders() {
    let (mut s, dest, march, route, _) = march(false, 20.0);
    assert!(s.is_realm(march) && s.is_off_map(march));
    let mut takings = 0.0;
    let mut t = 0;
    while t < 6_000 && s.route(route).trips == 0 {
        t += LOOK;
        takings += sold(&s.advance(t));
    }
    assert!(s.town(dest).stock[LEATHER] > 0.0, "the March's caravan sold its leather here");
    assert!(s.route(route).trips >= 1, "and went home: {} trips", s.route(route).trips);
    assert!(takings > 0.0, "paid for: {takings}");
}

#[test]
fn a_realm_sends_two_mouths_and_no_squad() {
    let (mut s, _, march, route, _) = self::march(false, 20.0);
    s.advance(LOOK);
    assert!(s.route(route).on_the_road, "it goes at once: no Merchant at a desk, no squad mustered");
    let food = s.road_food(route);
    // 64 blocks: seven points a mouth a leg (`blocks_per_food`), two mouths.
    assert_eq!(food.food_a_leg, 14.0, "two of its people eat: {food:?}");
    assert!(s.crew(route).crew.is_empty() && s.roster(march).hands.is_empty(), "never a hand of ours");
    assert_eq!(food.provisions, 0.0, "the far end holds 20 bread (100 points) to feed them home: {food:?}");
    // With two bread (10 points) it can't feed two mouths, so they pack the road home's food as well.
    let (mut s, _, _, route, _) = self::march(false, 2.0);
    s.advance(LOOK);
    assert_eq!(s.road_food(route).provisions, 14.0, "{:?}", s.road_food(route));
}

#[test]
fn a_realms_merchants_haggle_at_level_four() {
    let (mut s, dest, _, route, _) = march(false, 20.0);
    let mut gross = 0.0;
    let mut t = 0;
    while t < 3_000 {
        t += LOOK;
        gross += sold(&s.advance(t));
    }
    let n = s.town(dest).stock[LEATHER];
    assert_eq!(n, 64.0, "a stack, the far end's par");
    let value = s.content().goods[LEATHER].value;
    let shortage = bargain::shortage((n / 2.0).floor(), s.par(dest, LEATHER, false).max(n));
    let at4 = n * bargain::price(value, shortage, 4, 1).unwrap();
    let at1 = n * bargain::price(value, shortage, 1, 1).unwrap();
    assert!((gross - at4).abs() < 1e-9 && at4 > at1, "bargained at 4 to its 1: {gross} (4: {at4}, 1: {at1})");
    let _ = route;
}

#[test]
fn the_crown_takes_customs_on_what_a_foreign_trader_sells() {
    let (mut s, dest, march, route, capital) = march(true, 20.0);
    let capital = capital.unwrap();
    let home = s.town(march).money;
    let mut t = 0;
    while s.route(route).trips == 0 {
        t += LOOK;
        s.advance(t);
        assert!(t < 12_000);
    }
    // 64 leather at the 4-to-1 price: 10% of what the wagon took goes to the capital, the rest home.
    let value = s.content().goods[LEATHER].value;
    let paid = 64.0 * bargain::price(value, bargain::shortage(32.0, s.par(dest, LEATHER, false).max(64.0)), 4, 1).unwrap();
    let duty = s.town(capital).money - 10.0;
    assert!((duty - 0.1 * paid).abs() < 1e-9, "10% to the crown's treasury: {duty} of {paid}");
    assert!((s.realm(march).unwrap().customs_paid - duty).abs() < 1e-12, "and the realm's books say so");
    assert!(s.cash_book(capital, 2).iter().any(|(_, what, v)| what == "customs" && (*v - duty).abs() < 1e-9), "in its cash book");
    let net = s.town(march).money - home;
    assert!(net > 0.0 && net < paid, "the March kept the rest of its takings after its road food: {net}");
}

#[test]
fn with_no_capital_named_no_customs_are_paid() {
    let (mut s, _, march, route, _) = march(false, 20.0);
    let mut t = 0;
    while s.route(route).trips == 0 {
        t += LOOK;
        s.advance(t);
        assert!(t < 12_000);
    }
    assert_eq!(s.realm(march).unwrap().customs_paid, 0.0);
    let t1 = s.town(march).money;
    assert!(t1 > 2000.0 / 16.0 - 2.0, "all its takings came home: {t1}");
}

#[test]
fn the_coast_opens_with_three_days_goods_and_tops_up_what_it_makes() {
    let mut s = Society::new(content());
    let coast = s.add_coast(0.0, 0.0, 100.0);
    let view = s.realm(coast).unwrap();
    assert_eq!(view.wants, [WHEAT, PAPER, IRON, BOOK], "wheat, paper, iron, books");
    assert_eq!(view.makes.iter().map(|m| (m.good, m.per_day, m.keep)).collect::<Vec<_>>(),
        [(PIPE, 32.0, 256.0), (LEAF, 48.0, 384.0), (COCOA, 16.0, 128.0), (BREAD, 64.0, 192.0)]);
    let stock = s.town(coast).stock;
    assert_eq!((stock[PIPE], stock[LEAF], stock[COCOA], stock[BREAD]), (96.0, 144.0, 48.0, 192.0),
        "three days of each, none over its keep");
    assert_eq!(s.world.resource::<Ledger>().made[PIPE], 96.0, "made through the ledger");
    s.advance(DAY);
    let stock = s.town(coast).stock;
    assert_eq!((stock[PIPE], stock[LEAF], stock[COCOA], stock[BREAD]), (128.0, 192.0, 64.0, 192.0),
        "a day on: a day's more, bread at its keep");
    s.advance(10 * DAY);
    let stock = s.town(coast).stock;
    assert_eq!((stock[PIPE], stock[LEAF], stock[COCOA], stock[BREAD]), (256.0, 384.0, 128.0, 192.0), "and no more than it keeps");
    assert_eq!(s.realm(coast).unwrap().goods_made, 256.0 + 384.0 + 128.0 + 192.0, "its books: every good made up to its keep")
}

#[test]
fn a_realms_people_use_sixty_four_a_day_of_what_they_want_beyond_the_road_food() {
    let mut s = Society::new(content());
    let coast = s.add_coast(0.0, 0.0, 100.0);
    s.deliver(coast, PAPER, 200.0);
    s.deliver(coast, WHEAT, 600.0);
    s.advance(DAY);
    assert_eq!(s.town(coast).stock[PAPER], 136.0, "64 paper used a day");
    assert_eq!(s.town(coast).stock[WHEAT], 536.0, "and 64 wheat");
    s.advance(10 * DAY);
    assert_eq!(s.town(coast).stock[PAPER], 0.0, "paper all used");
    assert_eq!(s.town(coast).stock[WHEAT], 320.0, "but wheat, a food, keeps 640 points for the road: 320 of it at 2 points");
    assert_eq!(s.capacity(coast, PAPER), 64.0, "its demand is 64 a day: its par follows");
    assert_eq!(s.capacity(coast, BREAD), 0.0, "of what it doesn't want, none");
    assert_eq!(s.world.resource::<Ledger>().used[PAPER], 200.0, "through the ledger");
}

#[test]
fn a_realm_buys_what_it_wants_by_par_from_the_money_it_holds_comforts_included() {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 20.0);
    s.deliver(hub, BOOK, 48.0);
    s.deliver(hub, PAPER, 128.0);
    s.deliver(hub, BREAD, 40.0);
    let coast = s.add_coast(64.0, 0.0, 1000.0 / 16.0);
    let money = s.town(coast).money;
    let route = s.add_route(hub, coast, 64.0 / 1200.0, 0.0, 4.0);
    let mut t = 0;
    while s.route(route).trips == 0 {
        t += LOOK;
        s.advance(t);
        assert!(t < 12_000, "it never got home");
    }
    let stock = s.town(coast).stock;
    assert!(stock[BOOK] > 0.0 && stock[PAPER] > 0.0, "books (a comfort) and paper sold to the Coast: {stock:?}");
    assert!(s.town(coast).money < money, "from its money");
}

#[test]
fn goods_and_money_are_conserved_through_a_realms_trade() {
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", 50.0);
    let dest = s.add_town("Dest", 40.0);
    s.record_capacity(dest, 1, &[(LEATHER, 12.8), (PAPER, 12.8)]);
    s.set_trade(dest, LEATHER, 0.0, 1e-9);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 400.0);
    s.deliver(hub, BREAD, 100.0);
    s.deliver(hub, WHEAT, 200.0);
    s.deliver(dest, BREAD, 20.0);
    let capital = s.add_town("Capital", 10.0);
    s.set_capital(Some(capital));
    let (coast, march) = (s.add_coast(300.0, 0.0, 100.0), s.add_march(-300.0, 0.0, 100.0));
    for (a, b) in [(hub, coast), (coast, dest), (march, dest), (march, hub), (hub, march)] {
        s.add_route(a, b, 0.2, 0.1, 3.0);
    }
    let mut before = s.totals();
    for day in 1..=40 {
        s.advance(day * DAY);
        s.conserved(&before).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
        before = s.totals();
    }
    assert!(s.realm(coast).unwrap().goods_made > 0.0 && s.realm(march).unwrap().goods_made > 0.0, "they made goods");
    assert!(s.realm(march).unwrap().customs_paid > 0.0, "and the March paid customs: {:?}", s.realm(march));
}

#[test]
fn realms_survive_a_save() {
    let (mut s, _, march, route, capital) = march(true, 20.0);
    s.advance(2 * LOOK);
    assert!(s.unsaved().is_empty(), "everything of a realm is saved: {:?}", s.unsaved());
    let mut loaded = Society::load(&s.save()).unwrap();
    assert!(loaded.is_realm(march) && loaded.is_off_map(march));
    for t in 3..=20 {
        s.advance(t * LOOK);
        loaded.advance(t * LOOK);
    }
    assert_eq!(s.realm(march), loaded.realm(march));
    assert_eq!(s.route(route).trips, loaded.route(route).trips);
    assert_eq!(s.town(capital.unwrap()).money, loaded.town(capital.unwrap()).money, "the customs, to the bit");
    assert!(s.realm(march).unwrap().customs_paid > 0.0);
    let _ = Account::Outside;
}
