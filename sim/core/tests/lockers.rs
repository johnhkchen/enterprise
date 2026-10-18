//! A town's lockers, pinned as the mod's locker tests pin them: the Java fixture is a Librarian with one trade, 24
//! paper for an emerald, 16 uses, 2 XP, multiplier 0.05, and a Merchant at its desk; restocks are called directly.
//! Here the locker and the Merchant's stock are both the town's warehouse.

use enterprise_core::lockers::{worth, Offer, MAX_DAYS};
use enterprise_core::state::{Towns, Trades};
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;
const PAPER: usize = 0;
const WHEAT: usize = 1;
const STICK: usize = 2;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("wheat", 0.05, 64), Good::new("stick", 1.0 / 32.0, 64)],
        DAY)
}

/// The Java fixture's trade: 24 paper for an emerald, 16 uses, 2 XP, multiplier 0.05, at `demand`.
fn paper_trade(demand: i32) -> Offer {
    Offer::buys(PAPER, 24, 1, 16, 2, 0.05).with_demand(demand)
}

/// A town with a Librarian (hand 0) and, if `merchant`, a Merchant of `level` (hand 1); the Librarian has only the
/// paper trade, switched on if `on`, and the warehouse holds `paper`.
fn market(merchant: Option<u8>, demand: i32, on: bool, paper: f64) -> Society {
    let mut s = Society::new(content());
    let t = s.add_town("Inkwell", 0.0);
    let mut hands = vec![("librarian", 1)];
    if let Some(level) = merchant {
        hands.push(("merchant", level));
    }
    s.set_roster(t, &hands);
    s.set_offers(t, 0, vec![paper_trade(demand)]);
    if on {
        s.switch(t, 0, 0, true);
    }
    if paper > 0.0 {
        s.deliver(t, PAPER, paper);
    }
    s
}

fn paper(s: &Society) -> f64 {
    s.town(0).stock[PAPER]
}

// parity: LockerTradingTests.enabledTradeRunsAtRestock
#[test]
fn a_switched_on_trade_runs_at_restock() {
    let mut s = market(Some(1), 0, true, 64.0);
    assert_eq!(s.restock(0), 2, "two trades from 64 paper");
    assert_eq!((paper(&s), s.town(0).money), (16.0, enterprise_core::Fixed::from(2.0)), "16 paper left, 2 emeralds");
    let lockers = s.lockers(0).unwrap();
    assert_eq!(lockers.hands[0].offers[0].used, 2, "two trades used");
    let roster = s.roster(0);
    assert_eq!(roster.hands[0].xp, 4, "the Librarian earns trade XP");
    let merchant_xp = roster.hands[1].xp;
    assert!((6..=12).contains(&merchant_xp), "the Merchant earns 3 to 6 XP a trade, like a player: {merchant_xp}");
    // The income in the cash book.
    assert_eq!(s.cash_book(0, 1), vec![(0, "lockers: sold paper".to_string(), enterprise_core::Fixed::from(2.0))]);
}

// parity: LockerTradingTests.aVillagerLevelsUpAtOnceFromLockerTrades
#[test]
fn a_villager_levels_up_at_once_from_locker_trades() {
    let mut s = market(Some(1), 0, true, 128.0);
    assert_eq!(s.restock(0), 5, "five trades, ten XP");
    assert_eq!(s.roster(0).hands[0].level, 2, "an Apprentice there and then");
}

// parity: LockerTradingTests.switchedOffTradeDoesNotRun
#[test]
fn a_switched_off_trade_does_not_run() {
    let mut s = market(Some(1), 0, false, 64.0);
    assert_eq!(s.restock(0), 0);
    assert_eq!((paper(&s), s.town(0).money), (64.0, enterprise_core::Fixed::from(0.0)));
}

// parity: LockerTradingTests.noMerchantNoTrading
#[test]
fn no_merchant_no_trading() {
    // No Merchant hand: no income from lockers (the mod's: no desk's Merchant loaded in reach).
    let mut s = market(None, 0, true, 64.0);
    assert_eq!(s.restock(0), 0);
    assert_eq!((paper(&s), s.town(0).money), (64.0, enterprise_core::Fixed::from(0.0)));
    assert!(s.lockers(0).unwrap().managed.is_empty(), "nobody manages its locker");
}

// parity: LockerTradingTests.inflatedPriceIsSkipped
#[test]
fn an_inflated_price_is_skipped() {
    // A restock cools demand by 16 when unused; 100 stays well above zero.
    let mut s = market(Some(1), 100, true, 64.0);
    assert_eq!(s.restock(0), 0);
    assert_eq!((paper(&s), s.town(0).money), (64.0, enterprise_core::Fixed::from(0.0)));
    assert_eq!(s.lockers(0).unwrap().hands[0].offers[0].demand, 84, "100 - 16");
    assert!(paper_trade(84).inflated() && !paper_trade(0).inflated() && !paper_trade(-6).inflated());
}

// parity: LockerTradingTests.masterMerchantGetsADiscount
#[test]
fn a_master_merchant_gets_a_discount() {
    // Reputation 110 x 0.05 = 5 off: 19 paper an emerald, so 3 trades from 64.
    assert_eq!(paper_trade(0).price(110, 64), 19);
    assert_eq!([1, 2, 3, 4, 5].map(|l| paper_trade(0).price(enterprise_core::lockers::merchant_reputation(l), 64)),
        [24, 23, 22, 20, 19]);
    let mut s = market(Some(5), 0, true, 64.0);
    assert_eq!(s.restock(0), 3);
    assert_eq!((paper(&s), s.town(0).money), (7.0, enterprise_core::Fixed::from(3.0)));
}

// parity-partial: LockerTradingTests.merchantFillsLockerFromShulkerAndCollects (no shulker box or locker apart from the warehouse: the warehouse stands for the chest)
#[test]
fn the_merchant_supplies_from_the_warehouse_and_collects() {
    // The mod's shulker box of 128 paper in the Merchant's chest: here the warehouse supplies the locker and the
    // emeralds come back to the treasury (no locker apart from the warehouse, no shulker boxes).
    let mut s = market(Some(1), 0, true, 128.0);
    assert_eq!(s.restock(0), 5, "5 trades use 120");
    assert_eq!((paper(&s), s.town(0).money), (8.0, enterprise_core::Fixed::from(5.0)), "8 stay behind, 5 emeralds to the treasury");
}

// parity: LockerTradingTests.lockerRestocksDoNotSinkDemand
#[test]
fn locker_restocks_do_not_sink_demand() {
    let mut s = market(None, 10, false, 0.0);
    s.restock(0); // unused: vanilla would take demand to 10 - 16 = -6
    assert_eq!(s.lockers(0).unwrap().hands[0].offers[0].demand, 0, "cools to zero");
    s.restock(0);
    assert_eq!(s.lockers(0).unwrap().hands[0].offers[0].demand, 0, "stays at zero");
    // A player's trades (here: uses left from the last restock) still move it as vanilla does.
    let mut o = paper_trade(-5);
    o.used = 16;
    let mut s = Society::new(content());
    let t = s.add_town("Quill", 0.0);
    s.set_roster(t, &[("librarian", 1)]);
    s.set_offers(0, 0, vec![o]);
    s.restock(0);
    assert_eq!(s.lockers(0).unwrap().hands[0].offers[0].demand, 11, "-5 + 16 - 0, no floor after a used restock");
}

// parity: LockerTradingTests.unloadedDaysAreReplayedOnLoad
#[test]
fn missed_days_are_replayed_on_touch() {
    let mut s = market(Some(1), 0, true, 0.0);
    s.advance(10 * DAY);
    let money = s.town(0).money;
    s.set_offers(0, 0, vec![paper_trade(0)]);
    s.switch(0, 0, 0, true);
    s.deliver(0, PAPER, 27.0 * 64.0); // a shulker box of paper
    let last = s.now() - 3 * DAY - 100;
    s.set_restock_clock(0, Some(last));
    assert_eq!(s.catch_up_lockers(0), 6, "3 missed days replay 6 restocks");
    assert_eq!(s.lockers(0).unwrap().restocked_until, Some(last + 3 * DAY), "the clock moves by the 3 days, keeping 100");
    // Restocks alternate: trade 16, inflated (skip), trade 16, skip, trade 16, skip.
    assert_eq!(s.town(0).money - money, 48.0, "48 emeralds");
    assert_eq!(paper(&s), 27.0 * 64.0 - 48.0 * 24.0);
    // More than the cap: 60 days replayed, the clock to now.
    s.advance(80 * DAY);
    s.set_restock_clock(0, Some(s.now() - 70 * DAY - 5));
    assert_eq!(s.catch_up_lockers(0), (2 * MAX_DAYS) as u32);
    assert_eq!(s.lockers(0).unwrap().restocked_until, Some(s.now()));
}

// parity: LockerTradingTests.neverRestockedOwesNothing
#[test]
fn a_town_never_restocked_owes_nothing() {
    let mut s = market(None, 0, false, 0.0);
    s.advance(5 * DAY);
    s.set_restock_clock(0, None);
    assert_eq!(s.catch_up_lockers(0), 0, "never restocked: nothing owed");
    assert_eq!(s.lockers(0).unwrap().restocked_until, Some(s.now()), "but its clock starts now, so later days count");
}

#[test]
fn a_towns_lockers_trade_on_their_own_two_restocks_a_day() {
    // Left to the look: the clock starts at the first look, then two restocks a whole day, trading every other.
    let mut s = market(Some(1), 0, true, 27.0 * 64.0);
    s.advance(3 * DAY + 1200);
    assert_eq!(s.lockers(0).unwrap().restocked_until, Some(3 * DAY + 1200));
    assert_eq!(s.town(0).money, 48.0, "16 trades a day");
    let start = s.totals();
    s.advance(20 * DAY);
    s.conserved(&start).unwrap();
    s.solvent().unwrap();
}

// parity-partial: LockerTests.limitKeepsNearestLockers (no distances between lockers: the first in the roster stands for the nearest)
#[test]
fn the_merchants_limit_keeps_the_first_lockers() {
    // A Novice Merchant keeps 2; with nothing to tell them apart, the first in the roster (the mod's nearest).
    let mut s = Society::new(content());
    let t = s.add_town("Lectern", 0.0);
    s.set_roster(t, &[("librarian", 1), ("farmer", 1), ("fletcher", 1), ("merchant", 1)]);
    assert_eq!(s.lockers(t).unwrap().managed, vec![0, 1]);
    s.add_hand(t, "cleric", 1);
    assert_eq!(s.lockers(t).unwrap().managed, vec![0, 1], "the limit holds");
}

// parity: LockerTests.limitPrefersLockersWithTradesSwitchedOn
#[test]
fn the_merchants_limit_prefers_lockers_with_trades_switched_on() {
    let mut s = Society::new(content());
    let t = s.add_town("Lectern", 0.0);
    s.set_roster(t, &[("librarian", 1), ("farmer", 1), ("fletcher", 1), ("merchant", 1)]);
    let fletcher = s.lockers(t).unwrap().hands[2].offers.iter().position(|o| o.good == STICK).unwrap();
    s.switch(t, 2, fletcher, true);
    assert_eq!(s.lockers(t).unwrap().managed, vec![2, 0], "the locker with a trade on first, then the nearest");
}

// parity-partial: LogisticsTests.aMerchantShortOfLockersTendsTheOnesItsContractsNeedFirst (the training contract's need is set by hand, not drafted)
#[test]
fn a_merchant_short_of_lockers_tends_the_ones_its_contracts_need_first() {
    // The contract's need is set by hand here (`needed`); a training contract fills it in `training-and-tenders`.
    let mut s = Society::new(content());
    let t = s.add_town("Lectern", 0.0);
    s.set_roster(t, &[("librarian", 1), ("farmer", 1), ("weaponsmith", 1), ("merchant", 1)]);
    s.switch(t, 0, 0, true); // paper
    let wheat = s.lockers(t).unwrap().hands[1].offers.iter().position(|o| o.good == WHEAT).unwrap();
    s.switch(t, 1, wheat, true);
    assert!(!s.lockers(t).unwrap().managed.contains(&2), "a Novice Merchant's two: those with trades on");
    s.set_needed(t, &["weaponsmith"]);
    let managed = s.lockers(t).unwrap().managed;
    assert!(managed.contains(&2), "contracted to train a Weaponsmith: its locker comes first: {managed:?}");
    assert_eq!(managed[0], 2);
}

// parity: JobsTests.aMerchantValuesALockerByWhatItsTradesWouldDoForTheTown
#[test]
fn a_merchant_values_a_locker_by_what_its_trades_would_do_for_the_town() {
    let (carrot, bread) = (0, 1);
    let offers = [Offer::buys(carrot, 22, 1, 16, 2, 0.05), Offer::sells(bread, 1, 6, 16, 1, 0.05)];
    let (none, wanted) = ([false, false], [false, true]);
    assert_eq!(worth(&offers, 1, &[5.0, 0.0], &none), 5.0, "carrots worth 5 emeralds to sell: 5 a restock");
    assert_eq!(worth(&offers, 1, &[100.0, 0.0], &none), 16.0, "a glut: as much as its 16 uses pay");
    assert_eq!(worth(&offers, 1, &[0.0, 0.0], &wanted), 8.0, "bread the town wants: half what it charges");
    assert_eq!(worth(&offers, 1, &[0.0, 0.0], &none), 0.0, "nothing to sell it, nothing wanted: none");
}

/// A town's `Trades`, read off its entity.
fn trades(s: &Society, t: usize) -> Trades {
    s.world.get::<Trades>(s.world.resource::<Towns>().0[t]).unwrap().clone()
}

#[test]
fn a_towns_income_from_a_good_its_lockers_buy_is_its_lockers() {
    // Its people's 10 paper a day at its worth are set aside while a switched-on offer buys paper, and come back
    // when none does; what its lockers can take a day is for the warehouse's par: 24 x 16 uses (the mod counts a restock's worth as a day's).
    let mut s = market(Some(1), 0, false, 0.0);
    s.set_trade(0, PAPER, 0.0, 10.0);
    s.advance(1200);
    assert_eq!(trades(&s, 0).uses[PAPER], 10.0, "switched off: its people buy it");
    s.switch(0, 0, 0, true);
    s.advance(2400);
    assert_eq!(trades(&s, 0).uses[PAPER], 0.0, "switched on: its lockers do");
    let lockers = s.lockers(0).unwrap();
    assert_eq!((lockers.kept[PAPER], lockers.capacity[PAPER]), (Some(10.0), 384.0));
    assert_eq!(trades(&s, 0).capacity[PAPER], 384.0, "its par follows its lockers");
    assert_eq!(s.par(0, PAPER, true), 384.0 * 5.0);
    let workshop = s.world.get::<enterprise_core::workshop::Workshop>(s.world.resource::<Towns>().0[0]).unwrap();
    assert_eq!(workshop.par_of(PAPER), 384.0 * 5.0, "and its workshop makes it up to that");
    s.switch(0, 0, 0, false);
    s.advance(3600);
    assert_eq!(trades(&s, 0).uses[PAPER], 10.0, "and back");
    assert_eq!((trades(&s, 0).capacity[PAPER], s.lockers(0).unwrap().capacity[PAPER]), (10.0, 0.0));
    let workshop = s.world.get::<enterprise_core::workshop::Workshop>(s.world.resource::<Towns>().0[0]).unwrap();
    assert_eq!(workshop.par_of(PAPER), 0.0);
}

#[test]
fn the_mods_trades_have_vanillas_offers_for_the_hosts_goods() {
    let c = content();
    let librarian = &c.trades[c.trade("librarian").unwrap()];
    assert_eq!(librarian.offers, vec![paper_trade(0)], "24 paper an emerald, 16 uses, 2 XP, 0.05, at Novice");
    let farmer = &c.trades[c.trade("farmer").unwrap()];
    assert_eq!(farmer.offers, vec![Offer::buys(WHEAT, 20, 1, 16, 2, 0.05)]);
    assert!(c.trades[c.trade("merchant").unwrap()].offers.is_empty() && c.trades[c.trade("nitwit").unwrap()].offers.is_empty());
    // A host's namespaced names read the same.
    let c = Content::new(vec![Good::new("minecraft:bread", 0.2, 64)], DAY);
    assert_eq!(c.trades[c.trade("farmer").unwrap()].offers, vec![Offer::sells(0, 1, 6, 16, 1, 0.05)]);
}

#[test]
fn the_merchants_review_takes_clients_switches_on_their_trades_and_swaps_the_weakest() {
    // A Novice Merchant manages two; the Farmer, third in the roster, is worth 16 a restock with the wheat in the
    // warehouse, the others nothing: it takes the place of the first, whose trades are switched off, and its wheat
    // trade is switched on.
    let mut s = Society::new(content());
    let t = s.add_town("Lectern", 0.0);
    s.set_roster(t, &[("librarian", 1), ("cartographer", 1), ("farmer", 1), ("merchant", 1)]);
    s.switch(t, 0, 0, true);
    s.deliver(t, WHEAT, 1000.0);
    assert_eq!(s.review_lockers(t), Some(0), "the Librarian let go");
    let lockers = s.lockers(t).unwrap();
    let roster = s.roster(t);
    assert_eq!(lockers.clients, vec![roster.hands[2].id, roster.hands[1].id], "the Farmer and the Cartographer, by id");
    assert!(!lockers.hands[0].switched_on() && lockers.hands[2].offers[0].on);
    assert_eq!(lockers.hands[2].worth, 16.0);
    assert_eq!(lockers.managed, vec![2, 1], "its clients, in the order taken on");
    // A new client keeps its place two days; one doing nothing counts its days of slack.
    assert_eq!(s.review_lockers(t), None);
    assert_eq!(s.lockers(t).unwrap().hands[1].slack, 2, "the Cartographer, idle two reviews");
}

#[test]
fn two_restocks_a_day_dont_amplify_a_daily_swing_upstream() {
    // Order-up-to at each node (docs/core-math.md, bullwhip): a locker takes what the warehouse holds up to its cap,
    // never ahead of it, so a swing in what comes in each day passes through no larger than the lots it takes whole
    // (what's left of a lot one day goes the next: two lots at most, 24 paper each, fewer as the Merchant's discount
    // grows).
    let mut s = market(Some(1), 0, true, 0.0);
    s.advance(1200);
    let mut used = Vec::new();
    for day in 1..=40u64 {
        let before = paper(&s);
        let delivered = if day % 2 == 0 { 300.0 } else { 100.0 };
        s.deliver(0, PAPER, delivered);
        s.advance(day * DAY + 1200);
        used.push(before + delivered - paper(&s));
    }
    let spread = |v: &[f64]| v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min);
    let tail = &used[4..];
    assert!(spread(tail) <= 200.0 + 2.0 * 24.0, "what the lockers take swings {} against 200 coming in: {used:?}", spread(tail));
    let mean = tail.iter().sum::<f64>() / tail.len() as f64;
    assert!((mean - 200.0).abs() < 24.0, "and takes what comes in: {mean}");
}

#[test]
fn a_selling_offer_buys_within_the_towns_money() {
    // A Farmer's bread, 6 for an emerald, 16 uses: a town with 5 emeralds buys 5 lots.
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64)], DAY));
    let t = s.add_town("Crumb", 5.5);
    s.set_roster(t, &[("farmer", 1), ("merchant", 1)]);
    s.switch(t, 0, 0, true);
    let start = s.totals();
    assert_eq!(s.restock(t), 5);
    assert_eq!((s.town(t).stock[0], s.town(t).money), (30.0, enterprise_core::Fixed::from(0.5)));
    assert_eq!(s.cash_book(t, 1), vec![(0, "lockers: bought bread".to_string(), enterprise_core::Fixed::from(-5.0))]);
    s.conserved(&start).unwrap();
}

#[test]
fn lockers_are_saved_with_the_world() {
    let mut s = market(Some(1), 0, true, 27.0 * 64.0);
    s.seed(7);
    s.advance(2 * DAY + 1200);
    let mut loaded = Society::load(&s.save()).unwrap();
    assert_eq!(loaded.lockers(0), s.lockers(0));
    s.advance(6 * DAY);
    loaded.advance(6 * DAY);
    assert_eq!(loaded.lockers(0), s.lockers(0));
    assert_eq!(loaded.roster(0), s.roster(0), "the Merchant's rolls the same");
    assert_eq!(loaded.town(0).money.millis(), s.town(0).money.millis());
}

#[test]
fn a_hand_that_leaves_takes_its_locker() {
    // Settlers take a hand off the roster: the hands after it keep their own lockers, clients and managed (the
    // clients by their hands' ids, the managed by their places).
    let c = content();
    let mut l = enterprise_core::Lockers::default();
    l.hands = (0..4).map(|t| enterprise_core::lockers::Locker { hand: 11 + t as u32,
        ..enterprise_core::lockers::Locker::new(&c, t % c.trades.len()) }).collect();
    l.hands[3].worth = 7.0;
    (l.clients, l.managed) = (vec![12, 14], vec![3, 1, 0]);
    l.take_hand(1);
    assert_eq!(l.hands.len(), 3);
    assert_eq!(l.hands[2].worth, 7.0, "the last hand's locker moved down with it");
    assert_eq!((l.clients, l.managed), (vec![14], vec![2, 0]));
}
