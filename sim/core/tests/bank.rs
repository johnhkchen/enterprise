//! Banks and the discount desk (`bank.rs`), pinned as the mod's game tests pin them (`CaravanTests`,
//! `CountingHouseTests`, `ContractTests`), in milliseconds. A bank deals only where its own wagons call: it buys a
//! town's bills not yet due for coin now, lends against a contract in force and is repaid from the payment first; a
//! hub's desk prices a bill by the goods behind it and caps its book; a bill no one can carry lapses.

mod common;

use enterprise_core::bank::{discount_price, loan_cap, to_come, Discounts};
use enterprise_core::bills::{BillState, Bills, Held, LAPSE_DAYS};
use enterprise_core::contracts::Book;
use enterprise_core::{Content, Good, Incoterm, Society, State};

const DAY: u64 = 24_000;

/// One look on.
fn look(s: &mut Society) {
    let next = s.now() + s.content().dials.look_ticks;
    s.advance(next);
}

/// Looks on until `done` holds (at most `most` looks); whether it did.
fn until(s: &mut Society, most: u32, done: impl Fn(&Society) -> bool) -> bool {
    for _ in 0..most {
        if done(s) {
            return true;
        }
        look(s);
    }
    done(s)
}

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0, 64), Good::new("book", 1.0, 64)], DAY)
}

/// A hub (town 0) with two stacks of paper to sell and `hub_money`, and a town (town 1) with `town_money` that uses
/// paper; one route, the hub's, a few minutes each way (the mod's test arena: `CaravanTests.network`).
fn network(hub_money: f64, town_money: f64) -> Society {
    let mut s = Society::new(content());
    let hub = s.add_town("Hub", hub_money);
    let town = s.add_town("Town", town_money);
    s.deliver(hub, 0, 128.0);
    s.set_trade(town, 0, 0.0, 64.0);
    s.add_route(hub, town, 0.01, 0.0, 1.0);
    s
}

// parity: CaravanTests.aBankBuysATownsBillForCoinNow
#[test]
fn a_bank_buys_a_towns_bill_for_coin_now() {
    let mut s = network(40.0, 0.0);
    s.open_bank(0);
    let id = s.draw_bill(0, 1, 10.0); // the hub owes the town ten emeralds, in four days
    let start = s.totals();
    let due = s.bills()[id].due;
    assert!(until(&mut s, 4, |s| s.bank(0).bills_bought > 0), "the hub's wagon called: {:?}", s.bank(0));
    let book = s.bank(0);
    let bill = &s.bills()[id];
    assert_eq!(bill.state, BillState::Paid, "the hub's bank bought it back early");
    assert!(s.town(1).money > 0.0, "and the town has coin now, not in four days: {}", s.town(1).money);
    assert!(book.bills_paid < 10.0 * 1.02, "less the discount: {}", book.bills_paid);
    // Face less 1% and 1% a day to its due day: about 9.69 for a bill of 10.2 four days out.
    let days = (due - s.now()) as f64 / DAY as f64; // from the look its wagon arrived in
    assert!((book.bills_paid - 10.2 * (1.0 - 0.01 - 0.01 * days)).abs() < 1e-9, "{} for 10.2 due in {days} days",
        book.bills_paid);
    assert!((book.bills_paid - 9.69).abs() < 0.02, "about 9.69: {}", book.bills_paid);
    assert!((s.town(1).money - book.bills_paid).abs() < 1e-9, "in coin, from the wagon's purse");
    assert!((book.bills_face - 10.2).abs() < 1e-9 && book.bills_bought == 1);
    s.conserved(&start).unwrap();
}

// parity: CaravanTests.aBankLendsAgainstAContractAndIsRepaidFromThePayment
#[test]
fn a_bank_lends_against_a_contract_and_is_repaid_from_the_payment() {
    let mut s = network(40.0, 0.0); // a town with no money
    s.open_bank(0);
    let t = s.now();
    // The hub buys 64 books from the town, collected in two days; the town has nothing to pay its makers with.
    let id = s.draft(1, 0, 1, 64.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Exw, true);
    let start = s.totals();
    assert!(until(&mut s, 4, |s| s.owed_on(id) > 0.0), "lent: {:?}", s.bank(0));
    let lent = s.owed_on(id);
    assert!(lent > 5.0, "lent against the books to come: {lent}");
    // Half the 32 to come, at 3%.
    assert!((s.bank(0).lent - 16.0).abs() < 1e-9 && (lent - 16.0 * 1.03).abs() < 1e-9, "{:?}", s.bank(0));
    assert!((s.town(1).money - 16.0).abs() < 1e-9, "in the town's treasury: {}", s.town(1).money);
    s.deliver(1, 1, 64.0); // made, with the money
    assert!(until(&mut s, 3 * 20, |s| s.contract(id).delivered >= 64.0), "collected: {:?}", s.contract(id));
    let owed = s.owed_on(id);
    assert_eq!(s.contract(id).state, State::Completed);
    assert!(owed < 0.01 && s.bank(0).repaid > 5.0, "the loan came out of the payment: {owed}, {:?}", s.bank(0));
    assert!((s.bank(0).repaid - 16.48).abs() < 1e-9, "the principal and its 3%");
    assert!((s.town(1).money - (16.0 + 32.0 - 16.48)).abs() < 1e-9, "the town was paid the rest: {}", s.town(1).money);
    assert!(s.loans().is_empty(), "paid off");
    s.conserved(&start).unwrap();
}

// parity: CountingHouseTests.theDiscountDeskPricesByGoodsAndCapsItsBook
#[test]
fn the_discount_desk_prices_by_goods_and_caps_its_book() {
    let (hub, farm, other) = (0, 1, 2);
    let dials = content().dials;
    let mut bills = Bills::default();
    let book = Book::default();
    let t = 1_000_000;
    bills.draw(farm, hub, 30.0, t, DAY); // already in its book
    let more = bills.draw(farm, other, 10.0, t, DAY);
    let elsewhere = bills.draw(other, farm, 10.0, t, DAY);
    let means = 100.0;
    let quote = |id: usize, backed: bool, taking: f64, fill: f64| {
        Discounts::quote(&bills, &book, hub, false, &bills.bills[id], backed, taking, fill, means, t, DAY, &dials)
    };
    let on_farm = quote(more, false, 0.0, 0.0);
    assert!(!on_farm.taken(), "past a third of its book on one market: {on_farm:?}");
    let fresh = quote(elsewhere, false, 0.0, 0.0);
    let backed = quote(elsewhere, true, 0.0, 0.0);
    assert!(fresh.taken() && backed.share - fresh.share > 0.039,
        "goods it can see: a narrower discount, {} vs {}", fresh.share, backed.share);
    let full = quote(elsewhere, true, 0.0, 1.0);
    assert!(backed.share - full.share > 0.029, "a full warehouse: dearer, {}", full.share);
    let booked = quote(elsewhere, true, 65.0, 0.0);
    assert!(!booked.taken(), "its book at its limit: {booked:?}");
    // The numbers: the fee 2%, 1% a day for four days, 4% unbacked, and the throttle on a book 30.6 of 100 full.
    assert!((fresh.share - (1.0 - 0.02 - 0.04 - 0.04 - 0.03 * 0.306)).abs() < 1e-9, "{}", fresh.share);
    assert!((Discounts::limit(false, means, &dials) - 100.0).abs() < 1e-9);
    assert!((Discounts::book(&bills, hub) - 30.6).abs() < 1e-9, "only what it holds on other markets");
    // With a counting house: twice the paper, at half the fee (and the throttle on a book half as full).
    let counted =
        Discounts::quote(&bills, &book, hub, true, &bills.bills[elsewhere], true, 65.0, 0.0, means, t, DAY, &dials);
    assert!(counted.taken() && (counted.share - (1.0 - 0.01 - 0.04 - 0.03 * 95.6 / 200.0)).abs() < 1e-9, "{counted:?}");
    assert!((Discounts::limit(true, means, &dials) - 200.0).abs() < 1e-9);
}

#[test]
fn the_desk_sees_the_goods_on_a_bills_contract_and_charges_for_the_season_and_the_stale() {
    let dials = content().dials;
    let mut bills = Bills::default();
    let mut book = Book::default();
    let c = book.draft(1, 2, 0, 16.0, 1.0, 0, DAY, 2, Incoterm::Exw, true, true);
    let id = bills.draw_for(2, 1, 16.0, 0, DAY, Some(c));
    let quote = |bills: &Bills, book: &Book, id: usize, now: u64| {
        Discounts::quote(bills, book, 0, false, &bills.bills[id], false, 0.0, 0.0, 100.0, now, DAY, &dials)
    };
    assert!(!Discounts::backed(&book, &bills.bills[id]), "nothing shipped yet");
    let unbacked = quote(&bills, &book, id, 0);
    book.ship(c, 16.0);
    assert!(Discounts::backed(&book, &bills.bills[id]), "crated and on its way");
    let backed = quote(&bills, &book, id, 0);
    assert!((backed.share - unbacked.share - 0.04).abs() < 1e-9, "{} vs {}", backed.share, unbacked.share);
    // Due past its season (more than 12 days out): 5% more off; past its day, the stale 10% in place of the days.
    let long = bills.draw_for(2, 1, 1.0, 0, DAY, Some(c));
    bills.bills[long].due = 13 * DAY;
    let q = quote(&bills, &book, long, 0);
    assert!((q.share - (1.0 - 0.02 - 0.13 - 0.05)).abs() < 1e-9, "{}", q.share);
    let stale = quote(&bills, &book, id, 5 * DAY);
    assert!((stale.share - (1.0 - 0.02 - 0.1)).abs() < 1e-9, "{}", stale.share);
}

// parity: ContractTests.aBillNoOneCanCarryLapses
#[test]
fn a_bill_no_one_can_carry_lapses() {
    let mut bills = Bills::default();
    let mut book = Book::default();
    let t = 2_000_000;
    let id = bills.draw(0, 1, 10.0, t, DAY);
    let due = bills.bills[id].due;
    let lapse = (LAPSE_DAYS * DAY as f64) as u64;
    assert_eq!(LAPSE_DAYS, 12.0, "three times the four days to dishonour");
    bills.lapse(&mut book, due + lapse - 1, DAY, LAPSE_DAYS, |_, _| false);
    assert_eq!(bills.bills[id].state, BillState::Outstanding, "not yet");
    let lapsed = bills.lapse(&mut book, due + lapse + 1, DAY, LAPSE_DAYS, |_, _| false);
    assert_eq!(bills.bills[id].state, BillState::Dishonoured, "no road between them and no hub to buy it: it lapses");
    assert_eq!(lapsed, vec![id]);
    assert!(book.trust(0, 1, due + lapse + 1, DAY) < 1.0, "the drawee's miss");
    // One someone could still carry stands.
    let carried = bills.draw(0, 1, 10.0, t, DAY);
    bills.lapse(&mut book, due + 2 * lapse, DAY, LAPSE_DAYS, |payee, drawee| (payee, drawee) == (1, 0));
    assert_eq!(bills.bills[carried].state, BillState::Outstanding);
}

#[test]
fn in_the_society_a_bill_with_no_road_either_way_lapses_and_one_with_a_road_stands() {
    let mut s = Society::new(content());
    for name in ["A", "B", "C"] {
        s.add_town(name, 0.0);
    }
    s.add_route(0, 2, 1.0, 0.0, 1.0); // A's wagons call at C: a bill on A held by C can be carried (the drawee's way)
    let stranded = s.draw_bill(0, 1, 10.0);
    let carried = s.draw_bill(0, 2, 10.0);
    let due = s.bills()[stranded].due;
    let lapse = (LAPSE_DAYS * DAY as f64) as u64;
    let look_ticks = s.content().dials.look_ticks;
    s.advance(due + lapse - look_ticks);
    assert_eq!(s.bills()[stranded].state, BillState::Outstanding, "not yet");
    s.advance(due + lapse + 2 * look_ticks);
    assert_eq!(s.bills()[stranded].state, BillState::Dishonoured, "it lapsed");
    assert!(s.trust(0, 1) < 1.0, "the drawee's miss: {}", s.trust(0, 1));
    assert_eq!(s.bills()[carried].state, BillState::Outstanding, "a road runs between them: it stands");
    assert_eq!(s.bills()[carried].held, Held::Town(2));
}

#[test]
fn a_drawees_grace_grows_with_what_it_owes_the_same_holder() {
    let mut bills = Bills::default();
    let one = bills.draw(0, 1, 100.0, 0, DAY);
    let grace = |b: &Bills| b.grace(&b.bills[one], DAY, 8.0, 100.0);
    assert_eq!(grace(&bills), 8 * DAY + (1.02 * DAY as f64).round() as u64, "8 days, and a day per 100 owed");
    bills.draw(0, 1, 100.0, 0, DAY);
    bills.draw(0, 2, 100.0, 0, DAY); // owed another holder: no more grace
    assert_eq!(grace(&bills), 8 * DAY + (2.04 * DAY as f64).round() as u64, "owing twice as much: two days more");
    assert_eq!(bills.grace(&bills.bills[one], DAY, 8.0, 0.0), 8 * DAY, "no scaling at 0");
}

#[test]
fn a_bank_endorses_another_markets_bill_and_presents_it_at_home() {
    // The hub (0) banks; the town (1) holds a bill on a third market (2), which the hub's wagons also call at.
    let mut s = network(40.0, 0.0);
    let third = s.add_town("Third", 50.0);
    s.add_route(0, third, 0.01, 0.0, 1.0);
    s.open_bank(0);
    let id = s.draw_bill(third, 1, 10.0);
    let start = s.totals();
    assert!(until(&mut s, 4, |s| s.bank(0).bills_bought > 0), "bought: {:?}", s.bank(0));
    let bill = &s.bills()[id];
    assert_eq!(bill.payee, 0, "endorsed to the bank");
    assert_eq!(bill.state, BillState::Outstanding, "still owed, now to the bank");
    let paid_for = s.bank(0).bills_paid;
    assert!((s.town(1).money - paid_for).abs() < 1e-9 && paid_for < 10.2);
    let due = bill.due;
    s.advance(due + 2 * DAY);
    let bill = &s.bills()[id];
    assert_eq!(bill.state, BillState::Paid, "presented by the bank's wagon once due: {bill:?}");
    assert!(s.town(third).money < 50.0 - 10.2 + 1e-6, "the third market paid its face: {}", s.town(third).money);
    s.conserved(&start).unwrap();
}

#[test]
fn the_banks_rules_in_numbers() {
    let dials = content().dials;
    assert!((discount_price(10.2, 4 * DAY, 0, DAY, &dials) - 10.2 * 0.95).abs() < 1e-12, "1% and 1% a day");
    assert_eq!((dials.loan_share, dials.loan_interest, dials.bank_float), (0.5, 0.03, 48.0));
    let mut book = Book::default();
    let id = book.draft_with_advance(1, 0, 1, 16.0, 1.0, 0, DAY, 4, Incoterm::Exw, true, true, 0.25);
    book.advanced(id, 16.0);
    // Half of 64 to come, less the 16 paid up front, less 4 lent already.
    assert!((loan_cap(&book.contracts[id], 4.0, &dials) - 12.0).abs() < 1e-12);
    // A training contract's goods to come are its bonus (`Bank.lend`: `contract.worth()` when there's no delivery).
    let t = book.draft_with_advance(1, 0, 1, 0.0, 0.0, 0, DAY, 1, Incoterm::Exw, true, true, 0.0);
    book.contracts[t].training = Some(enterprise_core::training::Training::new(0, 2, 30.0, 0.0));
    assert!((to_come(&book.contracts[t]) - 30.0).abs() < 1e-12, "half the bonus to lend on");
}

#[test]
fn a_loan_on_a_broken_contract_is_collected_from_what_the_town_can_spare() {
    let mut s = network(40.0, 0.0);
    s.open_bank(0);
    let t = s.now();
    let id = s.draft(1, 0, 1, 64.0, 0.5, t + 2 * DAY, DAY, 3, Incoterm::Exw, true);
    assert!(until(&mut s, 4, |s| s.owed_on(id) > 0.0));
    let first = s.owed_on(id);
    // The town never makes the books: the contract breaks, and the loans are left (lent more as its wagon called).
    assert!(until(&mut s, 20 * 8, |s| s.contract(id).state == State::Broken), "{:?}", s.contract(id));
    let owed = s.owed_on(id);
    assert!(owed >= first && s.bank(0).repaid == 0.0, "nothing came out of a payment: {owed}, {:?}", s.bank(0));
    assert!(owed <= 1.03 * 0.5 * 96.0 + 1e-9, "never past half the goods to come: {owed}");
    let before = s.town(1).money;
    s.pay(1, 200.0); // the town's shops take money: beyond its reserve of 128, it can spare the loan
    assert!(until(&mut s, 20 * 3, |s| s.owed_on(id) < 0.01), "collected: {}", s.owed_on(id));
    assert!((s.bank(0).repaid - owed).abs() < 1e-6, "all of it: {:?}", s.bank(0));
    assert!(s.town(1).money < before + 200.0 - owed + 1e-6, "from its treasury: {}", s.town(1).money);
}

#[test]
fn a_bank_lends_nothing_to_a_town_with_the_money() {
    let mut s = network(40.0, 300.0); // its reserve is two days of its paper, 128: 172 to spare
    s.open_bank(0);
    let t = s.now();
    let id = s.draft(1, 0, 1, 64.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Exw, true);
    for _ in 0..4 {
        look(&mut s);
    }
    assert!(s.route(0).trips > 0 || s.route(0).on_the_road, "its wagon called");
    assert_eq!(s.owed_on(id), 0.0, "it has the money");
    assert_eq!(s.bank(0).loans_made, 0);
}

#[test]
fn a_market_without_a_bank_neither_discounts_nor_lends() {
    let mut s = network(40.0, 0.0);
    let id = s.draw_bill(0, 1, 10.0);
    let c = s.draft(1, 0, 1, 64.0, 0.5, 2 * DAY, DAY, 1, Incoterm::Exw, true);
    for _ in 0..4 {
        look(&mut s);
    }
    assert_eq!(s.bills()[id].state, BillState::Outstanding);
    assert_eq!(s.owed_on(c), 0.0);
    assert_eq!(s.bank(0), Default::default());
}

#[test]
fn the_banks_are_saved_whole() {
    let mut s = network(40.0, 0.0);
    s.open_bank(0);
    let t = s.now();
    let id = s.draft(1, 0, 1, 64.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Exw, true);
    assert!(until(&mut s, 4, |s| s.owed_on(id) > 0.0));
    let bytes = s.save();
    let loaded = Society::load(&bytes).unwrap();
    assert!(loaded.is_bank(0) && !loaded.is_bank(1));
    assert_eq!(loaded.loans(), s.loans());
    assert_eq!(loaded.bank(0), s.bank(0));
}

#[test]
fn the_random_worlds_bank() {
    let (mut bought, mut lent, mut repaid) = (0, 0, 0);
    for (b, l, r) in common::each_seed(0..300, |seed| {
        let mut s = common::world(seed);
        let start = s.totals();
        for day in 1..=120 {
            s.advance(day * DAY);
            common::slices::bank_and_discounts::daily(&mut s, &mut common::Rng::of(seed, "bank_and_discounts", day), day);
            s.conserved(&start).and_then(|_| s.solvent()).and_then(|_| common::slices::bank_and_discounts::check(&s, day))
                .unwrap_or_else(|e| panic!("seed {seed} day {day}: {e}"));
        }
        let (mut bought, mut lent, mut repaid) = (0, 0, 0);
        for t in 0..s.town_count() {
            let b = s.bank(t);
            bought += b.bills_bought;
            lent += b.loans_made;
            repaid += (b.repaid > 0.0) as u32;
        }
        (bought, lent, repaid)
    }) {
        bought += b;
        lent += l;
        repaid += r;
    }
    assert!(bought > 10 && lent > 10 && repaid > 3, "{bought} bills bought, {lent} loans made, {repaid} banks repaid");
}
