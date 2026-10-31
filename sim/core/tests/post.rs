//! The crown's post and the forwarder's consignments (`post.rs`, `rules/post.rs`), pinned as the mod's `PostTests` and
//! `CaravanTests.aContractsCopyGoesByPost` pin them: letters on the post days at a rider's speed for their postage, calls
//! a hub books one a day or declines, bills paid by post and the coin back, the forwarder's collections, and goods on
//! consignment sold and their money remitted by post. The mod's sums in spurs are emeralds here (the core's money).

use enterprise_core::bills::BillState;
use enterprise_core::post::{self, Kind};
use enterprise_core::{Content, Good, Incoterm, Society, State};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;
const LEATHER: usize = 0;
const DIRT: usize = 1;
const PAPER: usize = 2;

fn content() -> Content {
    Content::new(vec![Good::new("leather", 1.0, 64), Good::new("dirt", 0.01, 64), Good::new("paper", 0.5, 64)], DAY)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn look(s: &mut Society) {
    let next = s.now() + LOOK;
    s.advance(next);
}

/// The route from `from` to `to`, if any.
fn route(s: &Society, from: usize, to: usize) -> Option<usize> {
    (0..s.route_count()).find(|&r| s.route_ends(r) == (from, to))
}

/// A world checked as it runs: goods and money conserved, nobody in debt.
fn run(s: &mut Society, to: u64) {
    let start = s.totals();
    while s.now() < to {
        let next = (s.now() + DAY).min(to);
        s.advance(next);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap();
    }
}

#[test]
fn a_letter_arrives_on_the_next_post_day_at_a_riders_speed() {
    let c = content();
    // Every three days the riders leave; 2,000 blocks a day, a tenth of a day at least.
    assert_eq!(post::arrival(&c, 0, Some(4000)), 2 * DAY);
    assert_eq!(post::arrival(&c, 1, Some(4000)), 3 * DAY + 2 * DAY);
    assert_eq!(post::arrival(&c, 3 * DAY, Some(4000)), 5 * DAY);
    assert_eq!(post::arrival(&c, 1, Some(10)), 3 * DAY + 2400);
    assert_eq!(post::arrival(&c, 1, None), 3 * DAY + 2400);
}

// parity: PostTests.aLetterLeavesOnThePostDayAndWaitsAtTheDesk
#[test]
fn a_letter_leaves_on_the_post_day_and_waits_at_the_desk() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let from = s.add_town("From", 5.0);
    let to = s.add_town("To", 0.0);
    s.add_route(to, from, 4000.0 / 1200.0, 0.0, 4.0); // 4,000 blocks between them
    look(&mut s);
    let t = s.now();
    s.send_letter(Kind::Reply, from, to, 0.0);
    assert_eq!(s.mail().on_the_road(to, Kind::Reply), 1, "on the road");
    assert!(s.town(from).money == 4.9, "the postage paid: {}", s.town(from).money);
    let letter = s.mail().letters[0].clone();
    assert_eq!(letter.arrives, 3 * DAY + 2 * DAY, "it leaves on the next post day, and 4,000 blocks take two days");
    look(&mut s);
    assert_eq!(s.mail().on_the_road(to, Kind::Reply), 1, "4000 blocks at 2000 a day: not there yet");
    s.advance(t + 9 * DAY);
    assert_eq!(s.mail().on_the_road(to, Kind::Reply), 0, "read at the desk once it's there");
}

/// A hub with `emeralds` at the origin and towns 400 blocks east, south and west.
fn hub_and_callers(emeralds: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let hub = s.add_town("Hub", emeralds);
    for (i, (x, z)) in [(400.0, 0.0), (0.0, 400.0), (-400.0, 0.0)].into_iter().enumerate() {
        let t = s.add_town(["Near", "Other", "Poor"][i], 5.0);
        s.place_town(t, x, z);
    }
    s.place_town(hub, 0.0, 0.0);
    s.make_hub(hub);
    s
}

// parity: PostTests.aHubBooksACallOneADayAndDeclinesOneThatDoesntPay
#[test]
fn a_hub_books_a_call_one_a_day_and_declines_one_that_doesnt_pay() {
    let mut s = hub_and_callers(20.0);
    let (hub, near, other, poor) = (0, 1, 2, 3);
    s.book_call(near, hub, 200.0);
    s.book_call(other, hub, 200.0);
    s.book_call(poor, hub, 1.0);
    s.deliver_mail_now();
    look(&mut s);
    let calls = s.calls(hub);
    assert_eq!(calls.len(), 2, "two calls booked, the poor one declined: {calls:?}");
    assert_ne!(calls[0].day, calls[1].day, "on different days: one wagon a day, {calls:?}");
    // 400 blocks: 40 food points a leg, both ways, for two: 8 emeralds. 200 at a quarter is 50; 1 is a quarter.
    assert!(close(post::trip_cost(s.content(), 400), 8.0));
    assert!(post::call_pays(s.content(), 200.0, 8.0, 0.0) && !post::call_pays(s.content(), 1.0, 8.0, 0.0));
    let replies: Vec<_> = s.mail().letters.iter().filter(|l| l.kind == Kind::Reply).map(|l| (l.to, l.day)).collect();
    assert!(replies.contains(&(poor, None)), "the poor town hears it's declined: {replies:?}");
    assert!(replies.contains(&(near, Some(calls[0].day))) && replies.contains(&(other, Some(calls[1].day))));
    let run = route(&s, hub, near).expect("a run there");
    assert!(s.on_call(run), "a run there, on call");
    assert!(route(&s, hub, poor).is_none(), "and none to the town that didn't pay");
    let day = calls[0].day;
    assert!(!s.call_due(hub, calls[0].town, (day - 1) * DAY) && s.call_due(hub, calls[0].town, day * DAY),
        "the run's due on its booked day, not before");
}

#[test]
fn a_run_on_call_goes_on_its_day_and_only_then() {
    let mut s = hub_and_callers(40.0);
    let (hub, near) = (0, 1);
    s.book_call(near, hub, 200.0);
    s.deliver_mail_now();
    look(&mut s);
    let call = s.calls(hub)[0];
    let run = route(&s, hub, near).unwrap();
    while s.now() < call.day * DAY - LOOK {
        look(&mut s);
        assert!(!s.route(run).on_the_road, "on call: it waits for its day ({})", s.now());
    }
    run_until(&mut s, call.day * DAY + LOOK);
    assert!(s.route(run).on_the_road || s.route(run).trips > 0, "it goes on its booked day");
    assert!(s.calls(hub).is_empty(), "and its call is made");
}

fn run_until(s: &mut Society, to: u64) {
    while s.now() < to {
        look(s);
    }
}

#[test]
fn the_crown_pays_half_a_first_visits_trip() {
    let mut s = hub_and_callers(20.0);
    let capital = s.add_town("Capital", 100.0);
    s.set_capital(Some(capital));
    let (hub, near, poor) = (0, 1, 3);
    // 20 at a quarter is 5: short of the trip's 8, but not of half of it.
    s.book_call(near, hub, 20.0);
    s.book_call(poor, hub, 10.0);
    s.deliver_mail_now();
    let before = s.town(hub).money;
    look(&mut s);
    let calls = s.calls(hub);
    assert_eq!(calls.len(), 1, "the crown's half pays for one: {calls:?}");
    assert_eq!(calls[0].town, near);
    let cost = post::trip_cost(s.content(), s.mail_distance(hub, near).unwrap());
    assert!(s.town(capital).money == enterprise_core::Fixed::emeralds(100) - enterprise_core::Fixed::floor(0.5 * cost), "the crown paid half the trip: {}", s.town(capital).money);
    assert!(s.town(hub).money == before + enterprise_core::Fixed::floor(0.5 * cost) - enterprise_core::Fixed::from(0.2), "into the hub's money, less two replies' postage");
}

// parity: PostTests.aBillGoesByPostAndTheCoinComesBack
#[test]
fn a_bill_goes_by_post_and_the_coin_comes_back() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let holder = s.add_town("Holder", 2.0);
    let drawee = s.add_town("Drawee", 20.0);
    let id = s.draw_bill(drawee, holder, 5.0); // 5.1, due in four days: no wagon of either goes to the other
    assert!(s.bills()[id].amount == 5.1);
    let due = s.bills()[id].due;
    run(&mut s, due - LOOK);
    assert_eq!(s.mail().on_the_road(drawee, Kind::Bill), 0, "not before it's due");
    run(&mut s, due);
    assert_eq!(s.mail().on_the_road(drawee, Kind::Bill), 1, "the bill sent by post once due");
    assert!(s.town(holder).money == 1.9, "for the postage");
    s.deliver_mail_now();
    look(&mut s);
    assert_eq!(s.bills()[id].state, BillState::Paid, "paid when the drawee read it");
    assert!(s.town(drawee).money == 20.0 - 5.1 - 0.1, "the coin and the postage back: {}", s.town(drawee).money);
    assert_eq!(s.mail().on_the_road(holder, Kind::Remit), 1);
    let before = s.town(holder).money;
    s.deliver_mail_now();
    look(&mut s);
    let after = s.town(holder).money;
    assert!(after > before + enterprise_core::Fixed::emeralds(4) && after == before + enterprise_core::Fixed::from(5.1), "the coin back by post: {before} -> {after}");
}

#[test]
fn a_drawee_short_of_coin_borrows_its_households_savings_or_sends_the_bill_back() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let holder = s.add_town("Holder", 2.0);
    let drawee = s.add_town("Drawee", 3.0);
    s.post(enterprise_core::cash_book::Account::Outside, enterprise_core::cash_book::Account::Households(drawee), 10.0,
        enterprise_core::cash_book::Reason::PaidIn);
    let on_call = s.deposits_on_call(drawee);
    let id = s.draw_bill(drawee, holder, 5.0);
    let big = s.draw_bill(drawee, holder, 100.0);
    let due = s.bills()[id].due;
    run(&mut s, due);
    s.deliver_mail_now();
    look(&mut s);
    assert_eq!(s.bills()[id].state, BillState::Paid, "3 in hand and {on_call} on call pay 5.1");
    assert!(s.deposits(drawee) > 0.0, "the households lent the rest: {}", s.deposits(drawee));
    assert_eq!(s.bills()[big].state, BillState::Outstanding, "the big one it can't: back to its holder");
    assert!(s.bills()[big].presented.is_some(), "its grace runs from now");
    s.deliver_mail_now();
    look(&mut s);
    assert_eq!(s.bills()[big].held, enterprise_core::bills::Held::Town(holder), "home again with the coin");
}

#[test]
fn bills_and_copies_go_by_wagon_where_one_runs() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let a = s.add_town("A", 10.0);
    let b = s.add_town("B", 10.0);
    s.add_route(b, a, 1.0, 0.0, 30.0);
    s.draw_bill(b, a, 5.0);
    let c = s.draft(a, b, PAPER, 32.0, 0.5, 20 * DAY, DAY, 1, Incoterm::Dap, false);
    run(&mut s, 6 * DAY);
    assert!(s.mail().letters.is_empty() && s.mail().sent == 0, "a wagon runs between them: {:?}", s.mail());
    assert_eq!(s.contract(c).state, State::Drafted);
}

// parity-partial: CaravanTests.aContractsCopyGoesByPost (no road here between the towns: the core posts a copy only where no wagon runs, the mod's on whatever runs)
#[test]
fn a_contracts_copy_goes_by_post() {
    // The mod's network: the hub and the town 192 blocks apart (here with no road between them: the core posts a copy
    // only where no wagon runs, where the mod posts one a day on whatever runs).
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", 5.0);
    s.place_town(hub, 0.0, 0.0);
    s.place_town(dest, 192.0, 0.0);
    let t = s.now();
    let c = s.draft(hub, dest, PAPER, 32.0, 0.5, t + 20 * DAY, DAY, 1, Incoterm::Dap, false);
    assert!(!s.contract(c).in_force(), "only the hub holds it");
    s.advance(t + DAY - LOOK);
    assert_eq!(s.mail().on_the_road_from(dest, Kind::Copy, hub), 0, "not before a day");
    s.advance(t + DAY + LOOK);
    assert!(s.mail().on_the_road_from(dest, Kind::Copy, hub) > 0, "a day on, the copy goes by post");
    s.advance(t + 12 * DAY);
    assert!(s.contract(c).in_force(), "read at the town: in force, no wagon sent");
    assert_eq!(s.route_count(), 0);
}

// parity: PostTests.aTownsBookingsFillingAWagonBringTheForwarderToCollect
#[test]
fn a_towns_bookings_filling_a_wagon_bring_the_forwarder_to_collect() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let hub = s.add_town("Hub", 5.0);
    let small = s.add_town("Small", 0.0);
    let big = s.add_town("Big", 0.0);
    s.place_town(hub, 0.0, 0.0);
    s.place_town(small, 300.0, 0.0);
    s.place_town(big, 0.0, 300.0);
    s.make_hub(hub);
    s.book_with_forwarder(hub, small, 2, 10.0);
    s.book_with_forwarder(hub, big, 12, 60.0);
    s.forwarder_review(hub);
    assert!(!s.collecting(hub, small), "two stacks: it waits for more, or the week");
    assert!(s.collecting(hub, big), "twelve: it sends to collect");
    let run = route(&s, hub, big).expect("a run there");
    assert!(s.on_call(run), "on a run on call, the hub having none there");
    assert!(s.call_due(hub, big, s.now()), "due today");
    assert!(route(&s, hub, small).is_none());
    // Through the look, the week waited: the small one collected too.
    run_until(&mut s, 5 * DAY);
    assert!(!s.collecting(hub, small) && route(&s, hub, small).is_none(), "five days: still waiting");
    run_until(&mut s, 7 * DAY);
    let collected = !s.forwarder().bookings.iter().any(|b| b.town == small);
    assert!(s.collecting(hub, small) || collected, "a week waited: it collects the small one too");
    assert!(route(&s, hub, small).is_some_and(|r| s.on_call(r)), "on a run on call");
}

/// A hub whose leather sells at a buyer 300 blocks on, and a town 300 blocks off with leather to spare and no wagon.
fn forwarding() -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 60.0);
    let town = s.add_town("Town", 5.0);
    let buyer = s.add_town("Buyer", 400.0);
    s.place_town(hub, 0.0, 0.0);
    s.place_town(town, 300.0, 0.0);
    s.place_town(buyer, 0.0, 300.0);
    s.make_hub(hub);
    s.set_trade(buyer, LEATHER, 0.0, 64.0);
    s.add_road(hub, buyer, 1.0).unwrap();
    s.deliver(town, LEATHER, 640.0);
    s
}

#[test]
fn a_town_books_by_post_and_its_goods_are_collected_on_consignment() {
    let mut s = forwarding();
    let (hub, town) = (0, 1);
    run(&mut s, DAY / 2);
    assert_eq!(s.mail().on_the_road_from(hub, Kind::Book, town), 1, "it books with its hub by post");
    let booked = s.mail().letters.iter().find(|l| l.kind == Kind::Book).unwrap().clone();
    assert_eq!(booked.stacks, 10, "ten stacks of leather for sale");
    assert!(booked.value == 640.0);
    let start = s.totals();
    while s.forwarder().lots.is_empty() && s.now() < 8 * DAY {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap();
    }
    let lot = *s.forwarder().lots.first().expect("the hub's wagon collected it on consignment");
    assert!(lot.town == town && lot.good == LEATHER && lot.count > 0.0 && lot.sold == 0.0, "{lot:?}");
    assert!(close(s.town(town).stock[LEATHER], 640.0 - lot.count), "out of the town's warehouse");
    let collector = route(&s, hub, town).unwrap();
    assert!(s.on_call(collector), "on a run on call");
    // A run on call: a stack's fare half as much again, on the towns' distance as the booking reckoned it
    // (`Forwarder.fare`'s `distSqr`: here between their hexes' centres), not the road's (300 blocks, desk to desk).
    let (a, b) = (s.town_hex(hub).unwrap(), s.town_hex(town).unwrap());
    let (dx, dz) = (a.center_x() - b.center_x(), a.center_z() - b.center_z());
    let apart = (dx * dx + dz * dz).sqrt().ceil() as u32;
    let fare = post::fare(s.content(), apart, false);
    assert!(lot.fare == enterprise_core::Fixed::ceil(fare * (lot.count / 64.0).ceil()), "each stack its fare: {lot:?}");
    let before = s.town(town).money;
    run(&mut s, 20 * DAY);
    let f = s.forwarder();
    assert!(f.remitted > 0.0 && f.commission > 0.0, "sold on at the buyer's, the money remitted: {f:?}");
    assert!(s.town(town).money > before, "and come home by post: {before} -> {}", s.town(town).money);
}

/// Steps the look until the forwarder holds a lot, then until the hub's collecting wagon is loaded: the pallets it carries.
fn collected(s: &mut Society, hub: usize, town: usize) -> Vec<enterprise_core::pallet::Pallet> {
    let run = loop {
        look(s);
        assert!(s.now() < 8 * DAY, "the hub never collected");
        if !s.forwarder().lots.is_empty() {
            break route(s, hub, town).unwrap();
        }
    };
    s.wagon_pallets(run).0
}

// consignment-pallets: the town's booked goods go to the hub as pallets made out to it, paid, sealed aboard
#[test]
fn a_collection_carries_the_towns_goods_as_pallets_made_out_to_the_hub() {
    let mut s = forwarding();
    let (hub, town) = (0, 1);
    let papers = s.paperwork(town).by_kind.get(enterprise_core::cargo::LADING).copied().unwrap_or(0);
    let aboard = collected(&mut s, hub, town);
    let lot = *s.forwarder().lots.first().unwrap();
    assert!(!aboard.is_empty(), "the goods ride as pallets, not loose");
    // Packed and billed at the town: a bill of lading a pallet, and its crates bought from its treasury.
    let drawn = s.paperwork(town).by_kind.get(enterprise_core::cargo::LADING).copied().unwrap_or(0) - papers;
    assert_eq!(drawn, aboard.len() as u64, "a bill of lading for each pallet, drawn at the town");
    let crates: Vec<_> = s.cash_book(town, 30).into_iter().filter(|(_, why, _)| why.contains("crates")).collect();
    assert!(!crates.is_empty() && crates.iter().all(|(_, _, amount)| *amount < 0.0), "the town pays for its crates: {crates:?}");
    for p in &aboard {
        assert!(p.lading.shipper == town && p.lading.consignee == hub && p.lading.contract.is_none(), "{p:?}");
        assert!(p.lading.payment == enterprise_core::pallet::Payment::Paid, "nobody owes for them: {p:?}");
        assert!(p.seal.is_some(), "sealed as the wagon loaded them");
        assert!(p.packages.iter().all(|k| k.good == LEATHER));
    }
    let carried: f64 = aboard.iter().map(|p| p.count()).sum();
    assert!(close(carried, lot.count), "the lot is what the pallets hold: {carried} vs {lot:?}");
    assert!(aboard.iter().any(|p| p.packages.iter().any(|k| k.kind != enterprise_core::pallet::LOOSE)), "crated where it can");
    // At the hub they're opened into its stock; none left in the registry for the lot.
    run(&mut s, 6 * DAY);
    assert!(s.pallets().pallets.values().all(|p| !(p.lading.shipper == town && p.lading.consignee == hub)),
        "every consignment pallet opened at the hub, none left aboard or anywhere else");
}

// consignment-pallets: a town that can't pay for crates still consigns, loose on its pallets
#[test]
fn a_town_that_cant_pay_for_crates_still_consigns_loose() {
    let mut s = forwarding();
    s.content_mut().dials.crate_emeralds = 1000.0;
    let (hub, town) = (0, 1);
    let aboard = collected(&mut s, hub, town);
    let lot = *s.forwarder().lots.first().unwrap();
    assert!(!aboard.is_empty() && close(aboard.iter().map(|p| p.count()).sum::<f64>(), lot.count));
    assert!(aboard.iter().flat_map(|p| &p.packages).all(|k| k.kind == enterprise_core::pallet::LOOSE),
        "no crates to be had: loose packages");
    assert!(s.cash_book(town, 30).iter().all(|(_, why, _)| !why.contains("crates")), "and none bought");
}

// parity: PostTests.goodsOnConsignmentSellAndTheMoneyComesBackByPost
#[test]
fn goods_on_consignment_sell_and_the_money_comes_back_by_post() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let hub = s.add_town("Hub", 100.0);
    let town = s.add_town("Town", 0.0);
    s.consign_lot(hub, town, LEATHER, 20.0, 5.0);
    s.consign_sold(hub, LEATHER, 20.0, 100.0); // sold for 100
    s.forwarder_review(hub);
    let f = s.forwarder();
    assert!(f.remitted == 85.0 && f.commission == 10.0, "100: 10 commission, 5 fare, 85 remitted: {f:?}");
    assert!(f.lots.is_empty(), "done");
    assert_eq!(s.mail().on_the_road(town, Kind::Remit), 1, "the coin on its way by post");
    assert!(s.town(hub).money == 100.0 - 85.0 - 0.1);
    s.deliver_mail_now();
    look(&mut s);
    assert!(s.town(town).money == 85.0, "and in the town's chest: {}", s.town(town).money);
}

#[test]
fn a_lot_waits_for_an_emeralds_worth_and_one_unsold_the_forwarder_buys() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let hub = s.add_town("Hub", 100.0);
    let town = s.add_town("Town", 0.0);
    s.consign_lot(hub, town, LEATHER, 20.0, 0.0);
    s.consign_sold(hub, LEATHER, 1.0, 1.0);
    s.forwarder_review(hub);
    assert_eq!(s.forwarder().remitted, 0.0, "0.9 isn't an emerald yet");
    s.consign_sold(hub, LEATHER, 1.0, 1.0);
    s.forwarder_review(hub);
    assert!(s.forwarder().remitted == 1.8, "1.8 is");
    s.advance(20 * DAY + LOOK);
    s.forwarder_review(hub);
    // The 18 left, bought at a trader's floor, less the commission.
    let floor = enterprise_core::bargain::seller_floor(1.0);
    assert!(s.forwarder().remitted == 1.8 + 18.0 * floor * 0.9, "{:?}", s.forwarder());
    assert!(s.forwarder().lots.is_empty());
}

// parity: PostTests.whatATownsWagonCantSellAtTheHubIsLeftOnConsignment
#[test]
fn what_a_towns_wagon_cant_sell_at_the_hub_is_left_on_consignment() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let hub = s.add_town("Hub", 1000.0);
    let town = s.add_town("Town", 0.0);
    // The hub's lockers take leather, 32 a day (its par 160); nobody there wants dirt.
    s.set_trade(hub, LEATHER, 0.0, 32.0);
    let mut cargo = vec![64.0, 64.0, 0.0];
    let left = s.consign_unsold(hub, town, &mut cargo);
    assert!(close(left, 64.0) && close(s.town(hub).stock[LEATHER], 64.0),
        "the leather the hub's shelves want stays, on consignment: {left}");
    assert!(cargo[LEATHER] == 0.0 && cargo[DIRT] == 64.0, "the dirt rides home");
    let before = s.forwarder().remitted;
    s.consign_sold(hub, LEATHER, 64.0, 160.0);
    s.forwarder_review(hub);
    assert!(s.forwarder().remitted - before == 144.0,
        "sold: the town's 160, less 10% commission, on their way: {}", s.forwarder().remitted - before);
}

#[test]
fn the_hubs_par_bounds_what_it_takes_on_consignment() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    let hub = s.add_town("Hub", 10.0);
    let town = s.add_town("Town", 0.0);
    s.set_trade(hub, LEATHER, 0.0, 32.0); // a par of 160
    s.deliver(hub, LEATHER, 40.0);
    s.consign_lot(hub, town, LEATHER, 30.0, 0.0); // 30 on consignment already
    let mut cargo = vec![200.0, 0.0, 0.0];
    // 160 less the 40 it holds and the 30 on consignment: 90, in whole stacks: 64.
    assert!(close(s.consign_unsold(hub, town, &mut cargo), 64.0));
    assert!(close(cargo[LEATHER], 136.0));
}

#[test]
fn a_towns_wagon_leaves_what_the_hub_cant_buy_on_consignment_and_is_paid_by_post() {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 0.0);
    let town = s.add_town("Town", 5.0);
    let buyer = s.add_town("Buyer", 400.0);
    s.make_hub(hub);
    s.set_trade(hub, LEATHER, 0.0, 32.0);
    s.set_trade(buyer, LEATHER, 0.0, 64.0);
    s.deliver(town, LEATHER, 600.0);
    // The town's own wagon on a four-day timetable: waiting for a fuller load between, it books carriage by post
    // (`Forwarder.considerBooking`: a town whose wagon to the hub is waiting doesn't ship for itself). (A route
    // plans once a day after its wagon left, `DEPARTURE_INTERVAL`: a daily wagon of its own never waits.)
    s.add_route(town, hub, 0.25, 0.0, 4.0);
    s.add_route(hub, buyer, 0.25, 0.0, 1.0);
    run(&mut s, 2 * DAY);
    let f = s.forwarder().clone();
    assert!(f.consigned > 0.0, "the hub had no money: the town's leather left on consignment: {f:?}");
    run(&mut s, 12 * DAY);
    let f = s.forwarder().clone();
    assert!(f.remitted > 0.0 && f.commission > 0.0, "sold at the buyer's and the money sent home: {f:?}");
}

#[test]
fn the_post_is_saved_whole() {
    let mut s = hub_and_callers(20.0);
    s.book_call(1, 0, 200.0);
    s.consign_lot(0, 2, LEATHER, 20.0, 1.0);
    s.book_with_forwarder(0, 3, 2, 10.0);
    let bytes = s.save();
    let loaded = Society::load(&bytes).unwrap();
    assert_eq!(loaded.mail().letters, s.mail().letters);
    assert_eq!(loaded.forwarder().lots, s.forwarder().lots);
    assert_eq!(loaded.forwarder().bookings, s.forwarder().bookings);
}


#[test]
fn a_town_booked_with_a_second_hub_is_no_longer_collected_by_the_first() {
    // Its letter to one hub on the road, its nearest hub another by the time it books again: the second booking
    // replaces the first, and the first hub's collection goes with it (a random world's sweep, seed 208).
    let c = content();
    let mut f = post::Forwarder::default();
    f.booked(0, 2, 64, 100.into(), 0);
    assert_eq!(f.collections(&c, 0, 0).len(), 1);
    assert!(f.is_collecting(0, 2));
    f.booked(1, 2, 64, 100.into(), 0);
    assert!(!f.is_collecting(0, 2) && f.bookings.iter().all(|b| b.hub == 1), "{f:?}");
}
