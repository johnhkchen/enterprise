//! The sweep (`docs/core-plan.md` 5.1): the seeds run across the cores in their order, a failing seed is named, the
//! sweep's invariants (`common/slices/random_worlds_sweep.rs`) hold in a plain world and catch each kind of stall
//! they're for, and a sweep's tally counts what a world did. No Java: it hardens every slice's claims.

use crate::common;

use common::slices::random_worlds_sweep as sweep_checks;
use common::DAY;
use enterprise_core::contracts::Book;
use enterprise_core::state::{People, RouteState, Towns, Wagon};
use enterprise_core::sweep::{mood_floor, Sweep};
use enterprise_core::{Content, Dials, Good, Incoterm, Society};

/// A farm town making wheat and a mill town whose people use it, a day apart, a wagon every two days.
fn farm_and_mill() -> Society {
    let mut s = Society::new(Content::new(vec![Good::new("wheat", 0.05, 64)], DAY));
    let farm = s.add_town("Wheatfield", 20.0);
    let mill = s.add_town("Hayford", 50.0);
    s.set_people(farm, 6.0, 6.0, 12.0);
    s.set_people(mill, 6.0, 6.0, 12.0);
    s.set_trade(farm, 0, 128.0, 0.0);
    s.set_trade(mill, 0, 0.0, 64.0);
    s.add_route(farm, mill, 1.0, 0.5, 2.0);
    s
}

#[test]
fn each_seed_gives_the_results_in_the_seeds_order() {
    assert_eq!(common::each_seed([5, 3, 9, 1], |s| s * 2), vec![10, 6, 18, 2]);
    let squares = common::each_seed(0..500, |s| s * s);
    assert_eq!(squares, (0..500).map(|s| s * s).collect::<Vec<u64>>());
    assert!(common::each_seed(std::iter::empty(), |s| s).is_empty());
}

#[test]
#[should_panic(expected = "seed 7 failed: boom at 7")]
fn a_failing_seed_is_named() {
    common::each_seed(0..40, |s| assert!(s != 7, "boom at {s}"));
}

#[test]
fn the_mood_floor_is_its_size_its_crowding_its_unpaid_wages_and_its_grief() {
    let d = Dials::default();
    let people = People { villagers: 16.0, beds: 10.0, ..Default::default() };
    // 16 / 8 villagers a want, every one of the 16 without a bed at half a point (beds can come faster than mood
    // follows), 2 for wages unpaid, 2 for grief at its most.
    assert_eq!(mood_floor(&d, &people), -2.0 - 8.0 - 2.0 - 2.0);
    // Its beds don't move it: 32 villagers, a bed each or none, -24.
    assert_eq!(mood_floor(&d, &People { villagers: 32.0, beds: 32.0, ..Default::default() }), -24.0);
}

#[test]
fn a_plain_world_passes_the_sweeps_checks_and_its_tally_counts_it() {
    let mut s = farm_and_mill();
    let id = s.draft(0, 1, 0, 16.0, 0.05, 2 * DAY, DAY, 3, Incoterm::Dap, true);
    let start = s.totals();
    let mut sweep = Sweep::new();
    for day in 1..=40 {
        s.advance(day * DAY);
        s.conserved(&start).and_then(|_| s.solvent()).and_then(|_| sweep_checks::check(&s, day))
            .unwrap_or_else(|e| panic!("day {day}: {e}"));
        sweep.day(&s);
    }
    sweep.end(&s);
    assert_eq!((sweep.worlds, sweep.days, sweep.towns, sweep.routes), (1, 40, 2, 1));
    assert_eq!(sweep.trips, s.route(0).trips as u64);
    assert!(sweep.trips >= 10, "a wagon every two days: {sweep:?}");
    assert_eq!(s.contract(id).state, enterprise_core::State::Completed, "{:?}", s.contract(id));
    assert_eq!(sweep.contracts[2] as usize, s.contracts().iter().filter(|c| c.state == enterprise_core::State::Completed).count());
    // Every route-day is on the road or home for a reason.
    assert_eq!(sweep.idle.values().sum::<u64>() + sweep.road_days, 40);
    assert!(sweep.report().contains("1 worlds, 40 days: 2 towns, 1 routes"), "{}", sweep.report());
    let mut other = Sweep::new();
    other.merge(sweep.clone());
    other.merge(sweep);
    assert_eq!((other.worlds, other.days, other.moods.len()), (2, 80, 4));
}

/// The plain world after `days`, with the route's state changed by `f`.
fn with_route(days: u64, f: impl Fn(&mut RouteState)) -> Society {
    let mut s = farm_and_mill();
    s.advance(days * DAY);
    let mut q = s.world.query::<&mut RouteState>();
    for mut state in q.iter_mut(&mut s.world) {
        f(&mut state);
    }
    s
}

#[test]
fn a_wagon_out_a_month_is_a_stall() {
    let s = with_route(40, |st| {
        st.wagon = Some(Wagon { arrives: 50 * DAY, ..Default::default() });
        st.last_departure = Some(5 * DAY);
    });
    let e = sweep_checks::stalls(&s).unwrap_err();
    assert!(e.contains("route 0: its wagon out 35 days"), "{e}");
    // Out 29 days, it's on its way.
    let s = with_route(40, |st| {
        st.wagon = Some(Wagon { arrives: 50 * DAY, ..Default::default() });
        st.last_departure = Some(11 * DAY);
    });
    assert!(sweep_checks::stalls(&s).is_ok());
}

#[test]
fn a_route_home_with_no_word_why_is_a_stall() {
    let s = with_route(10, |st| {
        st.wagon = None;
        st.stayed = None;
    });
    assert!(s.route(0).trips > 0);
    assert!(sweep_checks::stalls(&s).unwrap_err().contains("no word why it stays"));
    // A route never planned yet (opened since the look), or wound down for a hub, says nothing and stalls nothing.
    let s = with_route(10, |st| {
        st.wagon = None;
        st.stayed = None;
        st.trips = 0;
        st.last_departure = None;
    });
    assert!(sweep_checks::stalls(&s).is_ok());
    let s = with_route(10, |st| {
        st.wagon = None;
        st.stayed = None;
        st.freight.wound_down = Some(5 * DAY);
    });
    assert!(sweep_checks::stalls(&s).is_ok());
}

#[test]
fn a_contract_left_unjudged_past_its_deadline_is_caught() {
    let mut s = farm_and_mill();
    s.draft(1, 0, 0, 16.0, 0.05, 2 * DAY, DAY, 2, Incoterm::Dap, true); // the mill sells the farm wheat it hasn't
    s.advance(20 * DAY);
    assert!(sweep_checks::contracts(&s).is_ok(), "judged: {:?}", s.contract(0));
    // Judging that stopped: three days without it is caught.
    s.world.resource_mut::<Book>().reviewed = Some(17);
    assert!(sweep_checks::contracts(&s).unwrap_err().contains("last judged on day Some(17)"));
    // A contract in force whose judging lags (its last copy came today): the next look judges it, so it passes.
    let mut book = s.world.resource_mut::<Book>();
    book.reviewed = Some(20);
    book.contracts[0].judged = 0;
    book.contracts[0].missed = 0;
    book.contracts[0].state = enterprise_core::State::Active;
    if !book.open.contains(&0) {
        book.open.push(0);
    }
    drop(book);
    assert!(sweep_checks::contracts(&s).is_ok());
    // Money on a contract below nothing is caught.
    let mut book = s.world.resource_mut::<Book>();
    book.contracts[0].paid = enterprise_core::Fixed::from(-1.0);
    drop(book);
    assert!(sweep_checks::contracts(&s).unwrap_err().contains("paid -1"));
}

#[test]
fn a_mood_below_its_floor_or_wages_left_owed_are_caught() {
    let mut s = farm_and_mill();
    s.advance(5 * DAY);
    assert!(sweep_checks::mood(&s).is_ok() && sweep_checks::wages(&s).is_ok());
    let town = s.world.resource::<Towns>().0[1];
    s.world.get_mut::<People>(town).unwrap().mood = -12.0; // its floor: -6/8 - 6/2 - 2 - 2
    assert!(sweep_checks::mood(&s).unwrap_err().contains("town 1: mood -12 below its floor -7.75"));
    s.world.get_mut::<People>(town).unwrap().mood = 0.0;
    s.world.get_mut::<People>(town).unwrap().paid_share = 1.5;
    assert!(sweep_checks::wages(&s).unwrap_err().contains("paid share 1.5"));
}
