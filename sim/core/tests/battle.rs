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
