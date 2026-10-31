//! The market (`market.rs`, `docs/markets-plan.md`): a town's bid comes from its use of a good and creeps while
//! unfilled; a wagon sells to a bid at it and marks the price; an input is bid for at what its output would fetch less
//! the other inputs; and a crop is tended for a bid down the road.

use std::collections::BTreeMap;

use enterprise_core::farms::{plan, Wants};
use enterprise_core::market::{rebid, uses, Use};
use enterprise_core::{Content, Good, Output, Society};

const DAY: u64 = 24_000;

#[test]
fn an_unfilled_bid_creeps_to_its_ceiling_and_a_filled_one_holds() {
    let paper = Use { want: 30.0, ceiling: 0.1 };
    let first = rebid(paper, 0.0, false, 0.04, 0.1);
    assert!((first - 0.02).abs() < 1e-12, "a new bid starts at the seller's floor, half its worth: {first}");
    let second = rebid(paper, first, false, 0.04, 0.1);
    assert!((second - 0.022).abs() < 1e-12, "unfilled, a tenth higher: {second}");
    assert_eq!(rebid(paper, second, true, 0.04, 0.1), second, "filled, it holds");
    assert_eq!(rebid(paper, 0.099, false, 0.04, 0.1), 0.1, "never over the ceiling");
    assert_eq!(rebid(Use { want: 0.0, ..paper }, 0.05, false, 0.04, 0.1), 0.0, "none wanted, no bid");
}

#[test]
fn an_input_is_bid_for_at_what_its_output_fetches_less_the_other_inputs() {
    // Cane, paper, leather, book: a book of 3 paper and a leather, a paper of a cane. The town wants 10 books at up to
    // half an emerald each, holds 6 paper, and prices leather at its mark of 1/6.
    let (cane, paper, leather, book) = (0, 1, 2, 3);
    let mut direct = vec![Use::default(); 4];
    direct[book] = Use { want: 10.0, ceiling: 0.5 };
    let conversions = vec![(vec![(paper, 3.0), (leather, 1.0)], book, 1.0), (vec![(cane, 1.0)], paper, 1.0)];
    let mark = |g: usize| if g == leather { 1.0 / 6.0 } else { 0.0 };
    let u = uses(direct, &conversions, &[0.0, 6.0, 0.0, 0.0], mark);
    let each = (0.5 - 1.0 / 6.0) / 3.0;
    assert!((u[paper].ceiling - each).abs() < 1e-12 && u[paper].want == 24.0, "30 for ten books, 6 held: {:?}", u[paper]);
    assert!((u[cane].ceiling - each).abs() < 1e-12 && u[cane].want == 24.0, "a cane for each paper wanted: {:?}", u[cane]);
    assert_eq!(u[leather].want, 10.0, "a leather a book");
}

/// A bakery with cookies to spare and a town of ten short of treats, a day apart; the road opened on day 10.
fn bakery_and_town() -> (Society, usize, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("cookie", 1.0 / 18.0, 64).comfort(0)], DAY));
    s.content_mut().dials.standing_orders = false;
    s.content_mut().dials.eager_most = 1.0;
    let bakery = s.add_town("Crumbton", 20.0);
    let town = s.add_town("Hayford", 500.0);
    s.set_people(town, 10.0, 10.0, 20.0);
    s.set_trade(bakery, 0, 128.0, 0.0);
    s.deliver(bakery, 0, 200.0);
    (s, bakery, town)
}

#[test]
fn a_town_short_of_a_comfort_bids_more_each_day_and_a_wagon_sells_to_it_at_the_bid() {
    let (mut s, bakery, town) = bakery_and_town();
    let worth = 1.0 / 18.0;
    s.advance(DAY / 2);
    let (first, want) = s.bid(town, 0).expect("the town bids for treats");
    assert!((first - worth / 2.0).abs() < 1e-9 && want > 0.0, "at the seller's floor: {first}, {want} wanted");
    s.advance(5 * DAY + DAY / 2);
    let (later, _) = s.bid(town, 0).expect("still short");
    assert!(later > first * 1.4, "five days unfilled, higher: {first} to {later}");
    s.advance(9 * DAY + DAY / 2);
    let (top, _) = s.bid(town, 0).unwrap();
    assert!(top > worth, "over its worth: its people, with none, would pay up to twice it: {top}");
    // The road opens: the bakery's wagon sells at the bid, over the cookie's worth, and marks the price.
    s.add_route(bakery, town, 1.0, 0.5, 2.0);
    let mut sale = None;
    for day in 10..=20 {
        for out in s.advance(day * DAY) {
            if let Output::WagonSold { sold, takings, .. } = out {
                if sold > 0.0 && sale.is_none() {
                    sale = Some((sold, takings));
                }
            }
        }
        if sale.is_some() {
            break;
        }
    }
    let (sold, takings) = sale.expect("a sale");
    let each = s.mark(town, 0);
    assert!(each > worth && takings.to_f64() >= sold * worth, "{sold} sold for {takings}, {each} each");
}

#[test]
fn a_good_worth_only_its_worth_to_a_town_is_bargained_for_not_bid() {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64)], DAY));
    let mill = s.add_town("Hayford", 500.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.advance(3 * DAY);
    assert_eq!(s.bid(mill, 0), None, "its people eat wheat at its worth: no bid over a bargain");
}

#[test]
fn a_farm_tends_cane_for_paper_bid_for_down_the_road_before_wheat_at_its_worth() {
    let c = Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("sugar_cane", 1.0 / 24.0, 64),
        Good::new("paper", 1.0 / 24.0, 64)], DAY);
    let (wheat, cane, paper) = (0, 1, 2);
    let plots: BTreeMap<usize, u32> = [(wheat, 64), (cane, 64)].into();
    let mut wants = Wants { demand: [(wheat, 100.0)].into(), ..Default::default() };
    let mut idle = BTreeMap::new();
    let (by_worth, _) = plan(&c, &plots, &[1], &wants, 1.0, true, &mut idle);
    assert_eq!(by_worth.get(&wheat).copied(), Some(16.0), "one Novice, 32 plots, all wheat: {by_worth:?}");
    // A market down the road bids a tenth of an emerald for 16 paper a day, after the haul: cane earns more than wheat.
    wants.draws.insert(paper, (16.0, 0.1));
    let (by_bid, _) = plan(&c, &plots, &[1], &wants, 1.0, true, &mut idle);
    assert_eq!(by_bid.get(&cane).copied(), Some(16.0), "all 32 plots to cane: {by_bid:?}");
    // And a town that bids for wheat itself over the paper's price tends wheat again.
    wants.prices.insert(wheat, 0.2);
    let (by_price, _) = plan(&c, &plots, &[1], &wants, 1.0, true, &mut idle);
    assert_eq!(by_price.get(&wheat).copied(), Some(16.0), "{by_price:?}");
}
