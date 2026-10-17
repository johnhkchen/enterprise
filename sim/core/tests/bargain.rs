//! Two Merchants bargaining by level, and the warehouse's par, pinned as the mod's `BargainTests` pin them; then the
//! bargain at the far end: wagons sell at the settled price, at the middle of the batch, and plan their load's worth by
//! it; and the price table.

use enterprise_core::bargain::{self, between, buyer_ceiling, price, seller_floor, shortage, MIN_PAR, SELLER_FLOOR};
use enterprise_core::prices::{emeralds_each, is_listed, items_per_emerald};
use enterprise_core::{Content, Good, Output, Society};

const DAY: u64 = 24_000;

// parity: BargainTests.noDealAtPar
#[test]
fn no_deal_at_par() {
    assert_eq!(price(1.0, 0.0, 3, 3), None, "a full shelf buys nothing");
    assert_eq!(shortage(64.0, 64.0), 0.0);
    assert_eq!(shortage(100.0, 64.0), 0.0, "above par, no shortage either");
    assert_eq!(shortage(10.0, 0.0), 0.0, "no par, no shortage");
}

// parity: BargainTests.shorterShelvesPayMore
#[test]
fn shorter_shelves_pay_more() {
    let half = price(1.0, 0.5, 1, 1).unwrap();
    let empty = price(1.0, 1.0, 1, 1).unwrap();
    assert!(empty > half && half > SELLER_FLOOR, "empty {empty} > half-full {half}");
    assert!(empty <= 1.0, "never above the local value");
    assert!((half - 0.625).abs() < 1e-12 && (empty - 0.75).abs() < 1e-12, "{half}, {empty}");
    assert_eq!(buyer_ceiling(1.0, 2.0), 1.0, "the ceiling is the full value, however short");
    assert_eq!(seller_floor(0.05), 0.025);
}

// parity: BargainTests.theHigherLevelMerchantTakesMoreOfTheGap
#[test]
fn the_higher_level_merchant_takes_more_of_the_gap() {
    let even = price(1.0, 1.0, 1, 1).unwrap();
    let master_selling = price(1.0, 1.0, 5, 1).unwrap();
    let master_buying = price(1.0, 1.0, 1, 5).unwrap();
    assert!((even - 0.75).abs() < 1e-9, "equals split the gap: {even}");
    assert!((master_selling - (0.5 + 0.5 * 5.0 / 6.0)).abs() < 1e-9, "a Master seller takes 5/6: {master_selling}");
    assert!(master_buying < even && master_selling > even, "levels move the price their way");
    assert_eq!(between(0.5, 1.0, 0, 0), between(0.5, 1.0, 1, 1), "a level below 1 counts as 1");
    assert_eq!(between(0.5, 0.5, 5, 1), None, "no gap, no deal");
}

// parity: BargainTests.parIsShelfDaysOfWhatTheLockersCanTake
#[test]
fn par_is_shelf_days_of_what_the_lockers_can_take() {
    let mut s = Society::new(Content::new(vec![Good::priced("paper", 64), Good::priced("carrot", 64)], DAY));
    let town = s.add_town("Inkwell", 10.0);
    let (paper, carrot) = (0, 1);
    assert_eq!(s.par(town, paper, true), MIN_PAR, "a wanted good starts at a stack");
    assert_eq!(s.par(town, carrot, false), 0.0, "an unwanted good has no par");
    // A librarian and a cartographer each buy 24 paper 16 times a restock.
    s.record_capacity(town, 1, &[(paper, 384.0)]);
    s.record_capacity(town, 2, &[(paper, 384.0)]);
    let days = s.content().dials.shelf_days;
    assert_eq!(s.par(town, paper, true), days * 768.0, "shelf_days of 768");
    s.record_capacity(town, 2, &[]); // the cartographer's trade switched off
    assert_eq!(s.par(town, paper, true), days * 384.0, "a session replaces that locker's figure");
    s.set_trade(town, paper, 0.0, 10.0);
    assert_eq!(s.par(town, paper, true), days * 384.0, "once lockers record, a change of use leaves their figures");
    assert_eq!(bargain::par(s.content(), 0.3, false), 2.0, "rounded up: 1.5 of a day's capacity is 2");
}

/// A farm with wheat to spare and a mill whose people use 64 a day (a shelf of 320), a day apart, its wagon every
/// two days; the mill rich enough that money never cuts a batch. Each town's Merchant at its level (none: 1).
fn farm_and_mill(farm_level: Option<u8>, mill_level: Option<u8>, bargain: bool) -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64)], DAY));
    s.content_mut().dials.standing_orders = false;
    s.content_mut().dials.bargain = bargain;
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 500.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.deliver(farm, 0, 200.0);
    if let Some(l) = farm_level {
        s.set_roster(farm, &[("merchant", l)]);
    }
    if let Some(l) = mill_level {
        s.set_roster(mill, &[("merchant", l)]);
    }
    s.add_route(farm, mill, 1.0, 0.5, 2.0);
    s
}

/// The first wagon's sale at the mill: (sold, takings), and the worth it left with.
fn first_sale(mut s: Society) -> (f64, f64, f64) {
    let mut worth = None;
    for day in 1..=10 {
        for out in s.advance(day * DAY) {
            match out {
                Output::WagonLeft { worth: w, .. } if worth.is_none() => worth = Some(w),
                Output::WagonSold { sold, takings, .. } if sold > 0.0 => return (sold, takings, worth.unwrap()),
                _ => {}
            }
        }
    }
    panic!("no sale in ten days");
}

#[test]
fn a_wagon_sells_at_the_price_settled_at_the_middle_of_the_batch() {
    // The mill's shelf is empty (its people use what it holds): the batch of n is priced at floor(n / 2) on hand,
    // short of 320.
    let (sold, takings, _) = first_sale(farm_and_mill(None, None, true));
    let each = price(0.05, shortage((sold / 2.0).floor(), 320.0), 1, 1).unwrap();
    assert!((takings - sold * each).abs() < 1e-9, "{sold} sold for {takings}, {each} each");
    assert!(each > 0.025 && each < 0.0375, "between the floor and an empty shelf's even split: {each}");
    let (flat_sold, flat_takings, _) = first_sale(farm_and_mill(None, None, false));
    assert_eq!(flat_sold, sold, "the same wheat");
    assert!((flat_takings - sold * 0.05).abs() < 1e-9, "the dial off: its full worth, {flat_takings}");
}

#[test]
fn the_seller_s_home_merchant_and_the_buyer_s_set_the_split() {
    let per = |farm, mill| {
        let (sold, takings, _) = first_sale(farm_and_mill(farm, mill, true));
        (sold, takings / sold)
    };
    let (sold, even) = per(Some(3), Some(3));
    let (_, master_selling) = per(Some(5), Some(1));
    let (_, master_buying) = per(Some(1), Some(5));
    let (_, none) = per(None, None);
    let s = shortage((sold / 2.0).floor(), 320.0);
    assert!((master_selling - price(0.05, s, 5, 1).unwrap()).abs() < 1e-12, "{master_selling}");
    assert!((master_buying - price(0.05, s, 1, 5).unwrap()).abs() < 1e-12, "{master_buying}");
    assert!(master_buying < even && even < master_selling, "{master_buying} < {even} < {master_selling}");
    assert!((none - even).abs() < 1e-12, "no Merchants: level 1 each, an even split like 3 and 3");
}

#[test]
fn a_load_is_worth_what_the_far_end_is_expected_to_pay() {
    // The plan loads the same wheat either way; with the bargain it's worth its price at the middle of the batch.
    let (_, _, flat) = first_sale(farm_and_mill(None, None, false));
    let (_, _, bargained) = first_sale(farm_and_mill(None, None, true));
    let going = flat / 0.05;
    let each = price(0.05, shortage((going / 2.0).floor(), 320.0), 1, 1).unwrap();
    assert!((bargained - going * each).abs() < 1e-9, "{going} wheat worth {bargained}, {each} each");
}

#[test]
fn the_lockers_capacity_sets_the_shelf() {
    // Lockers that can take 128 wheat a day: a shelf of 640, not the 320 its people's use would give.
    let mut s = farm_and_mill(None, None, true);
    s.deliver(0, 0, 1000.0);
    s.record_capacity(1, 7, &[(0, 128.0)]);
    let (sold, _, _) = first_sale(s);
    assert!(sold > 320.0, "{sold} sold onto a 640 shelf");
}

#[test]
fn the_mod_s_price_table() {
    assert_eq!(emeralds_each("wheat"), 0.05);
    assert_eq!(emeralds_each("minecraft:paper"), 1.0 / 24.0);
    assert_eq!(items_per_emerald("minecraft:leather_chestplate"), 1.0 / 7.0);
    assert_eq!(items_per_emerald("red_carpet"), 4.0, "every colour's carpet");
    assert_eq!(items_per_emerald("minecraft:lime_candle"), 3.0, "dyed candles as plain ones");
    assert_eq!(items_per_emerald("enterprise:pipe_tobacco"), 4.0);
    assert_eq!(items_per_emerald("minecraft:unobtainium"), 32.0, "unlisted: 32 to the emerald");
    assert!(is_listed("bread") && !is_listed("unobtainium"));
    let g = Good::priced("minecraft:book", 16);
    assert_eq!((g.value, g.stack), (0.25, 16));
}
