//! Llamas (`llamas.rs`): a road a wagon can't take (a river with no bridge) is a llama road; its train is the llamas
//! home, at nine stacks each, away with it and home with it; with none home it stays; a bridge makes it a wagon road;
//! the Merchant brings two and a Farmer breeds more while the roads want them.

use enterprise_core::content::Content;
use enterprise_core::llamas::{self, tend, Llamas};
use enterprise_core::map::Carrier;
use enterprise_core::road::Stay;
use enterprise_core::roster::{Hand, Roster};
use enterprise_core::state::Ledger;
use enterprise_core::{Good, Hex, Output, Society};

const DAY: u64 = 24_000;
const START: Hex = Hex::new(0, 0);
const GOAL: Hex = Hex::new(12, 0);
const FORD: Hex = Hex::new(6, 0);

/// A farm and a mill twelve hexes apart, a river cutting every hex along q = 6 (a fifth of each under water).
fn across_the_river() -> (Society, usize) {
    let goods = vec![Good::new("wheat", 0.05, 64), Good::new("bread", 0.2, 64).food(5.0)];
    let mut s = Society::new(Content::new(goods, DAY));
    s.content_mut().dials.standing_orders = false;
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 40.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 128.0);
    s.place_town(farm, START.center_x() + 3.0, START.center_z() - 2.0);
    s.place_town(mill, GOAL.center_x(), GOAL.center_z());
    for r in -20..=20 {
        s.set_ground(Hex::new(6, r), 0.2, 0.0);
        s.set_river(Hex::new(6, r), true);
    }
    let road = s.add_road(farm, mill, 2.0).expect("a way for llamas");
    (s, road)
}

#[test]
fn a_road_over_an_unbridged_river_goes_by_llama_and_its_train_comes_home() {
    let (mut s, road) = across_the_river();
    assert_eq!(s.route_carrier(road), Carrier::Llama, "no wagon crosses the river");
    assert!(s.route_path(road).unwrap().contains(&FORD), "straight through the ford");
    s.set_llamas(0, 2);
    let (mut left, mut home) = (false, false);
    for day in 1..=20 {
        for o in s.advance(day * DAY) {
            match o {
                Output::WagonLeft { stacks, .. } if !left => {
                    left = true;
                    // As many llamas as its load wants (nine stacks a llama), of the two home.
                    let n = s.crew(road).llamas;
                    assert_eq!(n, llamas::wanted(s.content(), stacks, 0.0).min(2), "{stacks} stacks");
                    assert_eq!(s.crew(road).slots, n * s.content().dials.llama_slots, "nine stacks a llama");
                    let l = s.llamas(0);
                    assert_eq!((l.home, l.away), (2 - n, n), "away with the train");
                }
                Output::WagonHome { .. } if left && !home => home = true,
                _ => {}
            }
        }
        if home {
            break;
        }
    }
    assert!(left && home, "a train went and came home");
    let l = s.llamas(0);
    assert_eq!((l.home, l.away), (2, 0), "home with it");
}

#[test]
fn a_llama_road_with_no_llamas_home_stays() {
    let (mut s, road) = across_the_river();
    for day in 1..=6 {
        s.advance(day * DAY);
    }
    assert_eq!(s.route(road).trips, 0);
    assert!(!s.route(road).on_the_road);
    assert_eq!(s.route(road).stayed, Some(Stay::NoLlamas), "no llamas at home");
}

#[test]
fn a_bridge_over_the_ford_makes_it_a_wagon_road() {
    let (mut s, road) = across_the_river();
    s.bridge(FORD);
    s.advance(DAY);
    assert_eq!(s.route_carrier(road), Carrier::Wagon, "over the bridge by wagon now");
    assert!(s.route_path(road).unwrap().contains(&FORD));
}

#[test]
fn the_merchant_brings_two_and_a_farmer_breeds_more_while_the_roads_want_them() {
    let goods = vec![Good::new("wheat", 0.05, 64)];
    let c = Content::new(goods, DAY);
    let d = &c.dials;
    let hand = |name: &str| Hand::new(&c, c.trade(name).unwrap(), 1);
    let mut ledger = Ledger { made: vec![0.0], used: vec![0.0], delivered: vec![0.0], ..Default::default() };
    let mut stock = vec![100.0];
    let mut l = Llamas::default();
    let merchant_only = Roster::new(&c, vec![hand("merchant")]);
    tend(&c, &mut l, Some(&merchant_only), &mut stock, &mut ledger, 0);
    assert_eq!((l.home, l.seeded), (d.merchant_llamas, true), "the Merchant's two");
    tend(&c, &mut l, Some(&merchant_only), &mut stock, &mut ledger, 0);
    assert_eq!(l.home, 2, "once only");
    // Two roads want three llamas each: a Farmer breeds one every four days, from a hay bale's wheat.
    l.wanted.insert(0, llamas::wanted(&c, 27, 0.0));
    l.wanted.insert(1, 3);
    assert_eq!(l.want(), 6);
    tend(&c, &mut l, Some(&merchant_only), &mut stock, &mut ledger, DAY);
    assert_eq!(l.home, 2, "no Farmer, no breeding");
    let farmed = Roster::new(&c, vec![hand("merchant"), hand("farmer")]);
    tend(&c, &mut l, Some(&farmed), &mut stock, &mut ledger, DAY);
    assert_eq!(l.home, 3);
    assert!((stock[0] - (100.0 - d.llama_breed_wheat)).abs() < 1e-9 && ledger.used[0] == d.llama_breed_wheat);
    tend(&c, &mut l, Some(&farmed), &mut stock, &mut ledger, 3 * DAY);
    assert_eq!(l.home, 3, "not for four days");
    l.away = 2; // a train on the road counts as had
    tend(&c, &mut l, Some(&farmed), &mut stock, &mut ledger, 5 * DAY);
    assert_eq!(l.home, 4);
    tend(&c, &mut l, Some(&farmed), &mut stock, &mut ledger, 9 * DAY);
    assert_eq!(l.home, 4, "six had: enough");
    stock[0] = 5.0;
    l.away = 0;
    tend(&c, &mut l, Some(&farmed), &mut stock, &mut ledger, 13 * DAY);
    assert_eq!(l.home, 4, "no hay, no foal");
}

#[test]
fn a_train_is_what_the_load_wants_of_the_llamas_home() {
    let c = Content::new(vec![Good::new("wheat", 0.05, 64)], DAY);
    assert_eq!(llamas::wanted(&c, 0, 0.0), 1, "at least one");
    assert_eq!(llamas::wanted(&c, 10, 0.0), 2, "ten stacks: two llamas");
    assert_eq!(llamas::wanted(&c, 9, 20.0), 3, "or its weight");
    assert_eq!(llamas::wanted(&c, 200, 0.0), c.dials.llama_train, "a train at most");
    assert_eq!(llamas::train(&c, 27, 0.0, 2), 2, "as far as are home");
    assert_eq!(llamas::team_caps(&c, 0), (c.dials.wagon_slots, c.dials.wagon_weight, c.dials.wagon_pallets), "a wagon");
    assert_eq!(llamas::team_caps(&c, 2), (18, 18.0, 2));
}

#[test]
fn a_town_pays_masons_to_bridge_the_river_its_llamas_ford_and_wagons_take_the_road() {
    let (mut s, road) = across_the_river();
    for hex in [START, Hex::new(1, 0), Hex::new(0, 1), Hex::new(1, -1)] {
        assert!(s.claim_hex(0, hex));
    }
    s.set_llamas(0, 2);
    let before = s.totals();
    let mut bridged = None;
    for day in 1..=60 {
        s.advance(day * DAY);
        let works = s.works(0);
        bridged = works.projects.iter().find_map(|p| match p.what {
            enterprise_core::works::What::Bridge { hex, .. } => Some(hex),
            _ => None,
        });
        if bridged.is_some_and(|h| s.map().ground(h).bridged) && s.route_carrier(road) == Carrier::Wagon {
            break;
        }
    }
    let hex = bridged.expect("a bridge started");
    assert_eq!(hex.q, 6, "on the river: {hex:?}");
    assert!(s.works(0).bridged(hex) && s.map().ground(hex).bridged, "built and on the map");
    assert_eq!(s.route_carrier(road), Carrier::Wagon, "a wagon road now");
    assert!(s.route_path(road).unwrap().contains(&hex), "over the bridge");
    assert!(s.works(1).projects.is_empty(), "the far end builds none: its road isn't the llamas'");
    s.conserved(&before).unwrap();
}
