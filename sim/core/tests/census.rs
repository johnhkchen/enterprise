//! The host's census as a town's roster's source, and its villagers as views of the core's hands (`census.rs`): what a
//! census sees joins, is seen or comes back as gone; what isn't seen for ten days has left; and what the rules do to
//! hands in an `Advance` (a child born, a party leaving and settling, a hand gone, a trade or level changed) comes out
//! for the host's villagers.

use enterprise_core::wire::{self, Answer, Command, ContentPack, Input, Output, PackGood, Query, Request, Resident, Response};
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;
const BREAD: usize = 0;

fn content() -> Content {
    Content::new(vec![Good::new("bread", 0.2, 64).food(5.0)], DAY)
}

fn seen(tag: &str, trade: &str, level: u8) -> Resident {
    Resident { tag: tag.into(), trade: trade.into(), level, baby: false }
}

fn child(tag: &str) -> Resident {
    Resident { tag: tag.into(), trade: "minecraft:none".into(), level: 1, baby: true }
}

/// A town's hands as (tag, trade, level, baby).
fn hands(s: &Society, town: usize) -> Vec<(String, String, u8, bool)> {
    s.hand_views(town).into_iter().map(|h| (h.tag, h.trade, h.level, h.baby)).collect()
}

fn hand(tag: &str, trade: &str, level: u8, baby: bool) -> (String, String, u8, bool) {
    (tag.into(), trade.into(), level, baby)
}

// The beds are counted by the host (`SettlementTests.bellBedsAndWorkstationsMakeASettlement`, Minecraft's own); the core takes them.
#[test]
fn a_census_joins_the_villagers_it_sees_and_counts_the_beds() {
    let mut s = Society::new(content());
    let t = s.add_town("Ashford", 10.0);
    let out = s.census(t, 4, &[
        seen("m", "enterprise:merchant", 2),
        seen("f", "minecraft:farmer", 3),
        child("c"),
        seen("b", "minecraft:baker", 1), // a trade the core hasn't: not counted
        seen("", "minecraft:mason", 1),   // no tag: not counted
        seen("f", "minecraft:mason", 1),  // listed twice: counted once
        seen("x", "minecraft:cleric", 9), // past Master: Master
    ]);
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(hands(&s, t), vec![hand("m", "merchant", 2, false), hand("f", "farmer", 3, false), hand("c", "none", 1, true),
        hand("x", "cleric", 5, false)]);
    assert_eq!(s.roster(t).merchant, Some(0), "the Merchant is the desk's");
    assert_eq!((s.town(t).people.beds, s.town(t).people.villagers), (4.0, 4.0));
    // Seen again: nothing joins twice.
    s.census(t, 5, &[seen("m", "enterprise:merchant", 2), seen("f", "farmer", 3), child("c"), seen("x", "cleric", 5)]);
    assert_eq!(s.roster(t).residents(), 4);
    assert_eq!(s.town(t).people.beds, 5.0, "the beds are the world's");
    // A child seen grows up by the time elapsed, as the core's own do.
    s.advance(DAY + 1);
    assert!(!s.roster(t).hands[2].is_baby());
}

#[test]
fn a_census_takes_a_profession_and_a_level_from_the_world_but_never_less() {
    let mut s = Society::new(content());
    let t = s.add_town("Ashford", 10.0);
    s.census(t, 4, &[seen("m", "merchant", 2), seen("f", "farmer", 3), seen("n", "none", 1)]);
    // The farmer took a lectern in the world (a Librarian now) but its level there is lower than the core's; the idle
    // villager took a composter; the Merchant shows a level earned at the desk.
    s.census(t, 4, &[seen("m", "merchant", 3), seen("f", "librarian", 2), seen("n", "farmer", 1)]);
    assert_eq!(hands(&s, t), vec![hand("m", "merchant", 3, false), hand("f", "librarian", 3, false), hand("n", "farmer", 1, false)]);
    assert_eq!(s.roster(t).hands[0].xp, 1395, "a Merchant's XP, at least what its level took: a player's at level 30");
    // A villager that lost its workstation in the world (no trade) keeps the core's; the desk's Merchant keeps its own.
    s.census(t, 4, &[seen("m", "farmer", 3), seen("f", "none", 4), seen("n", "nitwit", 1)]);
    assert_eq!(hands(&s, t), vec![hand("m", "merchant", 3, false), hand("f", "librarian", 4, false), hand("n", "nitwit", 1, false)]);
    assert_eq!(s.roster(t).hands[1].xp, 150, "a level earned in the world brings the XP it took");
}

// The core keeps the unseen ten days, as `CensusTaker.finish` does (`SettlementTests.theRosterCountsVillagersOutOfSight`,
// Minecraft's own: the chunks it sends for are the host's).
#[test]
fn a_hand_unseen_for_ten_days_has_left() {
    let mut s = Society::new(content());
    let t = s.add_town("Ashford", 10.0);
    s.census(t, 6, &[seen("a", "farmer", 1), seen("b", "mason", 1), seen("c", "fletcher", 1)]);
    let untagged = s.join(t, "cleric", 1, "").unwrap();
    // c goes out with a caravan (away: off the host's world, never looked for at home).
    let c = s.hand_tagged("c").unwrap();
    let (_, i) = s.find_hand(c).unwrap();
    let mut roster = s.roster(t);
    roster.hands[i].away = true;
    let e = s.world.resource::<enterprise_core::state::Towns>().0[t];
    s.world.entity_mut(e).insert(roster);
    s.advance(5 * DAY);
    s.census(t, 6, &[seen("a", "farmer", 1)]);
    assert_eq!(s.roster(t).residents(), 4, "five days unseen: still a resident");
    s.advance(10 * DAY);
    s.census(t, 6, &[seen("a", "farmer", 1)]);
    assert_eq!(s.roster(t).residents(), 4, "ten days unseen to the tick: still a resident");
    s.advance(10 * DAY + 1);
    s.census(t, 6, &[seen("a", "farmer", 1)]);
    let left: Vec<u32> = s.roster(t).hands.iter().map(|h| h.id).collect();
    assert_eq!(left, vec![s.hand_tagged("a").unwrap(), c, untagged], "b has left; the away and the untagged stay");
    assert_eq!(s.hand_tagged("b"), None, "and its name is free");
    assert_eq!(s.town(t).people.villagers, 3.0);
    // A census that sees nobody at all forgets nobody seen within ten days.
    s.census(t, 6, &[]);
    assert_eq!(s.roster(t).residents(), 3);
}

#[test]
fn a_villager_of_a_hand_away_or_another_towns_is_left_as_the_core_has_it() {
    let mut s = Society::new(content());
    let a = s.add_town("Ashford", 10.0);
    let b = s.add_town("Brent", 10.0);
    s.census(a, 4, &[seen("v", "farmer", 2)]);
    s.census(b, 4, &[seen("v", "librarian", 5)]);
    assert_eq!((s.roster(a).residents(), s.roster(b).residents()), (1, 0), "still Ashford's, and not Brent's too");
    assert_eq!(hands(&s, a), vec![hand("v", "farmer", 2, false)], "seen at another town: not its trade or level there");
    // Seen at Brent counts as seen: eleven days on, Ashford's census without it still keeps it.
    s.advance(6 * DAY);
    s.census(b, 4, &[seen("v", "librarian", 5)]);
    s.advance(11 * DAY);
    s.census(a, 4, &[]);
    assert_eq!(s.roster(a).residents(), 1);
}

#[test]
fn a_villager_whose_hand_is_gone_comes_back_as_gone() {
    let mut s = Society::new(content());
    let t = s.add_town("Ashford", 10.0);
    s.census(t, 4, &[seen("v", "farmer", 2), seen("w", "mason", 1)]);
    let v = s.hand_tagged("v").unwrap();
    // The rules take v from the world (a soldier fallen: off the roster, its tag kept), as `army.rs` does.
    let (_, i) = s.find_hand(v).unwrap();
    let mut roster = s.roster(t);
    enterprise_core::settlers::take_hand(&mut roster, i);
    let e = s.world.resource::<enterprise_core::state::Towns>().0[t];
    s.world.entity_mut(e).insert(roster);
    let out = s.census(t, 4, &[seen("v", "farmer", 2), seen("w", "mason", 1)]);
    assert_eq!(out, vec![Output::HandGone { hand: v, tag: "v".into() }], "its villager should no longer stand");
    assert_eq!(s.roster(t).residents(), 1, "and it doesn't join again");
}

#[test]
fn what_the_rules_do_to_hands_comes_out_for_their_villagers() {
    let mut s = Society::new(content());
    let t = s.add_town("Ashford", 10.0);
    s.census(t, 4, &[seen("f", "farmer", 1), seen("n", "none", 1), seen("g", "mason", 1)]);
    s.join(t, "cleric", 1, "").unwrap();
    let (f, n, g) = (s.hand_tagged("f").unwrap(), s.hand_tagged("n").unwrap(), s.hand_tagged("g").unwrap());
    let before = s.hands_now();
    let e = s.world.resource::<enterprise_core::state::Towns>().0[t];
    let mut roster = s.roster(t);
    roster.hands[0].xp += 4; // a locker's trade
    roster.hands[1].trade = s.content().trade("fletcher").unwrap(); // a trade taken up
    roster.hands[3].level = 2; // the cleric levels, but no villager stands for it
    enterprise_core::settlers::take_hand(&mut roster, 2); // the mason taken from the world
    s.world.entity_mut(e).insert(roster);
    assert_eq!(s.hand_outputs(&before), vec![
        Output::HandLevel { hand: f, tag: "f".into(), trade: "farmer".into(), level: 1, xp: 4 },
        Output::HandLevel { hand: n, tag: "n".into(), trade: "fletcher".into(), level: 1, xp: 0 },
        Output::HandGone { hand: g, tag: "g".into() },
    ]);
}

/// A town of ten at the origin grown past growing, five of them idle, named by the host (`tests/settlers.rs`'s home).
fn grown(s: &mut Society) -> usize {
    let t = s.add_town("Home", 14.0);
    s.set_roster(t, &[("merchant", 2), ("farmer", 1), ("librarian", 1), ("cartographer", 1), ("mason", 1), ("none", 1),
        ("none", 1), ("none", 1), ("nitwit", 1), ("none", 1)]);
    s.set_people(t, 10.0, 12.0, 12.0);
    s.deliver(t, BREAD, 150.0);
    s.place_town(t, Hex::new(0, 0).center_x(), Hex::new(0, 0).center_z());
    for (i, id) in s.roster(t).hands.iter().map(|h| h.id).enumerate().collect::<Vec<_>>() {
        assert!(s.tag_hand(id, &format!("villager-{i}")));
    }
    t
}

// parity-partial: ExpeditionTests.aMerchantSettlesAVillageNobodyTradesWith (the party's villagers leave the world and come to the village as views; the bell, beds and desk are Minecraft's)
#[test]
fn a_party_of_settlers_departs_its_villagers_and_settles_new_ones() {
    let mut s = Society::new(content());
    s.content_mut().dials.budget_share = 1.0;
    let home = grown(&mut s);
    let v = s.add_village(Hex::new(7, 0).center_x(), Hex::new(7, 0).center_z());
    let villagers: Vec<u32> = s.roster(v).hands.iter().map(|h| h.id).collect();
    let out = s.apply(&Input::Advance { to: DAY / 4 });
    assert_eq!(s.parties()[0].hands, 1);
    let gone = s.world.resource::<enterprise_core::settlers::Settlers>().parties[0].hands[0].id;
    let tag = "villager-5".to_string();
    assert_eq!(out, vec![Output::Departed { town: home as u32, hand: gone, tag: tag.clone() }],
        "the first idle hand's villager leaves the world");
    assert_eq!(s.hand_tag(gone), None, "its tag forgotten");
    // Its old villager, still about (in a chunk not loaded when it left), is seen by a census: it should go.
    let out = s.census(home, 12, &[seen(&tag, "none", 1)]);
    assert_eq!(out, vec![Output::HandGone { hand: gone, tag: tag.clone() }]);
    assert_eq!(s.roster(home).residents(), 9, "and it doesn't join again");
    let out = s.apply(&Input::Advance { to: DAY });
    assert_eq!(out, vec![Output::Settled { town: v as u32, hand: gone }], "the party's Merchant comes to the village");
    assert!(s.roster(v).hands.iter().any(|h| h.id == gone));
    assert_eq!(s.roster(v).hands.iter().filter(|h| villagers.contains(&h.id)).count(), 3, "beside the village's own");
    // The host spawns a villager for it and names it: the census then knows it.
    assert!(s.tag_hand(gone, "villager-new"));
    assert!(s.census(v, 3, &[seen("villager-new", "merchant", 1)]).is_empty());
    assert_eq!(s.roster(v).residents(), 4);
}

fn request(command: Command) -> Request {
    Request::new(command)
}

fn apply(s: &mut Option<Society>, inputs: Vec<Input>) -> Vec<Output> {
    match wire::handle(s, request(Command::Apply { inputs })) {
        Response::Done { outputs, .. } => outputs,
        r => panic!("{r:?}"),
    }
}

fn hands_of(s: &mut Option<Society>, town: u32) -> Vec<wire::HandView> {
    match wire::handle(s, request(Command::Ask { queries: vec![Query::Hands { town }] })) {
        Response::Done { mut answers, .. } => match answers.remove(0) {
            Answer::Hands { hands, .. } => hands,
            a => panic!("{a:?}"),
        },
        r => panic!("{r:?}"),
    }
}

// parity-partial: GrowthTests.aContentTownWithASpareBedHasAChild (the child is born in the core and comes out for the host to spawn by the bell and name)
#[test]
fn a_child_born_in_the_core_comes_out_for_the_host_to_spawn_and_name() {
    let mut bread = PackGood::new("minecraft:bread", 0.2, 64);
    bread.food = 5.0;
    let pack = ContentPack::new(DAY, vec![bread]);
    let mut s = None;
    assert!(matches!(wire::handle(&mut s, request(Command::Create { pack, seed: 7 })), Response::Done { .. }));
    let desk = wire::Point { kind: wire::PointKind::Desk, dim: "minecraft:overworld".into(), x: 0, y: 64, z: 0 };
    let out = apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk },
        Input::Deliver { town: 0, good: 0, count: 64.0 },
        Input::SetDial { name: "birth_mood".into(), value: Some(-100.0) },
        Input::SetDial { name: "birth_cost".into(), value: Some(0.0) },
        Input::Census { town: 0, beds: 4, residents: vec![seen("m", "enterprise:merchant", 1), seen("f", "farmer", 1)] },
    ]);
    assert!(out.is_empty());
    let out = apply(&mut s, vec![Input::Advance { to: 5 * DAY }]);
    let born: Vec<u32> = out.iter().filter_map(|o| match o {
        Output::Born { town: 0, hand } => Some(*hand),
        _ => None,
    }).collect();
    assert_eq!(born.len(), 1, "a child after four days: {out:?}");
    let baby = hands_of(&mut s, 0).into_iter().find(|h| h.hand == born[0]).expect("the child is a hand");
    assert!(baby.baby && baby.tag.is_empty(), "a child with no villager yet: {baby:?}");
    // The host spawns it and names it; its next census sees it as the hand it is.
    let out = apply(&mut s, vec![
        Input::TagHand { hand: born[0], tag: "baby-1".into() },
        Input::Census { town: 0, beds: 4, residents: vec![seen("m", "merchant", 1), seen("f", "farmer", 1), child("baby-1")] },
    ]);
    assert!(out.is_empty());
    let all = hands_of(&mut s, 0);
    assert_eq!(all.len(), 3, "{all:?}");
    assert_eq!(all.iter().find(|h| h.hand == born[0]).map(|h| h.tag.as_str()), Some("baby-1"));
    // A name taken, a hand that isn't there: ignored, not refused; an empty name refused.
    assert!(apply(&mut s, vec![Input::TagHand { hand: born[0], tag: "m".into() }, Input::TagHand { hand: 99, tag: "z".into() }])
        .is_empty());
    assert_eq!(hands_of(&mut s, 0).iter().find(|h| h.hand == born[0]).map(|h| h.tag.clone()), Some("baby-1".into()));
    assert!(matches!(wire::handle(&mut s, request(Command::Apply { inputs: vec![Input::TagHand { hand: born[0], tag: String::new() }] })),
        Response::Refused { why: wire::Refusal::Input { at: 0, .. } }));
    assert!(matches!(wire::handle(&mut s, request(Command::Apply { inputs: vec![
        Input::Census { town: 3, beds: 1, residents: vec![] }] })), Response::Refused { why: wire::Refusal::Input { at: 0, .. } }));
}

#[test]
fn sightings_survive_a_save() {
    let mut s = Society::new(content());
    let t = s.add_town("Ashford", 10.0);
    s.census(t, 4, &[seen("a", "farmer", 1), seen("b", "mason", 1)]);
    s.advance(6 * DAY);
    s.census(t, 4, &[seen("a", "farmer", 1)]);
    let mut loaded = Society::load(&s.save()).unwrap();
    // b was last seen on day 0: eleven days on it has left (a clock started again at the load would keep it).
    loaded.advance(11 * DAY);
    loaded.census(t, 4, &[seen("a", "farmer", 1)]);
    assert_eq!(loaded.roster(t).residents(), 1, "{:?}", loaded.hand_views(t));
}

#[test]
fn a_home_the_core_builds_is_laid_by_the_host_and_its_beds_count_till_then() {
    // A host's town (its land holds a settlers' town's 40 beds), full and content: it builds a home, two beds.
    let pack = ContentPack::new(DAY, vec![PackGood::new("minecraft:bread", 0.2, 64)]);
    let mut s = None;
    assert!(matches!(wire::handle(&mut s, request(Command::Create { pack, seed: 7 })), Response::Done { .. }));
    let desk = wire::Point { kind: wire::PointKind::Desk, dim: "minecraft:overworld".into(), x: 0, y: 64, z: 0 };
    let both = || vec![seen("m", "enterprise:merchant", 1), seen("f", "farmer", 1)];
    apply(&mut s, vec![
        Input::AddTown { name: "Ashford".into(), desk },
        Input::SetDial { name: "home_mood".into(), value: Some(-100.0) },
        Input::SetDial { name: "home_cost".into(), value: Some(0.0) },
        Input::SetDial { name: "birth_mood".into(), value: Some(100.0) },
        Input::Census { town: 0, beds: 2, residents: both() },
    ]);
    let beds = |s: &Option<Society>| s.as_ref().unwrap().town(0).people.beds;
    let homes = |out: &[Output]| out.iter().filter(|o| matches!(o, Output::Build { town: 0, work: wire::Work::Home, .. })).count();
    let survey = Input::Survey { town: 0, bell_x: 0, bell_z: 4 };
    let out = apply(&mut s, vec![Input::Advance { to: 6 * DAY }, survey.clone()]);
    assert_eq!(homes(&out), 1, "a home for the host to lay: {out:?}");
    assert_eq!(beds(&s), 4.0);
    // A census before the host has laid it finds the old two beds: the home's two still count.
    apply(&mut s, vec![Input::Census { town: 0, beds: 2, residents: both() }]);
    assert_eq!(beds(&s), 4.0, "the census took the unlaid home's beds away");
    assert_eq!(homes(&apply(&mut s, vec![survey.clone()])), 1, "asked again until it's laid");
    // Laid: not asked again, and the census counts its beds in the world.
    let out = apply(&mut s, vec![Input::Built { town: 0, work: wire::Work::Home, share: 1.0 }, survey,
        Input::Census { town: 0, beds: 4, residents: both() }]);
    assert_eq!(homes(&out), 0);
    assert_eq!(beds(&s), 4.0);
}
