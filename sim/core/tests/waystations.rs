//! Waystations' keep (`waystations.rs`): a passing wagon pays a stop for water and a rest, and its crew's meals where
//! it grows food; a stop rises a level out of what it banked, its keepers putting it up in a few days.

use enterprise_core::waystations::{fee, food_kept, rise_cost, rise_days};
use enterprise_core::works::What;
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const PAPER: usize = 0;
const BREAD: usize = 1;

/// A hub selling paper to a far end six hexes east, wagons every day, its road past Inkwell's stop on the hex halfway.
fn road() -> (Society, usize, Hex) {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0)],
        DAY));
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 40.0);
    let dest = s.add_town("Dest", 200.0);
    let inn = s.add_town("Inkwell", 50.0);
    let (a, b, stop) = (Hex::new(0, 0), Hex::new(6, 0), Hex::new(3, 0));
    s.place_town(hub, a.center_x(), a.center_z());
    s.place_town(dest, b.center_x(), b.center_z());
    s.place_town(inn, 999_000.5, 999_000.5);
    s.record_capacity(dest, 1, &[(PAPER, 640.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 6400.0);
    s.deliver(hub, BREAD, 64.0);
    s.deliver(dest, BREAD, 64.0);
    s.set_roster(hub, &[("merchant", 1), ("librarian", 2)]);
    s.set_offers(hub, 1, Vec::new()); // trained: it keeps its trade without a workstation
    s.add_road(hub, dest, 1.0).expect("a road over the map");
    let way = What::Waystation { hex: stop };
    s.start_project(inn, way);
    s.finish_project(inn, way);
    (s, inn, stop)
}

#[test]
fn a_wagon_passing_a_stop_pays_for_water_and_a_rest() {
    let (mut s, inn, stop) = road();
    let before = s.totals();
    let money = s.town(inn).money.to_f64();
    s.advance(10 * DAY);
    let st = s.waystations().into_iter().find(|w| w.hex == stop).expect("on the books");
    assert_eq!(st.level, 1);
    assert!(st.passes > 0, "the hub's wagons stopped: {st:?}; {} trips, whys {:?}", s.route(0).trips, s.whys(0).iter().map(|w| w.line.clone()).collect::<Vec<_>>());
    assert!((st.taken.to_f64() - st.passes as f64 * 2.0 * fee(1)).abs() < 1e-6, "a stop's fee each way: {st:?}");
    assert_eq!(st.meals, 0.0, "a stop grows no food");
    assert!((s.town(inn).money.to_f64() - money - st.taken.to_f64()).abs() < 1e-6, "paid to Inkwell's treasury, banked to its stop");
    s.conserved(&before).unwrap();
}

#[test]
fn a_stop_rises_a_level_out_of_its_takings_and_sells_meals() {
    let (mut s, inn, stop) = road();
    s.advance(1_200); // its first look: the stop on the books
    s.bank_at_waystation(stop, rise_cost(2));
    let money = s.town(inn).money.to_f64();
    s.advance(DAY);
    let st = s.waystations().into_iter().find(|w| w.hex == stop).unwrap();
    assert!(st.rising.is_some() && st.level == 1, "its keepers putting up a waystation: {st:?}");
    assert!(s.town(inn).money.to_f64() <= money - rise_cost(2) + st.taken.to_f64() + 1e-6, "out of Inkwell's treasury");
    s.advance((1.0 + rise_days(2)) as u64 * DAY);
    let st = s.waystations().into_iter().find(|w| w.hex == stop).unwrap();
    assert_eq!(st.level, 2, "a waystation: {st:?}");
    assert!(st.meals > 0.0 && st.food <= food_kept(2), "its garden's meals sold to the crews: {st:?}");
}
