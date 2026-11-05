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
    assert!(b.chest_left(d, Faction::Alliance) > 9.0, "Theramore's paid only its pace: no barracks it can't pay for whole");
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
    d.raid_hexes = 0.0; // (the campaign's own order of battles: raids are tried by themselves)
    let horde = [town(&mut s, "Razor Hill", "orc", 0, 0, 12, 0.0, None), town(&mut s, "Sen'jin", "orc", 0, 6, 12, 0.0, None)];
    let port = town(&mut s, "Southshore", "lordaeron", 10, 0, 12, 0.0, None);
    let far = town(&mut s, "Goldshire", "stormwind", 20, 0, 12, 0.0, None);
    let hex = Hex::new(10, 0);
    s.set_port(port, hex.center_x() + 8.0, hex.center_z());
    (s, [horde[0], horde[1], port, far])
}

#[test]
fn theramore_lands_at_the_alliance_port_nearest_the_horde_and_razes_their_towns() {
    let (mut s, [razor, _, port, _]) = war_world(20000.0);
    s.advance(2 * DAY);
    let c = s.battles().campaign.clone().expect("the war declared");
    assert_eq!(c.base, Some(port), "its base the Alliance's port nearest the Horde");
    let b = s.battles().battles.last().cloned().unwrap();
    assert_eq!((b.town, b.attacker), (Some(razor), Some(1)), "the Horde town nearest it first, the Alliance attacking");
    s.advance(7 * DAY);
    assert!(s.battles().campaign.as_ref().unwrap().landed, "landed after the run-up");
    s.advance(80 * DAY);
    let c = s.battles().campaign.clone().unwrap();
    assert!(c.razed.iter().any(|r| r.1 == razor), "Razor Hill razed: {c:?}");
    assert!(s.battles().ruins.contains(&razor));
    assert_eq!(s.roster(razor).hands.iter().filter(|h| !h.away).count(), 0, "no one at home in the ruins");
    assert!(c.refugees > 0, "its people taken in");
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

#[test]
fn a_towns_defenders_fight_better_at_home_behind_walls_and_a_watchtower() {
    use enterprise_core::rules::battle::defence_lift;
    use enterprise_core::works::{Kind, What};
    let (mut s, [razor, ..]) = war_world(20000.0);
    let d = s.content().dials.clone();
    assert!((defence_lift(&d, Some(&s.works(razor))) - 1.2).abs() < 1e-9, "at home: a fifth better");
    s.start_project(razor, What::Wall);
    s.finish_project(razor, What::Wall);
    s.start_project(razor, What::Hall(Kind::Watchtower));
    s.finish_project(razor, What::Hall(Kind::Watchtower));
    assert!((defence_lift(&d, Some(&s.works(razor))) - 1.5).abs() < 1e-9, "behind a whole wall, with a watchtower");
}

#[test]
fn every_town_of_the_defenders_side_sends_its_levy_however_far() {
    let (mut s, [razor, senjin, port, goldshire]) = war_world(20000.0);
    let far = town(&mut s, "Thrallmar", "orc", 0, -40, 12, 0.0, None);
    s.advance(2 * DAY);
    let b = s.battles().battles.last().cloned().unwrap();
    assert_eq!(b.town, Some(razor));
    let horde: Vec<(usize, usize)> = b.levies[0].clone();
    assert!(horde.contains(&(far, 3)), "Thrallmar, 40 hexes off, sends a fifth of its 13: {horde:?}");
    assert!(horde.contains(&(senjin, 3)) && horde.contains(&(razor, 3)));
    assert_eq!(b.levies[1], vec![(port, 4), (goldshire, 4)], "the attackers' towns within reach their battle_levy");
}

#[test]
fn orgrimmar_sends_mercenaries_once_its_last_battles_were_lost() {
    let (mut s, _) = war_world(20000.0);
    s.content_mut().dials.horde_last_stand = 0.0;
    s.advance(120 * DAY);
    let c = s.battles().campaign.clone().unwrap();
    let lost = s.battles().battles.iter().filter(|b| b.town.is_some() && b.won_by == Some(Faction::Alliance)).count();
    assert!(lost >= 2, "two battles lost: {c:?}");
    assert!(c.hired.is_some(), "mercenaries sent after a losing streak: {c:?}");
}

#[test]
fn theramore_pays_for_each_marine_it_sends_to_make_its_expedition_up() {
    let spent = |cost: f64| {
        let (mut s, _) = war_world(20000.0);
        s.content_mut().dials.marine_cost = cost;
        s.advance(120 * DAY);
        let b = s.battles();
        (b.spent.get(&Faction::Alliance).map_or(0.0, |x| x.to_f64()), b.campaign.clone().unwrap())
    };
    let (free, c) = spent(0.0);
    let (paid, _) = spent(30.0);
    assert!(c.razed.len() >= 1, "{c:?}");
    assert!(paid > free, "marines made up cost Theramore: {paid} against {free}");
}

#[test]
fn a_fight_kills_on_both_sides_the_winner_fewer() {
    // John: "more realistic partial damage to the winning side": two squads of 20 meet on the march.
    let (mut s, _) = squads_world(Hex::new(4, 0), 20, Hex::new(6, 0), 20);
    s.content_mut().dials.combat_losses = 0.5;
    s.advance(3 * DAY);
    let b = s.battles().battles.iter().find(|b| b.field).cloned().expect("they met");
    let dead = |side: usize| b.armies[side].records.values().map(|r| r.fallen).sum::<u32>();
    let won = b.factions.iter().position(|&f| Some(f) == b.won_by).expect("fought it out");
    assert!(dead(won) > 0 && dead(won) <= dead(1 - won), "the winner lost {}, the beaten {}", dead(won), dead(1 - won));
}

#[test]
fn a_levy_learns_its_arms_past_its_trades_level() {
    // John: "let combat grant real experience, particularly for new soldiers": a level-1 hand past a level-1 cap (4).
    let arms = |skill: f64| {
        let (mut s, _) = war_world(20000.0);
        s.content_mut().dials.combat_skill = skill;
        s.advance(60 * DAY);
        s.world.resource::<enterprise_core::persons::Persons>().hands.values()
            .filter_map(|p| p.skills.get(&enterprise_core::persons::Work::Arms).copied()).sum::<f64>()
    };
    let (green, taught) = (arms(0.0), arms(1.0));
    assert!(taught > green + 5.0, "the levies' arms, all told: {green} untaught, {taught} taught");
}

/// Two squads on the hexes, the Horde's at `h` and the Alliance's at `a`, of `nh` and `na` soldiers, each bound for the
/// other's hex (no war declared: the squads alone).
fn squads_world(h: Hex, nh: usize, a: Hex, na: usize) -> (Society, [usize; 4]) {
    let (mut s, towns) = war_world(20000.0);
    s.content_mut().dials.war_day = 0.0;
    let soldiers = |home: usize, n: usize, id: u32| (0..n).map(|_| army::Soldier { home, squad: Some(id), ..Default::default() })
        .collect::<Vec<_>>();
    let mut b = s.world.resource_mut::<enterprise_core::battle::Battles>();
    b.squads.push(enterprise_core::battle::Squad { id: 1, side: 0, home: towns[0], soldiers: soldiers(towns[0], nh, 1),
        hex: h, goal: Some(a), ..Default::default() });
    b.squads.push(enterprise_core::battle::Squad { id: 2, side: 1, home: towns[2], soldiers: soldiers(towns[2], na, 2),
        hex: a, goal: Some(h), ..Default::default() });
    b.next_squad = 2;
    (s, towns)
}

#[test]
fn a_squad_marches_its_pace_a_day() {
    // John: "hex-based squad movement": `squad_hexes_per_day` along the line to its goal.
    let (mut s, _) = squads_world(Hex::new(0, 0), 5, Hex::new(40, 0), 5);
    s.world.resource_mut::<enterprise_core::battle::Battles>().squads.retain(|q| q.id == 1);
    s.content_mut().dials.squad_hexes_per_day = 4.0;
    s.advance(DAY / 2);
    let q = s.battles().squads[0].clone();
    assert_eq!(q.hex.distance(Hex::new(0, 0)), 4, "four hexes on: {q:?}");
}

#[test]
fn squads_meeting_on_the_march_fight_and_the_beaten_fall_back() {
    let (mut s, _) = squads_world(Hex::new(4, 0), 10, Hex::new(6, 0), 10);
    s.advance(DAY + DAY / 2);
    let field: Vec<_> = s.battles().battles.iter().filter(|b| b.field).cloned().collect();
    assert_eq!(field.len(), 1, "they met: {:?}", s.battles().squads);
    s.advance(3 * DAY);
    let b = s.battles().battles.iter().find(|b| b.field).cloned().unwrap();
    assert!(b.won_by.is_some() && b.fights >= 1, "fought it out: {b:?}");
    let beaten = 1 - b.factions.iter().position(|&f| Some(f) == b.won_by).unwrap();
    assert!(s.battles().squads.iter().filter(|q| q.side == beaten).all(|q| q.homeward), "the beaten go home: {:?}",
        s.battles().squads);
}

#[test]
fn a_squad_far_the_smaller_steps_aside() {
    let (mut s, _) = squads_world(Hex::new(4, 0), 12, Hex::new(6, 0), 2);
    s.advance(DAY + DAY / 2);
    assert!(s.battles().battles.iter().all(|b| !b.field), "no stand to make at 12 to 2");
}

#[test]
fn a_squad_bound_where_no_battle_is_goes_home() {
    let (mut s, _) = squads_world(Hex::new(0, 0), 3, Hex::new(40, 0), 3);
    s.world.resource_mut::<enterprise_core::battle::Battles>().squads.retain(|q| q.id == 1);
    s.world.resource_mut::<enterprise_core::battle::Battles>().squads[0].goal = Some(Hex::new(4, 0));
    s.advance(2 * DAY + DAY / 2);
    let q = s.battles().squads.first().cloned();
    assert!(q.as_ref().is_none_or(|q| q.homeward), "there and nothing doing, home: {q:?}");
}

#[test]
fn a_hero_carries_the_fight_for_whichever_side_it_leads() {
    // John: "a heroic unit leading things"; "Both sides get one": ten to ten on the march, the side with the hero wins.
    for side in [0, 1] {
        let (mut s, _) = squads_world(Hex::new(4, 0), 10, Hex::new(6, 0), 10);
        s.content_mut().dials.skirmish_odds = 0.0;
        s.content_mut().dials.hero_worth = 10.0;
        // (A war over, its heroes still leading.)
        let mut heroes_alive = [0, 0];
        heroes_alive[side] = 1;
        s.world.resource_mut::<enterprise_core::battle::Battles>().campaign = Some(enterprise_core::battle::Campaign {
            heroes_alive, won_by: Some((0, enterprise_core::battle::Faction::Horde)), ..Default::default() });
        s.advance(5 * DAY);
        let b = s.battles().battles.iter().find(|b| b.field).cloned().expect("they met");
        assert_eq!(b.won_by, Some(b.factions[side]), "the hero's side: {b:?}");
        assert_eq!(b.armies[side].heroes, 1, "it led them");
    }
}

#[test]
fn each_capital_sends_its_heroes_once_when_theramore_lands_to_lead_its_largest_force() {
    let (mut s, _) = war_world(20000.0);
    s.content_mut().dials.hero_alliance = 1.0;
    s.content_mut().dials.hero_horde = 2.0;
    s.content_mut().dials.hero_cost = 500.0;
    let led = |s: &Society, side: usize| s.battles().battles.iter().filter(|b| b.won_by.is_none())
        .map(|b| b.armies[side].heroes).sum::<u32>();
    assert!(any_day(&mut s, 20, |s| led(s, 1) == 1), "the captain of renown leads: {:?}", s.battles().campaign);
    let c = s.battles().campaign.clone().unwrap();
    assert_eq!(c.heroes_alive, [2, 1], "{c:?}");
    let spent = |f| s.battles().spent.get(&f).map_or(0.0, |x| x.to_f64());
    assert!(spent(enterprise_core::battle::Faction::Horde) >= 1000.0, "two warchiefs paid for");
    assert!(spent(enterprise_core::battle::Faction::Alliance) >= 500.0, "a captain paid for");
    let largest = s.battles().battles.iter().filter(|b| b.won_by.is_none()).max_by_key(|b| b.armies[1].soldiers.len()).unwrap();
    assert_eq!(largest.armies[1].heroes, 1, "with the marines");
}

#[test]
fn theramore_keeps_a_garrison_at_its_base() {
    // (`base_garrison` of the expedition posted at the port; the rest march.)
    let (mut s, [_, _, port, _]) = war_world(20000.0);
    let hex = s.town_hex(port).unwrap();
    assert!(any_day(&mut s, 20, |s| s.battles().squads.iter().any(|q| q.capital && q.side == 1 && q.waits
        && q.goal == Some(hex) && q.size(&s.battles().battles) == 6)), "six of thirty posted: {:?}", s.battles().squads);
    let marching = s.battles().squads.iter().filter(|q| q.capital && q.side == 1 && !q.waits).map(|q| q.size(&s.battles().battles))
        .sum::<usize>();
    assert!(marching > 0 && marching <= 24, "the rest march: {marching}");
}

/// Each day of the war world for `days`, what `seen` makes of it, any day.
fn any_day(s: &mut Society, days: u64, seen: impl Fn(&Society) -> bool) -> bool {
    (1..=days).any(|day| {
        s.advance(day * DAY);
        seen(s)
    })
}

#[test]
fn the_hordes_defenders_mass_ahead_of_the_marines() {
    // John: "Mass at a chokepoint": Sen'jin's squad waits on the marines' road within `choke_reach` of Razor Hill.
    let (mut s, [razor, senjin, ..]) = war_world(20000.0);
    let town = s.town_hex(razor).unwrap();
    assert!(any_day(&mut s, 40, |s| s.battles().squads.iter().any(|q| q.home == senjin && q.waits && q.goal != Some(town)
        && q.goal.is_some_and(|g| (1..=4).contains(&g.distance(town))))), "{:?}", s.battles().squads);
}

#[test]
fn the_horde_surrounds_the_marines_at_its_town_and_they_sally() {
    // "Fortify and surround": posts round the town once the marines are there; cut off, the marines sally on one.
    let (mut s, _) = war_world(20000.0);
    // (Its levy made up every few days: the squads it sends after the marines are there take the posts.)
    s.content_mut().dials.surround_sides = 1.0;
    s.content_mut().dials.choke_reach = 0.0;
    s.content_mut().dials.battle_reinforce_days = 2.0;
    // (Round the town the Alliance is attacking.)
    let attacked = |s: &Society| s.battles().battles.iter().rfind(|b| b.town.is_some() && b.won_by.is_none() && b.attacker == Some(1))
        .map(|b| b.hex.neighbors());
    assert!(any_day(&mut s, 120, |s| attacked(s).is_some_and(|round| s.battles().squads.iter()
        .any(|q| q.side == 0 && q.waits && q.goal.is_some_and(|g| round.contains(&g))))), "a post round the town");
    assert!(any_day(&mut s, 120, |s| attacked(s).is_some_and(|round| s.battles().squads.iter()
        .any(|q| q.capital && q.side == 1 && q.battle.is_none() && q.goal.is_some_and(|g| round.contains(&g))))), "a sally on it");
}

#[test]
fn the_horde_raids_an_alliance_town_far_from_the_marines() {
    // "Raid the weak": an Alliance town far from the marines' port (Goldshire, 20 hexes, or one founded since).
    let (mut s, [_, _, port, _]) = war_world(20000.0);
    s.content_mut().dials.raid_hexes = 10.0;
    s.content_mut().dials.battle_reach = 30.0;
    assert!(any_day(&mut s, 60, |s| s.battles().battles.iter().any(|b| b.raid && b.town != Some(port) && b.attacker == Some(0))),
        "{:?}", s.battles().battles.iter().map(|b| (b.town, b.attacker, b.raid)).collect::<Vec<_>>());
}

#[test]
fn a_weaker_horde_squad_falls_back_rather_than_fight() {
    // "Skirmish and fall back": 10 to 18 is worse than `skirmish_odds` (0.6) but not an overrun: no stand.
    let (mut s, _) = squads_world(Hex::new(4, 0), 10, Hex::new(6, 0), 18);
    s.advance(DAY + DAY / 2);
    assert!(s.battles().battles.iter().all(|b| !b.field), "no field battle: {:?}", s.battles().squads);
}

#[test]
fn the_ground_lifts_the_side_holding_it() {
    let d = enterprise_core::Dials::default();
    let hills = enterprise_core::map::Ground { land: enterprise_core::map::Land::Hills, river: true, ..Default::default() };
    let lift = enterprise_core::rules::battle::ground_lift(&d, hills);
    assert!((lift - (1.0 + d.hills_lift + d.river_lift)).abs() < 1e-9, "{lift}");
}

#[test]
fn theramores_ships_come_once_every_convoy_days() {
    // John: "limit reinforcement frequency": its marines made up when its ships come, every `convoy_days` at most.
    let (mut s, _) = war_world(20000.0);
    s.content_mut().dials.convoy_days = 20.0;
    let mut sailed = vec![];
    for day in 1..=150 {
        s.advance(day * DAY);
        let c = s.battles().campaign.clone();
        if let Some(day) = c.and_then(|c| c.sailed).filter(|d| sailed.last() != Some(d)) {
            sailed.push(day);
        }
    }
    assert!(sailed.len() >= 2, "ships came again: {sailed:?}");
    assert!(sailed.windows(2).all(|w| w[1] - w[0] >= 20 * DAY), "20 days apart at least: {sailed:?}");
}

#[test]
fn the_horde_outnumbering_the_alliance_strikes_at_its_camp_with_every_town() {
    let (mut s, [razor, senjin, port, _]) = war_world(20000.0);
    s.content_mut().dials.pin_odds = 0.1;
    s.advance(150 * DAY);
    let towns: Vec<_> = s.battles().battles.iter().filter(|b| b.town.is_some()).cloned().collect();
    assert_eq!((towns[0].town, towns[0].attacker), (Some(razor), Some(1)), "the Alliance strikes first");
    let pin = towns.iter().find(|b| b.attacker == Some(0)).expect("the Horde strikes back");
    assert_eq!(pin.town, Some(port), "at the Alliance's camp, its base");
    let levied: Vec<usize> = pin.levies[0].iter().map(|l| l.0).collect();
    assert!(levied.contains(&senjin) || s.battles().ruins.contains(&senjin), "every Horde town levied: {levied:?}");
    if let Some(next) = towns.iter().skip_while(|b| !std::ptr::eq(*b, pin)).nth(1) {
        if pin.won_by == Some(Faction::Alliance) {
            assert_eq!(next.attacker, Some(1), "its strike beaten off, the Alliance strikes");
        }
    }
}

#[test]
fn the_horde_outnumbered_waits_to_be_struck() {
    let (mut s, _) = war_world(20000.0);
    s.content_mut().dials.pin_odds = 100.0;
    s.advance(120 * DAY);
    let towns: Vec<_> = s.battles().battles.iter().filter(|b| b.town.is_some()).cloned().collect();
    assert!(towns.len() >= 2);
    for w in towns.windows(2) {
        let beaten_off = w[0].attacker == Some(1) && w[0].won_by == Some(Faction::Horde);
        assert_eq!(w[1].attacker, Some(if beaten_off { 0 } else { 1 }), "{:?}", towns.iter().map(|b| (b.town, b.attacker, b.won_by)).collect::<Vec<_>>());
    }
}

#[test]
fn a_capitals_public_spending_keeps_to_its_pace_and_its_reserve() {
    let (mut s, _) = war_world(20000.0);
    s.content_mut().dials.war_chest_horde = 1200.0;
    let pace: f64 = 1200.0 * 0.75 / 120.0;
    let mut last = 0.0;
    for day in 2..60 {
        s.advance(day * DAY);
        let b = s.battles();
        let d = s.content().dials.clone();
        let spent = 1200.0 - b.chest_left(&d, Faction::Horde);
        let paid_today = b.paid_today.get(&Faction::Horde).filter(|p| p.0 == day - 1).map_or(0.0, |p| p.1.to_f64());
        assert!(paid_today <= pace + 1e-6, "day {day}: {paid_today} over its pace");
        assert!(spent <= 1200.0 * 0.75 + 1e-6 + 30.0 * 8.0, "its reserve kept for soldiers: {spent}");
        last = spent;
    }
    assert!(last > 0.0, "Orgrimmar paid its towns something");
}
