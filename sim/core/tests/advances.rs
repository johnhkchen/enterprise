//! Advances and refunds, pinned as the mod's game tests pin them (`ContractTests`, `CaravanTests`), in milliseconds.
//! Money before the goods: a share of a contract's worth paid up front, carried by the buyer's wagon with the copy,
//! worked off at the same share of each delivery, owed back when the contract breaks.

use crate::common;

use enterprise_core::contracts::Book;
use enterprise_core::{Content, Good, Incoterm, Society, State};

const DAY: u64 = 24_000;

/// A mill (town 0) and a scribes' town (town 1) a day apart, a route each way every `every` days.
fn two_towns(mill_money: f64, scribes_money: f64, every: f64) -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0, 64), Good::new("iron_ingot", 0.1, 64),
        Good::new("iron_sword", 1.0, 1)], DAY));
    let mill = s.add_town("Wheatfield", mill_money);
    let scribes = s.add_town("Hayford", scribes_money);
    s.set_trade(mill, 0, 128.0, 0.0);
    s.set_trade(scribes, 0, 0.0, 0.0);
    s.add_route(scribes, mill, 1.0, 0.5, every);
    s.add_route(mill, scribes, 1.0, 0.5, every);
    s
}

// parity: ContractTests.anAdvanceIsPaidUpFrontAndWorkedOff
#[test]
fn an_advance_is_paid_up_front_and_worked_off() {
    let mut book = Book::default();
    let t = 1_000_000;
    // Three swords a delivery, four deliveries, a coin each: twelve emeralds, a third of it up front.
    let id = book.draft_with_advance(0, 1, 0, 3.0, 1.0, t, DAY, 4, Incoterm::Exw, true, true, 1.0 / 3.0);
    assert!((book.contracts[id].worth() - 12.0).abs() < 1e-9);
    assert!(book.contracts[id].advance_due() == 4.0, "a third of twelve emeralds up front: {}",
        book.contracts[id].advance_due());
    book.advanced(id, 4.0);
    assert!(book.contracts[id].advance_due() < 1e-9, "paid");
    let off = book.contracts[id].recoup_from(3.0, 3.0);
    assert!(off == 1.0, "a third of each delivery's price is taken off: {off}");
    book.recouped(id, off);
    book.settle(id, 0.0, 3.0, 3.0, 0.0);
    for _ in 0..2 {
        let off = book.contracts[id].recoup_from(3.0, 3.0);
        book.recouped(id, off);
        book.settle(id, 0.0, 3.0, 3.0, 0.0);
    }
    let last = book.contracts[id].recoup_from(3.0, 3.0);
    assert!(last == 1.0, "and the last delivery takes off what's left: {last}");
}

// parity: ContractTests.aBrokenContractOwesBackItsUnearnedAdvance
#[test]
fn a_broken_contract_owes_back_its_unearned_advance() {
    let mut book = Book::default();
    let t = 1_000_000;
    let id = book.draft_with_advance(0, 1, 0, 3.0, 1.0, t, DAY, 4, Incoterm::Dap, true, true, 1.0 / 3.0);
    book.advanced(id, 4.0);
    book.review(t + 3 * DAY + 2 * DAY + 1, DAY, |_| false); // all four judged, none delivered
    let c = &book.contracts[id];
    assert_eq!(c.state, State::Broken, "broken: {c:?}");
    assert!(c.refund == 4.0, "the advance is owed back: {}", c.refund);
    assert!(book.refunds(0, 1).contains(&id), "for the buyer's caravan to collect");
    assert!(book.refunds(1, 0).is_empty(), "and only from the seller");
    let bytes = postcard::to_allocvec(&book).unwrap();
    let reloaded: Book = postcard::from_bytes(&bytes).unwrap();
    assert!(reloaded.contracts[id].refund == 4.0
        && reloaded.trust(0, 1, t, DAY).to_bits() == book.trust(0, 1, t, DAY).to_bits(), "saved and loaded whole");
}

#[test]
fn the_last_delivery_takes_what_is_left_and_never_more_than_the_price() {
    let mut book = Book::default();
    let id = book.draft_with_advance(0, 1, 0, 3.0, 1.0, 0, DAY, 2, Incoterm::Exw, true, true, 0.5);
    assert!(book.contracts[id].advance_due() == 3.0, "half of six");
    assert_eq!(book.contracts[id].recoup_from(3.0, 3.0), 0.0, "nothing paid yet, nothing to take off");
    book.advanced(id, 3.0);
    book.recouped(id, 1.5);
    book.settle(id, 0.0, 3.0, 3.0, 0.0);
    assert!(book.contracts[id].recoup_from(3.0, 3.0) == 1.5, "the last takes what is left");
    assert!(book.contracts[id].recoup_from(1.0, 3.0) == 1.0, "never more than the price");
}

// parity: CaravanTests.theAdvanceRidesWithTheCopyAndTheInputsFollow
#[test]
fn the_advance_rides_with_the_copy() {
    // The scribes (buyer) order eight iron swords made, two a day from the third day, a third paid up front; the copy is
    // theirs. They hold 64 iron.
    let mut s = two_towns(5.0, 50.0, 1.0);
    s.deliver(1, 1, 64.0);
    let id = s.draft_advanced(0, 1, 2, 2.0, 1.0, 3 * DAY, DAY, 4, Incoterm::Exw, false, false, true);
    let c = s.contract(id);
    assert_eq!(c.state, State::Drafted, "only the buyer holds the copy");
    // A third of eight, rounded up to the thousandth: the buyer's to pay (`fixed.rs`).
    assert!(c.advance_due() == enterprise_core::Fixed::ceil(8.0 / 3.0));
    let start = s.totals();
    let mill_before = s.town(0).money;
    s.advance(DAY / 2);
    assert_eq!(s.contract(id).advanced, 0.0, "the wagon is still on the road");
    s.advance(DAY + DAY / 4);
    let c = s.contract(id);
    assert_eq!(c.state, State::Active, "the wagon brought the copy");
    assert!(c.advanced == enterprise_core::Fixed::ceil(8.0 / 3.0), "and the advance: {}", c.advanced);
    assert!(s.town(0).money - mill_before == enterprise_core::Fixed::ceil(8.0 / 3.0), "in coin, from its purse: the seller holds it");
    assert!(s.advances_held(0) == enterprise_core::Fixed::ceil(8.0 / 3.0));
    assert!(s.bills().is_empty(), "no paper where the purse sufficed");
    s.conserved(&start).unwrap();
    // The inputs follow: the mill needs sixteen iron for eight swords, and the next wagon from the scribes brings it.
    s.advance(2 * DAY);
    assert_eq!(s.inputs(0)[1], 16.0, "the mill's par has the iron for the swords in it: {:?}", s.inputs(0));
    s.advance(5 * DAY);
    let iron = s.town(0).stock[1];
    assert!(iron >= 16.0, "the next wagon brings the iron for the swords: {iron}");
    s.conserved(&start).unwrap();
}

#[test]
fn what_the_purse_cannot_cover_goes_on_a_bill_on_the_buyer() {
    // The buyer has 3 emeralds: half a coin for the road, 2.5 in the purse of 8/3; the rest, a bill.
    let mut s = two_towns(5.0, 3.0, 1.0);
    let id = s.draft_advanced(0, 1, 0, 2.0, 1.0, 3 * DAY, DAY, 4, Incoterm::Exw, false, false, true);
    s.advance(DAY + DAY / 4);
    let c = s.contract(id);
    assert!(c.advanced == enterprise_core::Fixed::ceil(8.0 / 3.0), "the whole advance counts: {}", c.advanced);
    let bills = s.bills();
    assert_eq!(bills.len(), 1, "a promise to pay the rest");
    assert_eq!((bills[0].drawee, bills[0].payee), (1, 0));
    assert!(bills[0].amount == (enterprise_core::Fixed::ceil(8.0 / 3.0) - enterprise_core::Fixed::from(2.5)).mul_ceil(1.02), "with the wait's 2%: {}", bills[0].amount);
}

#[test]
fn an_advance_is_worked_off_at_its_share_of_each_delivery_and_the_books_balance() {
    // The mill sells 16 paper a day for 4 days at a coin each (worth 64), a third up front; Delivered Duty Paid.
    let mut s = two_towns(5.0, 100.0, 1.0);
    let id = s.draft_advanced(0, 1, 0, 16.0, 1.0, 2 * DAY, DAY, 4, Incoterm::Ddp, false, false, true);
    let start = s.totals();
    for day in 1..=10 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    let c = s.contract(id);
    assert_eq!(c.state, State::Completed, "{c:?}");
    assert!(c.advanced == enterprise_core::Fixed::ceil(64.0 / 3.0), "{}", c.advanced);
    assert!(c.recouped == c.advanced, "all worked off: {} of {}", c.recouped, c.advanced);
    assert!(c.paid == 64.0, "the whole worth counts as paid: {}", c.paid);
    assert!(s.town(1).money <= 100.0 - 64.0, "the buyer paid for the goods: {}", s.town(1).money);
}

// Beyond the mod's game tests: `Caravans.settleOwing`, the next wagon between the two collecting the refund.
#[test]
fn a_broken_contract_is_paid_back_by_the_next_wagon_between_the_two() {
    // The mill makes nothing, so it never delivers; it holds the buyer's advance, and owes it back when it breaks.
    let mut s = two_towns(5.0, 100.0, 1.0);
    s.set_trade(0, 0, 0.0, 0.0);
    let id = s.draft_advanced(0, 1, 0, 16.0, 1.0, 2 * DAY, DAY, 10, Incoterm::Dap, false, false, true);
    let start = s.totals();
    let mut owed_back = None;
    for day in 1..=20 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
        let c = s.contract(id);
        if c.state == State::Broken && owed_back.is_none() && c.refund > 0.0 {
            owed_back = Some(c.refund);
            assert!((c.refund - c.advanced).abs() < 1e-9 && c.advanced > 0.0, "all of it is owed back: {c:?}");
            assert_eq!(s.refunds(0, 1).len(), 1, "listed for the buyer's wagon");
        }
    }
    assert!(owed_back.is_some(), "it broke: {:?}", s.contract(id));
    let c = s.contract(id);
    assert!(c.refund < 1e-6 && s.refunds(0, 1).is_empty(), "paid back by a wagon between them: {c:?}");
    assert!(s.town(1).money >= 100.0 - 20.0 * 0.5 * 2.0, "the buyer has its advance again: {}", s.town(1).money);
}

#[test]
fn the_random_worlds_pay_work_off_and_refund_advances() {
    let (mut paid, mut worked, mut refunded, mut broken) = (0, 0, 0, 0);
    for (p, w, r, b) in common::each_seed(common::seeds(300), |seed| {
        let mut s = common::world(seed);
        let mut owed = std::collections::BTreeSet::new();
        for day in 1..=120 {
            s.advance(day * DAY);
            for c in s.contracts() {
                if c.refund > 0.0 {
                    owed.insert(c.id);
                }
            }
        }
        let (mut paid, mut worked, mut refunded, mut broken) = (0, 0, 0, 0);
        for c in s.contracts() {
            paid += (c.advanced > 0.0) as u32;
            worked += (c.recouped > 0.0 && c.state != State::Broken) as u32;
            broken += (c.advanced > 0.0 && c.state == State::Broken) as u32;
            refunded += (owed.contains(&c.id) && c.refund < 1e-6) as u32;
        }
        (paid, worked, refunded, broken)
    }) {
        paid += p;
        worked += w;
        refunded += r;
        broken += b;
    }
    assert!(paid > 20 && worked > 0 && broken > 5 && refunded > 0, "{paid} paid, {worked} worked off, {broken} broken, {refunded} refunded");
}

// Beyond the mod's game tests: `Caravans.payAdvances`' second half, a seller's wagon collecting the advance at the buyer.
#[test]
fn the_sellers_wagon_collects_the_advance_at_the_buyer() {
    // Only the mill runs a wagon; both hold the copy, so the scribes pay up front when it comes, from what they spare.
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0, 64)], DAY));
    let mill = s.add_town("Wheatfield", 5.0);
    let scribes = s.add_town("Hayford", 50.0);
    s.set_trade(mill, 0, 128.0, 0.0);
    s.set_trade(scribes, 0, 0.0, 0.0);
    s.add_route(mill, scribes, 1.0, 0.5, 1.0);
    let id = s.draft_advanced(0, 1, 0, 2.0, 1.0, 3 * DAY, DAY, 4, Incoterm::Dap, true, true, true);
    let start = s.totals();
    let scribes_before = s.town(1).money;
    let mut first = None;
    for quarter in 1..=24 {
        s.advance(quarter * DAY / 4);
        s.conserved(&start).unwrap();
        let c = s.contract(id);
        if first.is_none() && c.advanced > 0.0 {
            first = Some(quarter);
            assert!(c.advanced == enterprise_core::Fixed::ceil(8.0 / 3.0), "all of it, the scribes having it to spare: {c:?}");
            assert!(c.recouped <= c.advanced);
        }
    }
    assert!(first.is_some(), "the mill's wagon collected it: {:?}", s.contract(id));
    assert!(s.town(1).money < scribes_before, "from the buyer's treasury: {}", s.town(1).money);
    assert!(s.bills().is_empty());
}
