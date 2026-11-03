//! Bonds (`bonds.rs`): a town short of a work's money offers a bond, a town with money idle buys it where a wagon
//! between the two calls, and the issuer repays it and its coupon over its term the same way; the capital's royal bonds
//! pave the most worn trails.

use enterprise_core::bonds::{Purpose, State};
use enterprise_core::works::{Kind, What};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const PAPER: usize = 0;
const BREAD: usize = 1;

/// A hub selling paper to a far end six hexes east, a wagon every day; each with `hub` and `dest` emeralds.
fn road(hub_money: f64, dest_money: f64) -> (Society, usize, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0)],
        DAY));
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", hub_money);
    let dest = s.add_town("Dest", dest_money);
    let (a, b) = (Hex::new(0, 0), Hex::new(6, 0));
    s.place_town(hub, a.center_x(), a.center_z());
    s.place_town(dest, b.center_x(), b.center_z());
    s.record_capacity(dest, 1, &[(PAPER, 640.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 6400.0);
    s.deliver(hub, BREAD, 64.0);
    s.deliver(dest, BREAD, 64.0);
    s.set_roster(hub, &[("merchant", 1), ("librarian", 2), ("farmer", 2)]);
    s.set_offers(hub, 1, Vec::new());
    s.add_road(hub, dest, 1.0).expect("a road over the map");
    (s, hub, dest)
}

/// A farm town making wheat and a mill town whose people use it, a wagon every two days (`trade_loop.rs`'s); the farm
/// starts with `farm` emeralds, the mill with `mill`.
fn farm_and_mill(farm_money: f64, mill_money: f64) -> (Society, usize, usize) {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64)], DAY));
    s.content_mut().dials.standing_orders = false;
    let farm = s.add_town("Wheatfield", farm_money);
    let mill = s.add_town("Hayford", mill_money);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.add_route(farm, mill, 1.0, 0.5, 2.0);
    (s, farm, mill)
}

#[test]
fn a_wagons_town_buys_the_far_ends_bond_and_is_repaid_with_its_coupon() {
    let (mut s, farm, mill) = farm_and_mill(400.0, 200.0);
    let before = s.totals();
    let b = s.offer_bond(mill, Purpose::Hall(Kind::Library), 20.0);
    s.advance(3 * DAY);
    let bond = s.bonds()[b].clone();
    assert_eq!((bond.state, bond.holder), (State::Sold, Some(farm)), "the farm's wagon bought it at the mill: {bond:?}");
    let term = s.content().dials.bond_term_days;
    s.advance((term as u64 + 6) * DAY);
    let bond = s.bonds()[b].clone();
    assert_eq!(bond.state, State::Repaid, "repaid as the farm's wagons called: {bond:?}");
    assert!(bond.repaid.to_f64() >= 22.0 - 1e-9, "its coupon besides: {bond:?}");
    s.conserved(&before).unwrap();
}

#[test]
fn the_far_end_buys_the_wagons_towns_bond_out_of_its_treasury() {
    let (mut s, farm, mill) = farm_and_mill(20.0, 400.0);
    let before = s.totals();
    let b = s.offer_bond(farm, Purpose::Waystation, 15.0);
    s.advance(3 * DAY);
    let bond = s.bonds()[b].clone();
    assert_eq!((bond.state, bond.holder), (State::Sold, Some(mill)), "the mill bought it, its money home in the purse: {bond:?}");
    let term = s.content().dials.bond_term_days;
    s.advance((term as u64 + 6) * DAY);
    assert_eq!(s.bonds()[b].state, State::Repaid, "the farm's purse repaid it at the mill: {:?}", s.bonds()[b]);
    s.conserved(&before).unwrap();
}

#[test]
fn a_town_with_none_idle_buys_nothing_and_an_offer_lapses() {
    let (mut s, farm, _) = farm_and_mill(20.0, 15.0);
    let b = s.offer_bond(farm, Purpose::Hall(Kind::Library), 20.0);
    s.advance((s.content().dials.bond_offer_days as u64 + 2) * DAY);
    assert_eq!(s.bonds()[b].state, State::Lapsed, "the mill had nothing idle: {:?}", s.bonds()[b]);
}

#[test]
fn a_used_stop_short_of_its_level_offers_a_bond_for_what_it_lacks() {
    let (mut s, _, _) = road(400.0, 200.0);
    let inn = s.add_town("Inkwell", 50.0);
    s.place_town(inn, 999_000.5, 999_000.5);
    let stop = Hex::new(3, 0);
    let way = What::Waystation { hex: stop };
    s.start_project(inn, way);
    s.finish_project(inn, way);
    s.content_mut().dials.bond_stop_passes = 4;
    s.advance(12 * DAY);
    let st = s.waystations();
    let bond = s.bonds().into_iter().find(|b| b.issuer == inn).unwrap_or_else(|| panic!("Inkwell borrows for its stop once wagons pass it: {st:?}"));
    assert!(matches!(bond.purpose, Purpose::StopLevel(h) if h == stop), "{bond:?}");
    // Inkwell has no road: the hub's wagon never calls there, so it isn't sold.
    assert_eq!(bond.state, State::Offered);
    assert!((bond.principal.to_f64() - (enterprise_core::waystations::rise_cost(2) - st[0].banked.to_f64())).abs() < 1.0,
        "for what it lacks of its level's cost: {bond:?}");
}

#[test]
fn the_crowns_royal_bonds_pave_the_most_worn_trail() {
    let (mut s, hub, dest) = road(400.0, 400.0);
    s.name_capital(hub, 0);
    s.advance(12 * DAY);
    let royal: Vec<_> = s.bonds().into_iter().filter(|b| b.purpose == Purpose::Highways).collect();
    assert!(royal.iter().any(|b| b.holder == Some(dest)), "Dest bought the crown's bonds: {royal:?}");
    let (_, laid) = s.highways();
    assert!(laid > 0, "the king's highways laid: {royal:?}");
    let surfaced = s.route_path(0).unwrap().into_iter().filter(|&h| s.map().surface(h) > 0).count();
    assert!(surfaced > 0, "on the hub's road");
}
