//! Contracts, pinned as the mod's game tests pin them (`CaravanTests`, `ContractTests`), in milliseconds.

use enterprise_core::{Content, Good, Incoterm, Society, State};

const DAY: u64 = 24_000;

/// Paper from a mill town to a scribes' town a day away, the mill's wagon on a `every`-day timetable.
fn mill_and_scribes(every: f64) -> Society {
    let content = Content::new(vec![Good::new("paper", 0.04, 64)], DAY);
    let mut s = Society::new(content);
    let mill = s.add_town("Wheatfield", 20.0);
    let scribes = s.add_town("Hayford", 40.0);
    s.set_trade(mill, 0, 128.0, 0.0);
    s.set_trade(scribes, 0, 0.0, 32.0);
    s.add_route(mill, scribes, 1.0, 0.5, every);
    s
}

// parity: CaravanTests.contractedGoodsGoFirstAndSettleAtTheLockedPrice
#[test]
fn contracted_goods_go_first_and_settle_at_the_locked_price() {
    let mut s = mill_and_scribes(1.0);
    s.deliver(1, 0, 5.0 * 32.0); // the scribes' shelf full already: nothing would sell
    s.deliver(0, 0, 640.0);
    let id = s.draft(0, 1, 0, 64.0, 0.05, 0, DAY, 3, Incoterm::Ddp, true);
    s.advance(3 * DAY);
    let c = s.contract(id);
    assert!(c.delivered >= 64.0, "delivered under contract though the shelf was full: {}", c.delivered);
    assert!((c.paid - c.delivered * 0.05).abs() < 1e-9, "at the locked price, not the market's: {}", c.paid);
}

// parity: CaravanTests.missedInstallmentsCostTrustAndBreakTheContract
#[test]
fn missed_installments_cost_trust_and_break_the_contract() {
    let mut s = mill_and_scribes(30.0);
    s.set_trade(0, 0, 0.0, 0.0); // the mill makes nothing: it can't deliver
    let id = s.draft(0, 1, 0, 16.0, 0.04, 0, DAY, 10, Incoterm::Ddp, true);
    let before = s.trust(0, 1);
    s.advance(6 * DAY);
    let c = s.contract(id);
    assert!(c.missed >= 3 && c.state == State::Broken, "nothing delivered: missed {}, {:?}", c.missed, c.state);
    assert!(s.trust(0, 1) < before, "and trust falls: {} from {}", s.trust(0, 1), before);
}

// parity: ContractTests.aNewSupplierIsForgivenWhileItRampsUp
// parity: ContractTests.missesAreForgivenWithTime
#[test]
fn a_new_pairs_misses_weigh_a_quarter_and_fade() {
    let mut s = mill_and_scribes(30.0);
    s.set_trade(0, 0, 0.0, 0.0);
    s.draft(0, 1, 0, 16.0, 0.04, 0, DAY, 1, Incoterm::Ddp, true);
    s.advance(3 * DAY);
    let after_miss = s.trust(0, 1);
    // One miss at a quarter's weight: (0 + 2) / (0 + 0.25 + 4).
    assert!((after_miss - 2.0 / 4.25).abs() < 0.01, "a stranger's miss counts a quarter: {after_miss}");
    s.advance(43 * DAY);
    assert!(s.trust(0, 1) > after_miss + 0.01, "forgiven in time: {} after 40 days", s.trust(0, 1));
}

// parity: CaravanTests.aTravellingMerchantDraftsAStandingOrder
// parity: CaravanTests.aContractsCopyRidesWithTheCaravan
#[test]
fn a_wagon_that_sold_well_drafts_a_standing_order_whose_copy_rides_home() {
    let mut s = mill_and_scribes(4.0);
    s.deliver(0, 0, 640.0);
    s.advance(DAY + 1200 * 2); // out and sold: a day on the road
    let drafted = s.contracts();
    assert_eq!(drafted.len(), 1, "a standing order for the paper it sold");
    let c = &drafted[0];
    assert!(c.quantity >= 16.0 && c.quantity % 16.0 == 0.0, "in sixteens: {}", c.quantity);
    assert!(c.state == State::Drafted && c.known_by_buyer && !c.known_by_seller, "the buyer holds the copy");
    s.advance(3 * DAY);
    assert!(s.contract(c.id).in_force(), "the copy rode home: in force");
}

// parity: CaravanTests.aTownPromisesOnlyWhatItSells
#[test]
fn a_town_promises_only_what_it_sells_steadily() {
    let mut s = mill_and_scribes(4.0);
    s.set_trade(1, 0, 0.0, 2.0); // its people use two sheets a day: nothing steady to promise
    s.deliver(0, 0, 640.0);
    s.advance(10 * DAY);
    assert!(s.contracts().is_empty(), "no standing order for a trickle");
}

// parity-partial: CaravanTests.goodsOwedWaitTillTheyCantAndASmallLoadGoesByPorter
#[test]
fn goods_owed_wait_till_they_cant_then_go() {
    let mut s = mill_and_scribes(30.0);
    s.content_mut().dials.standing_orders = false; // only the contract below
    s.deliver(0, 0, 640.0);
    s.advance(DAY / 2); // its timetable's first trip
    s.advance(4 * DAY); // and home again
    // Due on day 6, judged on day 8, a day on the road: it can wait till day 6, no later.
    let id = s.draft(0, 1, 0, 64.0, 0.05, 6 * DAY, DAY, 1, Incoterm::Dap, true);
    s.advance(6 * DAY);
    assert!(!s.route(0).on_the_road, "owed, with two days' grace: it waits for its day or a full load");
    assert_eq!(s.contract(id).delivered, 0.0);
    s.advance(6 * DAY + DAY / 4);
    assert!(s.route(0).on_the_road, "waiting a day more would miss it: it goes");
    s.advance(8 * DAY);
    assert!(s.contract(id).delivered >= 64.0, "the day it couldn't wait, it went: {}", s.contract(id).delivered);
    assert_eq!(s.contract(id).missed, 0, "in time");
}

// parity: CaravanTests.exWorksTheBuyersCaravanCollects
#[test]
fn ex_works_the_buyer_collects_in_time_and_pays_from_its_purse() {
    let content = Content::new(vec![Good::new("book", 0.6, 64)], DAY);
    let mut s = Society::new(content);
    let hub = s.add_town("Haygate", 100.0);
    let town = s.add_town("Cloverby", 5.0);
    s.add_route(hub, town, 1.0, 0.5, 30.0);
    s.deliver(town, 0, 64.0);
    let id = s.draft(town, hub, 0, 64.0, 0.5, 2 * DAY, DAY, 1, Incoterm::Exw, true);
    s.advance(6 * DAY);
    let c = s.contract(id);
    assert!(c.delivered >= 64.0 && c.buyer_missed == 0, "collected: {} delivered", c.delivered);
    assert!(s.town(town).money >= 5.0 + 32.0 - 1e-9, "the town paid 32 emeralds: {}", s.town(town).money);
    assert!(s.town(hub).stock[0] >= 64.0, "and the books are home");
}

// parity: CaravanTests.shortOfCoinTheBuyerPaysWithABill
// parity: WarTests.aBillIsPaidWhenPresentedOnceDue
#[test]
fn short_of_coin_the_buyer_pays_with_a_bill_presented_once_due() {
    let mut s = mill_and_scribes(3.0);
    s.content_mut().dials.standing_orders = false;
    s.deliver(0, 0, 640.0);
    s.set_trade(1, 0, 0.0, 0.0); // its people use none: no income from them yet
    let id = s.draft(0, 1, 0, 64.0, 1.0, 0, DAY, 1, Incoterm::Ddp, true); // 64 emeralds; it has 40
    s.advance(DAY + DAY / 2);
    let c = s.contract(id);
    assert!(c.delivered >= 64.0 && c.owing == 0.0, "delivered, nothing owed: {} owed", c.owing);
    let bill = s.bills().into_iter().next().expect("the rest on a bill on the scribes");
    assert!(bill.drawee == 1 && bill.payee == 0 && (bill.amount - 24.0 * 1.02).abs() < 1e-6, "{bill:?}");
    s.pay(1, 30.0); // a player buys at the scribes' shops: money comes in
    s.advance(20 * DAY);
    assert_eq!(s.bills()[0].state, enterprise_core::bills::BillState::Paid, "the mill's next wagon, once it was due, was paid");
    assert!(s.trust(0, 1) > 0.5, "a promise kept: trust {}", s.trust(0, 1));
}

// parity: WarTests.aBillLongUnpaidIsDishonoured
#[test]
fn a_bill_unpaid_past_its_grace_is_dishonoured() {
    let mut s = mill_and_scribes(2.0);
    s.content_mut().dials.standing_orders = false;
    s.deliver(0, 0, 640.0);
    s.set_trade(1, 0, 0.0, 0.0); // the scribes never earn: they can't pay
    s.draft(0, 1, 0, 64.0, 1.0, 0, DAY, 1, Incoterm::Ddp, true);
    s.advance(30 * DAY);
    let bill = &s.bills()[0];
    assert_eq!(bill.state, enterprise_core::bills::BillState::Dishonoured, "{bill:?}");
    assert!(s.trust(0, 1) < 0.5, "and trust falls: {}", s.trust(0, 1));
}

#[test]
fn the_books_balance_with_contracts_running() {
    let mut s = mill_and_scribes(2.0);
    s.draft(0, 1, 0, 32.0, 0.05, 0, 2 * DAY, 50, Incoterm::Dap, true);
    let content_town = s.add_town("Cloverby", 10.0);
    s.add_route(content_town, 0, 1.0, 0.5, 3.0);
    s.draft(0, content_town, 0, 16.0, 0.04, DAY, DAY, 40, Incoterm::Exw, true);
    let start = s.totals();
    for day in 1..=200 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
}
