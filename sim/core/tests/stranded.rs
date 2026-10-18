//! Stranded paper (`stranded.rs`, `rules/stranded.rs`), pinned as the mod's game test pins it (`ContractTests`): a hub that
//! runs to a market buys a bill a town holds on it, whose wagons don't call, for coin as far as it trusts the drawee and can
//! spare and its own bills for the rest, with the goods the bill paid for; and the rest of what makes a closed road bearable:
//! the hub's wagon buys a town's paper where it calls, a town's wagon sells a hub what it brought, goods ride through the hub
//! on credit, the pupil's work waits for a collector, dead crates go back on the shelf, a bill a hub would buy doesn't lapse,
//! and a Merchant short of coin honours a bill with its households' savings.

use enterprise_core::bills::{BillState, Held, LAPSE_DAYS};
use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::cargo::Shipment;
use enterprise_core::contracts::Book;
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const LEGGINGS: usize = 2;
const CLOTH: usize = 3;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("iron_leggings", 3.0, 1), Good::new("cloth", 0.5, 64)], DAY)
}

/// One look on.
fn look(s: &mut Society) {
    let next = s.now() + s.content().dials.look_ticks;
    s.advance(next);
}

/// The mod's test: a hub (0), a town (1) and the capital (2), the drawee, 400 blocks off each: the hub runs to the capital
/// (`ledger.addRoute(hub, capital)`), the capital doesn't call; the town has no road to anywhere.
fn arena(hub_money: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    s.add_town("Hub", hub_money);
    s.add_town("Town", 5.0);
    s.add_town("Capital", 50.0);
    s.add_route(0, 2, 200.0 / 1200.0, 0.0, 4.0);
    s
}

/// A crate of `n` leggings the town made for the capital, paid for by a bill (`Papers.billOfLading(...)` marked settled).
fn leggings(shipper: usize, consignee: usize) -> Shipment {
    Shipment { good: LEGGINGS, count: 1.0, shipper, consignee: Some(consignee), contract: None, settled: true, copy: false }
}

// parity: ContractTests.aHubBuysATownsStrandedPaperForCoinAndItsOwnBill
#[test]
fn a_hub_buys_a_towns_stranded_paper_for_coin_and_its_own_bill() {
    let (hub, town, capital) = (0, 1, 2);
    let mut s = arena(100.0);
    let id = s.draw_bill(capital, town, 100.0); // for goods crated for the capital; long past its day by now
    s.advance(10 * DAY);
    let mut bundles = vec![id];
    let mut crates = vec![leggings(town, capital)];
    let rate = s.mint_rate();
    let deal = s.buy_stranded(hub, town, &mut crates, &mut bundles, 1000.0);
    assert!(deal.is_some(), "the hub buys it");
    let deal = deal.unwrap();
    let face = 100.0 * (1.0 + 0.02);
    let d = &s.content().dials;
    let ceiling = face * (1.0 - 0.02 - d.stale_bill_discount) * s.trust(hub, capital);
    let coin = deal.coin(rate);
    assert!(coin > 0.0 && coin <= ceiling + 0.2, "coin as far as it trusts the drawee, less the forwarding: {coin} (at most {ceiling})");
    assert!(deal.hub_bill().is_some() && deal.crates.len() == 1 && crates.is_empty(),
        "the rest in the hub's own bill, and the goods go with the bill");
    assert_eq!(s.bills()[id].payee, hub, "the bill endorsed to the hub");
    // The money adds up: a coin share and the hub's bills make the price, less the goods' forwarding.
    let promised: enterprise_core::Fixed = deal.hub_bills.iter().map(|&b| s.bills()[b].amount).sum();
    // (The price is a valuation; the coin is rounded down to the thousandth and the bill's debt and interest each up,
    // so they make it to within the two thousandths those roundings move: `fixed.rs`.)
    assert!(coin + promised >= enterprise_core::Fixed::floor(deal.price) && coin + promised <= enterprise_core::Fixed::ceil(deal.price) + enterprise_core::Fixed::from_millis(2),
        "coin {coin} and bills {promised} against a price of {}", deal.price);
    let bill = &s.bills()[deal.hub_bill().unwrap()];
    assert!(bill.drawee == hub && bill.payee == town, "the hub's own bill to the town");
    assert_eq!(s.stranded_book(hub).bills, 1);
}

#[test]
fn a_bill_is_stranded_where_its_drawees_wagons_dont_call_or_it_is_past_its_day() {
    let (hub, town, capital) = (0, 1, 2);
    let mut s = arena(100.0);
    // The capital's wagons call at the town: a bill not yet due is presented in the ordinary way.
    s.add_route(capital, town, 0.1, 0.0, 4.0);
    let id = s.draw_bill(capital, town, 10.0);
    let (mut crates, mut bills) = (Vec::new(), vec![id]);
    assert!(s.buy_stranded(hub, town, &mut crates, &mut bills, 100.0).is_none(), "its drawee's wagons call: not stranded");
    assert_eq!(bills, vec![id], "and the bill is left");
    // Past its day it is, though the wagons still call.
    s.advance(6 * DAY);
    assert!(s.bills()[id].due < s.now());
    let deal = s.buy_stranded(hub, town, &mut crates, &mut bills, 100.0);
    assert!(deal.is_some() && bills.is_empty(), "past its day: stranded");
    // The road closes: a fresh bill is stranded at once.
    let mut s = arena(100.0);
    s.add_route(capital, town, 0.1, 0.0, 4.0);
    let id = s.draw_bill(capital, town, 10.0);
    assert!(s.close_route(capital, town), "closed");
    let (mut crates, mut bills) = (Vec::new(), vec![id]);
    assert!(s.buy_stranded(hub, town, &mut crates, &mut bills, 100.0).is_some(), "wagons that don't call: stranded");
}

#[test]
fn never_a_bill_on_the_hub_itself_nor_on_a_market_it_does_not_run_to() {
    let (hub, town, capital) = (0, 1, 2);
    let mut s = arena(100.0);
    let on_hub = s.draw_bill(hub, town, 10.0);
    let elsewhere = s.add_town("Far", 10.0);
    let on_far = s.draw_bill(elsewhere, town, 10.0); // fresh: past its day the post would take it to its drawee
    let (mut crates, mut bills) = (Vec::new(), vec![on_hub, on_far]);
    assert!(s.buy_stranded(hub, town, &mut crates, &mut bills, 100.0).is_none(),
        "the town presents the hub's bill there, for its face; the hub runs nowhere to the other");
    assert_eq!(bills.len(), 2);
    // A crate not paid for by a bill isn't paper the hub buys either.
    let mut unpaid = leggings(town, capital);
    unpaid.settled = false;
    let on_capital = s.draw_bill(capital, town, 10.0); // not yet due, the capital's wagons don't call
    let (mut crates, mut bills) = (vec![unpaid.clone()], vec![on_capital]);
    let deal = s.buy_stranded(hub, town, &mut crates, &mut bills, 100.0).unwrap();
    assert!(deal.crates.is_empty() && crates == vec![unpaid], "an unsettled crate stays");
}

#[test]
fn the_hub_pays_in_up_to_six_installments_each_at_most_half_what_it_could_pay_and_otherwise_does_not_buy() {
    let (hub, town, capital) = (0, 1, 2);
    let mut s = arena(100.0);
    s.set_dial("discount_book_share", 4.0).unwrap();
    s.set_dial("discount_drawee_share", 1.0).unwrap();
    let id = s.draw_bill(capital, town, 300.0);
    s.advance(10 * DAY);
    let (mut crates, mut bills) = (Vec::new(), vec![id]);
    // Means of 40: six installments of 20 would cover 120, less than the 269 it would promise.
    assert!(s.buy_stranded_within(hub, town, &mut crates, &mut bills, 0.0, 40.0).is_none(), "more than it could pay: it leaves it");
    assert_eq!(bills, vec![id]);
    // Means of 100: the whole price in its own bills (no coin), each at most half of it, six at most.
    let deal = s.buy_stranded_within(hub, town, &mut crates, &mut bills, 0.0, 100.0).expect("bought");
    assert_eq!(deal.spurs, 0);
    assert_eq!(deal.hub_bills.len(), 6, "six installments");
    let term = 4 * DAY;
    for (i, &b) in deal.hub_bills.iter().enumerate() {
        let b = &s.bills()[b];
        assert_eq!(b.due, s.now() + term * (i as u64 + 1), "a bill term apart");
        assert!(b.amount <= 50.0, "each at most half of the hub's means: {}", b.amount);
        // A sixth of the price, its interest a bill term's for every term it runs (`Bills.draw` with a term).
        // (the debt and its interest each rounded up to the thousandth: the drawee's)
        let due = enterprise_core::Fixed::ceil(deal.price / 6.0 / 1.02).mul_ceil(1.0 + 0.02 * (i as f64 + 1.0));
        assert!(b.amount == due, "installment {i}: {} against {due}", b.amount);
    }
    let owed: enterprise_core::Fixed = deal.hub_bills.iter().map(|&b| s.bills()[b].amount).sum();
    let all: enterprise_core::Fixed = (0..6).map(|i| enterprise_core::Fixed::ceil(deal.price / 6.0 / 1.02).mul_ceil(1.0 + 0.02 * (i as f64 + 1.0))).sum();
    assert!(owed > deal.price && owed == all && (owed.to_f64() - deal.price * 1.07 / 1.02).abs() < 0.012,
        "the whole price, and the later terms' interest: {owed} of {}", deal.price);
}

#[test]
fn a_dishonoured_drawee_is_trusted_less_and_pays_less_coin() {
    let (hub, town, capital) = (0, 1, 2);
    let mut s = arena(100.0);
    let id = s.draw_bill(capital, town, 100.0);
    s.advance(10 * DAY);
    let coin_at = |s: &mut Society, id: usize| {
        let (mut crates, mut bills) = (Vec::new(), vec![id]);
        let deal = s.buy_stranded(hub, town, &mut crates, &mut bills, 1000.0).unwrap();
        deal.coin(s.mint_rate())
    };
    let strangers = coin_at(&mut s, id);
    let mut t = arena(100.0);
    let id = t.draw_bill(capital, town, 100.0);
    t.advance(10 * DAY);
    let now = t.now();
    t.world.resource_mut::<Book>().kept(capital, hub, 4.0, now, DAY);
    assert!(t.trust(hub, capital) > s.trust(hub, capital));
    let friends = coin_at(&mut t, id);
    assert!(friends > strangers, "a drawee the hub trusts is paid for in more coin: {friends} against {strangers}");
}

/// A hub (0) that runs to the capital (2) and calls at the town (1), with paper to sell the town; the town holds a stranded
/// bill on the capital (from `ago` days back).
fn hub_calls(hub_money: f64, ago: u64) -> (Society, usize) {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", hub_money);
    let town = s.add_town("Town", 5.0);
    let capital = s.add_town("Capital", 50.0);
    s.add_route(hub, capital, 200.0 / 1200.0, 0.0, 4.0);
    s.add_route(hub, town, 0.01, 0.0, 1.0);
    s.make_hub(hub);
    s.set_trade(town, PAPER, 0.0, 64.0);
    s.deliver(hub, PAPER, 128.0);
    s.deliver(hub, BREAD, 128.0);
    s.deliver(town, BREAD, 128.0);
    s.deliver(capital, BREAD, 128.0);
    let id = s.draw_bill(capital, town, 20.0);
    if ago > 0 {
        s.advance(ago * DAY);
    }
    (s, id)
}

#[test]
fn a_hubs_wagon_calling_at_a_town_buys_its_stranded_bill_and_carries_it_home() {
    let (mut s, id) = hub_calls(100.0, 0);
    let (hub, town) = (0, 1);
    let start = s.totals();
    let hub_bills_before = s.bills().len();
    let mut bought = false;
    for _ in 0..(8 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("{e}"));
        if s.bills()[id].payee == hub {
            bought = true;
            break;
        }
    }
    assert!(bought, "the hub's wagon bought it: {:?}", s.bills()[id]);
    let book = s.stranded_book(hub);
    assert_eq!(book.bills, 1, "{book:?}");
    // The hub's own bills (the wagon's purse beyond its road food was little) are the town's, filed with it.
    let own: Vec<_> = s.bills().into_iter().skip(hub_bills_before).filter(|b| b.drawee == hub && b.payee == town).collect();
    assert!(!own.is_empty() && own.iter().all(|b| b.held == Held::Town(town)), "{own:?}");
    assert!(s.bills()[id].amount > 0.0 && matches!(s.bills()[id].held, Held::Wagon | Held::Town(_)));
    // Home, the bill is in the hub's files.
    for _ in 0..(3 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
    }
    assert_eq!(s.bills()[id].held, Held::Town(hub), "home with the wagon, in the hub's files");
}

#[test]
fn the_goods_a_hubs_wagon_buys_with_a_bill_go_to_its_forwarder() {
    let (mut s, id) = hub_calls(100.0, 0);
    let (hub, town, capital) = (0, 1, 2);
    s.deliver_shipment(town, leggings(town, capital));
    let start = s.totals();
    for _ in 0..(12 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("{e}"));
    }
    assert_eq!(s.bills()[id].payee, hub);
    assert!(s.shipments(town).held.is_empty(), "the town's crate left with the wagon");
    let h = s.hub(hub);
    let at_hub = h.held.iter().filter(|x| x.shipment.good == LEGGINGS).count();
    let on_road = (0..s.route_count()).map(|r| s.wagon_shipments(r).0.iter().filter(|x| x.good == LEGGINGS).count()).sum::<usize>();
    let received = s.town(capital).stock[LEGGINGS] as usize;
    assert_eq!(at_hub + on_road + received, 1, "in the forwarder's hands, on its run to the capital, or there: {h:?}");
    assert_eq!(s.stranded_book(hub).crates, 1);
}

/// A town (0) whose wagon goes to the hub (1), which runs to the capital (2): the town holds a stranded bill and a crate.
fn town_to_hub(hub_money: f64) -> Society {
    town_to_hub_with(5.0, hub_money, true)
}

/// (`town_bread` false: the town sets out with no food of its own, and its wagon with a purse to buy the road home's at the hub.)
fn town_to_hub_with(town_money: f64, hub_money: f64, town_bread: bool) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let town = s.add_town("Town", town_money);
    let hub = s.add_town("Hub", hub_money);
    let capital = s.add_town("Capital", 50.0);
    s.add_route(town, hub, 0.01, 0.0, 1.0);
    s.add_route(hub, capital, 200.0 / 1200.0, 0.0, 4.0);
    s.make_hub(hub);
    s.set_trade(hub, PAPER, 0.0, 64.0);
    if town_bread {
        s.deliver(town, BREAD, 128.0);
    }
    s.deliver(hub, BREAD, 128.0);
    s.deliver(capital, BREAD, 128.0);
    s
}

/// A bill on `drawee` drawn mid-morning and let fall due later in a day, the town's wagon not yet gone: past its day, and the
/// post (which mails a due bill at a town's first look of a day, to a drawee it has no regular road to) not yet by. The
/// stranded sale then does what it is for within the day, before the post's daily send.
fn past_its_day(s: &mut Society, drawee: usize, town: usize) -> usize {
    s.advance(10 * LOOK);
    let id = s.draw_bill(drawee, town, 20.0);
    let due = s.bills()[id].due;
    s.advance(due + LOOK);
    id
}

#[test]
fn a_towns_wagon_to_a_hub_carries_its_stranded_paper_and_sells_it_there() {
    let (town, hub, capital) = (0, 1, 2);
    let mut s = town_to_hub(100.0);
    let id = past_its_day(&mut s, capital, town);
    s.deliver_shipment(town, leggings(town, capital));
    assert!(s.bills()[id].due < s.now() && s.route(0).trips == 0);
    s.deliver(town, PAPER, 128.0); // now it has: the wagon goes
    let start = s.totals();
    let before = s.town(town).money;
    let mut sold = false;
    for _ in 0..(3 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("{e}"));
        if s.bills()[id].payee == hub {
            sold = true;
            break;
        }
    }
    assert!(sold, "sold to the hub: {:?} {:?}", s.bills()[id], s.stranded_book(hub));
    let book = s.stranded_book(hub);
    assert!(book.coin > 0.0 && book.crates == 1, "coin, and the crate: {book:?}");
    assert!(matches!(s.bills()[id].held, Held::Town(h) if h == hub) || s.bills()[id].held == Held::Wagon,
        "filed with the hub, or already on the hub's own wagon to the capital: {:?}", s.bills()[id].held);
    // The coin rides home with the wagon.
    for _ in 0..(3 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
    }
    assert!(s.town(town).money > before + book.coin.mul_floor(0.5), "the town has the coin: {} from {before}", s.town(town).money);
    assert!(s.shipments(town).held.is_empty(), "the crate went with the wagon");
    // And the rest in the hub's bills, to the town (the wagon carried them home).
    let own: Vec<_> = s.bills().into_iter().filter(|b| b.drawee == hub && b.payee == town).collect();
    assert!(!own.is_empty() && own.iter().all(|b| b.held == Held::Town(town)), "the hub's own bill for the rest: {own:?}");
    // The hub's own wagon takes the bill it bought to the capital, which pays it.
    for _ in 0..(6 * DAY / LOOK) {
        look(&mut s);
    }
    assert_eq!(s.bills()[id].state, BillState::Paid, "presented at the capital by the hub's wagon: {:?}", s.bills()[id]);
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
}

/// The mod sells the stranded paper before it hands the forwarder the crates (`sellStrandedAtHub`, then `handOver`): a town's
/// wagon with a purse that could pay the forwarding still sells the bill with its crate, and the forwarder is paid nothing.
#[test]
fn a_wagon_with_a_purse_sells_the_hub_the_crate_with_its_bill_not_hands_it_over_for_forwarding() {
    let (town, hub, capital) = (0, 1, 2);
    let mut s = town_to_hub_with(200.0, 100.0, true);
    let id = past_its_day(&mut s, capital, town);
    s.deliver_shipment(town, leggings(town, capital));
    // The town owes the hub for cloth it collects there Ex Works, and the hub has none by it: the wagon goes with the purse
    // for it (64 emeralds) and comes back with it.
    s.draft(hub, town, CLOTH, 64.0, 1.0, s.now(), 7 * DAY, 4, Incoterm::Exw, true);
    s.deliver(town, PAPER, 128.0);
    let start = s.totals();
    for _ in 0..(3 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("{e}"));
        if s.bills()[id].payee == hub {
            break;
        }
    }
    assert_eq!(s.bills()[id].payee, hub, "sold: {:?}", s.stranded_book(hub));
    assert_eq!(s.stranded_book(hub).crates, 1, "the crate went with its bill: {:?}", s.stranded_book(hub));
    assert_eq!(s.hub(hub).book.forwarded, 0, "and was not handed the forwarder for its fee: {:?}", s.hub(hub).book);
}

/// What the hub won't buy with the bill (it has no bill of the town's to buy it with) is handed the forwarder as any crate
/// bound beyond, as the mod's later `handOver` does.
#[test]
fn a_stranded_crate_the_hub_does_not_buy_is_handed_the_forwarder_for_its_fee() {
    let (town, hub, capital) = (0, 1, 2);
    let mut s = town_to_hub_with(200.0, 100.0, true);
    // (a purse for the forwarding, as above)
    s.draft(hub, town, CLOTH, 64.0, 1.0, 0, 7 * DAY, 4, Incoterm::Exw, true);
    s.deliver_shipment(town, leggings(town, capital));
    s.deliver(town, PAPER, 128.0);
    for _ in 0..(3 * DAY / LOOK) {
        look(&mut s);
    }
    assert_eq!(s.stranded_book(hub).crates, 0);
    assert!(s.hub(hub).book.forwarded >= 1 || !s.hub(hub).held.is_empty(), "the forwarder has it: {:?}", s.hub(hub).book);
}

#[test]
fn a_hub_short_of_coin_borrows_its_households_savings_to_buy_stranded_paper() {
    let (town, hub, capital) = (0, 1, 2);
    let mut s = town_to_hub(0.0);
    s.post(Account::Outside, Account::Households(hub), 400.0, Reason::PaidIn);
    let id = past_its_day(&mut s, capital, town);
    assert!(s.bills()[id].due < s.now() && s.route(0).trips == 0);
    s.deliver(town, PAPER, 128.0);
    let start = s.totals();
    for _ in 0..(3 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
        if s.bills()[id].payee == hub {
            break;
        }
    }
    assert_eq!(s.bills()[id].payee, hub, "bought");
    assert!(s.deposits(hub) > 0.0, "with its households' savings: {}", s.deposits(hub));
    assert!(s.town(hub).money >= -1e-9);
}

#[test]
fn a_bill_a_hub_would_buy_does_not_lapse_but_one_no_one_can_carry_does() {
    let (town, hub, capital) = (0, 1, 2);
    // No road, no hub: it lapses 12 days past its day.
    let mut s = Society::new(content());
    s.add_town("Town", 5.0);
    s.add_town("Hub", 5.0);
    s.add_town("Capital", 5.0);
    let id = s.draw_bill(capital, town, 10.0);
    let due = s.bills()[id].due;
    s.advance(due + LAPSE_DAYS as u64 * DAY + DAY);
    assert_eq!(s.bills()[id].state, BillState::Dishonoured, "no road between them and no hub to buy it");
    // The town's wagons reach a hub that runs to the drawee: collectable, so it stands.
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    s.add_town("Town", 5.0);
    s.add_town("Hub", 5.0);
    s.add_town("Capital", 5.0);
    s.add_route(town, hub, 0.01, 0.0, 4.0);
    s.add_route(hub, capital, 0.2, 0.0, 4.0);
    s.make_hub(hub);
    assert!(s.collectable(town, capital) && !s.collectable(capital, town));
    let id = s.draw_bill(capital, town, 10.0);
    let due = s.bills()[id].due;
    s.advance(due + LAPSE_DAYS as u64 * DAY + DAY);
    assert_eq!(s.bills()[id].state, BillState::Outstanding, "a hub would buy it: it stands");
}

#[test]
fn a_merchant_short_of_coin_honours_a_bill_with_its_households_savings() {
    // The wagon of the payee presents the bill at the drawee, which has the savings but not the coin.
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let payee = s.add_town("Payee", 5.0);
    let drawee = s.add_town("Drawee", 1.0);
    s.add_route(payee, drawee, 0.01, 0.0, 1.0);
    s.set_trade(drawee, PAPER, 0.0, 64.0);
    s.deliver(payee, PAPER, 128.0);
    s.deliver(payee, BREAD, 128.0);
    s.deliver(drawee, BREAD, 128.0);
    s.post(Account::Outside, Account::Households(drawee), 200.0, Reason::PaidIn);
    let id = s.draw_bill(drawee, payee, 20.0);
    let start = s.totals();
    let mut paid = false;
    for _ in 0..(8 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("{e}"));
        if s.bills()[id].state == BillState::Paid {
            paid = true;
            break;
        }
    }
    assert!(paid, "honoured: {:?}", s.bills()[id]);
    assert!(s.deposits(drawee) > 0.0, "out of its households' savings, owed them: {}", s.deposits(drawee));
}

/// The smithy (0), the capital (1) that buys the pupil's work, and a hub (2): the town's wagons reach the hub, which runs
/// to the capital.
#[test]
fn the_pupils_work_is_crated_for_a_buyer_only_where_a_collector_comes() {
    let (smithy, capital, hub) = (0, 1, 2);
    let make = |roads: &dyn Fn(&mut Society)| {
        let mut s = Society::new(Content::new(vec![Good::new("coal", 0.1, 64), Good::new("iron_axe", 0.5, 1)], DAY));
        s.add_town("Smithy", 0.0);
        s.add_town("Capital", 0.0);
        s.add_town("Hub", 50.0);
        s.set_roster(smithy, &[("weaponsmith", 1), ("merchant", 1)]);
        s.pay(smithy, 40.0);
        s.set_offers(0, 0, vec![enterprise_core::lockers::Offer::sells(1, 1, 1, 12, 1, 0.05)]);
        s.draft_training(smithy, capital, "weaponsmith", 5, 24.0, 1.0, 10 * DAY, false, true);
        roads(&mut s);
        s.restock(smithy);
        s
    };
    // A road to a hub that doesn't run to the capital: nobody comes for it.
    let s = make(&|s| {
        s.add_route(smithy, hub, 0.1, 0.0, 2.0);
        s.make_hub(hub);
    });
    assert!(!s.collectable(smithy, capital));
    assert_eq!(s.town(smithy).stock[1], 12.0, "no collector: the work is the town's");
    assert!(s.bills().is_empty() && s.shipments(smithy).held.is_empty());
    // The same road, and the hub runs on to the capital: it will buy the bill and forward the goods.
    let s = make(&|s| {
        s.add_route(smithy, hub, 0.1, 0.0, 2.0);
        s.add_route(hub, capital, 0.1, 0.0, 2.0);
        s.make_hub(hub);
    });
    assert!(s.collectable(smithy, capital));
    assert_eq!(s.town(smithy).stock[1], 0.0, "crated for the buyer");
    assert_eq!(s.shipments(smithy).held.len(), 1);
    assert_eq!(s.bills().len(), 1, "with a bill on the capital");
    // A buyer's own wagon calling counts as before; a wound-down one no longer does.
    let mut s = make(&|s| {
        s.add_route(capital, smithy, 0.1, 0.0, 2.0);
    });
    assert!(s.collectable(smithy, capital));
    s.close_route(capital, smithy);
    assert!(!s.collectable(smithy, capital), "its wagons don't call any more");
}

/// A hub (0) that calls at the town (1) every three days, which owes the buyer (2) beyond the hub under a contract (three
/// stacks of paper an installment) and has `town_money`: its reserve and a little over. The post's postage comes out of that
/// little; 1.26 leaves the town 0.16 to spare at the hub's first call, the fare of one stack (0.0993) and two crates at 0.02
/// (the dial is set low so the crates are no obstacle: the credit is for the fare), so one stack goes for cash and two on
/// credit.
fn credit_world(town_money: f64) -> (Society, usize) {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    s.content_mut().dials.crate_emeralds = 0.02; // a crate the town can afford where it can't afford the fare
    let hub = s.add_town("Hub", 100.0);
    let town = s.add_town("Town", town_money);
    let buyer = s.add_town("Buyer", 50.0);
    s.add_route(hub, town, 0.01, 0.0, 3.0);
    s.add_route(hub, buyer, 200.0 / 1200.0, 0.0, 4.0);
    s.make_hub(hub);
    s.set_trade(town, CLOTH, 0.0, 1.0);
    s.set_trade(buyer, PAPER, 0.0, 64.0);
    s.deliver(hub, CLOTH, 128.0);
    s.deliver(hub, BREAD, 128.0);
    s.deliver(town, PAPER, 256.0);
    s.deliver(town, BREAD, 128.0);
    s.deliver(buyer, BREAD, 128.0);
    // The town owes the buyer 64 paper a week, delivered; both hold copies; it has no road there, the hub has.
    let id = s.draft(town, buyer, PAPER, 192.0, 1.0 / 24.0, 0, 7 * DAY, 4, Incoterm::Dap, true);
    (s, id)
}

#[test]
fn a_hubs_wagon_carries_a_towns_goods_on_credit_when_it_cannot_pay_the_fare() {
    let (mut s, _id) = credit_world(1.26);
    let (hub, town) = (0, 1);
    let start = s.totals();
    let bills_before = s.bills().len();
    let mut credited = false;
    for _ in 0..(6 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("{e}"));
        if s.stranded_book(hub).credit > 0.0 {
            credited = true;
            break;
        }
    }
    assert!(credited, "the hub's wagon carried them on credit: {:?} {:?}", s.stranded_book(hub), s.hub(hub).book);
    let book = s.stranded_book(hub);
    // The bill on the town, payable to the hub, in thirty days, for the fare of the stacks.
    let owed: Vec<_> = s.bills().into_iter().skip(bills_before).filter(|b| b.drawee == town && b.payee == hub).collect();
    assert_eq!(owed.len(), 1, "{:?}", s.bills());
    // Two per cent for every bill term it runs: 30 days are seven and a half of them.
    assert!(owed[0].amount == book.credit.mul_ceil(1.15), "the fare, and its fifteen per cent: {} of {}", owed[0].amount, book.credit);
    assert_eq!(owed[0].due - owed[0].drawn, 30 * DAY, "due in thirty days");
    assert!(s.hub(hub).book.forwarded >= 1 || !s.hub(hub).held.is_empty(), "the goods are the forwarder's, or on their way home");
    assert!(s.town(town).money >= 0.0 && s.hub(hub).book.fees > 0.0, "it paid what it could: {:?}", s.hub(hub).book);
}

#[test]
fn a_town_that_can_pay_the_fare_pays_it_and_owes_the_hub_nothing() {
    let (mut s, _) = credit_world(50.0);
    let (hub, town) = (0, 1);
    let start = s.totals();
    for _ in 0..(6 * DAY / LOOK) {
        look(&mut s);
        s.conserved(&start).and_then(|_| s.solvent()).unwrap_or_else(|e| panic!("{e}"));
        if s.hub(hub).book.forwarded > 0 {
            break;
        }
    }
    assert!(s.hub(hub).book.forwarded > 0, "the forwarder took the goods: {:?}", s.hub(hub).book);
    assert!(s.hub(hub).book.fees > 0.0, "for a fee");
    assert!(s.bills().iter().all(|b| !(b.drawee == town && b.payee == hub)), "no credit needed");
}

#[test]
fn a_crate_shipped_under_a_broken_contract_goes_back_on_the_shelf_unless_a_bill_is_still_to_be_collected() {
    let (town, buyer) = (0, 1);
    let make = |paid_by_bill: bool| {
        let mut s = Society::new(content());
        s.content_mut().dials.standing_orders = false;
        s.add_town("Town", 5.0);
        s.add_town("Buyer", 5.0);
        s.add_town("Else", 5.0);
        // A contract the town sold, no longer in force (never held by both: drafted by the seller alone, it lapses).
        let id = s.draft(town, buyer, PAPER, 64.0, 1.0 / 24.0, 10 * DAY, 4 * DAY, 2, Incoterm::Dap, false);
        let c = Shipment { good: PAPER, count: 64.0, shipper: town, consignee: Some(buyer), contract: Some(id), settled: true,
            copy: true };
        s.deliver_shipment(town, c);
        if paid_by_bill {
            s.draw_bill(buyer, town, 5.0);
        }
        s.world.resource_mut::<Book>().contracts[id].state = enterprise_core::State::Broken;
        s.advance(DAY);
        s
    };
    let s = make(false);
    assert!(s.shipments(town).held.is_empty() && s.town(town).stock[PAPER] == 64.0, "the goods are its own again");
    let s = make(true);
    assert_eq!(s.shipments(town).held.len(), 1, "paid for by a bill still to be collected: the buyer's");
    assert_eq!(s.town(town).stock[PAPER], 0.0);
}

#[test]
fn the_books_survive_a_save() {
    let (hub, town, capital) = (0, 1, 2);
    let mut s = arena(100.0);
    let id = s.draw_bill(capital, town, 100.0);
    s.advance(10 * DAY);
    let (mut crates, mut bills) = (vec![leggings(town, capital)], vec![id]);
    s.buy_stranded(hub, town, &mut crates, &mut bills, 1000.0).unwrap();
    let book = s.stranded_book(hub);
    let mut loaded = Society::load(&s.save()).unwrap();
    assert_eq!(loaded.stranded_book(hub), book);
    assert!(s.unsaved().is_empty());
    assert!(loaded.unsaved().is_empty());
    let _ = &mut loaded;
}

mod common;

/// The random worlds exercise it: hubs that run to markets, stale bills, closed roads, wagons that call; across the seeds the
/// hubs buy paper from towns and from wagons, carry goods on credit, and the books balance every day.
#[test]
fn the_random_worlds_strand_paper_and_the_hubs_buy_it() {
    let (mut deals, mut crates, mut credit, mut bills, mut borrowed, mut coined) = (0, 0, 0.0, 0, 0.0, 0.0);
    for (d, c, cr, b, bo, co) in common::each_seed(0..60, |seed| {
        let (mut deals, mut crates, mut credit, mut bills, mut borrowed, mut coined) = (0, 0, 0.0, 0, 0.0, 0.0);
        let mut s = common::world(seed);
        let start = s.totals();
        for day in 1..=60 {
            s.advance(day * DAY);
            common::slices::daily(&mut s, seed, day);
            s.conserved(&start).and_then(|_| s.solvent()).and_then(|_| common::slices::check(&s, day))
                .unwrap_or_else(|e| panic!("seed {seed}, day {day}: {e}"));
        }
        for t in 0..s.town_count() {
            let b = s.stranded_book(t);
            deals += b.deals;
            bills += b.bills;
            crates += b.crates;
            credit += b.credit.to_f64();
            coined += b.coin.to_f64();
            borrowed += s.deposits(t);
        }
        (deals, crates, credit, bills, borrowed, coined)
    }) {
        deals += d;
        crates += c;
        credit += cr;
        bills += b;
        borrowed += bo;
        coined += co;
    }
    assert!(deals > 10 && bills > 10 && crates > 0 && credit > 0.0 && coined > 0.0,
        "{deals} deals, {bills} bills, {crates} crates, {credit} credit, {coined} coin ({borrowed} borrowed)");
}
