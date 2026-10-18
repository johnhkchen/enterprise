//! Player orders (`orders.rs`, `rules/orders.rs`), pinned as the mod's market tests pin them (`MarketTests`): a master
//! Merchant at a desk fills a player's buying shop under an Ex Works contract, asks the region for what it can't fill
//! and posts what no market holds, buys from a player's selling shop for its orders, and stocks the town's own shop.
//! Paper at 24 an emerald, iron at 4, sticks at 32, nautilus shells at 8, apples at 4; 16 spurs an emerald.

use enterprise_core::cash_book::Account;
use enterprise_core::orders::Orders;
use enterprise_core::{Content, Good, Incoterm, Input, Output, Society, State};

const DAY: u64 = 24_000;
/// A look: the core looks every 1,200 ticks.
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const IRON: usize = 1;
const STICK: usize = 2;
const SHELL: usize = 3;
const APPLE: usize = 4;
const BREAD: usize = 5;
const ARROW: usize = 6;
const DESK: usize = 0;
// The same, as the wire numbers them (in a pattern).
const DESK_NO: u32 = DESK as u32;
const SHELL_NO: u32 = SHELL as u32;
const PAPER_NO: u32 = PAPER as u32;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("iron_ingot", 1.0 / 4.0, 64),
        Good::new("stick", 1.0 / 32.0, 64), Good::new("nautilus_shell", 1.0 / 8.0, 64), Good::new("apple", 1.0 / 4.0, 64),
        Good::new("bread", 1.0 / 6.0, 64).food(5.0), Good::new("arrow", 1.0 / 16.0, 64)], DAY)
}

/// A market whose Merchant is a master (level 5) and whose treasury holds `emeralds`.
fn desk(level: u8, emeralds: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    s.add_town("Desk", emeralds);
    s.set_roster(DESK, &[("merchant", level)]);
    s
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// parity: MarketTests.aMasterMerchantFillsAPlayersOrder
#[test]
fn a_master_merchant_fills_a_players_order() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, PAPER, 64.0);
    let before = s.totals();
    let order = s.order(DESK, PAPER, 16.0, 16.0, 40.0);
    s.advance(LOOK);
    let contract = s.contracts().into_iter().find(|c| c.good == PAPER).expect("a contract with the shop");
    assert!(contract.state == State::Active && contract.installments == 2 && contract.incoterm == Incoterm::Exw,
        "a contract with the shop: two installments, all its purse pays for, Ex Works, in force at once: {contract:?}");
    assert_eq!(s.orders(DESK)[0].delivered, 16.0, "the first lot today");
    s.advance(LOOK + DAY);
    let shop = &s.orders(DESK)[0];
    assert_eq!(shop.id, order);
    assert_eq!(shop.delivered, 32.0, "the second tomorrow");
    assert_eq!(shop.purse, 8.0, "paid 32 spurs out of 40");
    assert_eq!(s.town(DESK).stock[PAPER], 32.0, "from the warehouse");
    assert_eq!(s.contract(contract.id).state, State::Completed, "done");
    assert!(s.town(DESK).money == 2.0, "and the desk banked 32 spurs: {}", s.town(DESK).money);
    s.conserved(&before).expect("goods and money are all accounted for");
    let banked: enterprise_core::Fixed = s.cash_book(DESK, 3).iter().filter(|(_, what, _)| what == "order").map(|(_, _, v)| v).sum();
    assert!(banked == 2.0, "the cash book says so, a lot a day: {:?}", s.cash_book(DESK, 3));
}

// parity: MarketTests.whatItCantFillIsAskedOfTheRegion
#[test]
fn what_it_cant_fill_is_asked_of_the_region() {
    let mut s = desk(5, 0.0);
    s.order(DESK, SHELL, 2.0, 64.0, 640.0);
    let outs = s.advance(LOOK);
    assert_eq!(s.order_wants(DESK).get(&SHELL), Some(&20.0), "ten lots open, for caravans to bring: {:?}", s.order_wants(DESK));
    let signals = s.posted_signals(DESK);
    assert!(signals.iter().any(|g| g.good == SHELL), "and no market has them: posted as wanted {signals:?}");
    // The premium: half again the worth, rounded up to a spur (an eighth of 16 spurs, 1.5 times).
    assert_eq!(signals[0].spurs_each, 3.0);
    assert_eq!(signals[0].count, 20.0);
    assert!(outs.iter().any(|o| matches!(o, Output::Signal { town: DESK_NO, good: SHELL_NO, .. })), "told to the host: {outs:?}");
    // (Posted once: the next look doesn't tell the host again.)
    assert!(!s.advance(2 * LOOK).iter().any(|o| matches!(o, Output::Signal { .. })));
}

// parity: MarketTests.itBuysFromAPlayersStallForItsOrders
#[test]
fn it_buys_from_a_players_stall_for_its_orders() {
    let mut s = desk(5, 10.0);
    let before = s.totals();
    s.order(DESK, IRON, 4.0, 24.0, 48.0);
    let stall = s.stall(DESK, IRON, 4.0, 12.0, 8.0);
    s.advance(LOOK);
    assert_eq!(s.stalls(DESK)[0].lots(), 0.0, "it buys the stall's two lots for the order");
    assert_eq!(s.stalls(DESK)[0].id, stall);
    s.advance(LOOK + DAY);
    assert_eq!(s.orders(DESK)[0].delivered, 8.0, "and fills the order, a lot a day");
    let tally = s.tally(DESK);
    assert!(tally.spurs_from_sales - tally.spurs_on_purchases >= 24.0, "at a profit: {tally:?}");
    assert_eq!((tally.spurs_from_sales, tally.spurs_on_purchases, tally.lots_bought), (48.0, 24.0, 2.0));
    s.conserved(&before).expect("goods and money are all accounted for");
}

// parity: MarketTests.itsOwnShopSellsTheSurplus
#[test]
fn its_own_shop_sells_the_surplus() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, STICK, 64.0);
    let before = s.totals();
    let shop = s.own_shop(DESK);
    s.advance(LOOK);
    let own = &s.own_shops(DESK)[0];
    assert_eq!((own.id, own.good, own.lot, own.price, own.shelf), (shop, Some(STICK), 16.0, 9.0, 64.0),
        "the town's shop sells its sticks, in lots of 16 at a tenth over their worth: {own:?}");
    assert_eq!(s.town(DESK).stock[STICK], 0.0, "all on its shelf");
    s.conserved(&before).expect("goods on a shelf are still the world's");
    assert_eq!(s.buy(DESK, STICK, 16.0), Ok(9.0), "a player bought a lot");
    s.advance(2 * LOOK);
    assert_eq!(s.tally(DESK).spurs_from_own_shops, 9.0, "and the town banks the takings");
    // (9 spurs at 16, to the nearest thousandth: coin paid in)
    assert!(s.town(DESK).money == enterprise_core::Fixed::round(9.0 / 16.0));
    assert_eq!(s.own_shops(DESK)[0].shelf, 48.0);
    s.conserved(&before).expect("goods and money are all accounted for");
}

// parity: CaravanTests.aShopsOrderAtTheFarEndRidesOut
#[test]
fn a_shops_order_at_the_far_end_rides_out() {
    let mut s = hub_and_far_end();
    s.deliver(HUB, APPLE, 64.0);
    // A player's shop at the far end wants apples, and the Merchant there has none.
    s.order(FAR, APPLE, 16.0, 64.0, 640.0);
    s.advance(LOOK);
    assert!(aboard(&s, 0, APPLE) > 0.0, "a player's shop there ordered apples: the caravan brings some");
    // (With no order there, nobody wants them.)
    let mut s = hub_and_far_end();
    s.deliver(HUB, APPLE, 64.0);
    s.advance(LOOK);
    assert_eq!(aboard(&s, 0, APPLE), 0.0);
}

const HUB: usize = 0;
const FAR: usize = 1;

/// A hub with a Merchant and a squad of one Librarian, a far end four blocks off with a master Merchant.
fn hub_and_far_end() -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    s.add_town("Hub", 20.0);
    s.add_town("Far", 20.0);
    s.set_roster(HUB, &[("merchant", 1), ("librarian", 2)]);
    s.set_offers(HUB, 1, Vec::new()); // trained: it keeps its trade without a workstation
    s.set_roster(FAR, &[("merchant", 5)]);
    s.add_route(HUB, FAR, 4.0 / 1200.0, 0.0, 4.0);
    s
}

fn aboard(s: &Society, route: usize, good: usize) -> f64 {
    let loose = s.wagon_cargo(route).map_or(0.0, |c| c[good]);
    loose + s.wagon_crated(route, good)
}

#[test]
fn a_master_merchants_open_order_brings_the_wagon_and_is_filled_from_what_it_brought() {
    let mut s = hub_and_far_end();
    s.deliver(HUB, APPLE, 64.0);
    let before = s.totals();
    s.order(FAR, APPLE, 16.0, 64.0, 640.0); // 10 daily lots of 16 at their worth
    s.advance(LOOK);
    assert_eq!(s.order_wants(FAR).get(&APPLE), Some(&160.0), "the far end has nothing to fill it with");
    assert!(aboard(&s, 0, APPLE) > 0.0, "and the hub's wagon, bound there, brings apples: {:?}", s.wagon_cargo(0));
    s.advance(DAY / 2);
    assert!(s.town(FAR).stock[APPLE] > 0.0, "the far end bought what came: {:?}", s.town(FAR).stock);
    let at_the_desk = s.town(FAR).stock[APPLE];
    s.advance(2 * DAY);
    let shop = &s.orders(FAR)[0];
    assert!(shop.delivered >= 16.0_f64.min(at_the_desk), "and the Merchant filled the shop from it: {shop:?} of {at_the_desk}");
    s.conserved(&before).expect("goods and money are all accounted for");
}

#[test]
fn the_desks_own_wagons_buy_up_the_open_order_at_the_far_end() {
    // The hub's players ordered apples (open, nothing at the hub); the far end holds a glut beyond its keep.
    let (mut s, mut control) = (hub_and_far_end(), hub_and_far_end());
    for s in [&mut s, &mut control] {
        s.set_roster(HUB, &[("merchant", 5), ("librarian", 2)]);
        s.set_offers(HUB, 1, Vec::new());
        s.record_capacity(FAR, 1, &[(PAPER, 64.0 / 5.0)]);
        s.set_trade(FAR, PAPER, 0.0, 1e-9);
        s.deliver(HUB, PAPER, 128.0);
        s.deliver(FAR, APPLE, 200.0);
    }
    control.advance(3 * LOOK);
    assert_eq!(control.bought(0).0, 0.0, "(with no order at home, nobody wants the apples)");
    // (The far end's lockers take paper, which the wagon sells for its purse.)
    s.order(HUB, APPLE, 16.0, 64.0, 640.0);
    s.advance(LOOK);
    assert_eq!(s.order_wants(HUB).get(&APPLE), Some(&160.0));
    s.advance(3 * LOOK);
    let (bought, ..) = s.bought(0);
    assert!(bought > 0.0, "its wagon bought apples at the far end for the shop at home: {bought}");
}

#[test]
fn a_merchant_below_level_five_does_none_of_it() {
    let mut s = desk(4, 10.0);
    s.deliver(DESK, PAPER, 64.0);
    s.deliver(DESK, STICK, 64.0);
    s.order(DESK, PAPER, 16.0, 16.0, 40.0);
    s.order(DESK, SHELL, 2.0, 64.0, 640.0);
    s.stall(DESK, IRON, 4.0, 12.0, 8.0);
    s.own_shop(DESK);
    s.advance(3 * DAY);
    assert!(s.contracts().is_empty(), "no contract with any shop");
    assert!(s.orders(DESK).iter().all(|o| o.delivered == 0.0 && o.purse == o.purse.floor()));
    assert_eq!(s.orders(DESK)[0].purse, 40.0);
    assert!(s.order_wants(DESK).is_empty() && s.posted_signals(DESK).is_empty(), "nothing asked of the region");
    assert_eq!(s.stalls(DESK)[0].stock, 8.0, "nothing bought");
    assert_eq!(s.own_shops(DESK)[0].good, None, "and its shop stocked with nothing");
    assert_eq!(s.town(DESK).stock[STICK], 64.0);
}

#[test]
fn an_order_priced_under_nine_tenths_of_its_worth_isnt_taken_on() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, PAPER, 64.0);
    // 16 paper are worth 10.67 spurs; 90% of that is 9.6: 9 spurs is too little, 10 is just enough.
    s.order(DESK, PAPER, 16.0, 9.0, 90.0);
    s.advance(LOOK);
    assert!(s.contracts().is_empty(), "the Merchant won't fill it");
    assert!(s.order_wants(DESK).is_empty(), "nor ask the region for it");
    s.order(DESK, PAPER, 16.0, 10.0, 90.0);
    s.advance(2 * LOOK);
    assert_eq!(s.contracts().len(), 1, "but the one just under is taken on");
    assert_eq!(s.contracts()[0].installments, 9, "as many lots as the purse pays for");
}

#[test]
fn a_purse_that_pays_for_no_lot_gets_no_contract_and_a_rich_one_runs_ten_lots_at_most() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, PAPER, 640.0);
    s.order(DESK, PAPER, 16.0, 16.0, 15.0);
    s.order(DESK, PAPER, 16.0, 16.0, 1600.0);
    s.advance(LOOK);
    let contracts = s.contracts();
    assert_eq!(contracts.len(), 1, "the poor shop gets none");
    assert_eq!(contracts[0].installments, 10, "the rich one up to ten");
}

#[test]
fn it_fills_only_from_what_it_can_spare_beyond_its_par() {
    let mut s = desk(5, 0.0);
    // Its lockers take paper: a stack's par (5 days of 12.8 a day), so the first 64 aren't for sale.
    s.record_capacity(DESK, 1, &[(PAPER, 64.0 / 5.0)]);
    s.deliver(DESK, PAPER, 70.0);
    s.order(DESK, PAPER, 16.0, 16.0, 160.0);
    s.advance(LOOK);
    assert_eq!(s.orders(DESK)[0].delivered, 0.0, "6 spare of 16 wanted: not a lot");
    s.deliver(DESK, PAPER, 10.0);
    s.advance(2 * LOOK);
    assert_eq!(s.orders(DESK)[0].delivered, 16.0, "16 spare: one lot");
    assert_eq!(s.town(DESK).stock[PAPER], 64.0, "and its par is kept");
}

#[test]
fn it_wont_pay_a_stall_more_than_the_goods_are_worth() {
    let mut s = desk(5, 10.0);
    s.order(DESK, IRON, 4.0, 24.0, 48.0);
    s.stall(DESK, IRON, 4.0, 17.0, 8.0); // 4 iron are worth 16 spurs
    s.advance(LOOK);
    assert_eq!(s.stalls(DESK)[0].stock, 8.0, "17 is over their worth: not bought");
    assert_eq!(s.order_wants(DESK).get(&IRON), Some(&8.0), "the order stays open");
    assert!(s.posted_signals(DESK).is_empty(), "(a stall sells it, so it isn't posted as a want)");
}

#[test]
fn it_buys_from_a_stall_only_beyond_the_towns_reserve_and_what_its_orders_want() {
    // A treasury of a third of an emerald can't pay a lot of 12 spurs (3/4 of an emerald).
    let mut s = desk(5, 0.3);
    s.order(DESK, IRON, 4.0, 24.0, 48.0);
    s.stall(DESK, IRON, 4.0, 12.0, 8.0);
    s.advance(LOOK);
    assert_eq!(s.stalls(DESK)[0].stock, 8.0, "it can't pay");
    // With no order and no locker wanting the iron it buys nothing either.
    let mut s = desk(5, 10.0);
    s.stall(DESK, IRON, 4.0, 12.0, 8.0);
    s.advance(LOOK);
    assert_eq!(s.stalls(DESK)[0].stock, 8.0, "nothing wants it");
    // Its lockers' trade wants a stack of it (par): it buys up to that.
    s.pay(DESK, 100.0);
    s.record_capacity(DESK, 1, &[(IRON, 64.0 / 5.0)]);
    s.stall(DESK, IRON, 4.0, 12.0, 100.0);
    s.advance(2 * LOOK);
    assert_eq!(s.town(DESK).stock[IRON], 64.0, "a stack, the par its lockers want");
}

#[test]
fn the_own_shop_sells_the_biggest_surplus_but_no_food_and_nothing_ordered() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, BREAD, 640.0); // food: never
    s.deliver(DESK, PAPER, 64.0); // ordered below, and more than it can fill: never
    s.deliver(DESK, IRON, 64.0); // 16 emeralds
    s.deliver(DESK, STICK, 64.0); // 2 emeralds
    s.deliver(DESK, APPLE, 8.0); // under a lot of 16
    s.order(DESK, PAPER, 16.0, 16.0, 320.0);
    s.own_shop(DESK);
    s.own_shop(DESK);
    s.advance(LOOK);
    // (The paper is still ordered, ten lots and 144 paper beyond what it spared, so it's not sold: iron's worth most of
    // what's left, then sticks.)
    let shops = s.own_shops(DESK);
    assert_eq!(s.order_wants(DESK).get(&PAPER), Some(&96.0));
    assert_eq!(shops[0].good, Some(IRON), "{shops:?}");
    assert_eq!(shops[1].good, Some(STICK), "{shops:?}");
    assert_eq!(shops[0].price, (16.0f64 * 0.25 * 16.0 * 1.1).ceil());
    assert_eq!(s.town(DESK).stock[BREAD], 640.0);
    assert_eq!(s.town(DESK).stock[APPLE], 8.0);
}

#[test]
fn a_shop_relisted_for_a_bigger_surplus_gives_its_shelf_back() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, STICK, 64.0);
    s.own_shop(DESK);
    s.advance(LOOK);
    assert_eq!(s.own_shops(DESK)[0].shelf, 64.0);
    s.deliver(DESK, IRON, 64.0); // worth far more
    let before = s.totals();
    s.advance(2 * LOOK);
    let shop = &s.own_shops(DESK)[0];
    assert_eq!((shop.good, shop.shelf), (Some(IRON), 64.0), "{shop:?}");
    assert_eq!(s.town(DESK).stock[STICK], 64.0, "the sticks went back to the warehouse");
    s.conserved(&before).expect("goods only moved");
}

#[test]
fn a_player_cant_buy_what_isnt_on_the_shelf() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, STICK, 64.0);
    s.own_shop(DESK);
    assert!(s.buy(DESK, STICK, 16.0).is_err(), "nothing listed yet");
    s.advance(LOOK);
    assert!(s.buy(DESK, STICK, 80.0).is_err(), "more than the shelf holds");
    assert!(s.buy(DESK, PAPER, 1.0).is_err(), "not what it lists");
    assert!(s.buy(DESK, STICK, 1.0).is_err(), "less than a lot");
    assert_eq!(s.buy(DESK, STICK, 32.0), Ok(18.0));
    assert_eq!(s.buy(DESK, STICK, 20.0), Ok(9.0), "whole lots only");
    assert_eq!(s.own_shops(DESK)[0].shelf, 16.0);
}

#[test]
fn the_inputs_make_the_same_shops_as_the_calls() {
    let mut s = desk(5, 10.0);
    s.deliver(DESK, PAPER, 64.0);
    s.apply(&Input::Order { town: DESK as u32, good: PAPER as u32, lot: 16.0, price: 16.0, purse: 40.0 });
    s.apply(&Input::Stall { town: DESK as u32, good: IRON as u32, lot: 4.0, price: 12.0, stock: 8.0 });
    s.apply(&Input::OwnShop { town: DESK as u32 });
    assert_eq!((s.orders(DESK).len(), s.stalls(DESK).len(), s.own_shops(DESK).len()), (1, 1, 1));
    let outs = s.apply(&Input::Advance { to: LOOK });
    assert!(outs.iter().any(|o| matches!(o, Output::OrderFilled { town: DESK_NO, good: PAPER_NO, lots: 1, count, spurs, .. }
        if *count == 16.0 && *spurs == 16.0)), "{outs:?}");
    s.apply(&Input::Buy { town: DESK as u32, good: PAPER as u32, count: 1.0 });
}

#[test]
fn a_cancelled_order_closes_its_contract_and_goes_from_the_wants() {
    let mut s = desk(5, 0.0);
    let order = s.order(DESK, SHELL, 2.0, 64.0, 640.0);
    s.advance(LOOK);
    assert_eq!(s.contracts()[0].state, State::Active);
    s.cancel_order(order);
    assert_eq!(s.contracts()[0].state, State::Completed, "closed");
    assert!(s.order_wants(DESK).is_empty());
    s.advance(2 * LOOK);
    assert!(s.orders(DESK).is_empty() && s.order_wants(DESK).is_empty());
}

#[test]
fn the_shops_survive_a_save() {
    let mut s = desk(5, 10.0);
    s.deliver(DESK, PAPER, 64.0);
    s.deliver(DESK, STICK, 64.0);
    s.order(DESK, PAPER, 16.0, 16.0, 40.0);
    s.stall(DESK, IRON, 4.0, 12.0, 8.0);
    s.own_shop(DESK);
    s.advance(LOOK);
    assert_eq!(s.unsaved(), Vec::<String>::new());
    let mut loaded = Society::load(&s.save()).unwrap();
    s.advance(3 * DAY);
    loaded.advance(3 * DAY);
    assert_eq!(s.orders(DESK), loaded.orders(DESK));
    assert_eq!(s.own_shops(DESK), loaded.own_shops(DESK));
    assert_eq!(s.tally(DESK), loaded.tally(DESK));
    assert_eq!(s.balance(Account::Treasury(DESK)), loaded.balance(Account::Treasury(DESK)));
}

#[test]
fn what_a_known_market_posts_as_wanted_is_work_for_a_towns_idle_hands() {
    let mut s = desk(5, 0.0);
    let idle = s.add_town("Idle", 5.0);
    // 160 arrows ordered at the desk, and nobody anywhere has any: a signal at 2 spurs each (1.5 times their worth).
    s.order(DESK, ARROW, 16.0, 16.0, 160.0);
    s.advance(LOOK);
    assert_eq!((s.posted_signals(DESK)[0].count, s.posted_signals(DESK)[0].spurs_each), (160.0, 2.0));
    assert!(s.job_bag(idle).is_empty(), "a town that hasn't heard of the market sees nothing");
    s.learn(idle, DESK);
    s.advance(DAY + LOOK); // (who hears of a desk is looked at again with the day's sourcing)
    // A Fletcher's villagers sell arrows: 160 arrows at the signal's 2 spurs each, 16 spurs an emerald, is worth 20 to its
    // idle hands (`count * spursEach / rate`; once, not by two streams).
    let bag = s.job_bag(idle);
    let fletcher = bag.iter().find(|(t, _)| t == "fletcher").expect("a Fletcher would supply it");
    assert!(close(fletcher.1, 20.0), "{bag:?}");
    // Once the desk has them, the want is met and the marbles go.
    s.deliver(DESK, ARROW, 200.0);
    s.advance(3 * DAY); // (sourcing's unmet wants are judged once a day)
    assert!(s.posted_signals(DESK).is_empty());
    assert!(s.job_bag(idle).iter().all(|(t, _)| t != "fletcher"), "{:?}", s.job_bag(idle));
}

// Java's pickCargo/saleLoad room is `Warehouse.par(to, item, true) - onHand`: an order only adds the good to the wanted set.
#[test]
fn a_far_end_order_for_an_unused_good_sends_up_to_a_stacks_room() {
    let mut s = hub_and_far_end();
    s.deliver(HUB, APPLE, 64.0);
    s.order(FAR, APPLE, 20.0, 80.0, 80.0); // one lot of 20, at their worth
    s.advance(LOOK);
    assert_eq!(s.order_wants(FAR).get(&APPLE), Some(&20.0));
    let aboard = aboard(&s, 0, APPLE);
    assert!(aboard > 20.0 && aboard <= 64.0, "a stack's room, not just the order's 20: {aboard}");
}

#[test]
fn a_good_the_far_end_already_has_on_its_shelf_sends_none_for_an_order() {
    let mut s = hub_and_far_end();
    s.deliver(HUB, APPLE, 64.0);
    // Its people use apples, and it holds its shelf of them (no more than its par, so none is spare for the order).
    s.set_trade(FAR, APPLE, 0.0, 20.0);
    let shelf = (s.content().dials.shelf_days * 20.0).ceil();
    s.deliver(FAR, APPLE, shelf);
    s.order(FAR, APPLE, 20.0, 80.0, 80.0);
    s.advance(LOOK);
    // (Its people ate one this look, before the plan: a wagon goes for that one apple, worth its road food at an
    // apple's worth, as the mod's `saleLoad` weighs a load.)
    assert!(aboard(&s, 0, APPLE) <= 1.0, "its shelf is full: the order adds nothing to its room");
}

// An order the desk can't fill is sourced from a known market with a road, as the mod's `fill` does.
#[test]
fn an_order_the_desk_cannot_fill_gets_a_supplier_contract() {
    let mut s = desk(5, 0.0);
    let market = s.add_town("Market", 50.0);
    s.record_stocktake(market, &[(SHELL, 64.0)], Some(0));
    s.learn(DESK, market);
    s.add_route(DESK, market, 0.25, 0.5, 1.0);
    s.order(DESK, SHELL, 2.0, 64.0, 640.0);
    s.advance(2 * DAY);
    let c = s.contracts().into_iter().find(|c| c.seller == market && c.buyer == DESK && c.good == SHELL)
        .expect("a supplier's contract drafted for the shop's order");
    assert!(c.quantity > 0.0, "{c:?}");
    assert!(s.posted_signals(DESK).is_empty(), "and a market holds them: no signal");
}

// One shop's misses don't cost another its contract (`contractFor` keys trust by the shop).
#[test]
fn one_bad_shops_misses_do_not_block_another_shops_contract() {
    let mut s = desk(5, 10.0);
    s.deliver(DESK, PAPER, 128.0);
    let (bad, good) = (s.order(DESK, PAPER, 16.0, 16.0, 40.0), s.order(DESK, PAPER, 16.0, 16.0, 40.0));
    s.world.resource_mut::<Orders>().orders.iter_mut().find(|o| o.id == bad).unwrap().record =
        enterprise_core::contracts::Record { kept: 0.0, missed: 50.0, touched: 0 };
    s.advance(LOOK);
    let orders = s.orders(DESK);
    let (b, g) = (orders.iter().find(|o| o.id == bad).unwrap(), orders.iter().find(|o| o.id == good).unwrap());
    assert!(b.contract.is_none() && b.delivered == 0.0, "the bad shop gets no contract: {b:?}");
    assert!(g.contract.is_some() && g.delivered == 16.0, "the other is filled: {g:?}");
}

// A repriced shop keeps its contract (`book.open(desk, buyer, delivery)` matches the good alone): each lot at the lower
// of the locked price and what the shop now asks (`Broker.deliver`).
#[test]
fn a_repriced_shop_keeps_its_contract_at_the_lower_price() {
    let mut s = desk(5, 10.0);
    s.deliver(DESK, PAPER, 128.0);
    let id = s.order(DESK, PAPER, 16.0, 16.0, 80.0);
    s.advance(LOOK);
    let first = s.orders(DESK)[0].contract.unwrap();
    assert_eq!(s.orders(DESK)[0].paid, 16.0);
    s.world.resource_mut::<Orders>().orders.iter_mut().find(|o| o.id == id).unwrap().price = 12.0;
    s.advance(LOOK + DAY);
    assert_eq!(s.orders(DESK)[0].contract, Some(first), "the same contract");
    assert_eq!((s.orders(DESK)[0].delivered, s.orders(DESK)[0].paid), (32.0, 28.0), "a lot at the shop's 12");
    s.world.resource_mut::<Orders>().orders.iter_mut().find(|o| o.id == id).unwrap().price = 20.0;
    s.advance(LOOK + 2 * DAY);
    assert_eq!(s.orders(DESK)[0].contract, Some(first));
    assert_eq!((s.orders(DESK)[0].delivered, s.orders(DESK)[0].paid), (48.0, 44.0), "and at the locked 16, not 20");
}

// Without any shop a master desk still posts its lockers' wants.
#[test]
fn a_desk_with_no_shops_still_posts_its_lockers_shortfalls() {
    let mut s = desk(5, 0.0);
    s.record_capacity(DESK, 1, &[(PAPER, 64.0)]);
    s.advance(LOOK);
    let signals = s.posted_signals(DESK);
    assert!(signals.iter().any(|g| g.good == PAPER && g.why == "its lockers' trade"), "{signals:?}");
}

#[test]
fn cancelling_an_order_takes_its_signal_down() {
    let mut s = desk(5, 0.0);
    let id = s.order(DESK, SHELL, 2.0, 64.0, 640.0);
    s.advance(LOOK);
    assert!(!s.posted_signals(DESK).is_empty());
    s.cancel_order(id);
    assert!(s.posted_signals(DESK).is_empty(), "no stale signal waits for the next look");
}

#[test]
fn the_same_good_relisted_at_another_lot_keeps_its_shelf() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, STICK, 64.0);
    s.own_shop(DESK);
    s.advance(LOOK);
    assert_eq!((s.own_shops(DESK)[0].lot, s.own_shops(DESK)[0].shelf), (16.0, 64.0));
    s.content_mut().dials.own_lot = 8;
    s.deliver(DESK, STICK, 8.0); // (more to list, so the shop is looked at again)
    s.advance(2 * LOOK);
    let own = &s.own_shops(DESK)[0];
    assert_eq!((own.good, own.lot), (Some(STICK), 8.0), "{own:?}");
    assert_eq!(own.shelf, 64.0, "the whole shelf stays: {own:?}");
    assert_eq!(s.town(DESK).stock[STICK], 8.0, "nothing went home but what was delivered");
}

// An input to what the desk has contracted to sell is a good it wants (`Caravans.wantedAt`), so its par is a stack at
// least, and never under the inputs (`Warehouse.par`): it spares none of it for a player's order.
#[test]
fn a_contracts_inputs_are_not_spared_for_an_order() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64), Good::new("iron_sword", 1.5, 1)], DAY));
    s.content_mut().dials.standing_orders = false;
    let (desk, buyer) = (s.add_town("Desk", 0.0), s.add_town("Buyer", 50.0));
    s.set_roster(desk, &[("merchant", 5)]);
    s.deliver(desk, 0, 24.0);
    // Eight swords owed, from the third day: sixteen iron for them.
    s.draft(desk, buyer, 1, 2.0, 1.5, 3 * DAY, DAY, 4, Incoterm::Exw, true);
    s.advance(LOOK);
    assert_eq!(s.inputs(desk)[0], 16.0);
    s.order(desk, 0, 16.0, 64.0, 128.0);
    s.advance(2 * LOOK);
    assert_eq!(s.orders(desk)[0].delivered, 0.0, "24 held, all within its par of a stack");
    assert_eq!(s.town(desk).stock[0], 24.0);
    assert_eq!(s.order_wants(desk).get(&0), Some(&32.0), "both lots open: {:?}", s.order_wants(desk));
}

// `Shops.offer` sets a listing's price only when its good or lot changes: the mint's rate moving leaves it.
#[test]
fn the_towns_shop_keeps_its_price_as_the_rate_moves() {
    let mut s = desk(5, 0.0);
    s.deliver(DESK, STICK, 64.0);
    s.own_shop(DESK);
    s.advance(LOOK);
    assert_eq!(s.own_shops(DESK)[0].price, 9.0);
    s.sell_emeralds(2_000);
    let rate = s.mint_rate();
    assert!((1.0 / 32.0 * 16.0 * rate * 1.1).ceil() < 9.0, "the rate has fallen far enough to price it lower: {rate}");
    s.deliver(DESK, STICK, 32.0);
    s.advance(2 * LOOK);
    assert_eq!(s.own_shops(DESK)[0].price, 9.0, "listed at 9, still 9");
}
