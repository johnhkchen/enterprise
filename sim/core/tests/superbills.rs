//! Superbills (`superbills`, `docs/pallets.md` section 1 "Superbills", divergence 5): a container's manifest, written as it
//! leaves, drawn as a document when it holds two lines or more; a hub's consolidation of the pallets booked on one run for
//! one next stop, loaded, carried and handed out whole, then dissolved; drops by manifest line.

use enterprise_core::cargo::{crated, Lading, Payment, LADING, SUPERBILL};
use enterprise_core::pallet::{At, Line, Manifest, Package, Pallets, CRATE};
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;
const PAPER: usize = 0;
const BREAD: usize = 1;
const CLOTH: usize = 2;
const TOOLS: usize = 3;

fn content() -> Content {
    Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("cloth", 0.5, 64), Good::new("tools", 1.0, 64)], DAY)
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

fn superbills(s: &Society, town: usize) -> u64 {
    s.paperwork(town).by_kind.get(SUPERBILL).copied().unwrap_or(0)
}

/// The same goods with no road food (`tests/cargo.rs`' content): a wagon needs nothing to eat.
fn plain() -> Content {
    Content::new(["paper", "bread", "cloth", "tools"].iter().map(|n| Good::new(n, 0.05, 64)).collect(), DAY)
}

/// A town and its buyer a day apart, the town's wagon every 30 days.
fn seller_and_buyer(money: f64) -> Society {
    let mut s = Society::new(plain());
    s.add_town("Haygate", money);
    s.add_town("Cloverby", money);
    s.add_route(0, 1, 1.0, 0.0, 30.0);
    s
}

/// A crate of 64 of a good, paid for, from `shipper` made out to `buyer`.
fn crate_of(good: usize, shipper: usize, buyer: usize) -> (Lading, Vec<Package>) {
    (Lading { payment: Payment::Paid, ..Lading::new(shipper, buyer, 0) }, crated(&content(), good, 64.0))
}

/// The freight tests' network (`tests/freight.rs` `network`): a town and the hub two blocks apart, a third town 200
/// blocks beyond the hub; `onward` a route from the hub to it, else one from it to the hub.
fn network(onward: bool, hub_emeralds: f64) -> Society {
    let mut s = Society::new(content());
    s.content_mut().dials.standing_orders = false;
    let town = s.add_town("Town", 5.0);
    let hub = s.add_town("Hub", hub_emeralds);
    let beyond = s.add_town("Beyond", 5.0);
    s.add_route(town, hub, 2.0 / 1200.0, 0.0, 4.0);
    if onward {
        s.add_route(hub, beyond, 200.0 / 1200.0, 0.0, 4.0);
    } else {
        s.add_route(beyond, hub, 200.0 / 1200.0, 0.0, 4.0);
    }
    s.make_hub(hub);
    s
}

#[test]
fn a_wagon_with_one_pallet_draws_one_document_and_with_three_four() {
    // One pallet: its bill of lading is its own manifest.
    let mut s = seller_and_buyer(5.0);
    s.deliver(0, PAPER, 64.0);
    s.draft(0, 1, PAPER, 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road), "the wagon left");
    assert_eq!(s.wagon_manifest(0).unwrap().len(), 1);
    assert_eq!((s.paperwork(0).documents, superbills(&s, 0)), (1, 0), "one document: the bill of lading");
    // Three pallets (twenty stacks, nine to a pallet): three bills and the wagon's superbill.
    let mut s = seller_and_buyer(5.0);
    s.deliver(0, PAPER, 20.0 * 64.0);
    let id = s.draft(0, 1, PAPER, 20.0 * 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road), "the wagon left");
    let m = s.wagon_manifest(0).unwrap();
    assert!(m.len() == 3 && m.containers.is_empty() && m.needs_superbill(), "three lines: {m:?}");
    let p = s.paperwork(0);
    assert_eq!((p.documents, p.by_kind.get(LADING).copied(), superbills(&s, 0)), (4, Some(3), 1),
        "four documents: three bills of lading, one superbill, drawn at home: {:?}", p.by_kind);
    assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 20.0 * 64.0, "the three delivered as before");
    assert_eq!(superbills(&s, 0), 1, "it came home empty: no superbill for the road home");
}

#[test]
fn a_wagon_leaving_the_far_end_with_two_pallets_draws_a_superbill_at_its_home() {
    let mut s = seller_and_buyer(5.0);
    s.deliver(0, PAPER, 64.0);
    s.draft(0, 1, PAPER, 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    // Two crates of cloth waiting at the far end, made out to the wagon's home: fetched as it arrives.
    for _ in 0..2 {
        let (lading, packages) = crate_of(CLOTH, 1, 0);
        s.deliver_pallet(1, lading, packages).expect("waits to be fetched");
    }
    let start = s.totals();
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road), "the wagon left");
    assert_eq!(superbills(&s, 0), 0, "out with one pallet: no superbill");
    assert!(until(&mut s, 30, |s| s.wagon_freight(0).is_some_and(|f| f.reckoned == Some(enterprise_core::state::Leg::Home))),
        "it turned for home");
    assert_eq!(s.wagon_manifest(0).unwrap().len(), 2, "with the two crates fetched");
    assert_eq!(superbills(&s, 0), 1, "home's superbill for the road home, drawn as it left the far end");
    let cloth = s.town(0).stock[CLOTH];
    assert!(until(&mut s, 30, |s| s.route(0).trips >= 1));
    assert_eq!(s.town(0).stock[CLOTH] - cloth, 128.0, "both received at home");
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_hub_loading_three_pallets_for_one_next_stop_makes_one_consolidation_that_comes_off_whole_there() {
    let mut s = network(true, 10.0);
    let (hub, buyer) = (1, 2);
    let onward = 1;
    // The buyer uses tools; the hub has some to sell, and bread for the road (`a_hubs_own_run_loads_the_crates_booked_on_it_first`).
    s.set_trade(buyer, TOOLS, 0.0, 30.0);
    s.deliver(hub, TOOLS, 64.0);
    s.deliver(hub, BREAD, 64.0);
    s.deliver(buyer, BREAD, 64.0);
    let leg = s.leg(onward, true);
    let ids: Vec<u64> = (0..3).map(|_| {
        let (lading, packages) = crate_of(CLOTH, 0, buyer);
        s.book(hub, lading, packages, leg, 1)
    }).collect();
    let start = s.totals();
    s.advance(LOOK);
    assert!(s.route(onward).on_the_road, "the hub's run left");
    let m = s.wagon_manifest(onward).unwrap();
    assert!(m.lines.is_empty() && m.containers.len() == 1, "one line: the consolidation: {m:?}");
    let line = &m.containers[0];
    assert_eq!(line.drop, Some(buyer), "it comes off at the run's next stop, the buyer");
    let c = s.pallets().containers.get(&line.container).expect("in the registry").clone();
    assert_eq!(c.issuer, hub, "made up by the hub");
    assert_eq!(c.manifest.lines.iter().map(|l| l.pallet).collect::<Vec<_>>(), ids, "its three pallets, in the order booked");
    assert!(c.manifest.lines.iter().all(|l| l.booking.is_some_and(|b| b.leg == leg) && l.drop == Some(buyer)), "each on its ticket");
    assert_eq!(s.pallets().at(At::Container(line.container)), ids.as_slice(), "in the consolidation, not loose aboard");
    assert!(ids.iter().all(|id| s.pallets().get(*id).unwrap().seal.is_some_and(|seal| seal.by == 0)), "still under their shipper's seal");
    assert_eq!(s.wagon_pallets(onward).0.len(), 3, "the wagon carries the three");
    assert_eq!(superbills(&s, hub), 1, "the hub drew the consolidation's superbill; the wagon's one line needs none");
    assert!(s.hub(hub).custody.is_empty() && s.hub(hub).book.carried == 3, "out of the forwarder's custody");
    // Saved and loaded with the consolidation on the road: the wagon's line and the registry's container come back.
    let blob = s.save();
    let mut s = Society::load(&blob).expect("loads");
    assert_eq!(s.wagon_manifest(onward).unwrap(), m, "the wagon's manifest, its consolidation line with it");
    assert_eq!(s.pallets().containers.get(&line.container), Some(&c), "the consolidation, its issuer and day");
    assert!(s.save() == blob, "the same blob again");
    let cloth = s.town(buyer).stock[CLOTH];
    s.advance(DAY);
    assert_eq!(s.town(buyer).stock[CLOTH] - cloth, 3.0 * 64.0, "handed out whole at the buyer and received");
    assert!(s.pallets().containers.is_empty(), "the consolidation dissolved");
    assert!(ids.iter().all(|id| s.pallets().get(*id).is_none()), "its pallets opened by their consignee");
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn one_pallet_alone_booked_on_a_run_is_not_consolidated() {
    // One pallet booked on the run: a line of its own (the forwarder's parity test's case), no superbill.
    let mut s = network(true, 10.0);
    let (hub, buyer) = (1, 2);
    s.set_trade(buyer, TOOLS, 0.0, 30.0);
    s.deliver(hub, TOOLS, 64.0);
    s.deliver(hub, BREAD, 64.0);
    s.deliver(buyer, BREAD, 64.0);
    let leg = s.leg(1, true);
    let (lading, packages) = crate_of(CLOTH, 0, buyer);
    let id = s.book(hub, lading, packages, leg, 1);
    s.advance(LOOK);
    let m = s.wagon_manifest(1).unwrap();
    assert!(m.containers.is_empty() && m.booked(id), "loose aboard, on its ticket: {m:?}");
    assert_eq!(m.line(id).unwrap().drop, Some(buyer), "its drop written as it loaded");
    assert_eq!(superbills(&s, hub), 0);
}

#[test]
fn a_carriers_run_home_from_the_hub_takes_a_consolidation_home_whole() {
    // The carrier at the hub's far end: its wagon brings paper owed to the hub, and its run home loads the two crates of
    // cloth booked on it for the carrier, as one consolidation, received at home.
    let mut s = network(false, 20.0);
    let (hub, carrier) = (1, 2);
    let calling = 1;
    for t in [hub, carrier] {
        s.deliver(t, BREAD, 128.0);
    }
    s.deliver(carrier, PAPER, 576.0);
    s.draft(carrier, hub, PAPER, 576.0, 1.0 / 24.0, 0, DAY, 1, Incoterm::Ddp, true);
    let home = s.leg(calling, false);
    let ids: Vec<u64> = (0..2).map(|_| {
        let (lading, packages) = crate_of(CLOTH, 0, carrier);
        s.book(hub, lading, packages, home, 1)
    }).collect();
    let start = s.totals();
    let cloth = s.town(carrier).stock[CLOTH];
    assert!(until(&mut s, 60, |s| !s.wagon_manifest(calling).unwrap_or_default().containers.is_empty()),
        "the run home loaded a consolidation at the hub");
    let m = s.wagon_manifest(calling).unwrap();
    assert_eq!((m.containers.len(), m.containers[0].drop), (1, Some(carrier)), "for the carrier's home: {m:?}");
    assert_eq!(s.pallets().at(At::Container(m.containers[0].container)), ids.as_slice());
    assert_eq!(superbills(&s, hub), 1, "drawn at the hub");
    assert_eq!(superbills(&s, carrier), 0, "the wagon's own manifest has one line each way: none of its own");
    assert!(until(&mut s, 60, |s| s.route(calling).trips >= 1), "home");
    assert_eq!(s.town(carrier).stock[CLOTH] - cloth, 128.0, "both crates received at home");
    assert!(s.pallets().containers.is_empty() && ids.iter().all(|id| s.pallets().get(*id).is_none()));
    s.conserved(&start).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_consolidation_moves_its_pallets_in_and_out_whole_and_counts_aboard() {
    let mut p = Pallets::default();
    let bill = Lading::new(0, 2, 0);
    let a = p.pack(bill.clone(), vec![Package::new(CRATE, 0, 64.0)], At::Custody(1), 0);
    let b = p.pack(bill.clone(), vec![Package::new(CRATE, 0, 32.0)], At::Custody(1), 0);
    let loose = p.pack(bill, vec![Package::new(CRATE, 0, 16.0)], At::Wagon(4), 0);
    let lines = vec![Line { pallet: a, drop: Some(2), ..Default::default() }, Line { pallet: b, drop: Some(2), ..Default::default() }];
    let c = p.consolidate(1, lines.clone(), 5);
    assert_eq!((c, p.next_container), (0, 1));
    assert_eq!(p.at(At::Container(c)), &[a, b]);
    assert!(p.at(At::Custody(1)).is_empty(), "out of custody, into it");
    assert_eq!((p.containers[&c].issuer, p.containers[&c].issued, p.containers[&c].manifest.lines.clone()), (1, 5, lines.clone()));
    let mut m = Manifest { lines: vec![Line { pallet: loose, ..Default::default() }], ..Default::default() };
    assert!(!m.needs_superbill(), "one line is its own manifest");
    m.containers.push(enterprise_core::pallet::ContainerLine { container: c, drop: Some(2) });
    assert!(m.len() == 2 && m.needs_superbill(), "a pallet and a consolidation: two lines");
    assert!(!m.booked(c) && m.line(c).is_none(), "a consolidation's line is no pallet's");
    let aboard: Vec<u64> = p.aboard(4, &m).map(|x| x.id).collect();
    assert_eq!(aboard, vec![loose, a, b], "aboard: the loose pallets, then the consolidation's");
    let off = p.dissolve(c, At::Wagon(4), 9);
    assert_eq!(off, lines, "its lines back to carry on");
    assert!(p.containers.is_empty() && p.at(At::Container(c)).is_empty(), "dissolved");
    assert_eq!(p.at(At::Wagon(4)), &[loose, a, b], "its pallets on the wagon's floor, behind what was there");
    assert_eq!(p.get(a).unwrap().since, 9);
    assert!(p.dissolve(c, At::Wagon(4), 9).is_empty(), "gone: nothing more to dissolve");
}

#[test]
fn a_runs_booked_pallets_are_consolidated_by_next_stop_two_or_more_to_a_stop() {
    use enterprise_core::cargo::Paperwork;
    use enterprise_core::freight::{load_run, Booking, Leg};
    use enterprise_core::state::Wagon;
    let (hub, route) = (1, 3);
    let mut p = Pallets::default();
    let bill = Lading::new(0, 2, 0);
    let ids: Vec<u64> = (0..4).map(|_| p.pack(bill.clone(), vec![Package::new(CRATE, CLOTH, 64.0)], At::Custody(hub), 0)).collect();
    // Two next stops: the far end (2) and home (0), as no hub's run gives today (`Hub::load_booked` is one leg's).
    let out = Leg { route, home: 0, far: 2, outbound: true, at: hub };
    let back = Leg { outbound: false, ..out };
    let booking = |leg| Booking { leg, stacks: 1, ..Default::default() };
    let load = vec![(ids[0], booking(out)), (ids[1], booking(back)), (ids[2], booking(out)), (ids[3], booking(out))];
    let (mut wagon, mut paper) = (Wagon::default(), Paperwork::default());
    let made = load_run(&mut p, &mut wagon, route, hub, &mut paper, load, 7);
    assert_eq!(made.len(), 1, "one consolidation: the three for the far end");
    let c = &p.containers[&made[0]];
    assert_eq!(c.manifest.lines.iter().map(|l| l.pallet).collect::<Vec<_>>(), vec![ids[0], ids[2], ids[3]], "in load order");
    assert!(c.manifest.lines.iter().all(|l| l.drop == Some(2)) && c.issuer == hub && c.issued == 7);
    assert_eq!(wagon.manifest.containers, vec![enterprise_core::pallet::ContainerLine { container: made[0], drop: Some(2) }]);
    assert_eq!(wagon.manifest.lines.len(), 1, "the one for home loose aboard, a line of its own");
    assert_eq!((wagon.manifest.lines[0].pallet, wagon.manifest.lines[0].drop), (ids[1], Some(0)));
    assert_eq!(p.at(At::Wagon(route)), &[ids[1]]);
    assert!(ids.iter().all(|id| p.get(*id).unwrap().seal.is_some()), "each sealed leaving the hub");
    assert_eq!((paper.documents, paper.by_kind.get(SUPERBILL).copied()), (1, Some(1)), "the consolidation's superbill at the hub");
}
