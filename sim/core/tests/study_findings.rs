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

