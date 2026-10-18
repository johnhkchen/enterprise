//! Pallet caps (`pallet-caps`, `docs/pallets.md` sections 1 "Weight and volume" and 2 "Plan and load sizing"): packing
//! splits a shipment at the pallet kind's slots and weight, a bill of lading each; every load fits its carrier's slots,
//! weight and pallets (the plan's sale and owed goods, what's fetched, booked or bought on the way); porters are sized
//! by the larger of slots and weight; a porter run carries one pallet.

use enterprise_core::cargo::{self, Lading, LADING};
use enterprise_core::pallet::{palleted, pallet_most, At, Caps, Load};
use enterprise_core::squad;
use enterprise_core::{Content, Good, Incoterm, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;

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

fn goods(names: &[&str]) -> Content {
    Content::new(names.iter().map(|n| Good::new(n, 0.05, 64)).collect(), DAY)
}

/// A content pack's heavy good: a stack of it weighs `per_stack` (the base content's weighs 1).
fn heavy(content: &mut Content, good: usize, per_stack: f64) {
    content.goods[good].weight = per_stack / content.goods[good].stack as f64;
}

/// A town and its buyer a day apart, the town's wagon every `every` days.
fn seller_and_buyer(content: Content, money: f64, every: f64) -> Society {
    let mut s = Society::new(content);
    s.add_town("Haygate", money);
    s.add_town("Cloverby", money);
    s.add_route(0, 1, 1.0, 0.0, every);
    s
}

#[test]
fn twenty_stacks_owed_go_as_three_pallets_with_three_bills_and_are_all_delivered() {
    let mut s = seller_and_buyer(goods(&["paper", "flint"]), 5.0, 30.0);
    s.deliver(0, 0, 20.0 * 64.0);
    let id = s.draft(0, 1, 0, 20.0 * 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road), "the wagon left");
    let (aboard, lots, _) = s.wagon_pallets(0);
    assert!(lots.is_empty());
    let counts: Vec<f64> = aboard.iter().map(|p| p.count()).collect();
    assert_eq!(counts, vec![576.0, 576.0, 128.0], "nine stacks to a pallet (`pallet_slots`), the last two");
    assert!(aboard.iter().all(|p| p.crates() == 1 && p.consigned_to(1) && p.lading.contract == Some(id) && p.lading.copy),
        "a crate each, each its own bill of lading for the contract: {aboard:?}");
    assert!(aboard.iter().all(|p| p.seal.is_some_and(|seal| seal.by == 0)), "each sealed as it left home");
    assert_eq!(s.wagon_manifest(0).unwrap().lines.len(), 3, "three lines on the manifest");
    assert_eq!(s.paperwork(0).by_kind.get(LADING), Some(&3), "three bills of lading drawn");
    assert!(s.town(0).money == 5.0 - 3.0 * 0.125, "three new crates bought: {}", s.town(0).money);
    assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 20.0 * 64.0, "all three opened and delivered under the contract");
    assert!(s.shipments(1).empties >= 3, "their crates stay, to ship with");
}

#[test]
fn packing_splits_at_the_pallet_dials() {
    let mut c = goods(&["paper"]);
    // The base pallet: nine stacks, nine weight. A crate a pallet.
    assert_eq!(pallet_most(&c, 0), 576.0);
    let loads = cargo::pallet_loads(&c, 0, 20.0 * 64.0);
    assert_eq!(loads.iter().map(|l| l.len()).collect::<Vec<_>>(), vec![1, 1, 1]);
    // Twelve-stack pallets: a crate and a part crate each, the last pallet one crate.
    c.dials.pallet_slots = 12;
    c.dials.pallet_weight = 12.0;
    let loads = cargo::pallet_loads(&c, 0, 20.0 * 64.0);
    let counts: Vec<Vec<f64>> = loads.iter().map(|l| l.iter().map(|p| p.count).collect()).collect();
    assert_eq!(counts, vec![vec![576.0, 192.0], vec![512.0]], "twelve stacks and eight");
    // A pallet's weight binds first for a heavy good: 9 weight is 144 ore at 4 a stack, three slots.
    let mut c = goods(&["iron_ore"]);
    heavy(&mut c, 0, 4.0);
    assert_eq!(pallet_most(&c, 0), 144.0);
    let one = palleted(&c, 0, 144.0);
    assert!(one.slots == 3 && (one.weight - 9.0).abs() < 1e-9 && one.pallets == 1, "{one:?}");
    // A wagon's caps hold three such pallets: 432 ore, six stacks and three quarters, by weight.
    assert_eq!(Caps::wagon(&c).palleted(&c, 0), 432.0);
    assert_eq!(Caps::wagon(&c).loose(&c, 0), 432.0, "and loose, as much: 27 weight");
    // Paper weighs a stack a stack: the slots bind, as before.
    let p = goods(&["paper"]);
    assert_eq!(Caps::wagon(&p).palleted(&p, 0), 27.0 * 64.0);
    let room = Caps::wagon(&p).less(&Load { slots: 20, weight: 19.5, pallets: 3 });
    assert_eq!(room.palleted(&p, 0), 7.0 * 64.0, "the room left's seven slots");
}

#[test]
fn a_wagon_takes_what_weighs_its_cap_of_a_heavy_good() {
    let mut c = goods(&["iron_ore", "paper"]);
    heavy(&mut c, 0, 4.0);
    let mut s = seller_and_buyer(c, 5.0, 30.0);
    s.deliver(0, 0, 20.0 * 64.0);
    let id = s.draft(0, 1, 0, 20.0 * 64.0, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
    assert!(until(&mut s, 3, |s| s.route(0).on_the_road), "the wagon left");
    let (aboard, _, _) = s.wagon_pallets(0);
    assert_eq!(aboard.iter().map(|p| p.count()).collect::<Vec<_>>(), vec![144.0; 3],
        "three pallets of 144 ore, each at its nine weight");
    let load = Load::of(s.content(), &s.wagon_cargo(0).unwrap(), aboard.iter());
    assert!(load.slots == 9 && (load.weight - 27.0).abs() < 1e-9, "full by weight on nine slots of 27: {load:?}");
    assert_eq!(s.wagon_crated(0, 0), 432.0, "six stacks and three quarters of the twenty owed");
    assert_eq!(s.wagon_cargo(0).unwrap()[0], 0.0, "none loose");
    assert_eq!(s.town(0).stock[0], 20.0 * 64.0 - 432.0, "the rest waits for the next wagon");
    assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
    assert_eq!(s.contract(id).delivered, 432.0);
}

#[test]
fn porters_are_sized_by_the_larger_of_slots_and_weight() {
    let mut c = goods(&["iron_ore", "paper"]);
    let d = c.dials.clone();
    assert_eq!((d.wagon_slots, d.wagon_weight, d.pack_stacks, d.pack_weight), (27, 27.0, 9, 9.0));
    assert_eq!(squad::porters_needed(&c, 36, 36.0, false), 1, "a stack weighs 1: slots and weight agree");
    assert_eq!(squad::porters_needed(&c, 12, 12.0, false), 0, "a wagon holds it");
    assert_eq!(squad::porters_needed(&c, 12, 48.0, false), 3, "twelve stacks of ore at 4: 21 weight beyond the wagon");
    assert_eq!(squad::porters_needed(&c, 12, 48.0, true), 2, "a Leatherworker's pack carries nine of it");
    assert_eq!(squad::porters_needed(&c, 45, 30.0, false), 2, "and slots, where they're the larger");
    // What the far end wants, by weight: twelve stacks of home's ore weigh 48.
    heavy(&mut c, 0, 4.0);
    let home = vec![12.0 * 64.0, 0.0];
    let far = enterprise_core::state::Trades { uses: vec![1.0, 0.0], makes: vec![0.0; 2], ..Default::default() };
    let (stacks, weight) = squad::wanted_load(&c, &home, &far, None);
    assert_eq!(stacks, 12);
    assert!((weight - 48.0).abs() < 1e-9, "{weight}");
    assert_eq!(squad::porters_needed(&c, stacks, weight, false), 3);
}

// --- A porter run: one pack, one pallet ---------------------------------------------------------------------------

const PAPER: usize = 0;
const BREAD: usize = 1;
const HUB: usize = 0;
const DEST: usize = 1;

/// The mod's test network (`tests/squad.rs`): a hub with paper and bread, its Merchant and a Librarian; a far end four
/// blocks away whose lockers take paper up to a stack.
fn network() -> Society {
    let content = Content::new(vec![Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 1.0 / 6.0, 64).food(5.0),
        Good::new("saddle", 6.0, 1)], DAY);
    let mut s = Society::new(content);
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", 12.0);
    s.record_capacity(dest, 1, &[(PAPER, 64.0 / 5.0)]);
    s.set_trade(dest, PAPER, 0.0, 1e-9);
    s.deliver(hub, PAPER, 128.0);
    s.deliver(hub, BREAD, 8.0);
    s.deliver(dest, BREAD, 2.0);
    s.set_roster(hub, &[("merchant", 1), ("librarian", 2)]);
    s.set_offers(hub, 1, Vec::new());
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s
}

/// The far end's first trip done (all sold: nothing home), then what `owe` drafts owed in two days; looks on till the
/// wagon goes, the day it can't wait.
fn goes_owed(owe: impl Fn(&mut Society, u64)) -> Society {
    let mut s = network();
    s.advance(LOOK);
    s.advance(4 * LOOK);
    assert!(!s.route(0).on_the_road && s.crew(0).last_home == Some(0), "its first trip done, nothing home");
    let t = s.now();
    owe(&mut s, t);
    for look in 1..=(4 * DAY / LOOK) {
        s.advance(t + 2 * DAY + look * LOOK);
        if s.route(0).on_the_road {
            return s;
        }
    }
    panic!("it never went");
}

#[test]
fn a_porter_run_of_nine_slots_takes_one_pallet() {
    // Half a stack owed: one pallet, a pack's worth: on a porter's back (`squad.rs`'s test).
    let s = goes_owed(|s, t| {
        s.draft(HUB, DEST, PAPER, 32.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Dap, true);
    });
    let crew = s.crew(0);
    assert!(crew.porter && crew.slots == 9, "{crew:?}");
    assert_eq!(s.wagon_pallets(0).0.len(), 1, "its one pallet");
    let caps = Caps::of(s.content(), [s.content().trade("librarian").unwrap()], true);
    assert_eq!((caps.slots, caps.pallets), (9, 1), "a porter run's caps: a pack, a pallet");
    // Two contracts owed, a pallet each (a bill each, one contract to a bill): within the pack's slots, but two pallets
    // are more than a porter carries, so the wagon goes.
    let s = goes_owed(|s, t| {
        s.draft(HUB, DEST, PAPER, 16.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Dap, true);
        s.draft(HUB, DEST, PAPER, 16.0, 0.5, t + 2 * DAY, DAY, 1, Incoterm::Dap, true);
    });
    let crew = s.crew(0);
    assert!(!crew.porter && crew.slots == 27, "two pallets: by wagon, {crew:?}");
    assert_eq!(s.wagon_pallets(0).0.len(), 2);
}

#[test]
fn a_wagon_fetches_a_pallet_only_if_it_fits() {
    // A pallet of 144 ore (9 weight) made out to the wagon's home waits at the far end; the wagon arrives carrying `owed`
    // ore owed there. 320 ore (20 weight): the pallet doesn't fit the wagon's 27 and waits; 64 (4 weight), it's fetched.
    for (owed, fetched) in [(320.0, false), (64.0, true)] {
        let mut c = goods(&["iron_ore", "paper"]);
        heavy(&mut c, 0, 4.0);
        let mut s = seller_and_buyer(c, 5.0, 30.0);
        let waiting = s.deliver_pallet(1, Lading { payment: cargo::Payment::Paid, ..Lading::new(1, 0, 0) },
            cargo::one_pallet(s.content(), 0, 1000.0)).expect("held");
        assert_eq!(s.pallets().get(waiting).unwrap().count(), 144.0, "a host's pallet: one pallet's worth at most");
        s.deliver(0, 0, owed);
        let id = s.draft(0, 1, 0, owed, 0.05, 0, DAY, 1, Incoterm::Ddp, true);
        assert!(until(&mut s, 3, |s| s.route(0).on_the_road));
        assert!(until(&mut s, 30, |s| s.contract(id).delivered > 0.0));
        assert_eq!(s.contract(id).delivered, owed);
        // (The fetch comes first at the far end, with what it brought still aboard.)
        let at = s.pallets().get(waiting).unwrap().at;
        assert_eq!(at == At::Wagon(0), fetched, "{owed} ore owed: the pallet is at {at:?}");
    }
}
