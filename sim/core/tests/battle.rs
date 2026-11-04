//! A battle between factions for a hex (`battle.rs`, `rules/battle.rs`): seeded between the nearest Horde and Alliance
//! towns, each side's towns levying their hands, who march there and meet every few days; held 30 days in a row, the
//! hex is its holder's to settle and no one else's.

use enterprise_core::army::{self, FOODS};
use enterprise_core::battle::Faction;
use enterprise_core::persons::Grade;
use enterprise_core::{Content, Good, Hex, Society};

const DAY: u64 = 24_000;

fn content() -> Content {
    let mut goods: Vec<Good> = FOODS.iter().map(|f| army::food_good(f.0, 64)).collect();
    for g in army::WEAPONS.iter().chain(army::ARMOUR.iter().flatten()).chain([&army::SHIELD]) {
        goods.push(Good::priced(g.name, 1));
    }
    goods.push(Good::priced("arrow", 64));
    Content::new(goods, DAY)
}

/// A town of `people`, `n` idle hands (level 3) and a Merchant, at hex `(q, r)`, with bread and `swords` iron swords
/// (at `grade`, if any).
fn town(s: &mut Society, name: &str, people: &str, q: i32, r: i32, n: usize, swords: f64, grade: Option<Grade>) -> usize {
    let t = s.add_town(name, 100.0);
    let mut hands = vec![("merchant", 1)];
    hands.extend(std::iter::repeat_n(("none", 3), n));
    s.set_roster(t, &hands);
    let hex = Hex::new(q, r);
    s.place_town(t, hex.center_x(), hex.center_z());
    s.set_people_of(t, people);
    let (sword, bread) = (s.content().good("iron_sword").unwrap(), s.content().good("bread").unwrap());
    s.deliver(t, sword, swords);
    if let Some(g) = grade {
        s.set_graded(t, sword, g, swords);
    }
    s.deliver(t, bread, 3000.0);
    t
}

#[test]
fn peoples_take_their_sides() {
    assert_eq!([Faction::of("orc"), Faction::of("tauren"), Faction::of("minecraft:goblin")], [Some(Faction::Horde); 3]);
    assert_eq!([Faction::of("stormwind"), Faction::of("Lordaeron"), Faction::of("dalaran"), Faction::of("dwarf"),
        Faction::of("nightelf")], [Some(Faction::Alliance); 5]);
    assert_eq!(Faction::of("pirates"), None);
}

#[test]
fn the_horde_and_the_alliance_fight_for_the_hex_between_them_and_its_holder_alone_may_settle_it() {
    let mut s = Society::new(content());
    s.content_mut().dials.battle_day = 1.0;
    s.content_mut().dials.battle_mobilise_days = 0.0;
    s.content_mut().dials.battle_levy = 0.5;
    // The Horde with Fine swords, the Alliance with plain ones; a third town, of no side, out of the way.
    let orgrimmar = town(&mut s, "Orgrimmar", "orc", 0, 0, 20, 20.0, Some(Grade::Fine));
    let stormwind = town(&mut s, "Stormwind", "stormwind", 12, 0, 20, 20.0, None);
    town(&mut s, "Booty Bay", "pirates", 0, 30, 20, 0.0, None);
    s.advance(3 * DAY);
    let b = s.battles().battles.first().cloned().expect("a battle seeded");
    assert_eq!(b.camps, [orgrimmar, stormwind]);
    let (h, a) = (Hex::new(0, 0), Hex::new(12, 0));
    assert!(b.hex.distance(h) >= 4 && b.hex.distance(a) >= 4, "between them: {:?}", b.hex);
    assert!(b.levies[0].iter().all(|l| l.0 == orgrimmar) && b.levies[1].iter().all(|l| l.0 == stormwind), "{:?}", b.levies);
    assert!(!s.battles().may_settle(b.hex, Some(Faction::Horde)) && !s.battles().may_settle(b.hex, Some(Faction::Alliance)));
    s.advance(100 * DAY);
    let b = s.battles().battles[0].clone();
    assert!(b.fights > 0, "they met: {b:?}");
    let won = b.won_by.expect("held 30 days in a row");
    assert_eq!(won, Faction::Horde, "the better kit won it: {:?}", b.armies.iter().map(|a| &a.skirmishes).collect::<Vec<_>>());
    assert!(s.battles().may_settle(b.hex, Some(Faction::Horde)) && !s.battles().may_settle(b.hex, Some(Faction::Alliance))
        && !s.battles().may_settle(b.hex, None));
    assert!(b.armies.iter().all(|a| a.soldiers.is_empty()), "both armies home or on their way");
    // Other ground, anyone.
    assert!(s.battles().may_settle(Hex::new(5, 9), None));
}

#[test]
fn a_town_at_war_builds_a_barracks_which_drills_its_idle_hands_and_kits_out_those_it_sends() {
    use enterprise_core::persons::Work;
    use enterprise_core::works::Kind;
    let mut s = Society::new(content());
    s.content_mut().dials.battle_day = 1.0;
    s.content_mut().dials.battle_mobilise_days = 0.0;
    s.content_mut().dials.battle_levy = 0.25;
    s.content_mut().dials.battle_reinforce_days = 5.0;
    let orgrimmar = town(&mut s, "Orgrimmar", "orc", 0, 0, 20, 40.0, Some(Grade::Fine));
    let stormwind = town(&mut s, "Stormwind", "stormwind", 12, 0, 20, 0.0, None);
    // Orgrimmar's barracks stands already; Stormwind has none.
    s.start_project(orgrimmar, enterprise_core::works::What::Hall(Kind::Barracks));
    s.finish_project(orgrimmar, enterprise_core::works::What::Hall(Kind::Barracks));
    assert!(s.built(orgrimmar, Kind::Barracks));
    s.advance(2 * DAY);
    assert!(s.works(orgrimmar).at_war && s.works(stormwind).at_war, "both levied for the battle");
    // Stormwind's barracks is going up the day it went to war, the Alliance paying its materials and builders' wages.
    assert!(s.works(stormwind).has_started(Kind::Barracks), "{:?}", s.works(stormwind).projects);
    // Its levy came in its Fine swords, out of its warehouse; Stormwind's bare (at the hex, its camp's kit: none).
    let b = s.battles().battles[0].clone();
    let fine = |side: usize| b.armies[side].soldiers.iter()
        .filter(|x| x.weapon.as_ref().is_some_and(|w| w.grade == Some(Grade::Fine))).count();
    assert!(fine(0) > 0 && fine(0) == b.armies[0].soldiers.len(), "{:?}", b.armies[0].soldiers);
    assert_eq!(fine(1), 0);
    // Its idle hands at home drill: more skilled at arms than a Journeyman's 9 after ten days.
    s.advance(12 * DAY);
    let home = s.roster(orgrimmar);
    let drilled = home.hands.iter().filter(|h| !h.away && h.trade != 0)
        .filter_map(|h| s.person(h.id)).filter(|m| m.skill(Work::Arms) > 9.0).count();
    assert!(drilled > 0, "drilled hands");
}

#[test]
fn a_battles_towns_arm_on_the_crowns_behalf_before_their_levies_march() {
    use enterprise_core::crown::Sovereign;
    use enterprise_core::training::Tenders;
    let mut s = Society::new(content());
    s.content_mut().dials.battle_day = 1.0;
    s.content_mut().dials.battle_mobilise_days = 20.0;
    s.content_mut().dials.battle_levy = 0.25;
    let orgrimmar = town(&mut s, "Orgrimmar", "orc", 0, 0, 20, 0.0, None);
    town(&mut s, "Stormwind", "stormwind", 12, 0, 20, 0.0, None);
    s.advance(5 * DAY);
    let b = s.battles().battles[0].clone();
    assert!(!b.marched && b.marching.is_empty() && b.armies.iter().all(|a| a.soldiers.is_empty()), "not yet: {b:?}");
    let n = b.levies[0][0].1 as f64;
    // Its levy's swords, helmets, chestplates and shields wanted on its board, at the mobilising premium.
    let sword = s.content().good("iron_sword").unwrap();
    let sov = s.world.resource::<Sovereign>();
    let wanted: f64 = sov.requisitions.iter().filter(|r| r.buyer == orgrimmar && r.goods == [sword]).map(|r| r.wanted).sum();
    assert!(wanted >= n, "{n} swords for its levy: {:?}", sov.requisitions);
    assert!(sov.requisitions.iter().filter(|r| r.buyer == orgrimmar).all(|r| r.premium == s.content().dials.mobilising_premium));
    // Master smiths tendered for on its board, and its treasury made good to what it owes on them.
    let tenders = s.world.resource::<Tenders>();
    let tendered: f64 = tenders.on(orgrimmar).iter().map(|t| t.work.bonus.to_f64() * t.open as f64).sum();
    assert!(tendered > 0.0, "smiths tendered for");
    assert!(s.balance(enterprise_core::cash_book::Account::Treasury(orgrimmar)).unwrap().to_f64() >= 0.9 * tendered, "{} for {tendered} tendered (its day's spending since)", s.balance(enterprise_core::cash_book::Account::Treasury(orgrimmar)).unwrap().to_f64());
    // Its levy marches when the run-up's over.
    s.advance(30 * DAY);
    let b = s.battles().battles[0].clone();
    assert!(b.marched && (b.armies[0].soldiers.len() + b.marching.iter().filter(|m| m.side == 0).count()) > 0, "{b:?}");
}

#[test]
fn a_factions_war_spending_draws_on_its_capitals_chest_till_its_dry() {
    use enterprise_core::works::Kind;
    let mut s = Society::new(content());
    s.content_mut().dials.battle_day = 1.0;
    s.content_mut().dials.battle_levy = 0.25;
    // Orgrimmar's chest ample; Theramore's too little for a barracks.
    s.content_mut().dials.war_chest_horde = 5000.0;
    s.content_mut().dials.war_chest_alliance = 10.0;
    let orgrimmar = town(&mut s, "Durotar", "orc", 0, 0, 20, 0.0, None);
    let stormwind = town(&mut s, "Stormwind", "stormwind", 12, 0, 20, 0.0, None);
    s.advance(5 * DAY);
    assert!(s.works(orgrimmar).has_started(Kind::Barracks), "the Horde's paid for");
    assert!(!s.works(stormwind).has_started(Kind::Barracks), "Theramore's chest too little");
    let b = s.battles();
    let d = &s.content().dials;
    assert!(b.chest_left(d, Faction::Horde) < 5000.0, "Orgrimmar's spent on it");
    assert!(b.chest_left(d, Faction::Alliance) <= 10.0 && b.dry.contains_key(&Faction::Alliance), "Theramore's ran dry");
    assert_eq!((Faction::Horde.capital(), Faction::Alliance.capital()), ("Orgrimmar", "Theramore"));
}

/// The war for the region's world: two Horde towns and two Alliance, the Alliance's nearer one a port.
fn war_world(chest_alliance: f64) -> (Society, [usize; 4]) {
    let mut s = Society::new(content());
    let d = &mut s.content_mut().dials;
    d.war_day = 1.0;
    d.battle_mobilise_days = 4.0;
    d.battle_levy = 0.25;
    d.expedition_size = 30.0;
    d.raze_hold_days = 5.0;
    d.war_chest_alliance = chest_alliance;
    let horde = [town(&mut s, "Razor Hill", "orc", 0, 0, 12, 0.0, None), town(&mut s, "Sen'jin", "orc", 0, 6, 12, 0.0, None)];
    let port = town(&mut s, "Southshore", "lordaeron", 10, 0, 12, 0.0, None);
    let far = town(&mut s, "Goldshire", "stormwind", 20, 0, 12, 0.0, None);
    let hex = Hex::new(10, 0);
    s.set_port(port, hex.center_x() + 8.0, hex.center_z());
    (s, [horde[0], horde[1], port, far])
}

#[test]
fn theramore_lands_at_the_alliance_port_nearest_the_horde_and_razes_their_towns() {
    let (mut s, [razor, senjin, port, _]) = war_world(20000.0);
    s.advance(2 * DAY);
    let c = s.battles().campaign.clone().expect("the war declared");
    assert_eq!(c.base, Some(port), "its base the Alliance's port nearest the Horde");
    let b = s.battles().battles.last().cloned().unwrap();
    assert_eq!((b.town, b.attacker), (Some(razor), Some(1)), "the Horde town nearest it first, the Alliance attacking");
    s.advance(7 * DAY);
    assert!(s.battles().campaign.as_ref().unwrap().landed, "landed after the run-up");
    let before = s.roster(senjin).residents();
    s.advance(80 * DAY);
    let c = s.battles().campaign.clone().unwrap();
    assert!(c.razed.iter().any(|r| r.1 == razor), "Razor Hill razed: {c:?}");
    assert!(s.battles().ruins.contains(&razor));
    assert_eq!(s.roster(razor).hands.iter().filter(|h| !h.away).count(), 0, "no one at home in the ruins");
    assert!(c.refugees > 0, "its people taken in");
    assert!(s.roster(senjin).residents() > before || s.battles().ruins.contains(&senjin), "at Sen'jin, its nearest");
    assert!(s.battles().may_settle(Hex::new(0, 0), Some(Faction::Alliance))
        && !s.battles().may_settle(Hex::new(0, 0), Some(Faction::Horde)), "the razers' to settle");
    // On it goes, town by town: the Horde's settlers founding more, an attack beaten off struck back at, till a side
    // has no towns or Theramore's spent.
    s.advance(200 * DAY);
    let c = s.battles().campaign.clone().unwrap();
    assert!(c.razed.len() >= 2, "{c:?}");
    let chest = s.content().dials.war_chest_alliance;
    assert!(s.battles().spent.get(&Faction::Alliance).is_some_and(|x| x.to_f64() <= chest));
    match c.won_by {
        Some((_, Faction::Alliance)) => assert!(s.battles().battles.iter().all(|b| b.won_by.is_some())),
        Some((_, Faction::Horde)) => assert!(c.withdrew.is_some() || c.razed.iter().any(|r| r.2 == Faction::Horde)),
        None => assert!(s.battles().battles.iter().any(|b| b.won_by.is_none()), "a battle on"),
    }
}

#[test]
fn theramore_spent_withdraws_and_the_horde_has_held() {
    let (mut s, _) = war_world(200.0);
    s.advance(60 * DAY);
    let c = s.battles().campaign.clone().unwrap();
    assert!(c.withdrew.is_some(), "Theramore's chest dry: {c:?}");
    assert_eq!(c.won_by.map(|w| w.1), Some(Faction::Horde));
    assert!(s.battles().battles.iter().all(|b| b.armies[1].soldiers.iter().all(|x| !x.from_capital)), "its marines gone home");
}
