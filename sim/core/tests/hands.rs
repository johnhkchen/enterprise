//! Hands' ids (`roster.rs`, `hands.rs`): every hand a society holds has an id it keeps while it lives, wherever it
//! goes, never another's, and the host's name for it beside it; what the core keeps by hand follows the id, not the
//! hand's place in its roster (which shifts as others leave: `settlers::take_hand`).

use enterprise_core::wire::{self, Answer, Command, Input, Query, Request, Response};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const LOOK: u64 = 1_200;

fn ids(s: &Society, town: usize) -> Vec<u32> {
    s.roster(town).hands.iter().map(|h| h.id).collect()
}

fn trades(s: &Society, town: usize) -> Vec<String> {
    s.roster(town).hands.iter().map(|h| s.content().trades[h.trade].name.clone()).collect()
}

fn simple() -> Content {
    Content::new(vec![Good::new("wheat", 0.05, 64), Good::new("paper", 1.0 / 24.0, 64), Good::new("bread", 0.2, 64).food(5.0)],
        DAY)
}

#[test]
fn every_hand_gets_an_id_of_its_own_as_it_joins() {
    let mut s = Society::new(simple());
    let a = s.add_town("Ashford", 10.0);
    let b = s.add_town("Brent", 10.0);
    s.set_roster(a, &[("merchant", 1), ("farmer", 2), ("librarian", 1)]);
    s.set_roster(b, &[("mason", 1)]);
    assert_eq!((ids(&s, a), ids(&s, b)), (vec![1, 2, 3], vec![4]), "numbered as they join, across towns");
    let id = s.join(b, "minecraft:farmer", 3, "villager-7").unwrap();
    assert_eq!(id, 5);
    assert_eq!(s.hand_tag(id), Some("villager-7"));
    assert_eq!(s.hand_tagged("villager-7"), Some(id));
    assert_eq!(s.find_hand(id), Some((b, 1)));
    let h = s.roster(b).hands[1].clone();
    assert_eq!((h.level, h.xp), (3, 70), "at its level, with the XP it took");
    assert!(s.join(a, "farmer", 1, "villager-7").is_err(), "a name another hand has");
    assert!(s.join(a, "baker", 1, "").is_err(), "a trade the core hasn't");
    assert!(s.join(a, "farmer", 6, "").is_err(), "past Master");
    assert!(s.join(9, "farmer", 1, "").is_err(), "a town that isn't there");
    assert_eq!(s.join(a, "farmer", 1, "").unwrap(), 6, "a name isn't needed");
}

#[test]
fn a_hand_lost_from_the_middle_leaves_the_others_their_ids_and_lockers() {
    // The Librarian's offer is switched on; the Farmer before it in the roster dies: the Librarian, now second, keeps
    // its id and its locker with its offer on (a locker follows its hand's id, not its place).
    let mut s = Society::new(simple());
    let t = s.add_town("Ashford", 10.0);
    s.set_roster(t, &[("merchant", 1), ("farmer", 1), ("librarian", 1), ("cartographer", 1)]);
    s.switch(t, 2, 0, true);
    let before = ids(&s, t);
    let librarian = before[2];
    assert!(s.lockers(t).unwrap().hands[2].offers[0].on);
    assert_eq!(s.lose_hand(before[1]), Ok(t));
    assert_eq!(ids(&s, t), vec![before[0], before[2], before[3]]);
    assert_eq!(trades(&s, t), ["merchant", "librarian", "cartographer"]);
    assert_eq!(s.town(t).people.villagers, 3.0, "its villagers follow the roster");
    let lockers = s.lockers(t).unwrap();
    assert_eq!(lockers.hands[1].hand, librarian, "the Librarian's locker moved up with it");
    assert!(lockers.hands[1].offers[0].on, "its offer still on");
    assert!(lockers.hands[2].offers.iter().all(|o| !o.on), "the Cartographer's still off");
    assert_eq!(s.roster(t).merchant, Some(0), "the desk's Merchant where it was");
    assert!(s.lose_hand(before[1]).is_err(), "a hand lost already");
    assert_eq!(s.join(t, "farmer", 1, "").unwrap(), before[3] + 1, "an id never given before, not the dead Farmer's");
}

#[test]
fn a_hand_in_a_new_trade_keeps_its_id() {
    // A squad's trade taken on (`squad::sign_on`) and a job taken (`jobs.rs`, the hand made anew) keep the villager.
    let c = simple();
    let mut roster = enterprise_core::Roster::new(&c, vec![enterprise_core::Hand::new(&c, c.trade("none").unwrap(), 1)]);
    roster.hands[0].id = 41;
    enterprise_core::squad::sign_on(&c, &mut roster, None, 0, c.trade("cartographer").unwrap());
    assert_eq!((roster.hands[0].id, c.trades[roster.hands[0].trade].name.as_str()), (41, "cartographer"));
}

#[test]
fn a_crew_comes_home_with_the_ids_it_left_with() {
    // The test network of `tests/squad.rs`: a hub's Cartographer and Librarian go to a far end four blocks away and
    // come back the same hands, by id; a hand at home lost while they're away doesn't shift them.
    let mut s = Society::new(simple());
    s.content_mut().dials.standing_orders = false;
    let hub = s.add_town("Hub", 5.0);
    let dest = s.add_town("Dest", 12.0);
    s.record_capacity(dest, 1, &[(1, 64.0 / 5.0)]);
    s.set_trade(dest, 1, 0.0, 1e-9);
    s.deliver(hub, 1, 128.0);
    s.deliver(hub, 2, 70.0);
    s.deliver(dest, 2, 2.0);
    s.set_roster(hub, &[("merchant", 1), ("farmer", 1), ("cartographer", 2), ("librarian", 2)]);
    for i in 1..4 {
        s.set_offers(hub, i, Vec::new());
    }
    s.add_route(hub, dest, 4.0 / 1200.0, 0.0, 4.0);
    s.want_trades(0, &["cartographer", "librarian"]);
    let before = ids(&s, hub);
    s.advance(LOOK);
    assert_eq!(s.crew(0).crew.len(), 2, "both go: {:?}", s.crew(0));
    let away: Vec<u32> = s.roster(hub).hands.iter().filter(|h| h.away).map(|h| h.id).collect();
    assert_eq!(away, vec![before[2], before[3]], "away by the ids they had at home");
    assert!(s.lose_hand(before[2]).is_err(), "a hand away is the core's, not the host's to lose");
    s.lose_hand(before[1]).unwrap(); // the Farmer at home dies while they're on the road
    s.advance(5 * LOOK);
    assert!(!s.route(0).on_the_road, "home");
    let roster = s.roster(hub);
    assert!(roster.hands.iter().all(|h| !h.away), "both back");
    assert_eq!(ids(&s, hub), vec![before[0], before[2], before[3]], "the same hands, by id");
    assert_eq!(trades(&s, hub), ["merchant", "cartographer", "librarian"]);
}

#[test]
fn settlers_carry_their_ids_and_names_to_the_village_they_settle() {
    // `tests/settlers.rs`'s town past growing sends its Merchant to a village nobody trades with.
    let mut s = Society::new(Content::new(vec![Good::new("bread", 0.2, 64).food(5.0)], DAY));
    s.content_mut().dials.budget_share = 1.0;
    let home = s.add_town("Home", 14.0);
    // The party is the first idle hand at home, who goes as the village's Merchant: one the host named.
    s.set_roster(home, &[("merchant", 2), ("farmer", 1), ("librarian", 1), ("cartographer", 1), ("mason", 1), ("nitwit", 1)]);
    let merchant = s.join(home, "none", 1, "villager-m").unwrap();
    for _ in 0..3 {
        s.join(home, "none", 1, "").unwrap();
    }
    s.set_people(home, 10.0, 12.0, 12.0);
    s.deliver(home, 0, 150.0);
    s.place_town(home, Hex::new(0, 0).center_x(), Hex::new(0, 0).center_z());
    let v = s.add_village(Hex::new(5, 0).center_x(), Hex::new(5, 0).center_z());
    let at_home = ids(&s, home);
    s.advance(DAY / 4);
    assert_eq!(s.parties().len(), 1, "a party left");
    let left: Vec<u32> = at_home.iter().copied().filter(|id| !ids(&s, home).contains(id)).collect();
    assert_eq!(left, vec![merchant], "the named hand went");
    assert_eq!(s.hand_tagged("villager-m"), Some(merchant), "on the road, off every roster, it's still called so");
    assert!(s.can_be_hand("farmer", 1, "villager-m").is_err(), "and the name is taken");
    s.advance(DAY);
    assert!(s.parties().is_empty(), "arrived");
    let there = ids(&s, v);
    assert!(left.iter().all(|id| there.contains(id)), "it's at the village by the id it had at home: {there:?}");
    assert_eq!(s.find_hand(merchant).map(|(t, _)| t), Some(v));
    assert_eq!(s.roster(v).merchant.map(|m| s.roster(v).hands[m].id), Some(merchant), "the village's Merchant now");
    assert_eq!(s.hand_tag(merchant), Some("villager-m"));
    let mut all: Vec<u32> = (0..s.town_count()).flat_map(|t| ids(&s, t)).collect();
    let n = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), n, "no two hands alike");
}

#[test]
fn a_child_born_has_an_id_never_given_before() {
    // `tests/people.rs`'s content town with room to grow, its people a roster.
    let mut goods: Vec<Good> = Content::COMFORTS.iter().enumerate()
        .map(|(k, name)| Good::new(&format!("{name}_good"), 0.05, 64).comfort(k)).collect();
    goods.push(Good::new("wheat", 0.05, 64));
    goods.push(Good::new("bread", 0.2, 64).food(5.0));
    let mut s = Society::new(Content::new(goods, DAY));
    s.content_mut().dials.standing_orders = false;
    let t = s.add_town("Meadowworth", 60.0);
    s.set_people(t, 6.0, 6.0, 40.0);
    s.set_roster(t, &[("merchant", 1), ("farmer", 1), ("none", 1), ("none", 1), ("none", 1), ("none", 1)]);
    s.set_trade(t, 5, 128.0, 0.0);
    for k in 0..5 {
        s.deliver(t, k, (6.0f64 * 0.1 * 60.0 / 0.05).ceil());
    }
    s.deliver(t, 6, 704.0);
    let before = ids(&s, t);
    s.advance(60 * DAY);
    let roster = s.roster(t);
    assert!(s.town(t).people.born >= 1, "a child born: {:?}", s.town(t).people);
    let born: Vec<u32> = roster.hands[before.len()..].iter().map(|h| h.id).collect();
    assert!(!born.is_empty() && born.iter().all(|id| *id > *before.iter().max().unwrap()), "new ids: {born:?}");
    assert_eq!(&ids(&s, t)[..before.len()], &before[..], "the others' as they were");
}

#[test]
fn ids_and_names_come_back_from_a_save_and_go_on_from_there() {
    let mut s = Society::new(simple());
    let t = s.add_town("Ashford", 10.0);
    s.join(t, "merchant", 1, "villager-m").unwrap();
    s.join(t, "farmer", 1, "villager-f").unwrap();
    let gone = s.join(t, "librarian", 1, "villager-l").unwrap();
    s.lose_hand(gone).unwrap();
    let mut loaded = Society::load(&s.save()).unwrap();
    assert_eq!(ids(&loaded, t), ids(&s, t));
    assert_eq!(loaded.hand_tagged("villager-f"), Some(2));
    assert_eq!(loaded.hand_tagged("villager-l"), None, "a lost hand's name forgotten");
    assert_eq!(loaded.join(t, "mason", 1, "villager-l").unwrap(), gone + 1, "the count goes on from the save");
}

#[test]
fn a_host_finds_its_villagers_hands_by_asking() {
    // Through the wire, as the binding does: AddHand with the villager's UUID, then Query::Hands.
    let mut society = None;
    let mut call = |command: Command| wire::handle(&mut society, Request::new(command));
    let pack = wire::ContentPack::new(DAY, vec![wire::PackGood::new("minecraft:wheat", 0.05, 64)]);
    assert!(matches!(call(Command::Create { pack, seed: 1 }), Response::Done { .. }));
    let desk = wire::Point { kind: wire::PointKind::Desk, dim: "minecraft:overworld".into(), x: 3, y: 64, z: 3 };
    let r = call(Command::Apply { inputs: vec![
        Input::AddTown { name: "Ashford".into(), desk },
        Input::AddHand { town: 0, trade: "enterprise:merchant".into(), level: 2, tag: "uuid-1".into() },
        Input::AddHand { town: 0, trade: "minecraft:librarian".into(), level: 1, tag: "uuid-2".into() },
    ] });
    assert!(matches!(r, Response::Done { .. }), "{r:?}");
    let Response::Done { answers, .. } = call(Command::Ask { queries: vec![Query::Hands { town: 0 }] }) else { panic!() };
    let Answer::Hands { lockers, hands, .. } = &answers[0] else { panic!("{answers:?}") };
    assert_eq!(*lockers, 4, "a Journeyman Merchant manages four");
    let found: Vec<(u32, &str, &str, bool)> = hands.iter().map(|h| (h.hand, h.tag.as_str(), h.trade.as_str(), h.merchant)).collect();
    assert_eq!(found, vec![(1, "uuid-1", "merchant", true), (2, "uuid-2", "librarian", false)]);
    let r = call(Command::Apply { inputs: vec![Input::HandLost { hand: 1 }] });
    assert!(matches!(r, Response::Done { .. }));
    let Response::Done { answers, .. } = call(Command::Ask { queries: vec![Query::Hands { town: 0 }] }) else { panic!() };
    let Answer::Hands { lockers, hands, .. } = &answers[0] else { panic!() };
    assert_eq!((*lockers, hands.len(), hands[0].hand), (0, 1, 2), "no Merchant, no lockers; the Librarian still 2");
}
