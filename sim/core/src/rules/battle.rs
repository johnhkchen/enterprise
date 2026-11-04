//! Battles for hexes (`battle.rs`), once a day in **Pay**, after the crown's campaign: seeded on `battle_day`; then
//! for each battle not yet won, those marching who have got there join their side, those walking home who have got
//! there are home, each side's army has its day at its camp (it eats, draws its kit, runs its risks: the fallen
//! buried and grieved, deserters walking home), its towns make up their levies every `battle_reinforce_days`, and
//! every `skirmish_days` the two meet (`war::clash`); whoever holds it `battle_hold_days` in a row has won it.

use bevy_ecs::prelude::*;

use super::Step;
use crate::army::{blocks_between, bury, levy_order, mourn, travel_ticks, Army, Recruit, Soldier, Walker};
use crate::battle::{Battle, Battles, Faction, Marching};
use crate::content::Content;
use crate::contracts::Book;
use crate::hex::Hex;
use crate::kit::Kits;
use crate::lockers::Lockers;
use crate::messages::TownId;
use crate::roster::Roster;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(fight.in_set(Step::Pay).after(super::army::campaign).before(super::territory::upkeep));
}

type Homes<'w, 's> = Query<'w, 's, (&'static mut People, Option<&'static mut Roster>, Option<&'static mut Lockers>)>;

/// A side's levy from each of its towns: `battle_levy` of its residents, one at least.
fn levy_of(content: &Content, roster: &Roster) -> usize {
    ((roster.residents() as f64 * content.dials.battle_levy).ceil() as usize).max(1)
}

/// Up to `n` of a town's hands for a side, marching from home to the hex (a levy's order: idle hands first), each in
/// the kit `kit` gives it (a barracks town's, out of its warehouse).
#[allow(clippy::too_many_arguments)]
fn raise(content: &Content, roster: &mut Roster, town: TownId, from: Option<Hex>, hex: Hex, side: usize, n: usize,
        now: u64, kit: &mut dyn FnMut() -> Option<crate::army::Outfit>) -> Vec<Marching> {
    let picked = levy_order(content, roster, n);
    let ticks = travel_ticks(blocks_between(from, Some(hex))) as u64;
    let hands: Vec<crate::roster::Hand> = picked.into_iter().filter_map(|i| crate::squad::stamp(roster, i)).collect();
    hands.into_iter().map(|hand| Marching { side, recruit: Recruit { home: town, hand: Some(hand), mustered: now,
        levy: None, pay: content.dials.soldier_pay, kit: kit() }, arrives: now + ticks }).collect()
}

/// A barracks town's day (`works::Kind::Barracks`): its idle hands (no trade, or Nitwits) at home drill at arms,
/// five batches' skill a day (`skill_per_batch`; half again with a talent for it or Ambitious), up to their rank's.
fn drill(content: &Content, roster: &Roster, persons: &mut crate::persons::Persons) {
    use crate::persons::{draw, skill_cap, Habit, Work};
    let d = &content.dials;
    for h in roster.hands.iter().filter(|h| !h.is_baby() && !h.away
            && content.trades.get(h.trade).is_some_and(|t| t.none || t.nitwit)) {
        let m = persons.hands.entry(h.id).or_insert_with(|| draw(h.id, h.level, &[]));
        for w in Work::ALL {
            m.skills.entry(w).or_insert(3.0 * h.level.clamp(1, 5) as f64);
        }
        let gain = 5.0 * d.skill_per_batch * if m.gifted(Work::Arms) { 1.5 } else { 1.0 }
            * if m.has(Habit::Ambitious) { 1.5 } else { 1.0 };
        let skill = m.skills.entry(Work::Arms).or_insert(0.0);
        *skill = (*skill + gain).min(skill_cap(h.level).max(*skill));
    }
}

/// Soldiers walking home from the hex, as long as the road takes.
fn walk_home(army: &mut Army, soldiers: Vec<Soldier>, hexes: &dyn Fn(TownId) -> Option<Hex>, hex: Hex, now: u64) {
    for s in soldiers {
        let ticks = travel_ticks(blocks_between(hexes(s.home), Some(hex))) as u64;
        army.walking.push(Walker { home: s.home, hand: s.hand, arrives: now + ticks });
    }
}

/// The hex fought for between two towns: the nearest to the middle of the line between them that's dry and no
/// town's, within `RINGS` of it (the land between near towns is often all held), the nearer to both on a tie.
fn battleground(a: Hex, b: Hex, map: &crate::map::Map, territory: &crate::territory::Territory) -> Option<Hex> {
    let line = Hex::line(a, b);
    let mid = line[line.len() / 2];
    (-RINGS..=RINGS).flat_map(|dq| (-RINGS..=RINGS).map(move |dr| Hex::new(mid.q + dq, mid.r + dr)))
        .filter(|&h| h.distance(mid) <= RINGS && h != a && h != b && territory.owner(h).is_none()
            && map.ground(h).water_share <= 0.5)
        .min_by_key(|&h| (h.distance(mid), h.distance(a) + h.distance(b), h))
}

/// How far from the middle between two towns a battle's hex may be.
const RINGS: i32 = 6;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn fight(clock: Res<Clock>, content: Res<Content>, all: Res<Towns>, mut battles: ResMut<Battles>,
        taxes: Res<crate::taxes::Taxes>, territory: Res<crate::territory::Territory>, map: Res<crate::map::Map>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<Book>, mut persons: ResMut<crate::persons::Persons>,
        mut charters: ResMut<crate::charters::Charters>, places: Query<&Town>, mut camps: Query<(&mut Stock, &mut Kits)>,
        mut homes: Homes, crown: Res<Army>, mut works: Query<&mut crate::works::Works>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let today = now / day;
    let d = &content.dials;
    let entity = |t: TownId| all.0.get(t).copied();
    let town_hex = |t: TownId| entity(t).and_then(|e| places.get(e).ok()).and_then(|x| x.hex);
    let battles = &mut *battles;
    // Once a day: who's at war (its hands levied for a battle or serving the crown's army: it builds a barracks), and
    // the barracks towns' drill.
    if battles.drilled != Some(today) {
        battles.drilled = Some(today);
        for &e in &all.0 {
            let Ok(t) = places.get(e) else { continue };
            let levied = battles.battles.iter().any(|b| b.won_by.is_none() && b.levies.iter().flatten().any(|l| l.0 == t.id));
            let serving = crown.serving_from(t.id) > 0 || crown.barracks.iter().any(|r| r.home == t.id);
            let Ok(mut w) = works.get_mut(e) else { continue };
            if w.at_war != (levied || serving) {
                w.at_war = levied || serving;
            }
            if w.built(crate::works::Kind::Barracks) {
                if let Ok((_, Some(roster), _)) = homes.get(e) {
                    drill(&content, &roster, &mut persons);
                }
            }
        }
    }
    let barracks = |e: Entity, works: &Query<&mut crate::works::Works>| works.get(e).is_ok_and(|w| w.built(crate::works::Kind::Barracks));
    // Seeded: the hex between the nearest Horde and Alliance towns, each side levied from its towns within reach.
    if d.battle_day > 0.0 && !battles.seeded && today as f64 >= d.battle_day {
        battles.seeded = true;
        let towns: Vec<(TownId, Hex, Faction)> = all.0.iter().filter_map(|&e| places.get(e).ok())
            .filter_map(|t| Some((t.id, t.hex?, taxes.peoples.get(&t.id).and_then(|p| Faction::of(p))?))).collect();
        let pair = towns.iter().filter(|a| a.2 == Faction::Horde)
            .flat_map(|h| towns.iter().filter(|a| a.2 == Faction::Alliance).map(move |a| (h, a)))
            .min_by_key(|(h, a)| (h.1.distance(a.1), h.0, a.0));
        if let Some((h, a)) = pair {
            if let Some(hex) = battleground(h.1, a.1, &map, &territory) {
                let mut battle = Battle { hex, factions: [Faction::Horde, Faction::Alliance], armies: Default::default(),
                    camps: [h.0, a.0], levies: Default::default(), marching: Vec::new(), holder: None, held_since: None,
                    won_by: None, began: now, days: 0, fights: 0, reckoned: Some(today) };
                for (side, &faction) in battle.factions.clone().iter().enumerate() {
                    for &(t, at, _) in towns.iter().filter(|x| x.2 == faction && x.1.distance(hex) as f64 <= d.battle_reach) {
                        let Some(e) = entity(t) else { continue };
                        let kitted = barracks(e, &works);
                        let Ok((mut people, Some(mut roster), _)) = homes.get_mut(e) else { continue };
                        let n = levy_of(&content, &roster);
                        battle.levies[side].push((t, n));
                        let mut camp = camps.get_mut(e).ok();
                        let ledger = &mut *ledger;
                        let mut kit = || -> Option<crate::army::Outfit> {
                            let (stock, kits) = camp.as_mut().filter(|_| kitted)?;
                            Some(crate::army::outfit(&content, ledger, stock, kits, Some(&mut people.graded)))
                        };
                        battle.marching.extend(raise(&content, &mut roster, t, Some(at), hex, side, n, now, &mut kit));
                    }
                }
                battles.battles.push(battle);
            }
        }
    }
    for b in battles.battles.iter_mut().filter(|b| b.won_by.is_none() && b.reckoned != Some(today)) {
        b.reckoned = Some(today);
        let hex = b.hex;
        // There: those marching join their side; those walking home are home.
        let (there, on): (Vec<Marching>, Vec<Marching>) = std::mem::take(&mut b.marching).into_iter().partition(|m| m.arrives <= now);
        b.marching = on;
        for m in there {
            b.armies[m.side].enlist(m.recruit, now);
        }
        for army in b.armies.iter_mut() {
            let (home, away): (Vec<Walker>, Vec<Walker>) = std::mem::take(&mut army.walking).into_iter().partition(|w| w.arrives <= now);
            army.walking = away;
            for w in home {
                let (Some(hand), Some(e)) = (&w.hand, entity(w.home)) else { continue };
                if let Ok((_, Some(mut roster), _)) = homes.get_mut(e) {
                    crate::squad::release(&mut roster, std::slice::from_ref(hand));
                }
            }
        }
        // Each side's day at its camp: it eats, draws its kit (its graded pieces first), runs its risks.
        for side in 0..2 {
            let Some(camp) = entity(b.camps[side]) else { continue };
            let army = &mut b.armies[side];
            if army.soldiers.is_empty() {
                continue;
            }
            let mut graded = homes.get_mut(camp).ok().map(|(p, ..)| std::mem::take(&mut p.into_inner().graded));
            let losses = match camps.get_mut(camp) {
                Ok((mut stock, mut kits)) => Some(army.day(&content, &mut ledger, &mut book, &mut stock, &mut kits, None,
                    graded.as_mut())),
                Err(_) => None,
            };
            if let (Some(g), Ok((mut people, ..))) = (graded, homes.get_mut(camp)) {
                people.graded = g;
            }
            let Some(losses) = losses else { continue };
            for soldier in losses.fallen {
                let Some(e) = entity(soldier.home) else { continue };
                let Ok((mut people, roster, lockers)) = homes.get_mut(e) else { continue };
                if let (Some(hand), Some(mut roster)) = (&soldier.hand, roster) {
                    bury(&mut roster, lockers.map(|l| l.into_inner()), hand);
                    people.villagers = roster.residents() as f64;
                }
                mourn(&content, &mut people, now);
            }
            walk_home(army, losses.deserted, &town_hex, hex, now);
        }
        b.days += 1;
        // Its towns make up their levies.
        if d.battle_reinforce_days > 0.0 && b.days % d.battle_reinforce_days as u64 == 0 {
            for side in 0..2 {
                for &(t, n) in &b.levies[side].clone() {
                    let serving = b.armies[side].soldiers.iter().filter(|s| s.home == t).count()
                        + b.marching.iter().filter(|m| m.side == side && m.recruit.home == t).count();
                    let Some(e) = entity(t) else { continue };
                    let kitted = barracks(e, &works);
                    let Ok((mut people, Some(mut roster), _)) = homes.get_mut(e) else { continue };
                    if serving < n {
                        let mut camp = camps.get_mut(e).ok();
                        let ledger = &mut *ledger;
                        let mut kit = || -> Option<crate::army::Outfit> {
                            let (stock, kits) = camp.as_mut().filter(|_| kitted)?;
                            Some(crate::army::outfit(&content, ledger, stock, kits, Some(&mut people.graded)))
                        };
                        b.marching.extend(raise(&content, &mut roster, t, town_hex(t), hex, side, n - serving, now, &mut kit));
                    }
                }
            }
        }
        // They meet; or a side with no one there leaves the hex to the other.
        let every = d.skirmish_days as u64;
        if every > 0 && b.days % every == 0 {
            let [x, y] = &mut b.armies;
            let holder = match (x.soldiers.is_empty(), y.soldiers.is_empty()) {
                (false, false) => {
                    let (x_won, rx, ry) = crate::war::clash(&content, x, y, &mut persons, &mut charters, b.fights, now);
                    b.fights += 1;
                    walk_home(x, rx, &town_hex, hex, now);
                    walk_home(y, ry, &town_hex, hex, now);
                    Some(if x_won { 0 } else { 1 })
                }
                (false, true) => Some(0),
                (true, false) => Some(1),
                (true, true) => b.holder,
            };
            if holder != b.holder {
                b.holder = holder;
                b.held_since = holder.map(|_| now);
            }
        }
        // Held long enough, it's the holder's: the fighting ends, and both armies walk home.
        if b.holder.is_some() && b.held_days(now, day) as f64 >= d.battle_hold_days {
            b.won_by = b.holder.map(|s| b.factions[s]);
            for side in 0..2 {
                let soldiers = std::mem::take(&mut b.armies[side].soldiers);
                walk_home(&mut b.armies[side], soldiers, &town_hex, hex, now);
            }
            let marching = std::mem::take(&mut b.marching);
            for m in marching {
                let s = Soldier { home: m.recruit.home, hand: m.recruit.hand, ..Default::default() };
                walk_home(&mut b.armies[m.side], vec![s], &town_hex, hex, now);
            }
        }
    }
    // After a battle is won its armies' walkers still go home.
    for b in battles.battles.iter_mut().filter(|b| b.won_by.is_some()) {
        for army in b.armies.iter_mut() {
            let (home, away): (Vec<Walker>, Vec<Walker>) = std::mem::take(&mut army.walking).into_iter().partition(|w| w.arrives <= now);
            army.walking = away;
            for w in home {
                let (Some(hand), Some(e)) = (&w.hand, entity(w.home)) else { continue };
                if let Ok((_, Some(mut roster), _)) = homes.get_mut(e) {
                    crate::squad::release(&mut roster, std::slice::from_ref(hand));
                }
            }
        }
    }
}
