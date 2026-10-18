//! What the network study's world (`sim/host`, `dev/experiments/network_study.py`) showed the core got wrong, each
//! pinned by a small world as the mod's caravan tests pin it: the test network's hub and far end four blocks apart (a
//! point of food a leg), paper at 24 an emerald, bread at six an emerald for 5 points.

use enterprise_core::state::{Towns, Trades};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const WHEAT: usize = 2;
const PUMPKIN: usize = 3;
const STICK: usize = 4;
const HUB: usize = 0;
const DEST: usize = 1;

/// A town's `Trades`, read off its entity.
fn trades(s: &Society, t: usize) -> Trades {
    s.world.get::<Trades>(s.world.resource::<Towns>().0[t]).unwrap().clone()
}

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("wheat", 1.0 / 20.0, 64), Good::new("pumpkin", 1.0 / 6.0, 64), Good::new("stick", 1.0 / 32.0, 64)], DAY)
}

/// The mod's test network (`CaravanTests.network`): a hub with `paper` and bread for the road, 5 emeralds for the
/// purse; a far end four blocks away with `dest_money` and two bread, its villagers `hands` (a roster), every buying
/// offer of theirs switched on as the Merchant's screen would (`autoswitch selling`: the trades of the hand's level
/// and below, the ones the villager has), but those of the trades in `off`.
fn network(paper: f64, dest_money: f64, hands: &[(&str, u8)], off: &[&str]) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", dest_money);
    s.set_roster(dest, hands);
    let roster = s.roster(dest);
    let lockers = s.lockers(dest).expect("a roster's lockers");
    for (h, locker) in lockers.hands.iter().enumerate() {
        let trade = &s.content().trades[locker.trade].name;
        if off.contains(&trade.as_str()) {
            continue;
        }
        for (o, offer) in locker.offers.iter().enumerate() {
            if !offer.sells && offer.level <= roster.hands[h].level {
                s.switch(dest, h, o, true);
            }
        }
    }
    s.deliver(hub, PAPER, paper);
    s.deliver(hub, BREAD, 2.0);
    s.deliver(dest, BREAD, 2.0);
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

// parity-partial: CaravanTests.caravanSellsItsCargoAndBuysItsFood (the far end's paper wanted through a trade it has switched on, as the Java fixture's is: no Merchant manages it, so a stack's par; its road food in road_food.rs)
#[test]
fn a_wagon_brings_what_a_trade_switched_on_there_takes() {
    // The far end's Librarian buys paper (its trade switched on, `setEnabled`), and nobody there uses paper: the
    // warehouse's par for a good a switched-on trade takes is a stack (`Warehouse.par(desk, item, true)`, MIN_PAR),
    // with no Merchant managing the locker to record its capacity.
    let mut s = network(128.0, 12.0, &[("librarian", 1)], &[]);
    assert_eq!(trades(&s, DEST).uses[PAPER], 0.0, "nobody there uses paper");
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "it leaves");
    assert_eq!(s.route(0).carried, 64.0, "with the 64 paper the far end will take (its par, one stack)");
    assert_eq!(s.town(HUB).stock[PAPER], 64.0, "the rest of the paper stays home");
    s.advance(3 * LOOK);
    assert_eq!(s.town(DEST).stock[PAPER], 64.0, "the far end bought paper up to its par of a stack");
    assert_eq!(s.route(0).sold, 64.0);
}

#[test]
fn a_managed_lockers_par_is_its_capacity_and_any_other_switched_on_trade_a_stack() {
    // A Merchant of level 1 manages two lockers, the Librarian's and the Cartographer's (the first in its order): their
    // paper trades take 24 paper an emerald 16 times a restock each (a level-1 Merchant brings no reputation), 768 a
    // day, five days of which is the par, 3,840. The Farmer's locker is beyond the Merchant's limit: its wheat trade, switched on, is
    // wanted all the same ("a trade it has switched on takes it", every workstation's), at a stack's par. Its pumpkin
    // trade is a level-2 Farmer's: a level-1 Farmer has none to switch on. The Fletcher's sticks are switched off.
    let mut s = network(200.0, 100.0,
        &[("merchant", 1), ("librarian", 1), ("cartographer", 1), ("farmer", 1), ("fletcher", 1)], &["fletcher"]);
    s.deliver(HUB, WHEAT, 100.0);
    s.deliver(HUB, PUMPKIN, 64.0);
    s.deliver(HUB, STICK, 64.0);
    s.advance(LOOK);
    assert_eq!(s.capacity(DEST, PAPER), 768.0, "the managed lockers take 768 paper a day");
    let mut cargo = s.wagon_cargo(0).expect("a wagon out");
    for crate_ in s.wagon_shipments(0).0 {
        cargo[crate_.good] += crate_.count; // (a good of two stacks or more rides crated)
    }
    assert_eq!(cargo[PAPER], 200.0, "all the paper: the far end's par is 3,840");
    assert_eq!(cargo[WHEAT], 64.0, "a stack of wheat, for the Farmer's trade its Merchant doesn't manage");
    assert_eq!((cargo[PUMPKIN], cargo[STICK]), (0.0, 0.0), "no pumpkin (no trade there has it) nor sticks (switched off)");
    s.advance(3 * LOOK);
    assert_eq!(s.route(0).sold, 264.0, "the far end bought it all: its paper and its stack of wheat");
}


/// What a wagon left with, loose and crated (a good of two stacks or more rides crated: `rules/cargo.rs`).
fn aboard(s: &Society, route: usize) -> Vec<f64> {
    let mut cargo = s.wagon_cargo(route).expect("a wagon out");
    for crate_ in s.wagon_shipments(route).0 {
        cargo[crate_.good] += crate_.count;
    }
    cargo
}

/// A far end like the study's settlements: a Merchant of level 1 managing its Librarian's and Cartographer's paper
/// trades (a par of 3,840), its Farmer's wheat trade switched on beyond its limit (a stack's par).
const SETTLEMENT: &[(&str, u8)] = &[("merchant", 1), ("librarian", 1), ("cartographer", 1), ("farmer", 1)];

#[test]
fn no_more_of_one_good_than_640_nor_than_its_share_of_the_slots() {
    // `pickCargo`: of each good no more than `WANT_PER_ITEM` (640), nor than the slots left over the goods that could
    // go, 64 to a slot: a wagon's 27 slots over paper and wheat is 13 slots each, so 640 of the paper, though the far
    // end's par is 3,840 and the hub holds 2,000.
    let mut s = network(2000.0, 100.0, SETTLEMENT, &[]);
    s.deliver(HUB, WHEAT, 100.0);
    s.advance(LOOK);
    let cargo = aboard(&s, 0);
    assert_eq!((cargo[PAPER], cargo[WHEAT]), (640.0, 64.0), "640 paper (WANT_PER_ITEM) and a stack of wheat (its par)");
    // Four slots over the same two goods: two each (`max(1, slots / goods) * 64`), 128 of the paper, where it would
    // have filled every slot before the wheat's turn.
    let mut s = network(2000.0, 100.0, SETTLEMENT, &[]);
    s.content_mut().dials.wagon_slots = 4;
    s.deliver(HUB, WHEAT, 100.0);
    s.advance(LOOK);
    let cargo = aboard(&s, 0);
    assert_eq!((cargo[PAPER], cargo[WHEAT]), (128.0, 64.0), "two slots of paper, and the wheat");
}

#[test]
fn a_wagon_is_full_only_of_what_the_far_end_could_pay_for() {
    // `saleLoad`: what makes a wagon full is the stacks the far end could pay for when a wagon last sold there
    // (`farPurse`: its money beyond its payroll, within its day's budget). A far end that could pay for a stack
    // doesn't fill a wagon with paper: the wagon waits for its day (every four days), where a far end with the money
    // for it would have the next one the day after.
    let run = |dest_money: f64| {
        let mut s = network(5000.0, dest_money, &[("merchant", 1), ("librarian", 1), ("cartographer", 1)], &[]);
        s.content_mut().dials.wagon_slots = 8; // full at 6 stacks; 512 paper (a good's share of 8 slots) fills it
        s.deliver(DEST, BREAD, 64.0); // the far end feeds every wagon for the road home
        assert_eq!(s.route_state(0).1.far_purse, None, "never sold there");
        s.advance(LOOK);
        assert!(s.route(0).on_the_road, "the first wagon goes on its day");
        s.advance(3 * LOOK);
        assert!(s.route(0).sold > 0.0, "it sold there");
        let purse = s.route_state(0).1.far_purse.expect("the far end's purse, as its wagon saw it");
        s.advance(DAY + 3 * LOOK); // the next day's wagon there and home again, if one went
        (purse, s.route(0).trips, s.route_state(0).1.stayed)
    };
    let (purse, trips, stayed) = run(2.0);
    assert!(purse.to_f64() < 2.0, "the far end could spend {purse} on goods");
    assert_eq!(trips, 1, "no wagon the next day: 8 stacks of paper, but it could pay for one");
    assert_eq!(stayed, Some(enterprise_core::road::Stay::WaitingForALoad));
    let (purse, trips, _) = run(500.0);
    assert!(purse.to_f64() > 100.0, "a far end with money: {purse}");
    assert_eq!(trips, 2, "the next day's wagon, full of what it can pay for, goes");
}

#[test]
fn a_route_sends_one_wagon_a_day() {
    // `Caravans.poll` 172: a route dispatches once `DEPARTURE_INTERVAL` (a day) has passed since its last wagon left.
    // Four blocks away, the wagon is home within three looks, full again (a far end with money), and waits out the
    // day all the same.
    let mut s = network(5000.0, 500.0, &[("merchant", 1), ("librarian", 1), ("cartographer", 1)], &[]);
    s.content_mut().dials.wagon_slots = 8;
    s.deliver(DEST, BREAD, 64.0);
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "the first wagon goes");
    s.advance(DAY - LOOK);
    assert_eq!(s.route(0).trips, 1, "home within the day, and no second wagon that day");
    assert!(!s.route(0).on_the_road);
    s.advance(DAY + LOOK);
    assert!(s.route(0).on_the_road, "the next goes a day after the first left");
}

#[test]
fn a_load_is_weighed_against_its_road_food_at_its_goods_worth() {
    // `saleLoad` values the load at each good's worth (`Prices.emeraldsEach`), not at the price the Merchants will
    // settle: four blocks' road food, a point each way for the crew and a point each way for the team (0.2 emeralds),
    // is paid by five sheets of paper (a 24th of an emerald each, 0.21), though they'd fetch three quarters of that.
    let mut s = network(5.0, 12.0, &[("librarian", 1)], &[]);
    s.advance(LOOK);
    assert!(s.route(0).on_the_road, "five sheets go");
    let mut s = network(4.0, 12.0, &[("librarian", 1)], &[]);
    s.advance(LOOK);
    assert!(!s.route(0).on_the_road, "four don't pay their road");
    assert_eq!(s.route_state(0).1.stayed, Some(enterprise_core::road::Stay::WaitingForALoad));
}
