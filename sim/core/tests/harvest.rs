//! Forward crop sales, rents and a dealer in them, pinned as the mod's game tests pin them (`ContractTests`,
//! `MarketTests`), in milliseconds.

use enterprise_core::contracts::Book;
use enterprise_core::harvest::{self, Harvest, RentState, RentTerms, Rents};
use enterprise_core::{Content, Dials, Good, Incoterm, Society, State};

const DAY: u64 = 24_000;
const T: u64 = 1_000_000;

fn terms() -> RentTerms {
    RentTerms::of(&Dials::default(), DAY)
}

// parity: ContractTests.aThinHarvestRollsOverWhereAPlainContractMisses
#[test]
fn a_thin_harvest_rolls_over_where_a_plain_contract_misses() {
    let mut book = Book::default();
    let (farm, town, other) = (0, 1, 2);
    let window = 4 * DAY;
    let forward = book.draft_harvest(farm, town, 0, 16.0, 0.05, T, DAY, 5, Incoterm::Ddp, true, true,
        Harvest { window_days: 4, carried: false });
    let plain = book.draft(other, town, 0, 16.0, 0.05, T, DAY, 5, Incoterm::Ddp, true, true);
    book.settle(forward, 0.0, 8.0, 0.4, 0.0);
    book.settle(plain, 0.0, 8.0, 0.4, 0.0);
    book.review(T + window + 1, DAY, |_| false); // the first installment judged, half delivered
    assert!(book.contracts[forward].missed == 0 && book.contracts[forward].in_force(), "a thin harvest rolls over");
    assert_eq!(book.contracts[plain].state, State::Broken, "a plain contract, judged on its two days' grace, has missed three");
    book.review(T + 2 * DAY + window + 1, DAY, |_| false); // three judged, 40 behind: past two installments
    assert_eq!(book.contracts[forward].missed, 1, "arrears past two installments are a miss");
    book.settle(forward, 0.0, 40.0 + 16.0, 2.8, 0.0); // caught up, and the fourth
    book.review(T + 4 * DAY + window + 1, DAY, |_| false); // the last falls due: 16 short
    assert!(book.contracts[forward].missed == 2 && book.contracts[forward].in_force(),
        "what's owed at the last is a miss, not rolled");
}

#[test]
fn a_rolled_shortfall_costs_a_quarter_of_a_miss_in_trust() {
    let mut rolled = Book::default();
    let mut plain = Book::default();
    // Both new to each other (teething: a miss counts a quarter), so compare the record's missed weight directly.
    rolled.draft_harvest(0, 1, 0, 16.0, 1.0, 0, DAY, 5, Incoterm::Ddp, true, true, Harvest { window_days: 4, carried: false });
    plain.draft(0, 1, 0, 16.0, 1.0, 0, DAY, 5, Incoterm::Ddp, true, true);
    rolled.review(4 * DAY + 1, DAY, |_| false); // judged: installments 1 to 1 (due_by day 0), nothing delivered
    plain.review(2 * DAY + 1, DAY, |_| false);
    let (r, p) = (rolled.record(0, 1, 0, DAY), plain.record(0, 1, 0, DAY));
    // Rolled: a whole shortfall at a quarter of a miss, then the new pair's quarter. Plain: a whole miss, its quarter.
    assert!((r.missed - 0.25 * 0.25).abs() < 1e-9 && (p.missed - 0.25).abs() < 1e-9, "{r:?} against {p:?}");
}

// parity: ContractTests.aRentPaysACouponEachSeasonAndCarriesItsArrears
#[test]
fn a_rent_pays_a_coupon_each_season_and_carries_its_arrears() {
    let mut book = Book::default();
    let mut rents = Rents::default();
    let (farm, holder, potato) = (0, 1, 0);
    let t = terms();
    let season = t.season_ticks() + 4 * DAY;
    let capital = t.capital(64.0, 0.05);
    let id = rents.sell(farm, holder, potato, 64.0, capital, &mut book, T, &t);
    let first = rents.rents[id].coupon.unwrap();
    let c = &book.contracts[first];
    assert!(c.in_force() && c.quantity == 64.0 && c.price == 0.0, "a coupon at once, already paid for");
    assert!((rents.rents[id].capital - 8.0 * 64.0 * 0.05).abs() < 1e-9, "bought for eight seasons' crop");
    book.settle(first, 0.0, 40.0, 0.0, 0.0);
    book.review(T + season, DAY, |_| false); // the coupon judged: 24 short, carried by the rent, not a miss
    assert_eq!(book.contracts[first].missed, 0, "a coupon's shortfall isn't a miss");
    rents.review(&mut book, T + season, &t);
    let second = rents.rents[id].coupon.unwrap();
    assert!(book.contracts[first].state == State::Completed && rents.rents[id].arrears == 24.0
        && book.contracts[second].quantity == 88.0, "the shortfall rides on the next coupon");
    rents.review(&mut book, T + 2 * season, &t); // nothing delivered: 88 in arrears, within two seasons'
    assert!(rents.rents[id].state == RentState::Active && rents.rents[id].arrears == 88.0, "still in force");
    rents.review(&mut book, T + 3 * season, &t); // 152 in arrears: past two seasons' crop
    assert_eq!(rents.rents[id].state, RentState::Defaulted, "defaulted");
    assert!(book.trust(farm, holder, T + 3 * season, DAY) < 0.5, "and the farm's name suffers");
}

// parity-partial: MarketTests.aPlayerBuysARentsDeedCollectsItsCropAndSellsItBack
#[test]
fn a_bearers_rent_accrues_its_crop_and_the_dealer_quotes_a_tenth_either_side() {
    let mut book = Book::default();
    let mut rents = Rents::default();
    let t = terms();
    let id = rents.sell(0, 1, 0, 64.0, 10.0, &mut book, T, &t); // the dealer holds it
    rents.set_bearer(id, true); // a player buys its deed
    let coupon = rents.rents[id].coupon.unwrap();
    book.settle(coupon, 0.0, 64.0, 0.0, 0.0); // a season's coupon collected, in the dealer's keeping
    rents.review(&mut book, T + t.season_ticks() + 4 * DAY, &t);
    assert_eq!(rents.rents[id].accrued, 64.0, "accrued for the bearer");
    assert_eq!(rents.take_accrued(id, 100.0), 64.0, "the deed at the desk: its crop handed over");
    assert_eq!(rents.rents[id].accrued, 0.0);
    rents.set_bearer(id, false); // sold back
    rents.redeem(id, &mut book); // done with it
    assert_eq!(rents.rents[id].state, RentState::Redeemed);
    // The dealer's price: capital twice with none bought ahead... half with all of it; ask and bid a tenth either side.
    assert!((harvest::dealer_price(100.0, 10.0, 0.0) - 200.0).abs() < 1e-9, "twice, with none bought ahead");
    assert!((harvest::dealer_price(100.0, 0.0, 10.0) - 50.0).abs() < 1e-9, "half, with all of it");
    assert!((harvest::dealer_price(100.0, 0.0, 0.0) - 125.0).abs() < 1e-9, "no word of either: the middle");
    assert!((harvest::ask(100.0, 0.1) - 110.0).abs() < 1e-9 && (harvest::bid(100.0, 0.1) - 90.0).abs() < 1e-9);
}

/// A farm (town 0) and a market (town 1) a day apart; the farm's wagon every `every` days. Wheat is a crop.
fn farm_and_market(farm_money: f64, market_money: f64, every: f64) -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 1.0, 64)], DAY));
    s.add_town("Cloverby", farm_money);
    s.add_town("Haygate", market_money);
    s.set_trade(0, 0, 100.0, 0.0);
    s.set_trade(1, 0, 0.0, 32.0);
    s.add_route(0, 1, 1.0, 0.5, every);
    s.mark_crop(0);
    s
}

#[test]
fn a_market_that_uses_a_crop_offers_the_farm_a_forward_sale_a_third_up_front() {
    let mut s = farm_and_market(50.0, 1000.0, 2.0);
    s.deliver(0, 0, 300.0);
    s.advance(DAY * 3);
    let forward: Vec<_> = s.contracts().into_iter().filter(|c| c.harvest.is_some()).collect();
    assert_eq!(forward.len(), 1, "one forward sale: {forward:?}");
    let c = &forward[0];
    assert!(c.seller == 0 && c.buyer == 1 && c.installments == 10 && c.quantity == 64.0, "{c:?}");
    assert_eq!(c.harvest, Some(Harvest { window_days: 4, carried: false }));
    assert!((c.price - 1.0).abs() < 1e-9, "at today's price");
    assert!((c.advance_share - 1.0 / 3.0).abs() < 1e-9);
    assert!((c.advanced - c.worth() / 3.0).abs() < 1e-6, "a third paid up front: {}", c.advanced);
    assert!(c.start >= 2 * DAY + 4 * DAY, "the first falls due a round trip and four days to sow: {}", c.start);
    assert!(s.rents().is_empty(), "the farm could pay for its own sale: no rent");
}

#[test]
fn a_farm_short_of_money_sells_a_rent_to_a_market_with_savings() {
    let mut s = farm_and_market(2.0, 100_000.0, 2.0);
    s.deliver(0, 0, 300.0);
    let before = s.town(0).money;
    s.advance(DAY * 3);
    let rents = s.rents();
    assert_eq!(rents.len(), 1, "a rent");
    let r = &rents[0];
    assert!(r.farm == 0 && r.holder == 1 && r.quantity == 320.0 && (r.capital - 8.0 * 320.0).abs() < 1e-9, "{r:?}");
    assert!(s.town(0).money > before + r.capital - 1.0, "the capital rode home: {} from {before}", s.town(0).money);
    assert!(s.contracts().iter().any(|c| c.price == 0.0 && c.harvest.is_some_and(|h| h.carried) && c.in_force()),
        "its coupon in force");
}

#[test]
fn a_farm_that_can_spare_twice_a_rents_capital_buys_it_back() {
    let mut s = farm_and_market(1000.0, 0.0, 1.0);
    s.deliver(0, 0, 1000.0);
    assert!(s.sell_rent(0, 1, 0, 16.0, 0.5).is_none(), "a holder with no money can't buy");
    s.pay(1, 100.0);
    let id = s.sell_rent(0, 1, 0, 16.0, 0.5).expect("bought"); // capital 64
    assert_eq!(s.town(1).money, 36.0);
    s.advance(DAY * 3);
    assert_eq!(s.rents()[id].state, RentState::Redeemed, "bought back: {:?}", s.rents()[id]);
    assert!(s.contracts().iter().all(|c| !(c.price == 0.0 && c.harvest.is_some()) || !c.in_force()), "its coupon closed");
}

#[test]
fn owed_crop_is_set_aside_from_spot_sales() {
    // Town 0 owes town 2 sixty-four wheat a day (every day from day 0) under a harvest contract; town 1 (a market) would buy all the rest.
    let build = |contract: bool| {
        let mut s = Society::new(Content::new(vec![Good::new("wheat", 1.0, 64)], DAY));
        for n in ["Cloverby", "Haygate", "Millford"] {
            s.add_town(n, 100.0);
        }
        s.set_trade(1, 0, 0.0, 100.0);
        s.add_route(0, 1, 1.0, 0.0, 1.0);
        s.deliver(0, 0, 200.0);
        if contract {
            // Both hold copies: in force.
            s.draft_harvest(0, 2, 0, 64.0, 1.0, 0, DAY, 10, Incoterm::Ddp, true, true);
        }
        s
    };
    let mut with = build(true);
    let aside = with.set_aside(0, 0);
    assert!((aside - 5.0 * 64.0).abs() < 1e-9, "five installments fall due by the end of the next window, none delivered: {aside}");
    let mut without = build(false);
    assert_eq!(without.set_aside(0, 0), 0.0);
    with.advance(DAY / 2);
    without.advance(DAY / 2);
    let kept = with.town(0).stock[0];
    assert_eq!(kept, 200.0, "owed crop stays in the store ({aside} owed, 200 held): {kept} left");
    assert!(without.town(0).stock[0] < kept, "and without the contract it sold: {} left", without.town(0).stock[0]);
}

#[test]
fn a_rent_the_farm_never_pays_defaults_and_costs_it_its_name() {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 1.0, 64)], DAY));
    s.add_town("Cloverby", 0.0);
    s.add_town("Haygate", 1000.0);
    let id = s.sell_rent(0, 1, 0, 16.0, 0.5).unwrap();
    let trust = s.trust(0, 1);
    s.advance(60 * DAY);
    assert_eq!(s.rents()[id].state, RentState::Defaulted, "{:?}", s.rents()[id]);
    assert!(s.trust(0, 1) < trust, "the farm's name suffers");
    assert!(s.contracts().iter().all(|c| !c.in_force()), "no coupon stands");
}

#[test]
fn the_harvest_dials_are_the_mods() {
    let s = Society::new(Content::new(vec![Good::new("wheat", 1.0, 64)], DAY));
    for (name, value) in [("harvest_window_days", 4.0), ("harvest_arrears", 2.0), ("harvest_rolled_weight", 0.25),
        ("rent_season_days", 10.0), ("rent_purchase_seasons", 8.0), ("rent_arrears_seasons", 2.0), ("dealer_spread", 0.1),
        ("forward_sow_days", 4.0), ("forward_installments", 10.0), ("forward_max_quantity", 256.0)] {
        assert_eq!(s.dial(name), Some(value), "{name}");
    }
}
