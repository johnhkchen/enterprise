//! The pallets as a host shows them, through the wire (`pallet_host.rs`, task `pallet-views`): goods packed at a desk
//! for another town (`Input::Pack`), a desk's chests taking a pallet whole or not at all (`Query::Stow`, `Input::Stowed`),
//! the showing kept while the pallet stays as it is and lost when it moves or changes, a town's pallets and a wagon's
//! manifest asked (`Query::Pallets`, `Pallet`, `Manifest`), and the content pack's kinds of packing checked. The world is
//! the wire's own: Hub and Far, forty blocks apart, Hub's people short of books Far has, so Hub's wagon goes shopping
//! and fetches what Far packed for it.

use enterprise_core::wire::*;
use enterprise_core::{Fixed, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;
const BREAD: GoodNo = 1;
const BOOK: GoodNo = 2;
const PAPER: GoodNo = 3;

fn pack() -> ContentPack {
    let mut wheat = PackGood::new("minecraft:wheat", 0.05, 64);
    wheat.flags = vec![GoodFlag::Crop];
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    bread.saturation = 6.0;
    let mut book = PackGood::new("minecraft:book", 0.5, 64);
    book.comfort = Some("reading".into());
    let mut p = ContentPack::new(DAY, vec![wheat, bread, book, PackGood::new("minecraft:paper", 0.04, 64)]);
    p.comforts = vec!["reading".into()];
    p.trades = ["merchant", "farmer", "librarian", "none", "nitwit"].map(String::from).to_vec();
    p
}

fn desk(x: i32, z: i32) -> Point {
    Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x, y: -60, z }
}

const HUB: TownNo = 0;
const FAR: TownNo = 1;

struct Host {
    society: Option<Society>,
}

impl Host {
    /// Hub (town 0) and Far (town 1), with their Merchants, Hub's villager of no trade to lead its wagon, bread and
    /// money in both, books and 2,000 paper at Far.
    fn new() -> Host {
        let mut h = Host { society: None };
        h.done(Command::Create { pack: pack(), seed: 7 });
        h.apply(vec![
            Input::AddTown { name: "Hub".into(), desk: desk(27, 27) },
            Input::AddTown { name: "Far".into(), desk: desk(67, 27) },
            Input::AddHand { town: HUB, trade: "merchant".into(), level: 1, tag: "villager-hub".into() },
            Input::AddHand { town: HUB, trade: "none".into(), level: 1, tag: "villager-lead".into() },
            Input::AddHand { town: FAR, trade: "merchant".into(), level: 1, tag: "villager-far".into() },
            Input::Deliver { town: HUB, good: BREAD, count: 128.0 },
            Input::Deliver { town: FAR, good: BREAD, count: 128.0 },
            Input::Deliver { town: FAR, good: BOOK, count: 64.0 },
            Input::Deliver { town: FAR, good: PAPER, count: 2000.0 },
            Input::Pay { town: HUB, money: Fixed::from(50.0) },
            Input::Pay { town: FAR, money: Fixed::from(50.0) },
        ]);
        h
    }

    fn done(&mut self, command: Command) -> (Vec<Output>, Vec<Answer>) {
        match handle(&mut self.society, Request::new(command)) {
            Response::Done { outputs, answers } => (outputs, answers),
            r => panic!("{r:?}"),
        }
    }

    fn refused(&mut self, command: Command) -> Refusal {
        match handle(&mut self.society, Request::new(command)) {
            Response::Refused { why } => why,
            r => panic!("not refused: {r:?}"),
        }
    }

    fn apply(&mut self, inputs: Vec<Input>) -> Vec<Output> {
        self.done(Command::Apply { inputs }).0
    }

    fn ask(&mut self, query: Query) -> Answer {
        self.done(Command::Ask { queries: vec![query] }).1.remove(0)
    }

    fn held(&mut self, town: TownNo) -> Vec<PalletView> {
        match self.ask(Query::Pallets { town }) {
            Answer::Pallets { held, .. } => held,
            a => panic!("{a:?}"),
        }
    }

    fn stow(&mut self, at: Point, free: u32) -> Vec<PalletView> {
        match self.ask(Query::Stow { point: at, free }) {
            Answer::Stow { pallets, .. } => pallets,
            a => panic!("{a:?}"),
        }
    }

    fn stock(&mut self, town: TownNo, good: GoodNo) -> f64 {
        match self.ask(Query::Stock { town }) {
            Answer::Stock { stock, .. } => stock[good as usize],
            a => panic!("{a:?}"),
        }
    }

    fn society(&self) -> &Society {
        self.society.as_ref().unwrap()
    }
}

fn paper(count: f64) -> Vec<Count> {
    vec![Count { good: PAPER, count }]
}

#[test]
fn goods_packed_at_a_desk_are_pallets_in_its_warehouse_a_bill_each() {
    let mut h = Host::new();
    let out = h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(1200.0) }]);
    // Nine stacks to a pallet (the base content's load is one crate): 576, 576 and 48.
    let packed: Vec<_> = out.iter().filter_map(|o| match o {
        Output::PalletPacked { id, at: Place::Town { town: FAR }, packages } => Some((*id, packages.clone())),
        _ => None,
    }).collect();
    assert_eq!(packed.len(), 3, "{out:?}");
    assert!(packed.iter().all(|(_, p)| p.len() == 1 && p[0].kind == 1 && p[0].good == PAPER), "crates of paper: {packed:?}");
    assert_eq!(packed.iter().map(|(_, p)| p[0].count).sum::<f64>(), 1200.0);
    assert_eq!(h.stock(FAR, PAPER), 800.0, "out of the stock");
    let held = h.held(FAR);
    assert_eq!(held.len(), 3);
    let v = &held[0];
    assert!(v.shipper == FAR && v.consignee == HUB && v.from == "Far" && v.to == "Hub" && v.contract.is_none() && !v.paid
        && !v.sealed && v.items == 1 && v.shown.is_none(), "{v:?}");
    match h.ask(Query::Pallets { town: HUB }) {
        Answer::Pallets { held, inbound, .. } => {
            assert!(held.is_empty());
            assert_eq!(inbound.iter().map(|p| p.id).collect::<Vec<_>>(), packed.iter().map(|p| p.0).collect::<Vec<_>>(),
                "made out to Hub: inbound there");
        }
        a => panic!("{a:?}"),
    }
    // Packing for itself, or for a town that isn't there, is refused.
    assert!(matches!(h.refused(Command::Apply { inputs: vec![Input::Pack { town: FAR, consignee: FAR, goods: paper(1.0) }] }),
        Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(Command::Apply { inputs: vec![Input::Pack { town: FAR, consignee: 9, goods: paper(1.0) }] }),
        Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(Command::Apply { inputs: vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(-1.0) }] }),
        Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(Command::Apply { inputs: vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(0.5) }] }),
        Refusal::Input { at: 0, .. }));
    // More than the town holds packs what it holds.
    h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(5000.0) }]);
    assert_eq!(h.stock(FAR, PAPER), 0.0);
    assert_eq!(h.held(FAR).iter().map(|p| p.packages.iter().map(|k| k.count).sum::<f64>()).sum::<f64>(), 2000.0);
}

#[test]
fn a_desks_chests_take_a_pallet_whole_or_not_at_all() {
    let mut h = Host::new();
    h.apply(vec![Input::SetDial { name: "pallet_slots".into(), value: Some(27.0) },
        Input::SetDial { name: "pallet_weight".into(), value: Some(27.0) }]);
    // A pallet of three crates (576, 576, 48), then one of a single crate.
    h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(1200.0) }]);
    h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(100.0) }]);
    let held = h.held(FAR);
    assert_eq!(held.iter().map(|p| p.items).collect::<Vec<_>>(), vec![3, 1], "{held:?}");
    let far = desk(67, 27);
    assert!(h.stow(far.clone(), 0).is_empty(), "no room, nothing");
    let two = h.stow(far.clone(), 2);
    assert_eq!(two.iter().map(|p| p.id).collect::<Vec<_>>(), vec![held[1].id], "the three crates don't fit two slots: passed over");
    let all = h.stow(far.clone(), 4);
    assert_eq!(all.iter().map(|p| p.id).collect::<Vec<_>>(), vec![held[0].id, held[1].id], "both, in arrival order");
    // Asking moves nothing: the host says what it put in.
    h.apply(vec![Input::Stowed { point: far.clone(), pallet: held[1].id }]);
    assert_eq!(h.held(FAR)[1].shown, Some(far.clone()), "shown at the desk");
    assert_eq!(h.stow(far.clone(), 10).iter().map(|p| p.id).collect::<Vec<_>>(), vec![held[0].id], "not offered again");
    // A stowing at another town's desk, or of a pallet that isn't there, is a stale view; at a point that isn't a desk, refused.
    h.apply(vec![Input::Stowed { point: desk(27, 27), pallet: held[0].id }, Input::Stowed { point: far.clone(), pallet: 99 }]);
    assert_eq!(h.held(FAR)[0].shown, None);
    assert!(matches!(h.refused(Command::Apply { inputs: vec![Input::Stowed { point: desk(5, 5), pallet: 0 }] }),
        Refusal::Input { at: 0, .. }));
    assert!(matches!(h.refused(Command::Ask { queries: vec![Query::Stow { point: desk(5, 5), free: 1 }] }),
        Refusal::Query { at: 0, .. }));
}

#[test]
fn a_showing_holds_while_its_pallet_stays_as_it_is() {
    let mut h = Host::new();
    h.apply(vec![Input::SetDial { name: "pallet_slots".into(), value: Some(27.0) },
        Input::SetDial { name: "pallet_weight".into(), value: Some(27.0) }]);
    h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(1200.0) }]);
    let far = desk(67, 27);
    let p = h.held(FAR)[0].clone();
    assert_eq!(p.items, 3);
    h.apply(vec![Input::Stowed { point: far.clone(), pallet: p.id }]);
    h.apply(vec![Input::Advance { to: DAY / 2 }]);
    assert_eq!(h.held(FAR)[0].shown, Some(far.clone()), "time passing changes nothing");
    // A crate lost: the pallet goes on short (two crates), still shown where its other crates are, not offered again.
    h.apply(vec![Input::PackageLost { pallet: p.id, package: 1 }]);
    let short = h.held(FAR)[0].clone();
    assert!(short.items == 2 && short.digest != p.digest && short.shown == Some(far.clone()), "{short:?}");
    assert!(h.stow(far.clone(), 2).is_empty());
    match h.ask(Query::Pallets { town: FAR }) {
        Answer::Pallets { lost_goods, lost, .. } => {
            assert_eq!(lost_goods, vec![Count { good: PAPER, count: 576.0 }], "the lost crate booked");
            assert_eq!(lost, 0, "the pallet goes on");
        }
        a => panic!("{a:?}"),
    }
    // Carried whole by a player to Hub, its consignee: received there, gone, and Hub's stock has what came.
    let out = h.apply(vec![Input::PalletBrought { town: HUB, id: p.id }]);
    assert!(out.contains(&Output::PalletOpened { id: p.id, at: Place::Town { town: FAR } }), "{out:?}");
    assert_eq!(h.stock(HUB, PAPER), 624.0);
    assert!(matches!(h.ask(Query::Pallet { id: p.id }), Answer::Pallet { pallet: None, .. }));
    assert!(h.society().pallets().get(p.id).is_none());
}

#[test]
fn a_pallet_brought_to_a_town_not_its_consignee_waits_there_to_be_shown_afresh() {
    let mut h = Host::new();
    h.apply(vec![Input::AddTown { name: "Mid".into(), desk: desk(47, 27) }]);
    h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(100.0) }]);
    let p = h.held(FAR)[0].clone();
    h.apply(vec![Input::Stowed { point: desk(67, 27), pallet: p.id }]);
    let out = h.apply(vec![Input::Advance { to: 3 * LOOK }, Input::PalletBrought { town: 2, id: p.id }]);
    assert!(out.contains(&Output::PalletMoved { id: p.id, to: Place::Town { town: 2 } }), "{out:?}");
    let there = h.held(2)[0].clone();
    assert!(there.shown.is_none() && there.since == 3 * LOOK, "moved: shown nowhere yet ({there:?})");
    assert_eq!(h.stow(desk(47, 27), 1).len(), 1);
    assert!(h.stow(desk(67, 27), 9).is_empty(), "nothing left at Far");
}

#[test]
fn a_wagon_fetches_what_was_packed_for_its_home_and_its_manifest_says_so() {
    let mut h = Host::new();
    h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(100.0) }]);
    let p = h.held(FAR)[0].clone();
    h.apply(vec![Input::Stowed { point: desk(67, 27), pallet: p.id }]);
    h.apply(vec![Input::Route { from: HUB, to: FAR, send: true }]);
    assert!(matches!(h.ask(Query::Manifest { route: 0 }), Answer::Manifest { lines, superbill: false, .. } if lines.is_empty()));
    let mut moved = None;
    let mut aboard = None;
    for look in 1..=6u64 {
        for o in h.apply(vec![Input::Advance { to: look * LOOK }]) {
            if let Output::PalletMoved { id, to: Place::Wagon { route: 0 } } = o {
                assert_eq!(id, p.id);
                moved = Some(look);
            }
        }
        if let Answer::Manifest { lines, .. } = h.ask(Query::Manifest { route: 0 }) {
            if !lines.is_empty() {
                aboard = Some(lines);
            }
        }
    }
    assert!(moved.is_some(), "the wagon never fetched it");
    let lines = aboard.expect("a manifest line on its way home");
    assert!(lines.len() == 1 && lines[0].pallet.id == p.id && lines[0].pallet.sealed && lines[0].pallet.shown.is_none()
        && lines[0].drop == Some(HUB), "{lines:?}");
    assert!(h.held(FAR).is_empty());
    assert_eq!(h.stock(HUB, PAPER), 100.0, "home, received");
}

#[test]
fn the_showing_is_kept_by_a_save() {
    let mut h = Host::new();
    h.apply(vec![Input::Pack { town: FAR, consignee: HUB, goods: paper(100.0) }]);
    let p = h.held(FAR)[0].clone();
    h.apply(vec![Input::Stowed { point: desk(67, 27), pallet: p.id }]);
    let (save, _) = match handle(&mut h.society, Request::new(Command::Save)) {
        Response::Saved { save } => (save, ()),
        r => panic!("{r:?}"),
    };
    let mut loaded = Host { society: None };
    loaded.done(Command::Load { save });
    assert_eq!(loaded.held(FAR)[0].shown, Some(desk(67, 27)));
}

#[test]
fn a_content_packs_kinds_of_packing_are_checked() {
    let kind = |name: &str, slots: u32| PackPackage { name: name.into(), slots, tare: 0.0, emeralds: 0.5, returnable: true };
    let mut ok = pack();
    ok.packages = vec![kind("loose", 0), kind("crate", 9), kind("barrel", 9), kind("sack", 2)];
    let content = enterprise_core::Content::from_pack(&ok).expect("a pack's own kinds");
    assert_eq!(content.packages.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), vec!["loose", "crate", "barrel", "sack"]);
    let base = enterprise_core::Content::from_pack(&pack()).unwrap();
    assert_eq!(base.packages.len(), 4, "none listed: the core's own");
    for (packages, says) in [
        (vec![kind("crate", 9)], "must be \"loose\""),
        (vec![kind("loose", 0), kind("barrel", 9)], "must be \"crate\""),
        (vec![kind("loose", 3), kind("crate", 9)], "loose holds 3"),
        (vec![kind("loose", 0), kind("crate", 0)], "holds no stacks"),
        (vec![kind("loose", 0), kind("crate", 9), kind("crate", 9)], "named as another is"),
        (vec![kind("loose", 0), kind("crate", 9), kind("enterprise:barrel", 9)], "namespace"),
        (vec![kind("loose", 0), PackPackage { tare: -1.0, ..kind("crate", 9) }], "weighs -1"),
    ] {
        let mut bad = pack();
        bad.packages = packages;
        let why = enterprise_core::Content::from_pack(&bad).expect_err("refused");
        assert!(why.contains(says), "{why}");
    }
}
