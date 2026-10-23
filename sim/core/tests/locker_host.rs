//! A host's lockers (`locker_host.rs`, task `locker-adapter`): a town founded by its host has a locker the Merchant can
//! manage only where the host placed one; the Merchant's screen switches a hand's offers by their places; a locker point
//! broken, a hand gone or a point tied to another hand takes the tie; the restocks a town missed are traded at its next
//! look with nothing counted (no rounds, no catch-up on load); all kept by a save.

use crate::common;

use common::DAY;
use enterprise_core::points::Spot;
use enterprise_core::wire::*;
use enterprise_core::Society;

const PAPER: GoodNo = 0;
const WHEAT: GoodNo = 1;

fn pack() -> ContentPack {
    ContentPack::new(DAY, vec![PackGood::new("minecraft:paper", 0.04, 64), PackGood::new("minecraft:wheat", 0.05, 64)])
}

fn at(kind: PointKind, x: i32) -> Point {
    Point { kind, dim: "minecraft:overworld".into(), x, y: -60, z: 0 }
}

fn desk() -> Point {
    at(PointKind::Desk, 0)
}

fn locker(x: i32) -> Point {
    at(PointKind::Locker, x)
}

fn call(s: &mut Option<Society>, command: Command) -> Response {
    handle(s, Request::new(command))
}

fn apply(s: &mut Option<Society>, inputs: Vec<Input>) -> Vec<Output> {
    match call(s, Command::Apply { inputs }) {
        Response::Done { outputs, .. } => outputs,
        r => panic!("applied: {r:?}"),
    }
}

fn lockers(s: &mut Option<Society>) -> (u32, bool, Vec<LockerView>) {
    match call(s, Command::Ask { queries: vec![Query::Lockers { town: 0 }] }) {
        Response::Done { answers, .. } => match answers.into_iter().next() {
            Some(Answer::Lockers { limit, placed, lockers, .. }) => (limit, placed, lockers),
            a => panic!("{a:?}"),
        },
        r => panic!("{r:?}"),
    }
}

fn paper(s: &Option<Society>) -> f64 {
    s.as_ref().unwrap().town(0).stock[PAPER as usize]
}

/// A town at its desk with a Merchant of `level` and a Librarian, 1000 paper delivered; returns the Librarian's hand.
fn town(merchant: u8) -> (Option<Society>, u32) {
    let mut s = None;
    assert_eq!(call(&mut s, Command::Create { pack: pack(), seed: 7 }), Response::Done { outputs: vec![], answers: vec![] });
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk: desk() },
        Input::AddHand { town: 0, trade: "merchant".into(), level: merchant, tag: "m".into() },
        Input::AddHand { town: 0, trade: "librarian".into(), level: 1, tag: "l".into() },
        Input::Deliver { town: 0, good: PAPER, count: 1000.0 },
    ]);
    let hand = s.as_ref().unwrap().hand_tagged("l").unwrap();
    (s, hand)
}

/// The Librarian's paper offer's place.
fn paper_offer(s: &mut Option<Society>) -> u32 {
    lockers(s).2[0].offers.iter().find(|o| o.good == PAPER && !o.sells).expect("a paper offer").offer
}

fn place(s: &mut Option<Society>, hand: u32, x: i32) {
    apply(s, vec![Input::AddPoint { point: locker(x), town: 0 }, Input::Locker { hand, at: Some(locker(x)) }]);
}

#[test]
fn a_town_its_host_founded_has_a_locker_only_where_its_host_placed_one() {
    let (mut s, librarian) = town(1);
    let offer = paper_offer(&mut s);
    let (limit, placed, views) = lockers(&mut s);
    assert!(placed && limit == 2, "{limit} {placed}");
    assert!(!views[0].managed && views[0].at.is_none(), "{views:?}");
    apply(&mut s, vec![Input::Switch { hand: librarian, offer, good: PAPER, on: true }, Input::Advance { to: 3 * DAY }]);
    assert_eq!(paper(&s), 1000.0, "a Librarian with no locker traded");
    place(&mut s, librarian, 3);
    let views = lockers(&mut s).2;
    assert!(views[0].managed && views[0].at == Some(locker(3)), "{views:?}");
    apply(&mut s, vec![Input::Advance { to: 6 * DAY }]);
    assert!(paper(&s) < 1000.0, "its locker placed, it never traded");
}

#[test]
fn a_town_the_core_keeps_as_numbers_has_every_hands_locker() {
    let mut s = Society::new(enterprise_core::Content::from_pack(&pack()).unwrap());
    let t = s.add_town("Ashford", 0);
    s.join(t, "merchant", 1, "").unwrap();
    s.join(t, "librarian", 1, "").unwrap();
    let mut live = Some(s);
    let (_, placed, views) = lockers(&mut live);
    assert!(!placed && views[0].managed && views[0].at.is_none(), "{placed} {views:?}");
}

#[test]
fn the_screen_switches_an_offer_by_its_place_and_ignores_a_stale_one() {
    let (mut s, librarian) = town(1);
    place(&mut s, librarian, 3);
    let offer = paper_offer(&mut s);
    apply(&mut s, vec![
        Input::Switch { hand: librarian, offer, good: WHEAT, on: true },
        Input::Switch { hand: librarian, offer: 99, good: PAPER, on: true },
        Input::Switch { hand: 99, offer, good: PAPER, on: true },
    ]);
    assert!(lockers(&mut s).2[0].offers.iter().all(|o| !o.on), "a stale switch switched an offer");
    apply(&mut s, vec![Input::Switch { hand: librarian, offer, good: PAPER, on: true }]);
    assert!(lockers(&mut s).2[0].offers.iter().any(|o| o.offer == offer && o.on));
    apply(&mut s, vec![Input::Switch { hand: librarian, offer, good: PAPER, on: false }]);
    assert!(lockers(&mut s).2[0].offers.iter().all(|o| !o.on));
    let r = call(&mut s, Command::Apply { inputs: vec![Input::Switch { hand: librarian, offer, good: 9, on: true }] });
    assert!(matches!(r, Response::Refused { why: Refusal::Input { at: 0, .. } }), "a good that isn't: {r:?}");
}

#[test]
fn the_screen_shows_the_hands_offers_at_its_level_at_the_merchants_price() {
    let (mut s, librarian) = town(5);
    place(&mut s, librarian, 3);
    let views = lockers(&mut s).2;
    let o = views[0].offers.iter().find(|o| o.good == PAPER).unwrap();
    // A master Merchant's reputation, 110 x 0.05: 5 paper off the 24.
    assert_eq!((o.count, o.price, o.uses, o.level, o.inflated), (24, 19, 16, 1, false), "{o:?}");
    assert!(views[0].offers.iter().all(|o| o.level <= 1), "offers above a Novice's level shown: {views:?}");
}

#[test]
fn a_locker_broken_a_hand_gone_or_a_second_hand_at_it_takes_the_tie() {
    let (mut s, librarian) = town(1);
    place(&mut s, librarian, 3);
    apply(&mut s, vec![Input::RemovePoint { point: locker(3) }]);
    assert!(lockers(&mut s).2[0].at.is_none(), "a locker broken still stands");
    place(&mut s, librarian, 4);
    apply(&mut s, vec![Input::AddHand { town: 0, trade: "librarian".into(), level: 1, tag: "l2".into() }]);
    let second = s.as_ref().unwrap().hand_tagged("l2").unwrap();
    apply(&mut s, vec![Input::Locker { hand: second, at: Some(locker(4)) }]);
    let views = lockers(&mut s).2;
    let at_of = |h: u32| views.iter().find(|v| v.hand == h).unwrap().at.clone();
    assert_eq!((at_of(librarian), at_of(second)), (None, Some(locker(4))), "{views:?}");
    apply(&mut s, vec![Input::HandLost { hand: second }]);
    let s2 = s.as_ref().unwrap();
    let placed = s2.lockers(0).unwrap().placed.unwrap();
    assert!(placed.is_empty(), "a hand gone kept its locker: {placed:?}");
    // A point of another town's is no locker of this one's.
    apply(&mut s, vec![
        Input::AddTown { name: "Brent".into(), desk: at(PointKind::Desk, 100) },
        Input::AddPoint { point: locker(101), town: 1 },
        Input::Locker { hand: librarian, at: Some(locker(101)) },
    ]);
    assert!(lockers(&mut s).2[0].at.is_none(), "a hand's locker at another town's point");
}

/// What `Rounds` and `LockerCatchUp` did for an unloaded market (forced its chunks loaded, replayed its villagers'
/// restocks on load) the core does at its looks, loaded or not: three days with nothing counted are six restocks, every
/// other one inflated (`LockerTradingTests.unloadedDaysAreReplayedOnLoad`: 48 trades).
#[test]
fn the_restocks_a_town_missed_are_traded_with_nothing_counted() {
    let (mut s, librarian) = town(1);
    place(&mut s, librarian, 3);
    let offer = paper_offer(&mut s);
    apply(&mut s, vec![Input::Switch { hand: librarian, offer, good: PAPER, on: true }, Input::Advance { to: DAY / 2 },
        Input::Deliver { town: 0, good: PAPER, count: 1000.0 }]);
    let before = (paper(&s), s.as_ref().unwrap().lockers(0).unwrap().trades);
    apply(&mut s, vec![Input::Advance { to: DAY / 2 + 3 * DAY }]);
    let after = (paper(&s), s.as_ref().unwrap().lockers(0).unwrap().trades);
    assert_eq!(after.1 - before.1, 48, "trades in three days unseen: {before:?} {after:?}");
    assert_eq!(before.0 - after.0, 48.0 * 24.0);
}

#[test]
fn a_towns_lockers_and_their_switches_are_kept_by_a_save() {
    let (mut s, librarian) = town(1);
    place(&mut s, librarian, 3);
    let offer = paper_offer(&mut s);
    apply(&mut s, vec![Input::Switch { hand: librarian, offer, good: PAPER, on: true }]);
    let before = lockers(&mut s);
    let blob = s.as_mut().unwrap().save();
    let mut loaded = Some(Society::load(&blob).unwrap());
    assert_eq!(lockers(&mut loaded), before);
    assert_eq!(loaded.as_ref().unwrap().lockers(0).unwrap().placed.unwrap().get(&librarian),
        Some(&Spot { dim: "minecraft:overworld".into(), x: 3, y: -60, z: 0 }));
}
