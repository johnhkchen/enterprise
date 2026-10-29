//! Itineraries and drops (`itinerary-drops`, `docs/pallets.md` section 3): a leg's calls in order with the ticks between
//! (`freight::Itinerary`), each manifest line's drop chosen as its pallet loads (its consignee where a leg ends there,
//! else the first hub ahead that forwards it; no drop, no load), one drop and one pickup function at every call
//! (`cargo::drop_off`, `cargo::pick_up`), and the fit per stretch (`cargo::stretch_load`, `stretch_peak`, `room_to`).
//! Today's routes give today's calls: the freight, cargo and stranded tests keep their numbers, and the random worlds'
//! fingerprint is main's.

use enterprise_core::cargo::{crated, drop_off, fits_to, pick_up, room_to, stretch_load, stretch_peak, wagon_load, Lading, Payment};
use enterprise_core::freight::{Call, Itinerary};
use enterprise_core::pallet::{At, Line, Package, Pallets, CRATE};
use enterprise_core::state::Wagon;
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;
const PAPER: usize = 0;
const BREAD: usize = 1;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0)], DAY)
}

fn look(s: &mut Society) {
    let next = s.now() + s.content().dials.look_ticks;
    s.advance(next);
}

fn until(s: &mut Society, most: u32, done: impl Fn(&Society) -> bool) -> bool {
    for _ in 0..most {
        if done(s) {
            return true;
        }
        look(s);
    }
    done(s)
}

/// Four calls, a hundred ticks apart: towns 1, 2, 3 and 4 (the leg's end).
fn four_calls() -> Itinerary {
    Itinerary { calls: (1..=4).map(|town| Call { town, ticks: 100 }).collect(), next: 0 }
}

#[test]
fn a_leg_sets_out_for_its_end_and_a_call_on_its_way_splits_the_road() {
    let mut it = Itinerary::direct(2, 1000);
    assert_eq!((it.end(), it.on_way().len(), it.next_on_way()), (Some(2), 0, None), "straight to the far end");
    assert_eq!(it.call_on_way(5, 0.4), 400, "four tenths along");
    assert_eq!(it.calls, vec![Call { town: 5, ticks: 400 }, Call { town: 2, ticks: 600 }], "the rest kept for after");
    assert_eq!(it.next_on_way(), Some(Call { town: 5, ticks: 400 }), "bound for the call on its way");
    assert_eq!(it.called_on_way(10, 410), None, "not called yet");
    assert_eq!((it.after(0), it.after(5)), (0, 1), "home: setting out; the stop: the stretch after it");
    assert_eq!(it.made(), 600, "on to the far end");
    assert_eq!(it.next_on_way(), None, "the far end is no call on the way");
    assert_eq!((it.called_on_way(10, 610), it.called_on_way(11, 610)), (Some(5), None), "called there this look only");
    assert_eq!((it.after(5), it.after(2)), (1, 2), "the far end's pickups ride beyond the leg");
    assert_eq!((it.ride_to(0, Some(5)), it.ride_to(0, Some(2)), it.ride_to(1, Some(5)), it.ride_to(0, Some(0))), (0, 1, 2, 2),
        "a drop behind it, or on a later leg, rides beyond");
}

#[test]
fn a_pallet_drops_at_its_consignees_leg_end_else_the_first_hub_ahead_that_forwards_it() {
    // Out from home 0 to the far end 2, calling at the hub 5 on the way (and on the way back).
    let mut out = Itinerary::direct(2, 1000);
    out.call_on_way(5, 0.4);
    let drop = |to, from, forwards: &dyn Fn(usize) -> bool| out.drop_for(to, from, true, 2, 0, forwards);
    assert_eq!(drop(2, 0, &|_| true), Some(2), "made out to the far end: delivered there, whoever forwards");
    assert_eq!(drop(0, 0, &|_| true), Some(0), "made out home: the way back's end");
    assert_eq!(drop(9, 0, &|h| h == 5 || h == 2), Some(5), "beyond: the first hub ahead that forwards it");
    assert_eq!(drop(9, 0, &|h| h == 2), Some(2), "the hub at the far end");
    assert_eq!(drop(9, 1, &|h| h == 5), Some(5), "past the stop out: the stop again, on the way back");
    assert_eq!(drop(5, 0, &|h| h == 2), Some(2), "made out to the stop: a call on the way receives nothing, so on through");
    assert_eq!(drop(9, 0, &|h| h == 0), None, "nothing is handed on at home");
    assert_eq!(drop(9, 0, &|_| false), None, "no hub forwards it: no drop, and it doesn't load");
    // The leg home: nothing beyond home.
    let mut back = Itinerary::direct(0, 1000);
    back.call_on_way(5, 0.6);
    assert_eq!(back.drop_for(2, 0, false, 0, 0, |_| false), None, "the far end is behind it");
    assert_eq!(back.drop_for(9, 0, false, 0, 0, |h| h == 5), Some(5));
    assert_eq!(back.drop_for(9, 1, false, 0, 0, |h| h == 5), None, "past the stop home");
    // A leg with no calls written goes to its end.
    assert_eq!(Itinerary::default().drop_for(2, 0, true, 2, 0, |_| false), Some(2));
}

#[test]
fn four_calls_drop_each_line_at_its_call_and_a_drop_frees_room_for_a_later_pickup() {
    let c = content();
    let (route, home) = (7, 0);
    let mut p = Pallets::default();
    let pack = |p: &mut Pallets, at: usize, to: usize, n: f64| {
        p.pack(Lading { payment: Payment::Paid, ..Lading::new(at, to, 0) }, vec![Package::new(CRATE, PAPER, n)], At::Town(at), 0)
    };
    // Setting out with three stacks of room: two stacks for the second call, one for the fourth.
    let (for_2, for_4) = (pack(&mut p, home, 2, 128.0), pack(&mut p, home, 4, 64.0));
    let mut w = Wagon { itinerary: four_calls(), slots: 3, ..Default::default() };
    for (id, to) in [(for_2, 2), (for_4, 4)] {
        assert!(pick_up(&c, &mut p, &mut w, route, Line { pallet: id, drop: Some(to), ..Default::default() }, home, 0, 0));
    }
    let none = pack(&mut p, home, 9, 1.0);
    assert!(!pick_up(&c, &mut p, &mut w, route, Line { pallet: none, ..Default::default() }, home, 0, 0), "no drop, no load");
    assert_eq!(p.get(none).unwrap().at, At::Town(home));
    // The load on each stretch: the road into the first and second calls carries both, the rest only the fourth's.
    let slots: Vec<u32> = (0..4).map(|k| stretch_load(&c, &p, route, &w, 0, k).slots).collect();
    assert_eq!(slots, vec![3, 3, 1, 1]);
    assert_eq!(stretch_peak(&c, &p, route, &w, 0, Some(4)), wagon_load(&c, &p, route, &w), "the first stretch binds");
    // The first call: nothing drops there, and a stack for the fourth won't fit the road into the second.
    let at_1 = w.itinerary.after(1);
    assert_eq!(at_1, 1);
    let early = pack(&mut p, 1, 4, 64.0);
    assert_eq!(room_to(&c, &p, route, &w, at_1, Some(4)).slots, 0);
    assert!(!fits_to(&c, &p, route, &w, p.get(early).unwrap(), at_1, Some(4)));
    assert!(drop_off(&mut p, &mut w, route, 1, false, 0, |_, _| true).is_empty(), "nothing drops at the first");
    assert!(!pick_up(&c, &mut p, &mut w, route, Line { pallet: early, drop: Some(4), ..Default::default() }, 1, at_1, 100));
    w.itinerary.made();
    // The second call: its line drops.
    assert_eq!(drop_off(&mut p, &mut w, route, 2, false, 200, |_, _| true), vec![for_2]);
    p.open(for_2).expect("delivered there");
    w.itinerary.made();
    // The third: two stacks for the fourth fit, only because the second's drop freed the room.
    let at_3 = w.itinerary.after(3);
    let later = pack(&mut p, 3, 4, 128.0);
    assert_eq!(room_to(&c, &p, route, &w, at_3, Some(4)).slots, 2);
    assert!(pick_up(&c, &mut p, &mut w, route, Line { pallet: later, drop: Some(4), ..Default::default() }, 3, at_3, 300));
    assert!(p.get(later).unwrap().seal.is_some_and(|s| s.by == 3), "sealed as it left the third's");
    w.itinerary.made();
    // The fourth: both its lines, in the order aboard.
    assert_eq!(drop_off(&mut p, &mut w, route, 4, false, 400, |_, _| true), vec![for_4, later]);
    assert!(w.manifest.is_empty(), "nothing left on the manifest");
    assert_eq!(p.at(At::Wagon(route)), &[for_4, later], "still aboard in the registry, for the call to deliver");
}

#[test]
fn a_call_drops_only_what_it_takes_and_the_trips_end_takes_everything() {
    let c = content();
    let (route, home) = (3, 0);
    let mut p = Pallets::default();
    let pack = |p: &mut Pallets, to: usize| p.pack(Lading::new(home, to, 0), vec![Package::new(CRATE, PAPER, 64.0)], At::Town(home), 0);
    let mut w = Wagon { itinerary: Itinerary::direct(2, 100), ..Default::default() };
    let (a, b, d) = (pack(&mut p, 2), pack(&mut p, 2), pack(&mut p, 0));
    for (id, drop) in [(a, 2), (b, 2), (d, 0)] {
        assert!(pick_up(&c, &mut p, &mut w, route, Line { pallet: id, drop: Some(drop), ..Default::default() }, home, 0, 0));
    }
    assert_eq!(drop_off(&mut p, &mut w, route, 2, false, 1, |x, _| x.id == b), vec![b], "the call takes what it takes");
    p.open(b).expect("delivered");
    assert_eq!(w.manifest.lines.iter().map(|l| l.pallet).collect::<Vec<_>>(), vec![a, d], "the rest ride on");
    assert_eq!(drop_off(&mut p, &mut w, route, 0, true, 2, |_, _| true), vec![a, d], "home, the trip's end: all of it");
}

/// A town and a hub two blocks apart, and a market 200 blocks beyond the hub that the hub runs to
/// (`tests/freight.rs` `network`).
fn network() -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    s.content_mut().dials.standing_orders = false;
    let town = s.add_town("Town", 5.0);
    let hub = s.add_town("Hub", 30.0);
    let beyond = s.add_town("Beyond", 5.0);
    s.add_route(town, hub, 2.0 / 1200.0, 0.0, 4.0);
    s.add_route(hub, beyond, 200.0 / 1200.0, 0.0, 4.0);
    s.make_hub(hub);
    s
}

#[test]
fn goods_for_a_market_beyond_drop_at_the_hub_that_forwards_them_and_ride_its_run_to_their_consignee() {
    let mut s = network();
    let (shipper, hub, buyer) = (0, 1, 2);
    s.set_trade(buyer, PAPER, 0.0, 30.0);
    for t in [shipper, hub, buyer] {
        s.deliver(t, BREAD, 128.0);
    }
    s.deliver(shipper, PAPER, 256.0);
    s.deliver(hub, PAPER, 64.0);
    s.draft(shipper, buyer, PAPER, 64.0, 1.0 / 24.0, 0, 7 * DAY, 4, Incoterm::Dap, true);
    // The shipper's wagon leaves with the pallet for the buyer: its line drops at the hub, the far end, which runs on there.
    assert!(until(&mut s, 40, |s| s.wagon_manifest(0).is_some_and(|m| !m.lines.is_empty())), "a pallet aboard");
    let it = s.wagon_itinerary(0).unwrap();
    assert_eq!(it.calls.iter().map(|c| c.town).collect::<Vec<_>>(), vec![hub], "a bare route: the far end its one call");
    let m = s.wagon_manifest(0).unwrap();
    let line = &m.lines[0];
    assert!(s.pallets().get(line.pallet).is_some_and(|p| p.consigned_to(buyer)));
    assert_eq!(line.drop, Some(hub), "dropped at the hub that forwards it");
    // The hub's run picks it up booked, dropping at its run's end, the buyer.
    assert!(until(&mut s, 400, |s| s.wagon_manifest(1).is_some_and(|m| m.lines.iter().any(|l| l.booking.is_some()))),
        "the hub's run loads it, booked");
    let m = s.wagon_manifest(1).unwrap();
    assert!(m.lines.iter().filter(|l| l.booking.is_some()).all(|l| l.drop == Some(buyer)), "{m:?}");
    let start = s.totals();
    for day in 1..=10 {
        s.advance(s.now() + DAY);
        s.conserved(&start).unwrap_or_else(|e| panic!("day {day}: {e}"));
    }
}

#[test]
fn a_crate_fetched_at_the_far_end_for_home_drops_at_home() {
    // (`tests/cargo.rs`' goods: no road food, so the wagon goes for what it's owed.)
    let mut s = Society::new(Content::new(["paper", "flint", "iron_axe"].iter().map(|n| Good::new(n, 0.05, 64)).collect(), DAY));
    s.content_mut().dials.road_cost_share = 1.0; // the mod's road food
    s.add_town("Haygate", 5.0);
    s.add_town("Cloverby", 5.0);
    s.add_route(0, 1, 1.0, 0.0, 30.0);
    let id = s.deliver_pallet(1, Lading { payment: Payment::Paid, ..Lading::new(1, 0, 0) }, crated(s.content(), 2, 9.0))
        .expect("held");
    assert!(until(&mut s, 40, |s| s.wagon_manifest(0).is_some_and(|m| m.line(id).is_some())), "fetched");
    assert_eq!(s.wagon_manifest(0).unwrap().line(id).unwrap().drop, Some(0), "for home, the call after the far end");
    let it = s.wagon_itinerary(0).unwrap();
    assert!(it.end() == Some(0) || it.end() == Some(1), "{it:?}");
    assert!(until(&mut s, 60, |s| s.route(0).trips >= 1));
    assert!(s.pallets().get(id).is_none(), "home and opened");
}
