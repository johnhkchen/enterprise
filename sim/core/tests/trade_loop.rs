//! The trade loop's rules, each pinned by a small world: seconds, not an hour of a server.

use enterprise_core::{Content, Good, Input, Society};

const DAY: u64 = 24_000;

/// A farm town making wheat and a mill town whose people use it, a day apart: the farm's wagon runs every `every`
/// days. The mill starts with `money`.
fn farm_and_mill(every: f64, money: f64) -> Society {
    let content = Content::new(vec![Good::new("wheat", 0.05, 64)], DAY);
    let mut s = Society::new(content);
    s.content_mut().dials.standing_orders = false; // the selling alone, no contracts
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", money);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.add_route(farm, mill, 1.0, 0.5, every);
    s
}

#[test]
fn goods_and_money_are_conserved_every_day() {
    let mut s = farm_and_mill(2.0, 10.0);
    let start = s.totals();
    for day in 1..=1000 {
        s.advance(day * DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
        s.solvent().unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
}

/// Wheat a farm sells its mill in 120 days, its wagon every `every` days, the mill starting with `money`;
/// `compounding`: a buyer allows a seller who's been away what the daily looks it missed would have. At the flat
/// price (the dial `bargain` off): the mill pays the wagon what its people pay it, so its money only goes round, and
/// these tests measure the budget alone. With the bargain the mill keeps a margin and its money grows
/// (`a_bargain_leaves_the_buyer_a_margin_that_loosens_its_cash`).
fn sold(every: f64, money: f64, compounding: bool) -> f64 {
    sold_at(every, money, compounding, false)
}

fn sold_at(every: f64, money: f64, compounding: bool, bargain: bool) -> f64 {
    let mut s = farm_and_mill(every, money);
    s.content_mut().dials.bargain = bargain;
    if !compounding {
        s.content_mut().dials.max_budget_days = 1.0;
    }
    s.advance(120 * DAY);
    s.route(0).sold
}

#[test]
fn with_cash_enough_a_wagon_every_four_days_sells_what_daily_ones_would() {
    // A buyer spends half its free money a day. One that a wagon reaches every four days allows it what the four
    // daily looks would have, one after another: without that it would buy a quarter less.
    // (the mill keeps 6.4 emeralds back, two days of what its people use: its trading capital)
    let daily = sold(1.0, 26.4, true);
    let four = sold(4.0, 26.4, true);
    assert!(four >= 0.9 * daily, "four-day wagons sell {four}, daily ones {daily}");
    let without = sold(4.0, 26.4, false);
    assert!(without < 0.8 * daily, "and without it they don't: {without} against {daily}");
}

#[test]
fn a_buyer_short_of_cash_takes_less_from_fewer_wagons() {
    // Its money goes round: what it pays the wagon comes back as its people buy what it bought. Daily wagons meet
    // that money every day; one every four days meets it once, and a buyer holding 2 emeralds can't pay for four
    // days of wheat at once. Fewer, fuller wagons need buyers with working capital (or credit, or consignment).
    let daily = sold(1.0, 8.4, true);
    let four = sold(4.0, 8.4, true);
    assert!(four < 0.7 * daily, "four-day wagons sell {four}, daily ones {daily}");
    assert!(four > sold(4.0, 8.4, false), "though the compounding still helps");
}

#[test]
fn a_bargain_leaves_the_buyer_a_margin_that_loosens_its_cash() {
    // Buying at the bargained price (between half and the whole of its worth) and selling to its people at its worth,
    // the mill holding 8.4 emeralds grows its money, so four-day wagons sell it nearly what daily ones do: 6,799
    // against 7,469 wheat in 120 days (at the flat price, 1,085 against 1,782).
    let daily = sold_at(1.0, 8.4, true, true);
    let four = sold_at(4.0, 8.4, true, true);
    assert!(four > 0.85 * daily && four > 4.0 * sold(4.0, 8.4, true), "four-day wagons sell {four}, daily ones {daily}");
    let mut s = farm_and_mill(4.0, 8.4);
    s.advance(120 * DAY);
    assert!(s.town(1).money > 50.0, "the mill's margin: {} emeralds", s.town(1).money);
}

#[test]
fn a_route_with_work_keeps_its_timetable() {
    let mut s = farm_and_mill(3.0, 50.0);
    s.advance(300 * DAY);
    // A round trip takes two days and it goes every three: about a hundred trips in 300 days.
    assert!(s.route(0).trips >= 90, "{} trips in 300 days", s.route(0).trips);
}

#[test]
fn the_same_happenings_give_the_same_world() {
    let script: Vec<Input> = (1..=200)
        .flat_map(|day| {
            let mut inputs = vec![Input::Advance { to: day * DAY }];
            if day % 7 == 0 {
                inputs.push(Input::Deliver { town: 1, good: 0, count: 100.0 });
            }
            inputs
        })
        .collect();
    let mut a = farm_and_mill(2.0, 10.0);
    let mut b = farm_and_mill(2.0, 10.0);
    let outs_a: Vec<_> = script.iter().flat_map(|i| a.apply(i)).collect();
    let outs_b: Vec<_> = script.iter().flat_map(|i| b.apply(i)).collect();
    assert_eq!(outs_a, outs_b, "the same results, message for message");
    assert_eq!(a.town(0).money, b.town(0).money, "to the last thousandth");
}

#[test]
fn a_hundred_days_at_once_is_a_hundred_days_one_at_a_time() {
    let mut once = farm_and_mill(2.0, 10.0);
    once.advance(100 * DAY);
    let mut daily = farm_and_mill(2.0, 10.0);
    for day in 1..=100 {
        daily.advance(day * DAY);
    }
    assert_eq!(once.route(0).trips, daily.route(0).trips);
    for t in 0..2 {
        let (a, b) = (once.town(t), daily.town(t));
        assert!((a.money - b.money).abs() < 1e-9, "{}'s money", a.name);
        assert!((a.stock[0] - b.stock[0]).abs() < 1e-9, "{}'s wheat", a.name);
    }
}

// parity: CaravanTests.townsKeepTheirTradingCapital
#[test]
fn a_buyer_keeps_its_trading_capital_and_buys_only_the_excess() {
    // The mill uses 64 wheat a day at 0.05 each, reserve_days 2: its trading capital is 6.4 emeralds. Holding exactly
    // that, it buys nothing from the wagon; with 20 more, it buys with the excess.
    let mut held = farm_and_mill(2.0, 6.4);
    held.advance(20 * DAY);
    assert_eq!(held.route(0).sold, 0.0, "no excess, no sale");
    assert_eq!(held.town(1).stock[0], 0.0, "the mill bought no wheat");

    let mut excess = farm_and_mill(2.0, 26.4);
    excess.advance(20 * DAY);
    assert!(excess.route(0).sold > 0.0, "the excess buys wheat");
    assert!(excess.town(1).money >= 6.4, "and the mill keeps its trading capital");
}

// parity-partial: CaravanTests.aDestinationThatCantPayGetsNothing (no crates in the core yet: the Java test also checks the crated paper rides home)
#[test]
fn a_buyer_with_no_money_buys_nothing_and_the_wagon_brings_it_all_home() {
    let mut s = farm_and_mill(2.0, 0.0);
    let start = s.totals();
    s.advance(20 * DAY);
    assert!(s.route(0).trips >= 1, "the wagon still makes its trips");
    assert_eq!(s.route(0).sold, 0.0, "no money, no wheat sold");
    assert_eq!(s.town(1).stock[0], 0.0, "the mill holds no wheat");
    assert_eq!(s.town(1).money, 0.0, "and pays nothing");
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
}
