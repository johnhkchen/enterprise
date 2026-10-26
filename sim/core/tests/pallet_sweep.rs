//! The pallet sweep (`docs/pallets.md` section 5, task `pallet-sweep`): the invariants `common/slices/pallets.rs` checks in
//! every random world each day, shown to fail, each with a message that says what is wrong, on a registry broken on
//! purpose; and the host's doings there shown to bring each kind of pallet. The 300 seeds themselves run in
//! `random_worlds.rs`.

use crate::common;

use common::slices::pallets as sweep;
use common::slices::pallets::{View, WagonView, CAPS_BIND};
use common::{sellers, world, DAY};
use enterprise_core::pallet::*;
use enterprise_core::{Content, Good};
use std::collections::{BTreeMap, BTreeSet};

/// A small world's registry by hand: four towns, town 1 a hub, route 0's wagon on the road.
struct Fixture {
    content: Content,
    pallets: Pallets,
    contracts: BTreeSet<usize>,
    bills: Vec<Vec<PalletId>>,
    wagons: BTreeMap<usize, WagonView>,
    custody: BTreeMap<usize, BTreeSet<PalletId>>,
    hubs: BTreeSet<usize>,
    superbills: BTreeMap<usize, u64>,
    /// Pallets: on the town's shelf, aboard the wagon (paid by bill 0), in the hub's custody.
    shelf: PalletId,
    aboard: PalletId,
    held: PalletId,
}

impl Fixture {
    fn new() -> Fixture {
        let content = Content::new(vec![Good::new("paper", 0.05, 64), Good::new("ore", 0.05, 64)], DAY);
        let mut pallets = Pallets::default();
        let crates = |good, count| vec![Package::new(CRATE, good, count)];
        let shelf = pallets.pack(Lading { contract: Some(0), payment: Payment::Paid, ..Lading::new(0, 1, 1) }, crates(0, 10.0),
            At::Town(0), 1);
        let aboard = pallets.pack(Lading { payment: Payment::ByBill(0), ..Lading::new(0, 2, 1) }, crates(1, 20.0), At::Wagon(0), 1);
        let held = pallets.pack(Lading::new(0, 3, 1), crates(0, 30.0), At::Custody(1), 1);
        pallets.seal(aboard, 0, 2);
        pallets.seal(held, 0, 2);
        let manifest = Manifest { lines: vec![Line { pallet: aboard, ..Default::default() }], ..Default::default() };
        let caps = Caps::of(&content, [], false, 0);
        let wagons = BTreeMap::from([(0, WagonView { manifest, cargo: vec![0.0, 0.0], caps })]);
        Fixture {
            content,
            pallets,
            contracts: BTreeSet::from([0]),
            bills: vec![vec![aboard]],
            wagons,
            custody: BTreeMap::from([(1, BTreeSet::from([held]))]),
            hubs: BTreeSet::from([1]),
            superbills: BTreeMap::new(),
            shelf,
            aboard,
            held,
        }
    }

    fn view(&self) -> View<'_> {
        View {
            superbills: self.superbills.clone(),
            content: &self.content,
            pallets: &self.pallets,
            towns: 4,
            contracts: self.contracts.clone(),
            bills: self.bills.clone(),
            wagons: self.wagons.clone(),
            custody: self.custody.clone(),
            hubs: self.hubs.clone(),
            lots: BTreeMap::new(),
        }
    }

    fn pallet(&mut self, id: PalletId) -> &mut Pallet {
        self.pallets.pallets.get_mut(&id).unwrap()
    }
}

/// The check's error, which must be one, and say `says`.
fn fails(check: fn(&View) -> Result<(), String>, f: &Fixture, says: &str) {
    match check(&f.view()) {
        Ok(()) => panic!("a registry that should fail with {says:?} passed"),
        Err(e) => assert!(e.contains(says), "wanted {says:?} in the message, got: {e}"),
    }
}

#[test]
fn a_registry_in_order_passes_every_check() {
    let f = Fixture::new();
    sweep::all(&f.view()).unwrap();
    for check in [sweep::packages, sweep::bills, sweep::places, sweep::seals, sweep::wagon_caps, sweep::pallet_caps, sweep::manifests] {
        check(&f.view()).unwrap();
    }
}

#[test]
fn a_pallet_indexed_twice_fails_the_places() {
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallets.by_place.get_mut(&At::Town(0)).unwrap().push(id);
    fails(sweep::places, &f, "indexed twice");
}

#[test]
fn a_pallet_indexed_at_two_places_fails_the_places() {
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallets.by_place.entry(At::Town(2)).or_default().push(id);
    fails(sweep::places, &f, "indexed twice");
    // Or its record says another place than the index it is in.
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).at = At::Town(2);
    fails(sweep::places, &f, "but it's at");
}

#[test]
fn a_pallet_missing_from_the_index_or_indexed_without_a_pallet_fails_the_places() {
    let mut f = Fixture::new();
    f.pallets.by_place.remove(&At::Town(0));
    fails(sweep::places, &f, "in no place's index");
    let mut f = Fixture::new();
    f.pallets.by_place.entry(At::Town(2)).or_default().push(99);
    fails(sweep::places, &f, "isn't in the registry");
    let mut f = Fixture::new();
    f.pallets.by_place.insert(At::Town(2), Vec::new());
    fails(sweep::places, &f, "no pallets");
}

#[test]
fn a_pallet_where_it_cannot_be_fails_the_places() {
    // Aboard a wagon that isn't on the road.
    let mut f = Fixture::new();
    let id = f.aboard;
    f.pallets.move_to(id, At::Wagon(5), 3);
    fails(sweep::places, &f, "which isn't on the road");
    // In custody at a town that is no hub, and at a hub with no record.
    let mut f = Fixture::new();
    let id = f.held;
    f.pallets.move_to(id, At::Custody(2), 3);
    fails(sweep::places, &f, "which is no hub");
    let mut f = Fixture::new();
    f.custody.get_mut(&1).unwrap().clear();
    fails(sweep::places, &f, "with no custody record");
    // A record of a pallet the hub doesn't hold.
    let mut f = Fixture::new();
    let id = f.shelf;
    f.custody.get_mut(&1).unwrap().insert(id);
    fails(sweep::places, &f, "which it doesn't hold");
    // Waiting at its own consignee, unopened.
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallets.move_to(id, At::Town(1), 3);
    fails(sweep::places, &f, "waits at its own consignee");
    // In a consolidation that doesn't exist, in a town that doesn't.
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallets.move_to(id, At::Container(7), 3);
    fails(sweep::places, &f, "container 7, which doesn't exist");
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallets.move_to(id, At::Town(9), 3);
    fails(sweep::places, &f, "of 4 towns");
}

#[test]
fn a_sealed_pallet_changed_or_a_carried_one_unsealed_fails_the_seals() {
    let mut f = Fixture::new();
    let id = f.aboard;
    f.pallet(id).packages[0].count += 1.0;
    fails(sweep::seals, &f, "changed since it was sealed");
    let mut f = Fixture::new();
    let id = f.held;
    f.pallet(id).lading.consignee = 0;
    fails(sweep::seals, &f, "changed since it was sealed");
    let mut f = Fixture::new();
    let id = f.aboard;
    f.pallet(id).seal = None;
    fails(sweep::seals, &f, "isn't sealed");
    // A pallet on a shelf may change as it likes until it leaves.
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).packages[0].count += 1.0;
    sweep::seals(&f.view()).unwrap();
}

#[test]
fn a_pallet_that_holds_nothing_or_mixes_handling_fails_the_packages() {
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).packages.clear();
    fails(sweep::packages, &f, "holds no package");
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut f = Fixture::new();
        let id = f.shelf;
        f.pallet(id).packages[0].count = bad;
        fails(sweep::packages, &f, "positive and finite");
    }
    let mut f = Fixture::new();
    let id = f.shelf;
    // Goods of one key may share a pallet (`packing`): a second plain good is no fault; one that needs cool storage is.
    f.pallet(id).packages.push(Package::new(CRATE, 1, 5.0));
    sweep::packages(&f.view()).unwrap();
    f.content.goods[1].handling.cool = true;
    fails(sweep::packages, &f, "mixes handling");
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).packages[0].good = 9;
    fails(sweep::packages, &f, "which the content lacks");
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).kind = 4;
    fails(sweep::packages, &f, "which the content lacks");
}

#[test]
fn a_bill_that_names_nobody_real_fails_the_bills() {
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).lading.consignee = 4;
    fails(sweep::bills, &f, "from town 0 to town 4");
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).lading.contract = Some(3);
    fails(sweep::bills, &f, "contract 3, which isn't in the book");
    // Paid by a bill that doesn't exist, or one that names other pallets.
    let mut f = Fixture::new();
    let id = f.aboard;
    f.pallet(id).lading.payment = Payment::ByBill(5);
    fails(sweep::bills, &f, "which doesn't exist");
    let mut f = Fixture::new();
    f.bills[0].clear();
    fails(sweep::bills, &f, "which names [], not it");
    // Two bills of exchange name one pallet.
    let mut f = Fixture::new();
    let id = f.aboard;
    f.bills.push(vec![id]);
    fails(sweep::bills, &f, "named by two bills of exchange");
    // (A bill naming a pallet that is gone is fine: it was opened.)
    let mut f = Fixture::new();
    f.bills.push(vec![77]);
    sweep::bills(&f.view()).unwrap();
}

#[test]
fn a_wagon_over_its_carriers_caps_fails_the_wagon_caps() {
    // Slots: 3 crates' worth of 9 stacks on a porter's pack of 9, over the nine.
    let mut f = Fixture::new();
    f.wagons.get_mut(&0).unwrap().caps = Caps::of(&f.content, [], true, 0);
    let id = f.aboard;
    f.pallet(id).packages = vec![Package::new(CRATE, 1, 64.0 * 9.0), Package::new(CRATE, 1, 64.0 * 9.0)];
    fails(sweep::wagon_caps, &f, "over its carrier's");
    // Loose cargo counts too, and a lot for sale.
    let mut f = Fixture::new();
    f.wagons.get_mut(&0).unwrap().cargo = vec![64.0 * 28.0, 0.0];
    fails(sweep::wagon_caps, &f, "over its carrier's");
    // Weight: a good that weighs 4 a stack fills a wagon of 27 weight in 7 stacks.
    let mut f = Fixture::new();
    f.content.goods[1].weight = 4.0 / 64.0;
    f.wagons.get_mut(&0).unwrap().cargo = vec![0.0, 64.0 * 8.0];
    fails(sweep::wagon_caps, &f, "over its carrier's");
    // Pallets: a porter's one pack takes one pallet.
    let mut f = Fixture::new();
    f.wagons.get_mut(&0).unwrap().caps = Caps::of(&f.content, [], true, 0);
    let id = f.shelf;
    f.pallets.move_to(id, At::Wagon(0), 3);
    fails(sweep::wagon_caps, &f, "over its carrier's");
}

#[test]
fn a_pallet_over_its_kinds_caps_fails_the_pallet_caps() {
    let mut f = Fixture::new();
    let id = f.shelf;
    f.pallet(id).packages = vec![Package::new(CRATE, 0, 64.0 * 9.0), Package::new(CRATE, 0, 64.0)];
    fails(sweep::pallet_caps, &f, "slots and weighs");
    // Within the slots, over the weight: ore that weighs 4 a stack, three stacks of it against the dial's nine.
    let mut f = Fixture::new();
    f.content.goods[1].weight = 4.0 / 64.0;
    let id = f.shelf;
    f.pallet(id).packages = vec![Package::new(CRATE, 1, 64.0 * 2.0)];
    sweep::pallet_caps(&f.view()).unwrap();
    f.pallet(id).packages = vec![Package::new(CRATE, 1, 64.0 * 3.0)];
    fails(sweep::pallet_caps, &f, "slots and weighs");
}

#[test]
fn a_manifest_that_is_not_the_load_fails_the_manifests() {
    // A line for a pallet that isn't aboard, a pallet aboard with no line, a line twice.
    let mut f = Fixture::new();
    f.wagons.get_mut(&0).unwrap().manifest.lines.push(Line { pallet: 99, ..Default::default() });
    fails(sweep::manifests, &f, "manifest lists");
    let mut f = Fixture::new();
    f.wagons.get_mut(&0).unwrap().manifest.lines.clear();
    fails(sweep::manifests, &f, "are aboard");
    let mut f = Fixture::new();
    let id = f.aboard;
    f.wagons.get_mut(&0).unwrap().manifest.lines.push(Line { pallet: id, ..Default::default() });
    fails(sweep::manifests, &f, "twice");
    // A consolidation whose pallets drop at two stops, or that lists what it doesn't hold.
    let mut f = Fixture::new();
    let (a, b) = (f.shelf, f.held);
    f.pallets.containers.insert(0, Container { issuer: 1, manifest: Manifest { lines: vec![
        Line { pallet: a, drop: Some(2), ..Default::default() }, Line { pallet: b, drop: Some(3), ..Default::default() }],
        ..Default::default() }, ..Default::default() });
    f.pallets.move_to(a, At::Container(0), 3);
    f.pallets.move_to(b, At::Container(0), 3);
    f.custody.get_mut(&1).unwrap().clear();
    fails(sweep::manifests, &f, "not one next stop");
    f.pallets.containers.get_mut(&0).unwrap().manifest.lines.pop();
    fails(sweep::manifests, &f, "are in it");
}

/// A consolidation aboard the wagon (`superbills`): two booked pallets for one stop, its line on the wagon's manifest, its
/// superbill drawn at the hub. Taken off the manifest, listed twice, coming off elsewhere than its pallets drop, of one
/// pallet, of a pallet not on a ticket, or with no superbill drawn, it fails.
#[test]
fn a_consolidation_off_the_manifest_or_without_its_superbill_fails() {
    let consolidated = || {
        let mut f = Fixture::new();
        let held = f.held;
        let other = f.pallets.pack(enterprise_core::cargo::Lading::new(0, 3, 1), vec![Package::new(CRATE, 0, 8.0)], At::Custody(1), 1);
        f.pallets.seal(other, 0, 2);
        f.custody.get_mut(&1).unwrap().clear();
        let booking = Some(enterprise_core::freight::Booking::default());
        let lines = [held, other].map(|pallet| Line { pallet, drop: Some(3), booking, ..Default::default() }).to_vec();
        let c = f.pallets.consolidate(1, lines, 3);
        f.wagons.get_mut(&0).unwrap().manifest.containers.push(ContainerLine { container: c, drop: Some(3) });
        f.superbills.insert(1, 1);
        (f, c)
    };
    let (f, _) = consolidated();
    for check in [sweep::places, sweep::seals, sweep::manifests, sweep::superbills] {
        check(&f.view()).unwrap_or_else(|e| panic!("a consolidation in order fails: {e}"));
    }
    let (mut f, _) = consolidated();
    f.wagons.get_mut(&0).unwrap().manifest.containers.clear();
    fails(sweep::manifests, &f, "aboard no wagon");
    let (mut f, c) = consolidated();
    let line = ContainerLine { container: c, drop: Some(3) };
    f.wagons.insert(1, f.wagons[&0].clone());
    f.wagons.get_mut(&1).unwrap().manifest = Manifest { containers: vec![line], ..Default::default() };
    fails(sweep::manifests, &f, "and route 1's");
    let (mut f, _) = consolidated();
    f.wagons.get_mut(&0).unwrap().manifest.containers[0].drop = Some(2);
    fails(sweep::manifests, &f, "but its pallet");
    let (mut f, c) = consolidated();
    f.pallets.containers.get_mut(&c).unwrap().manifest.lines[1].booking = None;
    fails(sweep::manifests, &f, "isn't on freight tickets");
    let (mut f, c) = consolidated();
    let off = f.pallets.containers.get_mut(&c).unwrap().manifest.lines.pop().unwrap();
    f.pallets.move_to(off.pallet, At::Wagon(0), 4);
    f.wagons.get_mut(&0).unwrap().manifest.lines.push(off);
    fails(sweep::manifests, &f, "two pallets or more");
    let (mut f, _) = consolidated();
    f.superbills.clear();
    fails(sweep::superbills, &f, "drew 0 superbills");
}

#[test]
fn the_check_is_what_a_slice_runs_every_day() {
    // `check` is `all` over a world's own view: a world in order passes it, with or without a wagon on the road.
    let s = world(11);
    common::slices::pallets::check(&s, 0).unwrap();
}

/// Before `pallet-caps` the rules let a wagon and a pallet go over their caps in most worlds: a town crated a whole sale as
/// one pallet (so a pallet was many crates), `stranded::load` loaded while a wagon had room left, and a porter's run
/// carried any number of pallets. Those worlds (seed 8 day 15: 12 stacks on a wagon of 9; seed 1 day 4: a pallet of 42
/// stacks) now keep every wagon and pallet within its caps, and [`CAPS_BIND`] has the sweep check it every day.
#[test]
fn the_worlds_that_went_over_their_caps_keep_within_them() {
    assert!(CAPS_BIND, "pallet-caps turns the caps on in the sweep");
    for (seed, day) in [(8u64, 15u64), (1, 4)] {
        let mut s = world(seed);
        let daily = sellers(seed);
        for d in 1..=day {
            for &e in &daily {
                s.sell_emeralds(e);
            }
            s.advance(d * DAY);
            common::slices::daily(&mut s, seed, d);
            let v = View::of(&s);
            sweep::wagon_caps(&v).unwrap_or_else(|e| panic!("seed {seed} day {d}: {e}"));
            sweep::pallet_caps(&v).unwrap_or_else(|e| panic!("seed {seed} day {d}: {e}"));
        }
    }
}

#[test]
fn about_a_third_of_the_worlds_have_a_heavy_good() {
    // The weight dials count a stack as 1; a content pack's heavy good weighs 4. In about a third of the worlds the
    // sweep makes one (seed 4's coal: 41 weight on 12 stacks of a wagon holding 27).
    let heavy = |seed| world(seed).content().goods.iter().filter(|g| (g.weight * g.stack as f64 - 4.0).abs() < 1e-9).count();
    let worlds_with = (0..30).filter(|&seed| heavy(seed) > 0).count();
    assert!((5..=20).contains(&worlds_with), "{worlds_with} of 30 worlds have a heavy good");
    assert!(heavy(4) > 0);
}

#[test]
fn the_hosts_doings_bring_every_kind_of_pallet() {
    // Over a few worlds' 60 days, this slice's own doings (`pallets::daily`, alone: the pallets it brings are the ids new
    // after it runs; a town's own crating and the other slices' doings make pallets too, and aren't counted) bring a
    // pallet owed, paid, paid by a bill of exchange, under a contract, and one made out to a town other than the one it
    // waits in.
    let (mut owed, mut paid, mut by_bill, mut contracted, mut elsewhere) = (0, 0, 0, 0, 0);
    for seed in 0..12u64 {
        let mut s = world(seed);
        let daily = sellers(seed);
        for day in 1..=60u64 {
            for &e in &daily {
                s.sell_emeralds(e);
            }
            s.advance(day * DAY);
            common::slices::daily(&mut s, seed, day);
            let before: BTreeSet<PalletId> = s.pallets().pallets.keys().copied().collect();
            sweep::daily(&mut s, &mut common::Rng::of(seed, "pallets-test", day), day);
            for p in s.pallets().pallets.values().filter(|p| !before.contains(&p.id)) {
                match p.lading.payment {
                    Payment::Owed => owed += 1,
                    Payment::Paid => paid += 1,
                    Payment::ByBill(_) => by_bill += 1,
                }
                contracted += p.lading.contract.is_some() as u32;
                elsewhere += matches!(p.at, At::Town(t) if !p.consigned_to(t)) as u32;
            }
        }
    }
    println!("pallets brought: {owed} owed, {paid} paid, {by_bill} by bill, {contracted} under contract, {elsewhere} elsewhere");
    assert!(owed > 0 && paid > 0 && by_bill > 0 && contracted > 0 && elsewhere > 0);
}
