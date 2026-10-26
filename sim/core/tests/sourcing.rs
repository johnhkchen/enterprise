//! Sourcing and procurement (`sourcing.rs`, `rules/sourcing.rs`), pinned as the mod's game tests pin them
//! (`ContractTests`, `LogisticsTests`, `LockerTradingTests`, `CaravanTests`): a town short of a good contracts a supplier,
//! its Merchant buys from its lockers what its contracts want within two days, a contract's inputs raise the par so
//! wagons bring them, and the capital's metal exchange buys nearest holder first at a premium that rises as it runs short.

use crate::common;

use enterprise_core::contracts::{Book, MIN_TRUST};
use enterprise_core::lockers::Offer;
use enterprise_core::sourcing::{Phase, MASTER};
use enterprise_core::{Content, Good, Incoterm, Society, State};

const DAY: u64 = 24_000;

/// The desk (town 0) and a supplier (town 1) 300 blocks apart, the desk's wagons going to the supplier (so Ex Works),
/// the desk knowing the supplier; the supplier counted `held` of the good (good 0) at tick 0.
fn pair(goods: Vec<Good>, held: f64) -> Society {
    let mut s = Society::new(Content::new(goods, DAY));
    let (desk, supplier) = (s.add_town("Desk", 50.0), s.add_town("Supplier", 50.0));
    s.place_town(desk, 0.0, 0.0);
    s.place_town(supplier, 300.0, 0.0);
    s.record_stocktake(supplier, &[(0, held)], Some(0));
    s.learn(desk, supplier);
    s.add_route(desk, supplier, 0.25, 0.5, 1.0);
    s
}

/// As [`pair`], with a second good (1) the desk holds and the supplier's people use, so the desk's wagons have a load
/// worth their road (a wagon with only a copy to carry doesn't go).
fn trading_pair() -> Society {
    let mut s = pair(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("tin", 0.5, 64)], 64.0);
    s.deliver(0, 1, 128.0);
    s.set_trade(1, 1, 0.0, 20.0);
    s
}

fn from_supplier(s: &Society) -> Vec<enterprise_core::Contract> {
    s.contracts().into_iter().filter(|c| c.seller == 1 && c.buyer == 0).collect()
}

// parity: ContractTests.aTownShortOfGoodsSourcesThemFromASupplier
#[test]
fn a_town_short_of_goods_sources_them_from_a_supplier() {
    // Five pickaxes wanted; the supplier counted six, and a town keeps half its kit: three.
    let mut s = pair(vec![Good::new("iron_pickaxe", 0.5, 1)], 6.0);
    assert!(!s.source(0, 0, 5.0), "three of the five: the need isn't covered yet");
    let contracts = from_supplier(&s);
    assert_eq!(contracts.len(), 1, "one contract with the supplier");
    let c = &contracts[0];
    assert!(c.incoterm == Incoterm::Exw && c.quantity == 3.0 && c.installments == 1,
        "Ex Works, since its wagons go there; three, half the supplier's kit: {c:?}");
    assert_ne!(c.state, State::Active, "in force once a wagon carries the copy");
    assert_eq!(c.state, State::Drafted);
    assert!((c.price - 0.5 * 1.1).abs() < 1e-9, "the worth and ten percent, nothing for the road Ex Works: {}", c.price);
    assert_eq!(c.advance_share, 0.0, "goods in stock: nothing up front");
    assert!(!s.source(0, 0, 5.0), "and the three on the way count, not drafted again");
    assert_eq!(from_supplier(&s).len(), 1, "one open contract per good");
}

// parity: ContractTests.belowTrustTheyStillDealOnTrialTerms
#[test]
fn below_trust_they_still_deal_on_trial_terms() {
    let mut s = pair(vec![Good::new("paper", 1.0 / 24.0, 64)], 64.0);
    let now = s.now();
    for _ in 0..3 {
        s.world.resource_mut::<Book>().kept(1, 0, 1.0, now, DAY);
    }
    for _ in 0..12 {
        s.world.resource_mut::<Book>().missed(1, 0, 1.0, now, DAY);
    }
    assert!(s.trust(1, 0) < MIN_TRUST, "a supplier that let it down: {}", s.trust(1, 0));
    s.source(0, 0, 40.0);
    let c = &from_supplier(&s)[0];
    assert!(c.quantity == s.dial("trial_lot").unwrap() && c.advance_share == 0.0,
        "still asked, for a trial lot with nothing up front: {c:?}");
    assert_eq!((c.quantity, c.installments), (4.0, 1));
}

// parity: LogisticsTests.aMerchantBuysForWhatsDueSoonNotTheWholeContract
#[test]
fn a_merchant_buys_for_whats_due_soon_not_the_whole_contract() {
    let mut s = Society::new(Content::new(vec![Good::new("diamond_sword", 4.0, 1)], DAY));
    let (desk, front) = (s.add_town("Desk", 50.0), s.add_town("Front", 50.0));
    s.advance(10 * DAY);
    let t = s.now();
    // Ten swords, one a day from now, Ex Works, in force at once.
    s.draft(desk, front, 0, 1.0, 30.0, t, DAY, 10, Incoterm::Exw, true);
    let soon = s.soon(desk).iter().find(|(g, _)| *g == 0).map_or(0.0, |&(_, n)| n);
    let all = s.wanted(desk).iter().find(|(g, _)| *g == 0).map_or(0.0, |&(_, n)| n);
    assert_eq!((soon, all), (3.0, 10.0), "three due within two days, of ten in all: {soon} of {all}");
    // What it holds comes off both.
    s.deliver(desk, 0, 2.0);
    assert_eq!(s.soon(desk), vec![(0, 1.0)]);
    assert_eq!(s.wanted(desk), vec![(0, 8.0)]);
}

/// A town of a master smith (hand 0, level 5, a sword offer: 20 emeralds, `uses` a restock) and a Merchant (hand 1),
/// with 64 emeralds, under a contract to deliver swords to a second town: `installments` of one, a day apart from now.
fn smithy(uses: u8, installments: u32) -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("diamond_sword", 4.0, 1)], DAY));
    let (town, front) = (s.add_town("Smithy", 0.0), s.add_town("Front", 50.0));
    s.set_roster(town, &[("weaponsmith", 5), ("merchant", 1)]);
    s.set_offers(town, 0, vec![Offer::sells(0, 20, 1, uses, 30, 0.2).at(5)]); // a master's sword
    s.pay(town, 64.0);
    s.draft(town, front, 0, 1.0, 30.0, 0, DAY, installments, Incoterm::Exw, true);
    s
}

// parity: LockerTradingTests.atItsRestockTheMerchantBuysWhatItsContractsWantOfAVillager
#[test]
fn at_its_restock_the_merchant_buys_what_its_contracts_want_of_a_villager() {
    let mut s = smithy(3, 4);
    s.restock(0);
    let offer = &s.lockers(0).unwrap().hands[0].offers[0];
    assert!(offer.on && offer.procured, "its sword trade switched on, at the restock");
    assert!(s.town(0).stock[0] >= 1.0, "and swords bought for the contract: {}", s.town(0).stock[0]);
}

#[test]
fn what_it_buys_of_a_villager_is_only_what_is_due_soon() {
    // Twelve uses a restock, ten swords owed one a day: three due within two days, so three bought, not twelve.
    let mut s = smithy(12, 10);
    s.restock(0);
    assert_eq!(s.town(0).stock[0], 3.0, "three due within two days");
    let offers = s.lockers(0).unwrap().hands[0].offers.clone();
    assert_eq!(offers[0].used, 3);
    // Nothing is due that it doesn't hold: a second restock the same day buys none.
    s.restock(0);
    assert_eq!(s.town(0).stock[0], 3.0, "what it holds comes off what's due");
}

#[test]
fn the_offer_goes_off_again_when_it_is_no_longer_wanted() {
    let mut s = smithy(3, 1);
    s.restock(0);
    assert!(s.lockers(0).unwrap().hands[0].offers[0].on);
    // The one sword delivered, the contract done: nothing is wanted, and what the Merchant switched on goes off.
    s.advance(s.now() + 4 * DAY);
    s.restock(0);
    let offer = &s.lockers(0).unwrap().hands[0].offers[0];
    assert!(!offer.on && !offer.procured, "switched off again: {offer:?}");
    // But an offer a host switched on stays on.
    s.switch(0, 0, 0, true);
    s.restock(0);
    assert!(s.lockers(0).unwrap().hands[0].offers[0].on, "a host's offer isn't the Merchant's to turn off");
}

#[test]
fn a_supplier_that_can_make_it_is_asked_for_a_lot_in_installments_a_third_up_front() {
    // The supplier holds no swords but has a Weaponsmith and iron; the desk's wagons go there.
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64), Good::new("iron_sword", 1.5, 1)], DAY));
    let (desk, supplier) = (s.add_town("Desk", 50.0), s.add_town("Supplier", 50.0));
    s.set_roster(supplier, &[("weaponsmith", 1)]);
    s.record_stocktake(supplier, &[(0, 5.0)], Some(0));
    s.learn(desk, supplier);
    s.add_route(desk, supplier, 0.25, 0.5, 1.0);
    let t = s.now();
    assert!(!s.source(desk, 1, 40.0), "a lot of sixteen at most");
    let c = &from_supplier(&s)[0];
    assert_eq!((c.quantity, c.installments), (4.0, 4), "sixteen, in four installments");
    assert_eq!(c.every, DAY, "a day apart");
    assert_eq!(c.start, t + 3 * DAY, "the first after three days' lead");
    assert!((c.advance_share - 1.0 / 3.0).abs() < 1e-9, "a third up front");
    assert_eq!(c.incoterm, Incoterm::Exw);
    // With no iron anywhere it can't make them.
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64), Good::new("iron_sword", 1.5, 1)], DAY));
    let (desk, supplier) = (s.add_town("Desk", 50.0), s.add_town("Supplier", 50.0));
    s.set_roster(supplier, &[("weaponsmith", 1)]);
    s.learn(desk, supplier);
    s.add_route(desk, supplier, 0.25, 0.5, 1.0);
    s.source(desk, 1, 40.0);
    assert!(s.contracts().is_empty(), "no iron, no swords");
}

#[test]
fn a_trial_lot_to_make_has_nothing_up_front() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64), Good::new("iron_sword", 1.5, 1)], DAY));
    let (desk, supplier) = (s.add_town("Desk", 50.0), s.add_town("Supplier", 50.0));
    s.set_roster(supplier, &[("weaponsmith", 1)]);
    s.record_stocktake(supplier, &[(0, 5.0)], Some(0));
    s.learn(desk, supplier);
    s.add_route(desk, supplier, 0.25, 0.5, 1.0);
    for _ in 0..12 {
        s.world.resource_mut::<Book>().missed(supplier, desk, 1.0, 0, DAY);
    }
    s.source(desk, 1, 40.0);
    let c = &from_supplier(&s)[0];
    assert_eq!((c.quantity, c.installments, c.advance_share), (4.0, 1, 0.0), "{c:?}");
}

#[test]
fn delivered_at_place_when_only_the_suppliers_wagons_go_there_and_the_road_is_in_the_price() {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64)], DAY));
    let (desk, supplier) = (s.add_town("Desk", 50.0), s.add_town("Supplier", 50.0));
    s.record_stocktake(supplier, &[(0, 64.0)], Some(0));
    s.learn(desk, supplier);
    // The supplier's wagons come to the desk, two days each way; the desk has none going there.
    s.add_route(supplier, desk, 2.0, 0.54, 1.0);
    s.source(desk, 0, 40.0);
    let c = &from_supplier(&s)[0];
    assert_eq!(c.incoterm, Incoterm::Dap);
    // Worth and ten percent, and the round trip's cost spread over a wagon's 27 stacks of 64.
    let want = 1.0 / 24.0 * 1.1 + 0.54 / (27.0 * 64.0);
    assert!((c.price - want).abs() < 1e-9, "carriage in the price: {} against {want}", c.price);
    // The first installment due once the supplier's wagon can be there: its road's two days.
    assert_eq!(c.start, s.now() + 2 * DAY);
    assert_eq!(c.quantity, 40.0);
}

#[test]
fn with_no_road_the_want_stays_a_signal_not_a_contract() {
    // A master Merchant's town is contracted to deliver swords, has no way to make them, and the one market that holds
    // some has no road to it: no contract, but the idle hands of the towns that know of it hear.
    let mut s = Society::new(Content::new(vec![Good::new("iron_sword", 1.5, 1), Good::new("iron_ingot", 0.25, 64)], DAY));
    let (town, front, market, idle) = (s.add_town("Town", 50.0), s.add_town("Front", 50.0), s.add_town("Market", 50.0),
        s.add_town("Idle", 50.0));
    s.set_roster(town, &[("merchant", MASTER)]);
    s.record_stocktake(market, &[(0, 20.0)], Some(0));
    s.learn(town, market);
    s.learn(idle, town);
    s.draft(town, front, 0, 8.0, 2.0, 0, DAY, 4, Incoterm::Exw, true);
    s.advance(DAY / 2);
    assert!(s.contracts().iter().all(|c| c.seller != market), "no road to the market: no contract");
    assert!(!s.source(town, 0, 32.0));
    let hears = s.signals(idle);
    assert!(hears.iter().any(|&(trade, emeralds)| trade == s.content().trade("weaponsmith").unwrap() && emeralds > 0.0),
        "the Weaponsmith's trade is a marble for the town that knows of it: {hears:?}");
    // A road, and the want is sourced at the next day's review.
    s.add_route(town, market, 0.25, 0.5, 1.0);
    s.advance(2 * DAY);
    let c = s.contracts().into_iter().find(|c| c.seller == market && c.buyer == town).expect("sourced from the market");
    assert!(c.quantity > 0.0, "{c:?}");
    assert!(s.signals(idle).is_empty(), "and the signal is gone: {:?}", s.signals(idle));
}

#[test]
fn a_town_with_no_master_merchant_sources_nothing_of_its_own_accord() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_sword", 1.5, 1)], DAY));
    let (town, front, market) = (s.add_town("Town", 50.0), s.add_town("Front", 50.0), s.add_town("Market", 50.0));
    s.set_roster(town, &[("merchant", MASTER - 1)]);
    s.record_stocktake(market, &[(0, 20.0)], Some(0));
    s.learn(town, market);
    s.add_route(town, market, 0.25, 0.5, 1.0);
    s.draft(town, front, 0, 8.0, 2.0, 0, DAY, 4, Incoterm::Exw, true);
    s.advance(3 * DAY);
    assert!(s.contracts().iter().all(|c| c.seller != market));
}

#[test]
fn the_copy_rides_with_the_wagon_and_the_contract_comes_into_force() {
    let mut s = trading_pair();
    s.source(0, 0, 40.0);
    let id = from_supplier(&s)[0].id;
    assert_eq!(s.contract(id).state, State::Drafted);
    s.advance(2 * DAY);
    assert_eq!(s.contract(id).state, State::Active, "the desk's wagon carried the copy to the supplier");
}

#[test]
fn the_suppliers_wagon_brings_the_goods_and_takes_the_copy_home() {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64)], DAY));
    let (desk, supplier) = (s.add_town("Desk", 50.0), s.add_town("Supplier", 50.0));
    s.deliver(supplier, 0, 128.0);
    s.set_trade(desk, 0, 0.0, 20.0); // its people use paper: the supplier's wagon has a load
    s.record_stocktake(supplier, &[(0, 128.0)], Some(0));
    s.learn(desk, supplier);
    s.add_route(supplier, desk, 1.0, 0.5, 1.0);
    s.source(desk, 0, 40.0);
    let id = from_supplier(&s)[0].id;
    assert_eq!(s.contract(id).incoterm, Incoterm::Dap);
    s.advance(DAY + DAY / 2);
    assert_eq!(s.contract(id).state, State::Drafted, "the wagon is still on the road with the copy");
    s.advance(3 * DAY);
    assert_ne!(s.contract(id).state, State::Drafted, "home again, the copy is the supplier's: {:?}", s.contract(id));
}

// parity: LogisticsTests.theExchangeBuysMetalFromTheNearestHolderFirst
#[test]
fn the_exchange_buys_metal_from_the_nearest_holder_first() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64)], DAY));
    let (capital, mine, overseas) = (s.add_town("Capital", 50.0), s.add_town("Mine", 50.0), s.add_town("Overseas", 50.0));
    s.place_town(capital, 0.0, 0.0);
    s.place_town(mine, 300.0, 0.0); // a town with a mine, near
    s.place_town(overseas, -2400.0, 0.0); // across the sea, far
    s.record_stocktake(mine, &[(0, 40.0)], Some(0));
    s.record_stocktake(overseas, &[(0, 512.0)], Some(0));
    s.learn(capital, mine);
    s.learn(capital, overseas);
    s.add_route(capital, mine, 0.25, 0.5, 1.0);
    s.add_route(capital, overseas, 2.0, 1.0, 4.0);
    s.open_exchange(capital);
    let premium = s.exchange_premium(0);
    assert!((premium - 1.5).abs() < 1e-9, "holding none, it pays half again: {premium}");
    s.source_at(capital, 0, 100.0, premium);
    let from = |town: usize| -> f64 {
        s.contracts().iter().filter(|c| c.seller == town && c.buyer == capital).map(|c| c.quantity * c.installments as f64).sum()
    };
    assert_eq!((from(mine), from(overseas)), (40.0, 60.0), "the mine's forty first, the rest from overseas");
    let c = s.contracts().into_iter().find(|c| c.seller == mine).unwrap();
    assert!((c.price - 0.25 * 1.1 * 1.5).abs() < 1e-9, "at the premium: {}", c.price);
    s.set_exchange_held(0, 192.0);
    assert!((s.exchange_premium(0) - 1.0).abs() < 1e-9, "holding its target, its worth");
}

#[test]
fn the_premium_rises_as_the_exchange_runs_short_and_its_target_with_the_phase() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64), Good::new("diamond", 4.0, 64),
        Good::new("paper", 0.04, 64)], DAY));
    s.set_exchange_held(0, 96.0);
    assert!((s.exchange_premium(0) - 1.25).abs() < 1e-9, "half short: a quarter over");
    s.set_exchange_held(0, 384.0);
    assert!((s.exchange_premium(0) - 0.75).abs() < 1e-9, "twice its target: three quarters, no less");
    s.set_exchange_held(0, 192.0);
    assert_eq!((s.exchange_target(0), s.exchange_target(1), s.exchange_target(2)), (192.0, 12.0, 0.0));
    s.set_phase(Phase::Mobilising);
    assert_eq!((s.exchange_target(0), s.exchange_target(1)), (384.0, 24.0), "twice as much while it mobilises");
    assert!((s.exchange_premium(0) - 1.25).abs() < 1e-9, "so it is short at 192: {}", s.exchange_premium(0));
    s.set_phase(Phase::War);
    assert_eq!((s.exchange_target(0), s.exchange_target(1)), (576.0, 36.0), "three times at war");
    assert_eq!(s.exchange_premium(2), 1.0, "a good it doesn't keep: its worth");
}

#[test]
fn each_day_the_exchange_counts_its_metal_and_contracts_the_shortfall() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64), Good::new("bread", 0.17, 64).food(5.0)], DAY));
    let capital = s.add_town("Capital", 100.0);
    s.place_town(capital, 0.0, 0.0);
    let mine = s.add_town("Mine", 50.0);
    s.place_town(mine, 300.0, 0.0);
    s.deliver(mine, 0, 40.0);
    s.record_stocktake(mine, &[(0, 40.0)], Some(0));
    s.learn(capital, mine);
    s.add_route(capital, mine, 0.25, 0.5, 1.0);
    let sea = s.open_exchange_overseas(capital, 200.0).expect("the capital is placed");
    assert!(s.is_off_map(sea));
    let start = s.totals();
    s.advance(DAY + DAY / 2);
    // The market across the sea made its iron and bread, and the capital's Merchant heard of it.
    assert_eq!(s.town(sea).stock[0], 512.0);
    assert_eq!(s.town(sea).stock[1], 192.0);
    assert_eq!(s.stocktake(sea).map(|t| t.stock[0]), Some(512.0));
    // 192 wanted: forty from the mine, the rest from across the sea, each at half again its worth.
    let contracts = s.contracts();
    let to_capital = |town: usize| -> f64 {
        contracts.iter().filter(|c| c.seller == town && c.buyer == capital && c.good == 0)
            .map(|c| c.quantity * c.installments as f64).sum()
    };
    assert_eq!((to_capital(mine), to_capital(sea)), (40.0, 152.0));
    let from_sea = contracts.iter().find(|c| c.seller == sea).unwrap();
    assert!((from_sea.price - 0.25 * 1.1 * 1.5).abs() < 1e-9, "at half again: {}", from_sea.price);
    assert_eq!(from_sea.incoterm, Incoterm::Exw, "the capital's wagons go there and collect");
    s.conserved(&start).unwrap();
    // Day after day the books hold, and the capital's wagons bring the metal home.
    for day in 2..=12 {
        s.advance(day * DAY + DAY / 2);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
    assert!(s.town(capital).stock[0] > 0.0, "iron reached the capital: {}", s.town(capital).stock[0]);
    assert!(s.exchange().held(0) > 0.0, "and the exchange counted it: {:?}", s.exchange());
}

#[test]
fn a_seller_contracted_to_make_something_needs_the_inputs_for_it() {
    // Eight swords owed, two a day: sixteen iron for them (a sword is two ingots).
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.25, 64), Good::new("iron_sword", 1.5, 1)], DAY));
    let (smith, buyer) = (s.add_town("Smith", 50.0), s.add_town("Buyer", 50.0));
    assert!(s.inputs(smith).iter().all(|n| *n == 0.0), "nothing contracted, nothing needed");
    s.draft(smith, buyer, 1, 2.0, 1.5, 3 * DAY, DAY, 4, Incoterm::Exw, true);
    s.advance(1_200);
    assert_eq!(s.inputs(smith), vec![16.0, 0.0], "sixteen iron");
    assert!(s.inputs(buyer).is_empty(), "the buyer makes nothing for it");
}

// (The inputs half of CaravanTests.theAdvanceRidesWithTheCopyAndTheInputsFollow, whole in tests/advances.rs, here on a
// longer road: the same contract, the next wagon brings the iron.)
#[test]
fn a_contracts_inputs_raise_the_par_and_the_next_wagon_brings_them() {
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.1, 64), Good::new("iron_sword", 1.5, 1)], DAY));
    let (hub, town) = (s.add_town("Hub", 50.0), s.add_town("Town", 8.0));
    s.deliver(hub, 0, 64.0);
    s.add_route(hub, town, 1.0, 0.5, 1.0);
    s.add_route(town, hub, 1.0, 0.5, 1.0);
    // The hub orders eight swords made, two a day from the third day; the copy is the hub's, its wagon carries it.
    let id = s.draft_advanced(town, hub, 1, 2.0, 1.5, 3 * DAY, DAY, 4, Incoterm::Exw, false, false, true);
    assert_eq!(s.contract(id).state, State::Drafted);
    s.advance(DAY + DAY / 4);
    assert_eq!(s.contract(id).state, State::Active, "the wagon brought the copy");
    s.advance(DAY + DAY / 2);
    assert_eq!(s.inputs(town), vec![16.0, 0.0], "the town needs sixteen iron");
    s.advance(5 * DAY);
    let iron = s.town(town).stock[0];
    assert!(iron >= 16.0, "the next wagon brings the iron for the swords: {iron}");
}

#[test]
fn a_trip_with_the_inputs_a_contract_needs_cant_wait_for_its_day() {
    // The hub's wagon goes to the town every twenty days. It has gone and come home; then the hub orders swords of the
    // town, which needs iron the hub holds: the wagon goes at once with it, once an installment.
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.1, 64), Good::new("iron_sword", 1.5, 1),
        Good::new("paper", 0.04, 64)], DAY));
    let (hub, town) = (s.add_town("Hub", 50.0), s.add_town("Town", 50.0));
    s.deliver(hub, 0, 64.0);
    s.deliver(hub, 2, 128.0);
    s.set_trade(town, 2, 0.0, 20.0);
    s.add_route(hub, town, 1.0, 0.5, 20.0);
    s.advance(DAY / 2);
    assert!(s.route(0).on_the_road, "its first trip, on its day");
    s.advance(2 * DAY + DAY / 2);
    assert!(!s.route(0).on_the_road, "home again, and not due for twenty days");
    s.draft(town, hub, 1, 2.0, 1.5, 5 * DAY, DAY, 4, Incoterm::Exw, true);
    s.advance(2 * DAY + DAY / 2 + 6 * 1_200);
    let cargo = s.wagon_cargo(0).expect("the wagon went at once, not on its day");
    assert!(cargo[0] > 0.0, "with the iron for the swords: {cargo:?}");
}

#[test]
fn inputs_are_carried_before_other_goods_when_slots_are_short() {
    // Two goods the far end wants, one slot's worth of room aboard: the input goes, the other stays.
    let mut s = Society::new(Content::new(vec![Good::new("iron_ingot", 0.1, 64), Good::new("iron_sword", 1.5, 1),
        Good::new("paper", 0.04, 64)], DAY));
    s.set_dial("wagon_slots", 1.0).unwrap();
    let (hub, town) = (s.add_town("Hub", 50.0), s.add_town("Town", 50.0));
    s.deliver(hub, 0, 64.0);
    s.deliver(hub, 2, 64.0);
    s.set_trade(town, 2, 0.0, 1.0); // its people use paper, cheaper than iron
    s.add_route(hub, town, 1.0, 0.5, 1.0);
    s.add_route(town, hub, 1.0, 0.5, 1.0);
    s.draft(town, hub, 1, 4.0, 1.5, 0, DAY, 4, Incoterm::Exw, true);
    s.advance(DAY / 2);
    assert!(s.inputs(town)[0] > 0.0);
    s.advance(DAY);
    let cargo = s.wagon_cargo(0).expect("the hub's wagon is on the road");
    assert!(cargo[0] > 0.0 && cargo[2] == 0.0, "the iron took the only slot: {cargo:?}");
}

#[test]
fn the_save_keeps_sourcing_whole() {
    let mut s = trading_pair();
    s.source(0, 0, 40.0);
    s.set_phase(Phase::War);
    s.set_exchange_held(0, 7.0);
    let bytes = s.save();
    let mut loaded = Society::load(&bytes).unwrap();
    assert_eq!(loaded.contracts().len(), 1);
    assert_eq!(loaded.exchange(), s.exchange());
    // The copy still rides: run on, both come out the same.
    s.advance(3 * DAY);
    loaded.advance(3 * DAY);
    assert_eq!(s.contracts()[0].state, State::Active);
    assert_eq!(loaded.contracts()[0].state, State::Active);
    assert!(loaded.unsaved().is_empty(), "{:?}", loaded.unsaved());
}

#[test]
fn the_random_worlds_source_and_the_exchange_buys() {
    // The generator exercises it: over sixty worlds, towns sourced goods, some of those came into force, the
    // exchange contracted metal, and the jobs heard of wants nobody supplies.
    let (mut sourced, mut in_force, mut trial, mut advances, mut exchange, mut signals) = (0, 0, 0, 0, 0, 0);
    for n in common::each_seed(common::seeds(60), |seed| {
        let (mut sourced, mut in_force, mut trial, mut advances, mut exchange, mut signals) = (0, 0, 0, 0, 0, 0);
        let mut s = common::world(seed);
        for day in 1..=45 {
            s.advance(day * DAY);
            common::slices::daily(&mut s, seed, day);
            signals += (0..s.town_count()).map(|t| s.signals(t).len()).sum::<usize>();
        }
        let capital = s.exchange().desk;
        for c in s.contracts().iter().filter(|c| c.drafted > 0) {
            sourced += 1;
            in_force += (c.state != State::Drafted) as usize;
            trial += (c.quantity == 4.0 && c.installments == 1) as usize;
            advances += (c.advance_share > 0.0) as usize;
            exchange += (Some(c.buyer) == capital) as usize;
        }
        [sourced, in_force, trial, advances, exchange, signals]
    }) {
        sourced += n[0];
        in_force += n[1];
        trial += n[2];
        advances += n[3];
        exchange += n[4];
        signals += n[5];
    }
    println!("sourced {sourced}, in force {in_force}, trial {trial}, advances {advances}, exchange {exchange}, signals {signals}");
    // (98 once a town's people took a share of its lockers' takings as wages: a little less free money for contracts;
    // 88 once a caravan signed only a hand the job draw left idle: fewer wagons crewed to carry the contracts' news.)
    assert!(sourced > 80 && in_force > 20 && exchange > 20 && signals > 5, "{sourced} {in_force} {exchange} {signals}");
}
