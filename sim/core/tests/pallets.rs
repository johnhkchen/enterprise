//! Pallets and their registry (`pallet.rs`, `docs/pallets.md` section 1): the types, and the movers the rules use.
//! Reference for the rules already ported: `CargoTests.aCrateTakesTheSpaceOfWhatsInIt`, `Caravan.capacity`.

use enterprise_core::pallet::*;
use enterprise_core::{Content, Good, Society};

const DAY: u64 = 24_000;

/// Paper (stack 64), ore (stack 64, weighs 2 a piece).
fn content() -> Content {
    Content::new(vec![Good::new("paper", 0.05, 64), Good::new("ore", 0.05, 64).weight(2.0)], DAY)
}

fn bill() -> Lading {
    Lading { shipper: 0, consignee: 1, ..Default::default() }
}

#[test]
fn move_to_keeps_the_index_in_step_and_in_arrival_order() {
    let mut p = Pallets::default();
    let (a, b, c) = (p.pack(bill(), vec![], At::Town(0), 1), p.pack(bill(), vec![], At::Town(0), 2), p.pack(bill(), vec![], At::Town(0), 3));
    assert_eq!(p.at(At::Town(0)), &[a, b, c]);
    p.move_to(a, At::Wagon(0), 10);
    p.move_to(c, At::Wagon(0), 11);
    assert_eq!(p.at(At::Town(0)), &[b]);
    assert_eq!(p.at(At::Wagon(0)), &[a, c], "in the order they came");
    assert_eq!((p.get(a).unwrap().at, p.get(a).unwrap().since), (At::Wagon(0), 10));
    p.move_to(a, At::Wagon(0), 12);
    assert_eq!(p.at(At::Wagon(0)), &[c, a], "arriving again goes to the back");
    p.move_to(b, At::Custody(1), 13);
    assert!(p.at(At::Town(0)).is_empty());
    assert!(!p.by_place.contains_key(&At::Town(0)), "an empty place is not kept");
}

#[test]
fn opening_removes_it_and_counts_it_received() {
    let mut p = Pallets::default();
    let a = p.pack(bill(), vec![Package::new(CRATE, 0, 5.0)], At::Town(1), 0);
    let opened = p.open(a).unwrap();
    assert_eq!(opened.packages.len(), 1);
    assert!(p.get(a).is_none() && p.at(At::Town(1)).is_empty());
    assert_eq!(p.received, 1);
    assert!(p.open(a).is_none());
}

#[test]
fn the_digest_changes_with_any_package_or_the_bill_and_not_with_where_or_when() {
    let mut p = Pallets::default();
    let a = p.pack(bill(), vec![Package::new(CRATE, 0, 5.0), Package::new(BARREL, 1, 3.0)], At::Town(0), 0);
    let d = p.digest(a).unwrap();
    p.move_to(a, At::Wagon(2), 999);
    assert_eq!(p.digest(a), Some(d), "not with at or since");
    p.pallets.get_mut(&a).unwrap().packages[1].count = 4.0;
    assert_ne!(p.digest(a), Some(d), "a package's count");
    p.pallets.get_mut(&a).unwrap().packages[1].count = 3.0;
    assert_eq!(p.digest(a), Some(d));
    p.pallets.get_mut(&a).unwrap().packages[0].good = 1;
    assert_ne!(p.digest(a), Some(d), "a package's good");
    p.pallets.get_mut(&a).unwrap().packages[0].good = 0;
    p.pallets.get_mut(&a).unwrap().lading.payment = Payment::Paid;
    assert_ne!(p.digest(a), Some(d), "the bill");
    p.pallets.get_mut(&a).unwrap().lading.payment = Payment::Owed;
    p.pallets.get_mut(&a).unwrap().packages.pop();
    assert_ne!(p.digest(a), Some(d), "a package gone");
}

#[test]
fn a_seal_holds_until_a_package_is_touched() {
    let mut p = Pallets::default();
    let a = p.pack(bill(), vec![Package::new(CRATE, 0, 5.0)], At::Town(0), 0);
    p.seal(a, 0, 5);
    assert!(p.get(a).unwrap().intact());
    p.move_to(a, At::Wagon(0), 6);
    assert!(p.get(a).unwrap().intact(), "moving doesn't break it");
    p.pallets.get_mut(&a).unwrap().packages[0].count = 4.0;
    assert!(!p.get(a).unwrap().intact());
}

// parity-partial: CargoTests.aCrateTakesTheSpaceOfWhatsInIt (the numbers only; cargo.rs pins the shipment itself)
#[test]
fn a_packages_slots_are_its_stacks_at_least_one() {
    let c = content();
    assert_eq!(Package::new(CRATE, 0, 0.5).slots(&c), 1, "a crate of nearly nothing still takes a slot");
    assert_eq!(Package::new(CRATE, 0, 64.0).slots(&c), 1);
    assert_eq!(Package::new(CRATE, 0, 65.0).slots(&c), 2);
    assert_eq!(Package::new(CRATE, 0, 200.0).slots(&c), 4);
    assert_eq!(Package::new(LOOSE, 0, 0.0).slots(&c), 0, "nothing loose takes nothing");
    assert_eq!(Package::new(LOOSE, 0, 65.0).slots(&c), 2);
}

#[test]
fn a_loads_slots_weight_and_pallets() {
    let c = content();
    let mut p = Pallets::default();
    let a = p.pack(bill(), vec![Package::new(CRATE, 0, 64.0), Package::new(CRATE, 1, 10.0)], At::Town(0), 0);
    let b = p.pack(bill(), vec![Package::new(BUNDLE, 1, 64.0)], At::Town(0), 0);
    // Loose: 70 paper (2 stacks) and 5 ore (1 stack).
    let load = Load::of(&c, &[70.0, 5.0], [p.get(a).unwrap(), p.get(b).unwrap()]);
    assert_eq!(load.pallets, 2);
    assert_eq!(load.slots, 3 + (1 + 1 + 1), "loose stacks, then each package its stacks");
    let want = 70.0 / 64.0 + 5.0 * 2.0 + 64.0 / 64.0 + 10.0 * 2.0 + 64.0 * 2.0 + 0.0;
    assert!((load.weight - want).abs() < 1e-9, "{} vs {want}", load.weight);
    assert_eq!(Load::of(&c, &[0.0, 0.0], []), Load::default());
}

#[test]
fn a_stack_weighs_one_unless_the_content_says_heavy() {
    let c = content();
    let one_stack = Load::of(&c, &[64.0, 0.0], []);
    assert!((one_stack.weight - 1.0).abs() < 1e-9);
    assert_eq!(one_stack.slots, 1);
    let mut tared = content();
    tared.packages[CRATE].tare = 0.25;
    assert!((Package::new(CRATE, 0, 64.0).weight(&tared) - 1.25).abs() < 1e-9);
}

#[test]
fn caps_for_a_wagon_a_squad_with_two_packs_and_a_porter_run() {
    let c = content();
    let d = c.dials.clone();
    let leather = c.trades.iter().position(|t| t.name == enterprise_core::squad::LEATHERWORKER).expect("a leatherworker trade");
    let porter = c.trades.iter().position(|t| t.idle()).expect("an idle trade");
    let plain = c.trades.iter().position(|t| !t.idle() && t.name != enterprise_core::squad::LEATHERWORKER).unwrap();

    let wagon = Caps::of(&c, [plain], false);
    assert_eq!((wagon.slots, wagon.weight, wagon.pallets), (d.wagon_slots, d.wagon_weight, d.wagon_pallets));
    assert_eq!(wagon.slots, enterprise_core::squad::capacity(&c, [plain], false), "the slots are squad::capacity's");

    let squad = Caps::of(&c, [leather, porter, plain], false);
    assert_eq!(squad.slots, d.wagon_slots + 2 * d.pack_stacks);
    assert_eq!(squad.slots, enterprise_core::squad::capacity(&c, [leather, porter, plain], false));
    assert!((squad.weight - (d.wagon_weight + 2.0 * d.pack_weight)).abs() < 1e-9);
    assert_eq!(squad.pallets, d.wagon_pallets + 2);

    let run = Caps::of(&c, [porter], true);
    assert_eq!((run.slots, run.weight, run.pallets), (d.pack_stacks, d.pack_weight, 1));

    assert!(wagon.holds(&Load { slots: wagon.slots, weight: wagon.weight, pallets: wagon.pallets }));
    assert!(!wagon.holds(&Load { slots: wagon.slots + 1, weight: 0.0, pallets: 0 }));
    assert!(!wagon.holds(&Load { slots: 0, weight: wagon.weight + 0.5, pallets: 0 }));
    assert!(!wagon.holds(&Load { slots: 0, weight: 0.0, pallets: wagon.pallets + 1 }));
}

#[test]
fn the_base_content_has_the_crate_barrel_bundle_and_one_pallet_kind() {
    let c = content();
    assert_eq!(c.packages.len(), 4);
    assert_eq!(c.packages[CRATE].slots, enterprise_core::cargo::CRATE_STACKS);
    assert!(c.packages[CRATE].returnable && c.packages[BARREL].returnable && !c.packages[BUNDLE].returnable);
    assert_eq!(c.pallet_kinds.len(), 1);
    let load = &c.pallet_kinds[LOAD];
    assert_eq!((load.slots(&c.dials), load.weight(&c.dials)), (c.dials.pallet_slots, c.dials.pallet_weight));
    let mut s = Society::new(content());
    s.set_dial("pallet_slots", 12.0).unwrap();
    s.set_dial("pallet_weight", 6.5).unwrap();
    let load = &s.content().pallet_kinds[LOAD];
    assert_eq!((load.slots(&s.content().dials), load.weight(&s.content().dials)), (12, 6.5), "the load follows its dials");
    assert_eq!(c.goods[0].package, CRATE);
    assert!((c.goods[0].weight - 1.0 / 64.0).abs() < 1e-12, "a stack weighs 1");
}

#[test]
fn pallets_survive_a_save_and_load() {
    let mut s = Society::new(content());
    {
        let mut p = s.world.resource_mut::<Pallets>();
        let a = p.pack(bill(), vec![Package::new(CRATE, 0, 12.0)], At::Town(0), 3);
        p.pack(Lading { payment: Payment::ByBill(4), contract: Some(2), ..bill() }, vec![Package::new(BARREL, 1, 7.0)], At::Custody(1), 4);
        p.move_to(a, At::Wagon(1), 9);
        p.seal(a, 0, 9);
        p.containers.insert(0, Container { issuer: 0, manifest: Manifest { lines: vec![Line { pallet: a, drop: Some(1), ..Default::default() }] } });
        p.next_container = 1;
    }
    let blob = s.save();
    let mut back = Society::load(&blob).unwrap();
    let (a, b) = (s.world.resource::<Pallets>(), back.world.resource::<Pallets>());
    assert_eq!(a.pallets.len(), 2);
    assert_eq!(b.pallets.len(), 2);
    assert_eq!(b.at(At::Wagon(1)), a.at(At::Wagon(1)));
    assert_eq!(b.at(At::Custody(1)), a.at(At::Custody(1)));
    for (id, p) in &a.pallets {
        let q = b.get(*id).unwrap();
        assert_eq!((q.at, q.since, q.seal, q.digest()), (p.at, p.since, p.seal, p.digest()));
        assert_eq!(q.lading, p.lading);
    }
    assert_eq!(b.containers, a.containers);
    assert_eq!((b.next, b.next_container), (a.next, a.next_container));
    assert!(back.save() == blob, "the same blob again");
}

// The rules on the registry (`pallets-for-crates`): how a shipment's goods lie in their crates, and the registry's
// other movers.

#[test]
fn a_good_crated_lies_in_crates_of_nine_stacks_the_last_part_full_and_sums_back_exactly() {
    let c = content();
    let per = 9.0 * 64.0;
    let lie = |n: f64| enterprise_core::cargo::crated(&c, 0, n).iter().map(|p| p.count).collect::<Vec<_>>();
    assert_eq!(lie(0.0), vec![0.0], "nothing: one crate, empty");
    assert_eq!(lie(per), vec![per]);
    assert_eq!(lie(per + 1.0), vec![per, 1.0]);
    assert_eq!(lie(2.0 * per + 0.5), vec![per, per, 0.5]);
    // As a shipment took its space before pallets: its stacks at least one, its crates nine stacks each, at least one;
    // and its count, summed back from the crates, the very number crated.
    let stacks_of = |n: f64| if n <= 0.0 { 0 } else { (n / 64.0).ceil() as u32 };
    for i in 0..4000 {
        let n = i as f64 * 1.37 + if i % 3 == 0 { 0.0 } else { 0.1 * (i % 7) as f64 };
        let p = Pallet { id: 0, kind: LOAD, lading: bill(), packages: enterprise_core::cargo::crated(&c, 0, n),
            at: At::Town(0), since: 0, seal: None, claimed: None };
        assert_eq!(p.count().to_bits(), n.to_bits(), "{n} sums back");
        assert_eq!(p.slots(&c), stacks_of(n).max(1), "{n}'s stacks");
        assert_eq!(p.crates(), stacks_of(n).max(1).div_ceil(9).max(1), "{n}'s crates");
        assert_eq!(p.good(), 0);
    }
}

#[test]
fn unpacking_takes_a_pallet_apart_without_counting_it_received() {
    let mut p = Pallets::default();
    let a = p.pack(bill(), vec![Package::new(CRATE, 0, 5.0)], At::Town(0), 0);
    let taken = p.unpack(a).expect("there");
    assert_eq!(taken.packages.len(), 1);
    assert!(p.get(a).is_none() && p.at(At::Town(0)).is_empty() && p.received == 0, "gone, not received");
}

#[test]
fn arranging_a_place_puts_its_pallets_in_another_order_without_moving_them() {
    let mut p = Pallets::default();
    let (a, b, c) = (p.pack(bill(), vec![], At::Wagon(3), 1), p.pack(bill(), vec![], At::Wagon(3), 2),
        p.pack(bill(), vec![], At::Wagon(3), 3));
    p.arrange(At::Wagon(3), vec![b, c, a]);
    assert_eq!(p.at(At::Wagon(3)), &[b, c, a]);
    assert_eq!((p.get(a).unwrap().at, p.get(a).unwrap().since), (At::Wagon(3), 1), "nothing moved");
    p.move_to(c, At::Town(1), 9);
    assert_eq!(p.at(At::Wagon(3)), &[b, a]);
}

#[test]
fn leaving_premises_seals_a_pallet_once_and_again_only_if_it_changed() {
    let mut p = Pallets::default();
    let a = p.pack(bill(), vec![Package::new(CRATE, 0, 5.0)], At::Town(0), 0);
    p.seal_leaving(a, 0, 4);
    let first = p.get(a).unwrap().seal.expect("sealed as it leaves");
    assert_eq!((first.at, first.by), (4, 0));
    p.seal_leaving(a, 7, 9);
    assert_eq!(p.get(a).unwrap().seal, Some(first), "sealed and unchanged: the seal it had");
    p.pallets.get_mut(&a).unwrap().packages[0].count = 3.0;
    p.seal_leaving(a, 7, 9);
    let again = p.get(a).unwrap().seal.unwrap();
    assert!((again.at, again.by) == (9, 7) && p.get(a).unwrap().intact(), "changed at a holder's: sealed anew");
}

#[test]
fn a_manifest_finds_takes_and_tells_its_lines() {
    let booking = enterprise_core::freight::Booking::default();
    let mut m = Manifest { lines: vec![Line { pallet: 4, booking: Some(booking), ..Default::default() },
        Line { pallet: 7, for_forwarder: true, ..Default::default() }] };
    assert!(m.booked(4) && !m.booked(7) && !m.booked(9));
    assert!(m.for_forwarder(7) && !m.for_forwarder(4));
    assert_eq!(m.take(4).map(|l| l.pallet), Some(4));
    assert!(m.line(4).is_none() && m.take(4).is_none() && m.lines.len() == 1);
}

#[test]
fn a_bill_paid_for_in_coin_is_prepaid_and_owed_or_billed_ones_are_not() {
    for (payment, prepaid) in [(Payment::Paid, true), (Payment::Owed, false), (Payment::ByBill(3), false)] {
        assert_eq!(Lading { payment, ..bill() }.prepaid(), prepaid, "{payment:?}");
    }
    let l = Lading::new(2, 5, 77);
    assert!(l.shipper == 2 && l.consignee == 5 && l.issued == 77 && l.payment == Payment::Owed && l.contract.is_none());
}

#[test]
fn the_first_claim_holds_a_pallet_till_it_is_released_or_the_look_ends() {
    let mut p = Pallets::default();
    let a = p.pack(bill(), vec![Package::new(CRATE, 0, 5.0)], At::Town(0), 0);
    let b = p.pack(bill(), vec![Package::new(CRATE, 0, 5.0)], At::Town(0), 0);
    let d = p.digest(a);
    assert!(p.claim(a, 0), "free: the first route takes it");
    assert!(p.claim(a, 0), "its own claim again");
    assert!(!p.claim(a, 1), "another route's plan goes without");
    assert_eq!(p.claimed_by(a), Some(0));
    assert_eq!(p.digest(a), d, "a claim is no part of the pallet");
    p.release(a);
    assert!(p.claim(a, 1), "released: free");
    assert!(p.claim(b, 0));
    p.clear_claims();
    assert!(p.claimed_by(a).is_none() && p.claimed_by(b).is_none(), "every claim let go at a look's start");
    assert!(!p.claim(99, 0), "no such pallet");
}
