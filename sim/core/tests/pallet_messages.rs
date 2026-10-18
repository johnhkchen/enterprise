//! The binding's protocol for pallets (`pallet_messages.rs`, `docs/pallets.md` section 4): the outputs the registry
//! queues for a host (`PalletPacked`, `PalletMoved`, `PalletOpened`) and the inputs a host's players make
//! (`PalletBrought`, `PackageBrought`, `PackageLost`).

use enterprise_core::cargo::{crated, Lading, Payment};
use enterprise_core::pallet::{At, Package, Pallets, CRATE};
use enterprise_core::wire::{Input, Output, PackageLine, Place};
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;

fn goods() -> Content {
    Content::new(["paper", "flint", "iron_axe"].iter().map(|n| Good::new(n, 0.05, 64)).collect(), DAY)
}

fn hub_and_town() -> Society {
    let mut s = Society::new(goods());
    s.add_town("Haygate", 5.0);
    s.add_town("Cloverby", 10.0);
    s.add_route(0, 1, 1.0, 0.0, 30.0);
    s
}

fn bill(shipper: usize, consignee: usize, contract: Option<usize>, payment: Payment, copy: bool) -> Lading {
    Lading { contract, payment, copy, ..Lading::new(shipper, consignee, 0) }
}

fn look(s: &mut Society) {
    let next = s.now() + s.content().dials.look_ticks;
    s.apply(&Input::Advance { to: next });
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

/// A host listening: its first happening turns the outputs on.
fn listening(s: &mut Society) {
    s.apply(&Input::Advance { to: s.now() });
}

/// Three crates of paper (576, 576, 48) made out to Cloverby, left in Haygate's warehouse by a player's hand.
fn three_crates(s: &mut Society, contract: Option<usize>) -> u64 {
    let packages = crated(s.content(), 0, 1200.0);
    assert_eq!(packages.len(), 3);
    let id = s.deliver_pallet(0, bill(0, 1, contract, Payment::Owed, contract.is_some()), packages).expect("waits, made out elsewhere");
    listening(s); // (takes the PalletPacked it queued)
    id
}

// parity: CaravanTests.whatAPlayerBroughtIsPaidForByTheNextCaravan
#[test]
fn a_pallet_a_player_brought_is_paid_for_by_the_next_wagon() {
    let mut s = hub_and_town();
    listening(&mut s);
    let c = s.draft(0, 1, 1, 16.0, 0.25, 0, DAY, 1, Incoterm::Dap, true);
    let id = s.deliver_pallet(0, bill(0, 1, Some(c), Payment::Owed, true), crated(s.content(), 1, 16.0)).expect("waits");
    let before = s.totals();
    // A player carried the crate from the hub's warehouse to the town's desk.
    let out = s.apply(&Input::PalletBrought { town: 1, id });
    assert!(s.contract(c).delivered == 16.0 && s.town(1).stock[1] == 16.0, "the Merchant opened it and took delivery");
    assert!(s.contract(c).owing == 4.0, "4 emeralds owed to the hub: {}", s.contract(c).owing);
    assert_eq!(s.shipments(1).empties, 1, "and keeps the crate");
    assert!(s.pallets().get(id).is_none(), "the pallet is gone, opened");
    assert_eq!(s.pallets().received, 1);
    assert_eq!(out, vec![
        Output::PalletPacked { id, at: Place::Town { town: 0 }, packages: vec![PackageLine { kind: CRATE as u32, good: 1, count: 16.0 }] },
        Output::PalletOpened { id, at: Place::Town { town: 0 } },
    ], "its crate appeared in the hub's warehouse, and was emptied");
    s.conserved(&before).expect("what was delivered is kept");
    assert!(until(&mut s, 60, |s| s.route(0).trips >= 1));
    let c = s.contract(c);
    assert!(c.owing < 0.01 && c.paid >= 3.9, "the hub's wagon collected it: owing {}, paid {}", c.owing, c.paid);
    assert!(s.town(0).money == 9.0, "home with it: {}", s.town(0).money);
}

#[test]
fn a_pallet_made_out_elsewhere_brought_to_a_desk_waits_there() {
    let mut s = hub_and_town();
    listening(&mut s);
    let id = three_crates(&mut s, None);
    s.apply(&Input::PalletBrought { town: 0, id }); // already there: nothing moves
    let out = s.apply(&Input::PalletBrought { town: 1, id: id + 99 }); // a pallet that isn't there
    assert!(out.is_empty());
    s.add_town("Dunmere", 1.0);
    let out = s.apply(&Input::PalletBrought { town: 2, id });
    assert_eq!(out, vec![Output::PalletMoved { id, to: Place::Town { town: 2 } }]);
    assert_eq!(s.pallets().get(id).unwrap().at, At::Town(2), "waiting to be fetched, still packed");
    assert_eq!(s.town(2).stock[0], 0.0);
}

#[test]
fn a_pallet_arriving_in_parts_is_received_when_the_last_comes() {
    let mut s = hub_and_town();
    listening(&mut s);
    let id = three_crates(&mut s, None);
    s.world.resource_mut::<Pallets>().seal(id, 0, 0);
    let before = s.totals();
    for package in [2, 0] {
        assert!(s.apply(&Input::PackageBrought { town: 1, pallet: id, package }).is_empty());
        assert!(s.pallets().get(id).is_some(), "not complete: still arriving");
    }
    let out = s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 1 });
    assert_eq!(out, vec![Output::PalletOpened { id, at: Place::Town { town: 0 } }]);
    assert_eq!(s.town(1).stock[0], 1200.0, "all of it, the seal kept");
    assert_eq!(s.pallets().lost, 0);
    assert!(s.pallets().lost_goods.is_empty());
    s.conserved(&before).unwrap();
}

#[test]
fn a_pallet_short_a_package_is_received_at_the_next_look_with_what_came() {
    let mut s = hub_and_town();
    listening(&mut s);
    let id = three_crates(&mut s, None);
    s.world.resource_mut::<Pallets>().seal(id, 0, 0);
    let before = s.totals();
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 0 });
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 2 });
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 2 }); // twice: counted once
    assert_eq!(s.town(1).stock[0], 0.0, "not yet: the third may still come");
    look(&mut s);
    assert!(s.pallets().get(id).is_none(), "received at the look");
    assert_eq!(s.town(1).stock[0], 576.0 + 48.0, "the consignee takes what came");
    assert_eq!(s.pallets().lost_goods.get(&0), Some(&576.0), "the crate that didn't is a loss on the bill, in the books");
    assert_eq!(s.pallets().received, 1);
    assert_eq!(s.shipments(1).empties, 2, "two crates to keep");
    s.conserved(&before).expect("the loss is booked as used");
}

#[test]
fn a_lost_package_breaks_the_seal_and_the_pallet_goes_on_short() {
    let mut s = hub_and_town();
    listening(&mut s);
    let c = s.draft(0, 1, 0, 1200.0, 0.05, 0, DAY, 1, Incoterm::Dap, true);
    let id = three_crates(&mut s, Some(c));
    s.world.resource_mut::<Pallets>().seal(id, 0, 0);
    let before = s.totals();
    assert!(s.pallets().get(id).unwrap().intact());
    assert!(s.apply(&Input::PackageLost { pallet: id, package: 1 }).is_empty());
    let p = s.pallets().get(id).unwrap();
    assert_eq!(p.packages.iter().map(|p| p.count).collect::<Vec<_>>(), vec![576.0, 48.0], "the others move up a place");
    assert!(p.seal.is_none(), "the seal is broken");
    assert_eq!(s.pallets().lost_goods.get(&0), Some(&576.0));
    s.conserved(&before).expect("lost goods are used");
    // It goes on short: the consignee takes the 624 that are left, under the contract.
    s.apply(&Input::PalletBrought { town: 1, id });
    assert_eq!(s.town(1).stock[0], 624.0);
    assert_eq!(s.contract(c).delivered, 624.0, "delivered under the contract: what came");
    assert_eq!(s.pallets().received, 1);
    s.conserved(&before).unwrap();
    // Out of range or unknown: ignored.
    assert!(s.apply(&Input::PackageLost { pallet: id, package: 0 }).is_empty());
}

#[test]
fn a_package_lost_from_a_pallet_arriving_shifts_the_parts_brought() {
    let mut s = hub_and_town();
    listening(&mut s);
    let id = three_crates(&mut s, None);
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 2 }); // the 48
    s.apply(&Input::PackageLost { pallet: id, package: 0 }); // a 576: the 48 is now place 1
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 0 }); // the other 576: complete
    assert!(s.pallets().get(id).is_none());
    assert_eq!(s.town(1).stock[0], 576.0 + 48.0);
}

#[test]
fn a_pallet_lost_whole_is_gone_and_its_bill_is_unbacked() {
    let mut s = hub_and_town();
    listening(&mut s);
    let c = s.draft(0, 1, 0, 100.0, 0.05, 0, DAY, 1, Incoterm::Dap, true);
    let id = s.deliver_pallet(0, bill(0, 1, Some(c), Payment::Owed, true), crated(s.content(), 0, 100.0)).unwrap();
    let b = s.draw_bill(1, 0, 5.0);
    s.world.resource_mut::<enterprise_core::bills::Bills>().bills[b].pallets = vec![id];
    let before = s.totals();
    let backed = s.quote(0, b, true, 0.0, 0.0, 1000.0).share;
    s.apply(&Input::PackageLost { pallet: id, package: 0 });
    assert!(s.pallets().get(id).is_none() && s.pallets().at(At::Town(0)).is_empty());
    assert_eq!(s.pallets().lost, 1);
    s.conserved(&before).unwrap();
    let after = s.quote(0, b, true, 0.0, 0.0, 1000.0).share;
    let unbacked = s.content().dials.discount_unbacked;
    assert!(unbacked > 0.0 && (backed - after - unbacked).abs() < 1e-9, "unbacked costs {unbacked} more: {backed} then {after}");
    // Once more with the explicit backing a hub gets from crates in hand: the bill's goods are still gone.
    assert!(s.bills()[b].goods_lost());
}

/// A script of happenings, played on a new world: what a host would log and replay.
fn script(s: &mut Society) -> Vec<Vec<Output>> {
    let mut outputs = vec![s.apply(&Input::Advance { to: DAY })];
    let a = s.deliver_pallet(0, bill(0, 1, None, Payment::Owed, false), crated(s.content(), 0, 1200.0)).unwrap();
    let b = s.deliver_pallet(0, bill(0, 1, None, Payment::Paid, false), vec![Package::new(CRATE, 1, 30.0)]).unwrap();
    for input in [
        Input::PackageBrought { town: 1, pallet: a, package: 1 },
        Input::PackageLost { pallet: a, package: 0 },
        Input::PalletBrought { town: 1, id: b },
        Input::Advance { to: 3 * DAY },
        Input::PackageBrought { town: 1, pallet: a, package: 0 },
        Input::Advance { to: 20 * DAY },
    ] {
        outputs.push(s.apply(&input));
    }
    outputs
}

#[test]
fn a_replay_of_the_inputs_gives_the_same_outputs_and_the_same_world() {
    let (mut x, mut y) = (hub_and_town(), hub_and_town());
    let (ox, oy) = (script(&mut x), script(&mut y));
    assert_eq!(ox, oy);
    assert!(ox.iter().flatten().any(|o| matches!(o, Output::PalletPacked { .. })));
    assert!(ox.iter().flatten().any(|o| matches!(o, Output::PalletMoved { .. } | Output::PalletOpened { .. })));
    assert_eq!(x.save(), y.save());
    // Saved and loaded in the middle, the rest plays the same (the queue is not state).
    let mut z = hub_and_town();
    z.apply(&Input::Advance { to: DAY });
    let blob = z.save();
    let mut z2 = Society::load(&blob).unwrap();
    assert_eq!(z.apply(&Input::Advance { to: 5 * DAY }), z2.apply(&Input::Advance { to: 5 * DAY }));
}

#[test]
fn nothing_is_queued_before_a_host_listens() {
    let mut s = hub_and_town();
    three_crates(&mut s, None);
    s.advance(DAY);
    assert!(s.pallets().events.is_empty(), "a run with no host keeps no outputs");
}

#[test]
fn a_pallet_arriving_that_its_shipper_takes_apart_is_no_longer_arriving() {
    // At landing: the shipper opened a void pallet (`cargo::earmark`, Step::Work) after a package of it was brought, and
    // the town's look then took in a pallet that was gone (a panic).
    let mut s = hub_and_town();
    listening(&mut s);
    let c = s.draft(0, 1, 0, 1200.0, 0.05, 0, DAY, 1, Incoterm::Dap, true);
    let id = three_crates(&mut s, Some(c));
    let before = s.totals();
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 0 });
    s.world.resource_mut::<enterprise_core::contracts::Book>().contracts[c].state = enterprise_core::contracts::State::Broken;
    look(&mut s);
    assert!(s.pallets().get(id).is_none() && s.pallets().arriving.is_empty(), "taken apart, not arriving");
    assert_eq!(s.pallets().received, 0);
    s.conserved(&before).unwrap();
}

#[test]
fn a_pallet_arriving_that_moves_on_is_no_longer_arriving() {
    // At landing: a wagon fetched (or a hub took into custody) a pallet a package of which had been brought, and the
    // look then opened it off the wagon, its manifest line left behind.
    let mut s = hub_and_town();
    listening(&mut s);
    let id = three_crates(&mut s, None);
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 0 });
    let now = s.now();
    s.world.resource_mut::<Pallets>().move_to(id, At::Custody(0), now);
    assert!(s.pallets().arriving.is_empty());
    look(&mut s);
    assert_eq!(s.pallets().get(id).unwrap().packages.len(), 3, "still whole, in custody");
    assert_eq!(s.town(1).stock[0], 0.0);
}

#[test]
fn a_pallet_crated_again_is_drawn_again_and_no_longer_arriving() {
    let mut s = hub_and_town();
    listening(&mut s);
    let id = three_crates(&mut s, None);
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 2 });
    let packages = crated(s.content(), 0, 600.0);
    s.world.resource_mut::<Pallets>().repack(id, packages.clone());
    assert!(s.pallets().arriving.is_empty(), "the places it was named by start over");
    let lines = packages.iter().map(|p| PackageLine { kind: p.kind as u32, good: p.good as u32, count: p.count }).collect();
    let out = s.apply(&Input::Advance { to: s.now() });
    assert_eq!(out, vec![
        Output::PalletOpened { id, at: Place::Town { town: 0 } },
        Output::PalletPacked { id, at: Place::Town { town: 0 }, packages: lines },
    ]);
}

#[test]
fn a_package_brought_then_lost_leaves_nothing_arriving() {
    let mut s = hub_and_town();
    listening(&mut s);
    let id = three_crates(&mut s, None);
    s.apply(&Input::PackageBrought { town: 1, pallet: id, package: 0 });
    s.apply(&Input::PackageLost { pallet: id, package: 0 });
    assert!(s.pallets().arriving.is_empty(), "what was brought is gone: not an empty pallet received at the look");
    look(&mut s);
    assert_eq!(s.pallets().received, 0);
    assert_eq!(s.pallets().get(id).unwrap().packages.len(), 2);
}

#[test]
fn a_world_with_a_pallet_arriving_and_a_bill_lost_saves_and_carries_on_the_same() {
    let mut s = hub_and_town();
    listening(&mut s);
    let a = three_crates(&mut s, None);
    let b = s.deliver_pallet(0, bill(0, 1, None, Payment::Owed, false), vec![Package::new(CRATE, 1, 30.0)]).unwrap();
    let paid = s.draw_bill(1, 0, 5.0);
    s.world.resource_mut::<enterprise_core::bills::Bills>().bills[paid].pallets = vec![b];
    s.apply(&Input::PackageLost { pallet: b, package: 0 });
    s.apply(&Input::PackageBrought { town: 1, pallet: a, package: 1 });
    assert!(!s.pallets().arriving.is_empty() && s.bills()[paid].goods_lost());
    let mut t = Society::load(&s.save()).unwrap();
    assert!(!t.pallets().arriving.is_empty() && t.bills()[paid].goods_lost(), "both kept");
    listening(&mut t);
    let next = s.now() + 3 * DAY;
    assert_eq!(s.apply(&Input::Advance { to: next }), t.apply(&Input::Advance { to: next }));
    assert_eq!(s.save(), t.save());
    assert_eq!(s.town(1).stock[0], 576.0, "the part brought, taken in at the look, on both");
}
