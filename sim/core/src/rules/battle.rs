//! Battles for hexes (`battle.rs`), once a day in **Pay**, after the crown's campaign: seeded on `battle_day`; then
//! for each battle not yet won, those marching who have got there join their side, those walking home who have got
//! there are home, each side's army has its day at its camp (it eats, draws its kit, runs its risks: the fallen
//! buried and grieved, deserters walking home), its towns make up their levies every `battle_reinforce_days`, and
//! every `skirmish_days` the two meet (`war::clash`); whoever holds it `battle_hold_days` in a row has won it.

use bevy_ecs::prelude::*;

use super::Step;
use crate::army::{blocks_between, bury, levy_order, mourn, travel_ticks, Army, Recruit, Soldier, Walker};
use crate::battle::{Battle, Battles, Campaign, Faction, Marching};
use crate::content::Content;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::contracts::Book;
use crate::crown::Sovereign;
use crate::training::Tenders;
use crate::hex::Hex;
use crate::kit::Kits;
use crate::lockers::Lockers;
use crate::messages::TownId;
use crate::roster::Roster;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        march.in_set(Step::Pay).after(super::army::campaign).before(fight),
        supply.in_set(Step::Pay).after(march).before(fight),
        fight.in_set(Step::Pay).after(super::army::campaign).before(super::territory::upkeep),
        procure.in_set(Step::Review).after(super::crown::review).after(super::market::review).before(super::training::lapse),
        campaign.in_set(Step::Pay).after(fight).before(super::territory::upkeep),
        council.in_set(Step::Pay).after(campaign).before(super::territory::upkeep),
    ));
}

type Homes<'w, 's> = Query<'w, 's, (&'static mut People, Option<&'static mut Roster>, Option<&'static mut Lockers>)>;

/// A side's levy from each of its towns: `battle_levy` of its residents, one at least.
fn levy_of(content: &Content, roster: &Roster) -> usize {
    levy_at(roster, content.dials.battle_levy)
}

/// `share` of a town's residents, one at least.
fn levy_at(roster: &Roster, share: f64) -> usize {
    ((roster.residents() as f64 * share).ceil() as usize).max(1)
}

/// How much better a town's defenders fight for it (John: "the horde should be able to put up a better fight"):
/// `home_ground_lift` at home, `walls_lift` more behind a whole wall (as much of it as stands), `watchtower_lift`
/// more with a watchtower.
pub fn defence_lift(d: &crate::dials::Dials, works: Option<&crate::works::Works>) -> f64 {
    1.0 + d.home_ground_lift + works.map_or(0.0, |w| d.walls_lift * w.walled()
        + if w.built(crate::works::Kind::Watchtower) { d.watchtower_lift } else { 0.0 })
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
        mut homes: Homes, crown: Res<Army>,
        (mut works, mut purses, mut cash): (Query<&mut crate::works::Works>, Query<&mut Money>, ResMut<CashBook>)) {
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
            // Its faction pays for its barracks (John: "factions pay"): the hall's materials and its builders' wages,
            // from the faction's war chest (from outside the society, as the crown's mint); its builders the faction's,
            // whatever Mason the town has.
            let faction = taxes.peoples.get(&t.id).and_then(|p| Faction::of(p));
            if let (true, Some(faction), false) = (w.at_war, faction, w.has_started(crate::works::Kind::Barracks)) {
                let kind = crate::works::Kind::Barracks;
                if let (Ok((stock, _)), Ok(mut money)) = (camps.get(e), purses.get_mut(e)) {
                    let cost = crate::fixed::Fixed::ceil(crate::works::cost(&content, &stock.0, kind));
                    // (Only when its capital can pay it all today, at its pace.)
                    if battles.public_left(d, faction, today) < cost {
                        continue;
                    }
                    let grant = battles.spend_public(d, faction, cost, today);
                    let materials = crate::fixed::Fixed::ceil(crate::works::materials(&content, &stock.0, kind));
                    let chest = Reason::Other(format!("{}'s war chest", faction.capital()));
                    post(&mut ledger, &mut cash, Side::outside(), Side::treasury(t.id, &mut money.0), grant, chest, now, day);
                    post(&mut ledger, &mut cash, Side::treasury(t.id, &mut money.0), Side::outside(), materials,
                        Reason::Other("a barracks".into()), now, day);
                    w.start(crate::works::What::Hall(kind), kind.blocks());
                }
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
                    won_by: None, began: now, days: 0, fights: 0, reckoned: None,
                    marches: now + (d.battle_mobilise_days.max(0.0) * day as f64) as u64, marched: false, town: None,
                    attacker: None, settled: false, field: false, raid: false, ground_side: None, depots: Default::default() };
                // Each side's towns near it are levied; their levies march once its towns have had time to arm.
                for (side, &faction) in battle.factions.clone().iter().enumerate() {
                    for &(t, _, _) in towns.iter().filter(|x| x.2 == faction && x.1.distance(hex) as f64 <= d.battle_reach) {
                        let Some(e) = entity(t) else { continue };
                        let Ok((_, Some(roster), _)) = homes.get(e) else { continue };
                        battle.levies[side].push((t, levy_of(&content, &roster)));
                    }
                }
                battles.battles.push(battle);
            }
        }
    }
    // Each side's heroes lead its largest force (John: "a heroic unit leading things").
    let alive = battles.campaign.as_ref().map_or([0, 0], |c| c.heroes_alive);
    for side in 0..2 {
        let largest = (0..battles.battles.len()).filter(|&i| battles.battles[i].won_by.is_none())
            .max_by_key(|&i| (battles.battles[i].armies[side].soldiers.len(), std::cmp::Reverse(i)));
        for (i, b) in battles.battles.iter_mut().enumerate().filter(|(_, b)| b.won_by.is_none()) {
            b.armies[side].heroes = if Some(i) == largest && !b.armies[side].soldiers.is_empty() { alive[side] } else { 0 };
        }
    }
    let mut heroes_fell: Vec<usize> = Vec::new();
    let mut foraged = [0.0f64; 2];
    let (list, squads, next_squad) = (&mut battles.battles, &mut battles.squads, &mut battles.next_squad);
    for (bi, b) in list.iter_mut().enumerate().filter(|(_, b)| b.won_by.is_none() && b.reckoned != Some(today)) {
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
        // Each side's day: at home it eats and draws its kit at its town (its graded pieces first); away, at its depot
        // on the hex (what its supply wagons brought: `supply`), then its packs; it runs its risks.
        for side in 0..2 {
            let Some(camp) = entity(b.camps[side]) else { continue };
            let away = b.town.is_some() && town_hex(b.camps[side]) != Some(hex) || b.field;
            let mut depot = Stock(std::mem::take(&mut b.depots[side]));
            depot.0.resize(content.goods.len(), 0.0);
            let army = &mut b.armies[side];
            if army.soldiers.is_empty() {
                b.depots[side] = depot.0;
                continue;
            }
            let mut graded = homes.get_mut(camp).ok().map(|(p, ..)| std::mem::take(&mut p.into_inner().graded));
            let losses = if away {
                // At an enemy town's battle its attackers forage: a day's rations of the town's harvest into their
                // depot (John: "enemy fields first").
                if b.town.is_some() && b.attacker == Some(side) {
                    if let Some(Ok((mut theirs, _))) = entity(b.camps[1 - side]).map(|e| camps.get_mut(e)) {
                        let want = army.soldiers.len() as f64 * d.army_ration;
                        foraged[side] += load_food(&content, &mut theirs.0, &mut depot.0, want);
                    }
                }
                let losses = army.day(&content, &mut ledger, &mut book, &mut depot, &mut Kits::default(), None, None);
                b.depots[side] = depot.0;
                Some(losses)
            } else {
                b.depots[side] = depot.0;
                match camps.get_mut(camp) {
                    Ok((mut stock, mut kits)) => Some(army.day(&content, &mut ledger, &mut book, &mut stock, &mut kits, None,
                        graded.as_mut())),
                    Err(_) => None,
                }
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
        // Till its levies march, its towns arm (`procure`); then it's fought day by day.
        if now < b.marches {
            continue;
        }
        let first = !b.marched;
        b.marched = true;
        b.days += 1;
        // Its towns send their levies, and make them up.
        if first || d.battle_reinforce_days > 0.0 && b.days % d.battle_reinforce_days as u64 == 0 {
            for side in 0..2 {
                for &(t, n) in &b.levies[side].clone() {
                    let serving = b.armies[side].soldiers.iter().filter(|s| s.home == t).count()
                        + b.marching.iter().filter(|m| m.side == side && m.recruit.home == t).count()
                        + squads.iter().filter(|q| q.side == side && q.home == t && !q.homeward && q.bound == Some(bi))
                            .map(|q| q.soldiers.len()).sum::<usize>();
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
                        let raised = raise(&content, &mut roster, t, town_hex(t), hex, side, n - serving, now, &mut kit);
                        // A town's levy for the war for the region marches as a squad, hex by hex (`march`): an
                        // attacker's to its side's camp first, to gather with the rest, till they've set off together.
                        if b.town.is_some() {
                            let soldiers = raised.into_iter().map(|m| crate::army::soldier_of(m.recruit, now)).collect();
                            *next_squad += 1;
                            let id = *next_squad;
                            let camp = town_hex(b.camps[side]).filter(|&c| c != hex && b.attacker == Some(side)
                                && now < b.marches + MUSTER_DAYS * day);
                            let mut q = squad(id, side, t, false, soldiers, town_hex(t).unwrap_or(hex), Some(camp.unwrap_or(hex)));
                            q.bound = Some(bi);
                            squads.push(q);
                        } else {
                            b.marching.extend(raised);
                        }
                    }
                }
            }
        }
        // Its attackers gathered at their camp (every town's levy bound there come, or `MUSTER_DAYS` since they were
        // raised): they set off together. (A squad gathering is one bound for it, its side attacking, its goal the camp.)
        if let Some(camp) = b.attacker.and_then(|a| town_hex(b.camps[a]).map(|c| (a, c))).filter(|&(_, c)| c != hex) {
            let (a, camp) = camp;
            let gathering: Vec<usize> = (0..squads.len()).filter(|&k| squads[k].bound == Some(bi) && squads[k].side == a
                && !squads[k].homeward && squads[k].goal == Some(camp)).collect();
            let all_there = gathering.iter().all(|&k| squads[k].hex == camp);
            if !gathering.is_empty() && (all_there || now >= b.marches + MUSTER_DAYS * day) {
                for k in gathering {
                    squads[k].goal = Some(hex);
                }
            }
        }
        // They meet; or a side with no one there leaves the hex to the other.
        // (Squads met on the march fight it out day by day.)
        let every = if b.field { 1 } else { d.skirmish_days as u64 };
        let mut fell: Vec<Soldier> = Vec::new();
        let mut fell_back: Option<usize> = None;
        if every > 0 && b.days % every == 0 {
            let [x, y] = &mut b.armies;
            let holder = match (x.soldiers.is_empty(), y.soldiers.is_empty()) {
                (false, false) => {
                    // (A town's defenders, `defence_lift` the better.)
                    let mut lift = [1.0; 2];
                    if let (Some(t), Some(a)) = (b.town, b.attacker) {
                        lift[1 - a] = defence_lift(d, entity(t).and_then(|e| works.get(e).ok()));
                    }
                    // (Its ground the better for the side holding it: a town's defenders, or the side waiting there.)
                    if let Some(g) = b.attacker.map(|a| 1 - a).or(b.ground_side) {
                        lift[g] *= ground_lift(d, map.ground(hex));
                    }
                    if d.numbers_weight > 0.0 {
                        let (nx, ny) = (x.soldiers.len() as f64, y.soldiers.len() as f64);
                        lift[0] *= (nx / ny).powf(d.numbers_weight);
                        lift[1] *= (ny / nx).powf(d.numbers_weight);
                    }
                    let (x_won, rx, ry) = crate::war::clash_lifted(&content, x, y, &mut persons, &mut charters, b.fights,
                        now, lift);
                    fell.extend(casualties(d, x, 2 * b.fights));
                    fell.extend(casualties(d, y, 2 * b.fights + 1));
                    // (A hero leading a fight lost falls `hero_fall_odds` of the time.)
                    for (side, army) in [(0, &*x), (1, &*y)] {
                        let lost = army.skirmishes.last().is_some_and(|k| !k.won);
                        for h in 0..army.heroes {
                            if lost && crate::persons::roll(b.fights, 2 * h as u64 + side as u64, 81) < d.hero_fall_odds {
                                heroes_fell.push(side);
                            }
                        }
                    }
                    b.fights += 1;
                    walk_home(x, rx, &town_hex, hex, now);
                    walk_home(y, ry, &town_hex, hex, now);
                    // (Met on the march, the beaten fall back home.)
                    if b.field {
                        fell_back = Some(if x_won { 1 } else { 0 });
                    }
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
        if let Some(side) = fell_back {
            let soldiers = std::mem::take(&mut b.armies[side].soldiers);
            let (theirs, loose): (Vec<Soldier>, Vec<Soldier>) = soldiers.into_iter()
                .partition(|s| s.squad.is_some_and(|id| squads.iter().any(|q| q.id == id)));
            walk_home(&mut b.armies[side], loose, &town_hex, hex, now);
            for s in theirs {
                if let Some(q) = squads.iter_mut().find(|q| Some(q.id) == s.squad) {
                    q.soldiers.push(s);
                }
            }
            for q in squads.iter_mut().filter(|q| q.battle == Some(bi) && q.side == side) {
                q.battle = None;
                q.hex = hex;
                if !q.capital {
                    q.homeward = true;
                    q.goal = town_hex(q.home);
                } else if let Some(camp) = town_hex(q.home) {
                    // (A capital's two hexes back toward its camp, to wait for the campaign's next.)
                    let back = Hex::line(hex, camp);
                    q.hex = back[back.len().saturating_sub(1).min(2)];
                    q.goal = None;
                }
            }
        }
        // The fight's dead, gone from their towns for good.
        for soldier in fell {
            let Some(e) = entity(soldier.home) else { continue };
            let Ok((mut people, roster, lockers)) = homes.get_mut(e) else { continue };
            if let (Some(hand), Some(mut roster)) = (&soldier.hand, roster) {
                bury(&mut roster, lockers.map(|l| l.into_inner()), hand);
                people.villagers = roster.residents() as f64;
            }
            mourn(&content, &mut people, now);
        }
        // Held long enough, it's the holder's: the fighting ends, and both armies walk home. A town's hex its
        // attackers hold `raze_hold_days` (its razing is the campaign's: `campaign`).
        let hold = match (b.town, b.attacker) {
            (Some(_), Some(a)) if b.holder == Some(a) => d.raze_hold_days,
            _ => d.battle_hold_days,
        };
        // (Squads met on the march: over when one side has no one left there.)
        let field_over = b.field && b.marched && (b.armies[0].soldiers.is_empty() || b.armies[1].soldiers.is_empty());
        if field_over {
            b.holder = if !b.armies[0].soldiers.is_empty() { Some(0) } else if !b.armies[1].soldiers.is_empty() { Some(1) }
                else { b.holder.or(Some(0)) };
        }
        // (A town's assault beaten `assault_breaks` fights in a row breaks off: its defenders have held it.)
        let broken = match (b.town, b.attacker) {
            (Some(_), Some(a)) if d.assault_breaks > 0.0 && b.holder == Some(1 - a) =>
                b.armies[a].skirmishes.iter().rev().take_while(|k| !k.won).count() as f64 >= d.assault_breaks,
            _ => false,
        };
        if b.holder.is_some() && (field_over || broken || b.held_days(now, day) as f64 >= hold) {
            b.won_by = b.holder.map(|s| b.factions[s]);
            for side in 0..2 {
                let soldiers = std::mem::take(&mut b.armies[side].soldiers);
                // A squad's soldiers back to it, on the hex: a town's levy home after a town's battle, on its way
                // after one on the march; a capital's waiting where it stands for the campaign's next.
                let (theirs, loose): (Vec<Soldier>, Vec<Soldier>) = soldiers.into_iter()
                    .partition(|s| s.squad.is_some_and(|id| squads.iter().any(|q| q.id == id)));
                walk_home(&mut b.armies[side], loose, &town_hex, hex, now);
                for s in theirs {
                    if let Some(q) = squads.iter_mut().find(|q| Some(q.id) == s.squad) {
                        q.soldiers.push(s);
                    }
                }
            }
            // Those on their way to it turn back: a town's home, a capital's to wait for the campaign's next.
            if !b.field {
                for q in squads.iter_mut().filter(|q| q.battle.is_none() && !q.homeward
                        && (q.bound == Some(bi) || q.goal == Some(hex))) {
                    q.waits = q.capital && q.waits;
                    if q.capital {
                        q.goal = None;
                    } else {
                        q.homeward = true;
                        q.goal = town_hex(q.home);
                    }
                }
            }
            for q in squads.iter_mut().filter(|q| q.battle == Some(bi)) {
                q.battle = None;
                q.hex = hex;
                if b.field {
                    continue;
                }
                if q.capital {
                    q.goal = None;
                } else {
                    q.homeward = true;
                    q.goal = town_hex(q.home);
                }
            }
            let marching = std::mem::take(&mut b.marching);
            for m in marching {
                let s = Soldier { home: m.recruit.home, hand: m.recruit.hand, ..Default::default() };
                walk_home(&mut b.armies[m.side], vec![s], &town_hex, hex, now);
            }
        }
    }
    // A hero fallen, for good; what was foraged.
    if let Some(c) = battles.campaign.as_mut() {
        for side in 0..2 {
            c.foraged[side] += foraged[side];
        }
        for side in heroes_fell {
            c.heroes_alive[side] = c.heroes_alive[side].saturating_sub(1);
            c.heroes_fell[side] = Some(today);
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

/// A squad of `soldiers` (each marked as its) for `side` at `hex`, bound for `goal`.
fn squad(id: u32, side: usize, home: TownId, capital: bool, mut soldiers: Vec<Soldier>, hex: Hex, goal: Option<Hex>)
        -> crate::battle::Squad {
    for s in &mut soldiers {
        s.squad = Some(id);
    }
    crate::battle::Squad { id, side, home, capital, soldiers, hex, goal, homeward: false, battle: None, bound: None, waits: false,
        way: Vec::new(), deserting: 0.0 }
}

/// A town's hex.
fn town_hex_of(world: &World, t: TownId) -> Option<Hex> {
    world.resource::<Towns>().0.get(t).and_then(|&e| world.get::<Town>(e)).and_then(|x| x.hex)
}

/// A capital's soldiers into the field at `hex`, bound for `goal`, in squads of `capital_squad`.
fn muster_capital(battles: &mut Battles, d: &crate::dials::Dials, side: usize, camp: TownId, soldiers: Vec<Soldier>,
        hex: Hex, goal: Option<Hex>) {
    let size = (d.capital_squad.max(1.0) as usize).max(1);
    for chunk in soldiers.chunks(size) {
        battles.next_squad += 1;
        let id = battles.next_squad;
        battles.squads.push(squad(id, side, camp, true, chunk.to_vec(), hex, goal));
    }
}

/// Theramore's marines landed at its camp at `hex`: its garrison there made up first (`base_garrison` of its
/// expedition, posted: John, a navy "well supplied"), the rest bound for `goal`.
fn land_marines(battles: &mut Battles, d: &crate::dials::Dials, camp: TownId, mut soldiers: Vec<Soldier>, hex: Hex,
        goal: Option<Hex>) {
    let want = (d.expedition_size.max(0.0) * d.base_garrison.clamp(0.0, 1.0)).round() as usize;
    let have: usize = battles.squads.iter().filter(|q| q.capital && q.side == 1 && q.waits).map(|q| q.size(&battles.battles)).sum();
    let posted: Vec<Soldier> = soldiers.drain(..want.saturating_sub(have).min(soldiers.len())).collect();
    if !posted.is_empty() {
        battles.next_squad += 1;
        let mut q = squad(battles.next_squad, 1, camp, true, posted, hex, Some(hex));
        q.waits = true;
        battles.squads.push(q);
    }
    muster_capital(battles, d, 1, camp, soldiers, hex, goal);
}

/// A side's capital soldiers in the field, marching or fighting.
fn capital_count(battles: &Battles, side: usize) -> u32 {
    battles.squads.iter().filter(|q| q.capital && q.side == side).map(|q| q.size(&battles.battles)).sum::<usize>() as u32
}

/// A side's capital squads not in a battle bound for `goal`.
fn send_capital(battles: &mut Battles, side: usize, goal: Hex) {
    for q in battles.squads.iter_mut().filter(|q| q.capital && q.side == side && q.battle.is_none() && !q.waits) {
        q.goal = Some(goal);
    }
}

/// A side's capital soldiers, unpaid, gone home: off the hexes and out of the battles.
fn dismiss_capital(battles: &mut Battles, side: usize) {
    battles.squads.retain(|q| !(q.capital && q.side == side));
    for b in battles.battles.iter_mut().filter(|b| b.won_by.is_none()) {
        b.armies[side].soldiers.retain(|s| !s.from_capital);
    }
}

/// The war's squads march, once a day (`docs/war-plan.md`, squads on the hexes; John: "hex-based squad movement"):
/// each not in a battle goes `squad_hexes_per_day` hexes along the line to its goal. Where it stands, a battle going
/// on there it joins on its side; a hostile squad on the march on its hex or the next, they fight there (a field
/// battle, each side's camp its town nearest). Home, its hands are home.
pub fn march(world: &mut World) {
    let content = world.resource::<Content>().clone();
    let d = &content.dials;
    let (now, day) = (world.resource::<Clock>().now, content.day_ticks.max(1));
    let today = now / day;
    if world.resource::<Battles>().squads.is_empty() || world.resource::<Battles>().squads_moved == Some(today) {
        return;
    }
    let factions = [Faction::Horde, Faction::Alliance];
    let towns_of = [faction_towns(world, factions[0]), faction_towns(world, factions[1])];
    let towns = world.resource::<Towns>().0.clone();
    let homes: Vec<Option<Hex>> = towns.iter().map(|&e| world.get::<Town>(e).and_then(|x| x.hex)).collect();
    // A squad not fighting at a town of its side fills its packs there (`pack_days` of rations a soldier): the
    // Alliance's of Theramore's stores first, then of the town's warehouse, bought.
    let refills: Vec<(usize, TownId)> = world.resource::<Battles>().squads.iter().enumerate()
        .filter(|(_, q)| q.battle.is_none() && !q.soldiers.is_empty())
        .filter_map(|(k, q)| towns_of[q.side].iter().find(|x| x.1 == q.hex).map(|x| (k, x.0))).collect();
    if !refills.is_empty() {
        let mut bt = std::mem::take(&mut *world.resource_mut::<Battles>());
        let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
        for (k, t) in refills {
            let (side, mut soldiers) = (bt.squads[k].side, std::mem::take(&mut bt.squads[k].soldiers));
            if let Some(stores) = bt.stores.get_mut(&t).filter(|_| side == 1) {
                let mut held = Stock(std::mem::take(stores));
                held.0.resize(content.goods.len(), 0.0);
                fill_packs(&content, &mut ledger, &mut held, &mut soldiers);
                *stores = held.0;
            }
            if let Some(mut stock) = world.get_mut::<Stock>(towns[t]) {
                let before = stock.0.clone();
                fill_packs(&content, &mut ledger, &mut stock, &mut soldiers);
                let taken: Vec<f64> = before.iter().zip(&stock.0).map(|(a, b)| a - b).collect();
                drop(stock);
                if side == 1 {
                    pay_for(world, &content, &mut bt, &mut ledger, t, &taken, today);
                }
            }
            bt.squads[k].soldiers = soldiers;
        }
        *world.resource_mut::<Ledger>() = ledger;
        *world.resource_mut::<Battles>() = bt;
    }
    // Elsewhere, from its side's depot on the hex, if a battle left one there: a forward base.
    {
        let mut bt = std::mem::take(&mut *world.resource_mut::<Battles>());
        let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
        for k in 0..bt.squads.len() {
            let (side, hex) = (bt.squads[k].side, bt.squads[k].hex);
            if bt.squads[k].battle.is_some() || bt.squads[k].soldiers.is_empty() || towns_of[side].iter().any(|x| x.1 == hex) {
                continue;
            }
            let Some(i) = bt.battles.iter().position(|x| x.hex == hex && food_points(&content, &x.depots[side]) > 0.0) else { continue };
            let mut depot = Stock(std::mem::take(&mut bt.battles[i].depots[side]));
            depot.0.resize(content.goods.len(), 0.0);
            fill_packs(&content, &mut ledger, &mut depot, &mut bt.squads[k].soldiers);
            bt.battles[i].depots[side] = depot.0;
        }
        *world.resource_mut::<Ledger>() = ledger;
        *world.resource_mut::<Battles>() = bt;
    }
    forage(world, &content, &towns_of, &towns);
    let pace = d.squad_hexes_per_day.max(1.0);
    let map = world.resource::<crate::map::Map>().clone();
    let per_hex = crate::settlers::per_hex(&content);
    let dry = |h: &Hex| map.ground(*h).water_share <= 0.5;
    let persons = world.resource::<crate::persons::Persons>().clone();
    let mut home: Vec<(TownId, Vec<crate::roster::Hand>)> = Vec::new();
    {
        let mut battles = world.resource_mut::<Battles>();
        let b = &mut *battles;
        b.squads_moved = Some(today);
        let ids: Vec<u32> = b.squads.iter().map(|q| q.id).collect();
        for id in ids {
            let Some(k) = b.squads.iter().position(|q| q.id == id) else { continue };
            if b.squads[k].battle.is_some() {
                continue;
            }
            // Its day's rations out of its packs; the hungry desert at the army's rate, carried, the hungriest first: a
            // town's hands home, a capital's soldiers gone.
            {
                let q = &mut b.squads[k];
                let mut hungry: Vec<usize> = Vec::new();
                for (i, s) in q.soldiers.iter_mut().enumerate() {
                    if s.pack + 1e-9 >= d.army_ration {
                        s.pack -= d.army_ration;
                    } else {
                        s.pack = 0.0;
                        s.hungry_days += 1;
                        hungry.push(i);
                    }
                }
                q.deserting += hungry.len() as f64 * d.desertion;
                let going = (q.deserting.floor().max(0.0) as usize).min(hungry.len());
                if going > 0 {
                    q.deserting -= going as f64;
                    hungry.sort_by(|&a, &c| q.soldiers[c].hungry_days.cmp(&q.soldiers[a].hungry_days).then(c.cmp(&a)));
                    let mut gone: Vec<usize> = hungry[..going].to_vec();
                    gone.sort_unstable_by(|a, c| c.cmp(a));
                    let hands: Vec<crate::roster::Hand> = gone.into_iter().filter_map(|i| q.soldiers.remove(i).hand).collect();
                    home.push((q.home, hands));
                }
            }
            // (Its day's going: `pace` hexes of open ground, harder ground costing more of it, a road less.)
            let mut left = pace;
            loop {
                let (side, at, homeward, goal) = { let q = &b.squads[k]; (q.side, q.hex, q.homeward, q.goal) };
                if b.squads[k].soldiers.is_empty() {
                    break;
                }
                if !homeward {
                    // (A battle on its way it passes; the one it's bound for it joins.)
                    if let Some(i) = b.battles.iter().position(|x| x.won_by.is_none() && x.hex == at)
                            .filter(|_| goal.is_none_or(|g| g == at)) {
                        join(b, k, i);
                        break;
                    }
                    // (A squad `overrun_odds` the smaller falls back, no stand to make: a town's home, a capital's
                    // two hexes toward its camp.)
                    // (Squads of a side on one hex stand together, its stack.)
                    let stack = |b: &Battles, side: usize, hex: Hex| -> Vec<usize> {
                        b.squads.iter().enumerate().filter(|(_, o)| o.side == side && o.hex == hex && o.battle.is_none()
                            && !o.homeward && !o.soldiers.is_empty()).map(|(j, _)| j).collect()
                    };
                    let count = |b: &Battles, of: &[usize]| of.iter().map(|&j| b.squads[j].soldiers.len()).sum::<usize>() as f64;
                    let enemy = b.squads.iter().position(|e| e.side != side && e.battle.is_none() && !e.homeward
                        && !e.soldiers.is_empty() && e.hex.distance(at) <= 1);
                    if let Some(e) = enemy {
                        let there = b.squads[e].hex;
                        let (ours, theirs) = (stack(b, side, at), stack(b, b.squads[e].side, there));
                        let (n_ours, n_theirs) = (count(b, &ours), count(b, &theirs));
                        // (A stack `overrun_odds` the smaller falls back, no stand to make: a town's squads home, a
                        // capital's to wait for the campaign's next; out of the way first.)
                        // (A stack with no drier hex farther off to fall back to is cornered, and stands.)
                        let cornered = |b: &Battles, of: &[usize], from: Hex| of.iter().any(|&j| {
                            let h = b.squads[j].hex;
                            !h.neighbors().into_iter().any(|n| dry(&n) && n.distance(from) > h.distance(from))
                        });
                        let (small, big) = if n_theirs < n_ours { (theirs.clone(), at) } else { (ours.clone(), there) };
                        if n_ours.max(n_theirs) >= d.overrun_odds * n_ours.min(n_theirs) && !cornered(b, &small, big) {
                            let mine = small.contains(&k);
                            for j in small {
                                let camp = homes.get(b.squads[j].home).copied().flatten();
                                let q = &mut b.squads[j];
                                q.waits = q.capital && q.waits;
                                q.hex = q.hex.neighbors().into_iter().filter(dry).max_by_key(|h| (h.distance(big), *h)).unwrap_or(q.hex);
                                if q.capital {
                                    q.goal = None;
                                } else {
                                    q.homeward = true;
                                    q.goal = camp;
                                }
                            }
                            if mine {
                                break;
                            }
                            // (A step of its day spent, so a stack with nowhere to fall back to doesn't hold it.)
                            left -= 1.0;
                            if left <= 0.0 {
                                break;
                            }
                            continue;
                        }
                        // (Skirmish and fall back, John: the Horde's stack met at worse than `skirmish_odds` of the
                        // enemy's strength steps back a hex, and comes on again.)
                        let worth = |b: &Battles, of: &[usize]| of.iter().map(|&j| strength(&content, &persons, &b.squads[j].soldiers)).sum::<f64>();
                        let (s_ours, s_theirs) = (worth(b, &ours), worth(b, &theirs));
                        let (horde, foe, hordes, foe_hex) = if side == 0 { (s_ours, s_theirs, ours.clone(), there) }
                            else { (s_theirs, s_ours, theirs.clone(), at) };
                        if horde < d.skirmish_odds * foe && !cornered(b, &hordes, foe_hex) {
                            let mine = hordes.contains(&k);
                            for j in hordes {
                                let q = &mut b.squads[j];
                                q.hex = q.hex.neighbors().into_iter().filter(dry).max_by_key(|h| (h.distance(foe_hex), *h)).unwrap_or(q.hex);
                            }
                            if mine {
                                break;
                            }
                            left -= 1.0;
                            if left <= 0.0 {
                                break;
                            }
                            continue;
                        }
                        // They fight where the side waiting stands, on its ground.
                        let waiting = theirs.iter().any(|&j| b.squads[j].waits && b.squads[j].goal == Some(there));
                        let hex = if waiting { there } else { at };
                        let ground_side = waiting.then_some(b.squads[e].side);
                        let camps = [0, 1].map(|s| nearest(&towns_of[s], hex).map_or(b.squads[k].home, |x| x.0));
                        b.battles.push(Battle { hex, factions, armies: Default::default(), camps, levies: Default::default(),
                            marching: Vec::new(), holder: None, held_since: None, won_by: None, began: now, days: 0,
                            fights: 0, reckoned: None, marches: now, marched: false, town: None, attacker: None,
                            settled: false, field: true, raid: false, ground_side, depots: Default::default() });
                        let i = b.battles.len() - 1;
                        for j in ours.into_iter().chain(theirs) {
                            b.squads[j].hex = hex;
                            join(b, j, i);
                        }
                        break;
                    }
                }
                let Some(goal) = goal else { break };
                if at == goal {
                    // (A town's squad bound for a battle that's over goes home; one at its post, or gathering at its
                    // camp for the battle its side attacks, waits.)
                    let gathering = b.squads[k].bound.and_then(|i| b.battles.get(i)).is_some_and(|x| x.won_by.is_none()
                        && x.attacker == Some(side) && x.hex != goal && homes.get(x.camps[side]).copied().flatten() == Some(goal));
                    if !homeward && !b.squads[k].capital && !b.squads[k].waits && !gathering {
                        b.squads[k].homeward = true;
                        b.squads[k].goal = homes.get(b.squads[k].home).copied().flatten();
                    }
                    break;
                }
                let next = way_on(&map, per_hex, &mut b.squads[k], goal);
                let going = Some(map.going(at, next, crate::map::Carrier::Llama)).filter(|g| g.is_finite())
                    .unwrap_or_else(|| map.going(at, next, crate::map::Carrier::Boat));
                let going = if going.is_finite() { going.max(0.25) } else { pace };
                // (A step harder than its day's going whole, on a fresh day.)
                if going > left && left < pace {
                    b.squads[k].way.insert(0, next);
                    break;
                }
                left -= going;
                b.squads[k].hex = next;
            }
        }
        // Squads of one town (or one capital's) marching together, one squad.
        let mut i = 0;
        while i < b.squads.len() {
            // (The capitals' march as one wherever they meet bound for one goal: the deathball.)
            let key = |q: &crate::battle::Squad| if q.capital { (q.side, 0, true, q.hex, q.goal, false, None, q.waits) }
                else { (q.side, q.home, false, q.hex, q.goal, q.homeward, q.bound, q.waits) };
            let id = b.squads[i].id;
            let mine = key(&b.squads[i]);
            let mate = b.squads.iter().position(|o| o.id != id && o.battle.is_none() && b.squads[i].battle.is_none()
                && key(o) == mine && o.id < id);
            if let Some(m) = mate {
                let mut moving = std::mem::take(&mut b.squads[i].soldiers);
                let into = b.squads[m].id;
                for s in &mut moving {
                    s.squad = Some(into);
                }
                b.squads[m].soldiers.extend(moving);
                b.squads.remove(i);
                continue;
            }
            i += 1;
        }
        // Home: its hands at home again.
        b.squads.retain_mut(|q| {
            let at_home = q.homeward && homes.get(q.home).copied().flatten() == Some(q.hex);
            if at_home || q.soldiers.is_empty() && q.battle.is_none() {
                home.push((q.home, q.soldiers.drain(..).filter_map(|s| s.hand).collect()));
                return false;
            }
            true
        });
    }
    for (t, hands) in home {
        if let Some(mut roster) = towns.get(t).and_then(|&e| world.get_mut::<Roster>(e)) {
            crate::squad::release(&mut roster, &hands);
        }
    }
}

/// The next hex of a squad's way to `goal` on foot (`Map::plan` as a llama goes: round water, over the steeps the
/// harder), planned when its goal changes or it's been moved off its way; no way round on land within the planner's
/// reach (a bay), by boat; no way at all, straight.
fn way_on(map: &crate::map::Map, per_hex: f64, q: &mut crate::battle::Squad, goal: Hex) -> Hex {
    if q.way.last() != Some(&goal) || q.way.first().is_none_or(|h| h.distance(q.hex) != 1) {
        let none = Default::default();
        q.way = [crate::map::Carrier::Llama, crate::map::Carrier::Wagon, crate::map::Carrier::Boat].into_iter()
            .find_map(|c| map.plan(q.hex, goal, per_hex, 0.0, &none, &Default::default(), c, &Default::default()))
            .map(|p| p.hexes[1..].to_vec()).filter(|w| !w.is_empty())
            .unwrap_or_else(|| Hex::line(q.hex, goal)[1..].to_vec());
    }
    q.way.remove(0)
}

/// The squads on the march forage (John: "armies forage, enemy fields first"): on an enemy town's land each fills its
/// packs from the town's harvest (`fill_packs` of its warehouse: the town eats less); elsewhere on dry ground the land
/// feeds `forage_wild` soldiers a hex, shared among those standing there.
fn forage(world: &mut World, content: &Content, towns_of: &[Vec<(TownId, Hex)>; 2], towns: &[Entity]) {
    let d = &content.dials;
    let cap = d.pack_days.max(0.0) * d.army_ration;
    let wild = d.forage_wild.max(0.0) * d.army_ration;
    let mut on_hex: std::collections::BTreeMap<Hex, usize> = std::collections::BTreeMap::new();
    for q in world.resource::<Battles>().squads.iter().filter(|q| q.battle.is_none()) {
        *on_hex.entry(q.hex).or_default() += q.soldiers.len();
    }
    let side_of: std::collections::BTreeMap<TownId, usize> =
        (0..2).flat_map(|s| towns_of[s].iter().map(move |x| (x.0, s))).collect();
    let territory = world.resource::<crate::territory::Territory>().clone();
    let map = world.resource::<crate::map::Map>().clone();
    let mut bt = std::mem::take(&mut *world.resource_mut::<Battles>());
    let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
    for k in 0..bt.squads.len() {
        let (side, hex) = (bt.squads[k].side, bt.squads[k].hex);
        if bt.squads[k].battle.is_some() || bt.squads[k].soldiers.is_empty() {
            continue;
        }
        let enemy = territory.owner(hex).filter(|t| side_of.get(t).is_some_and(|&s| s != side));
        if let Some(mut stock) = enemy.and_then(|t| world.get_mut::<Stock>(towns[t])) {
            let before: f64 = bt.squads[k].soldiers.iter().map(|s| s.pack).sum();
            fill_packs(content, &mut ledger, &mut stock, &mut bt.squads[k].soldiers);
            let after: f64 = bt.squads[k].soldiers.iter().map(|s| s.pack).sum();
            if let Some(c) = bt.campaign.as_mut() {
                c.foraged[side] += after - before;
            }
        } else if wild > 0.0 && map.ground(hex).water_share <= 0.5 {
            let each = (wild / on_hex.get(&hex).copied().unwrap_or(1).max(1) as f64).min(d.army_ration);
            for s in bt.squads[k].soldiers.iter_mut() {
                s.pack = (s.pack + each).min(cap.max(s.pack));
            }
        }
    }
    *world.resource_mut::<Ledger>() = ledger;
    *world.resource_mut::<Battles>() = bt;
}

/// Fills soldiers' packs from a warehouse, up to `pack_days` of rations each, the heartiest food first (`army::mess`:
/// out of the world's goods, eaten as it's carried).
fn fill_packs(content: &Content, ledger: &mut Ledger, stock: &mut Stock, soldiers: &mut [Soldier]) {
    let cap = content.dials.pack_days.max(0.0) * content.dials.army_ration;
    let need: f64 = soldiers.iter().map(|s| (cap - s.pack).max(0.0)).sum();
    if need <= 1e-9 {
        return;
    }
    let meal = crate::army::mess(content, ledger, stock, need);
    let share = (meal.eaten / need).min(1.0);
    for s in soldiers.iter_mut() {
        s.pack += (cap - s.pack).max(0.0) * share;
    }
}

/// The food points in goods held (a depot, a wagon's load).
fn food_points(content: &Content, goods: &[f64]) -> f64 {
    goods.iter().enumerate().map(|(g, n)| n * content.goods.get(g).map_or(0.0, |x| x.food)).sum()
}

/// A wagon's way between two hexes (`Map::plan` as a wagon goes: roads, round water; a llama's if a wagon has none;
/// by boat across a bay; else the line), the hexes after `from`.
fn supply_way(map: &crate::map::Map, per_hex: f64, from: Hex, to: Hex) -> Vec<Hex> {
    let none = Default::default();
    [crate::map::Carrier::Wagon, crate::map::Carrier::Llama, crate::map::Carrier::Boat].into_iter()
        .find_map(|c| map.plan(from, to, per_hex, 0.0, &none, &Default::default(), c, &Default::default()))
        .map(|p| p.hexes[1..].to_vec()).filter(|w| !w.is_empty())
        .unwrap_or_else(|| Hex::line(from, to)[1..].to_vec())
}

/// The war's supply wagons, once a day, after the squads march (John: "the long supply chains in hostile territory
/// should carry a vulnerability and ... attacking the convoys should constitute a core strategy for the horde
/// forces"): each side of a town's battle away from its camp is sent a wagon when its depot and the wagons on their
/// way hold under `depot_days` of its rations (one a day), loaded at its camp with `wagon_load` food points and
/// `wagon_arrows` arrows and escorted by `wagon_escort` of its capital's soldiers there; each goes
/// `wagon_hexes_per_day` of open ground on its way, unloads at the depot and comes home. A hostile squad within a hex
/// of a wagon fights its escort: stronger, it takes the load (the food into its packs, the rest spoilt) and the escort
/// falls; else it falls back a hex.
pub fn supply(world: &mut World) {
    let content = world.resource::<Content>().clone();
    let d = &content.dials;
    let (now, day) = (world.resource::<Clock>().now, content.day_ticks.max(1));
    let today = now / day;
    {
        let b = world.resource::<Battles>();
        if b.supplied == Some(today) || b.campaign.is_none() && b.supplies.is_empty() {
            return;
        }
    }
    let map = world.resource::<crate::map::Map>().clone();
    let per_hex = crate::settlers::per_hex(&content);
    let towns = world.resource::<Towns>().0.clone();
    let n_goods = content.goods.len();
    let factions = [Faction::Horde, Faction::Alliance];
    let towns_of = [faction_towns(world, factions[0]), faction_towns(world, factions[1])];
    let hex_of: Vec<Option<Hex>> = towns.iter().map(|&e| world.get::<Town>(e).and_then(|x| x.hex)).collect();
    let arrow = crate::army::good_named(&content, crate::army::ARROW);
    let persons = world.resource::<crate::persons::Persons>().clone();
    let mut b = std::mem::take(&mut *world.resource_mut::<Battles>());
    let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
    b.supplied = Some(today);
    // Sent: a wagon for each side of a town's battle short of its rations, loaded at its camp.
    let mut sends: Vec<(usize, usize, TownId, Hex, Hex)> = Vec::new();
    for (i, battle) in b.battles.iter().enumerate().filter(|(_, x)| x.won_by.is_none() && x.town.is_some()) {
        for side in 0..2 {
            let camp = battle.camps[side];
            let Some(camp_hex) = hex_of.get(camp).copied().flatten() else { continue };
            if camp_hex == battle.hex || b.ruins.contains(&camp) {
                continue;
            }
            let soldiers = battle.armies[side].soldiers.len() + b.squads.iter()
                .filter(|q| q.side == side && q.battle.is_none() && !q.homeward && (q.goal == Some(battle.hex) || q.bound == Some(i)))
                .map(|q| q.soldiers.len()).sum::<usize>();
            if soldiers == 0 || b.supplies.iter().any(|w| w.battle == i && w.side == side && w.left == today) {
                continue;
            }
            let need = soldiers as f64 * d.army_ration * d.depot_days.max(0.0);
            let have = food_points(&content, &battle.depots[side]) + b.supplies.iter()
                .filter(|w| w.battle == i && w.side == side && !w.homeward).map(|w| food_points(&content, &w.load)).sum::<f64>();
            if have < need {
                sends.push((i, side, camp, camp_hex, battle.hex));
            }
        }
    }
    for (i, side, camp, camp_hex, goal) in sends {
        // Loaded of Theramore's stores there first, then of the camp's warehouse, bought.
        let mut load = vec![0.0; n_goods];
        let mut points = 0.0;
        if let Some(stores) = b.stores.get_mut(&camp).filter(|_| side == 1) {
            stores.resize(n_goods, 0.0);
            points += load_food(&content, stores, &mut load, d.wagon_load);
        }
        let Some(mut stock) = world.get_mut::<Stock>(towns[camp]) else { continue };
        let mut bought = vec![0.0; n_goods];
        points += load_food(&content, &mut stock.0, &mut bought, d.wagon_load - points);
        if points <= 0.0 {
            continue;
        }
        if let Some(a) = arrow {
            let taken = crate::kit::whole(stock.0[a]).min(d.wagon_arrows.max(0.0).floor());
            stock.0[a] -= taken;
            bought[a] += taken;
        }
        drop(stock);
        for (g, n) in bought.iter().enumerate() {
            load[g] += n;
        }
        if side == 1 {
            pay_for(world, &content, &mut b, &mut ledger, camp, &bought, today);
        }
        // Its escort: the capital's soldiers waiting at the camp, some left there.
        let escort_n = d.wagon_escort.max(0.0) as usize;
        let escort = b.squads.iter_mut().filter(|q| q.side == side && q.capital && q.battle.is_none() && q.hex == camp_hex
                && q.soldiers.len() > escort_n).max_by_key(|q| (q.soldiers.len(), std::cmp::Reverse(q.id)))
            .map(|q| { let keep = q.soldiers.len() - escort_n; q.soldiers.split_off(keep) }).unwrap_or_default();
        b.next_supply += 1;
        let way = supply_way(&map, per_hex, camp_hex, goal);
        b.supplies.push(crate::battle::Supply { id: b.next_supply, side, battle: i, camp, hex: camp_hex, way, load, escort,
            homeward: false, left: today });
        if let Some(c) = b.campaign.as_mut() {
            c.wagons_sent[side] += 1;
        }
    }
    // On the road: each its day's going, met by the enemy or not.
    let pace = d.wagon_hexes_per_day.max(1.0);
    let mut k = 0;
    while k < b.supplies.len() {
        let side = b.supplies[k].side;
        let open = b.battles.get(b.supplies[k].battle).is_some_and(|x| x.won_by.is_none());
        if !open {
            b.supplies[k].homeward = true;
        }
        let mut left = pace;
        let mut gone = false;
        loop {
            let at = b.supplies[k].hex;
            // Met: the hostile squads within a hex.
            let raiders: Vec<usize> = (0..b.squads.len()).filter(|&j| {
                let q = &b.squads[j];
                q.side != side && q.battle.is_none() && !q.homeward && !q.soldiers.is_empty() && q.hex.distance(at) <= 1
            }).collect();
            if !raiders.is_empty() {
                let theirs: f64 = raiders.iter().map(|&j| strength(&content, &persons, &b.squads[j].soldiers)).sum();
                let ours = strength(&content, &persons, &b.supplies[k].escort);
                if theirs > ours {
                    // Taken: its food into the raiders' packs, the rest spoilt; its escort fallen.
                    let mut stock = Stock(std::mem::take(&mut b.supplies[k].load));
                    for &j in &raiders {
                        fill_packs(&content, &mut ledger, &mut stock, &mut b.squads[j].soldiers);
                    }
                    for (g, n) in stock.0.iter().enumerate() {
                        ledger.used[g] += n;
                    }
                    if let Some(c) = b.campaign.as_mut() {
                        c.wagons_taken[side] += 1;
                    }
                    b.raids.push((today, at, side));
                    b.supplies.remove(k);
                    gone = true;
                    break;
                }
                for &j in &raiders {
                    let q = &mut b.squads[j];
                    q.hex = q.hex.neighbors().into_iter().filter(|h| map.ground(*h).water_share <= 0.5)
                        .max_by_key(|h| (h.distance(at), *h)).unwrap_or(q.hex);
                }
            }
            let w = &b.supplies[k];
            let goal = if w.homeward {
                hex_of.get(w.camp).copied().flatten().filter(|_| !b.ruins.contains(&w.camp))
                    .or_else(|| nearest(&towns_of[side], at).map(|x| x.1))
            } else {
                b.battles.get(w.battle).map(|x| x.hex)
            };
            let Some(goal) = goal else { break };
            if at == goal {
                let w = &mut b.supplies[k];
                if !w.homeward {
                    // At its depot: unloaded, and home.
                    let depot = &mut b.battles[w.battle].depots[side];
                    depot.resize(n_goods, 0.0);
                    for (g, n) in std::mem::take(&mut w.load).into_iter().enumerate() {
                        depot[g] += n;
                    }
                    w.load = vec![0.0; n_goods];
                    w.homeward = true;
                    w.way.clear();
                    if let Some(c) = b.campaign.as_mut() {
                        c.wagons_delivered[side] += 1;
                    }
                    continue;
                }
                // Home: what it brought back to the town's warehouse, its escort to wait there.
                let w = b.supplies.remove(k);
                let town = towns_of[side].iter().find(|x| x.1 == at).map(|x| x.0);
                if let Some(mut stock) = town.and_then(|t| world.get_mut::<Stock>(towns[t])) {
                    for (g, n) in w.load.iter().enumerate() {
                        stock.0[g] += n;
                    }
                } else {
                    for (g, n) in w.load.iter().enumerate() {
                        ledger.used[g] += n;
                    }
                }
                if !w.escort.is_empty() {
                    b.next_squad += 1;
                    let mut q = squad(b.next_squad, side, w.camp, true, w.escort, at, Some(at));
                    q.waits = true;
                    b.squads.push(q);
                }
                gone = true;
                break;
            }
            let w = &mut b.supplies[k];
            if w.way.last() != Some(&goal) || w.way.first().is_none_or(|h| h.distance(at) != 1) {
                w.way = supply_way(&map, per_hex, at, goal);
            }
            let next = w.way[0];
            let going = Some(map.going(at, next, crate::map::Carrier::Wagon)).filter(|g| g.is_finite())
                .unwrap_or_else(|| map.going(at, next, crate::map::Carrier::Boat));
            let going = if going.is_finite() { going.max(0.25) } else { pace };
            if going > left && left < pace {
                break;
            }
            left -= going;
            w.way.remove(0);
            w.hex = next;
        }
        if !gone {
            k += 1;
        }
    }
    *world.resource_mut::<Ledger>() = ledger;
    *world.resource_mut::<Battles>() = b;
}

/// Theramore's ships' rations (John: the marines' "both" ways): `convoy_rations` of the expedition's bread till its
/// ships come again into its camp's warehouse, paid of its chest at the bread's worth, as much as it can.
fn ship_rations(world: &mut World, content: &Content, camp: TownId, today: u64) {
    let d = &content.dials;
    let Some(bread) = crate::army::good_named(content, crate::army::BREAD) else { return };
    let (food, worth) = (content.goods[bread].food.max(1e-9), content.goods[bread].value.max(0.0));
    let loaves = (d.expedition_size.max(0.0) * d.army_ration * d.convoy_days.max(1.0) * d.convoy_rations.max(0.0) / food).floor();
    if loaves <= 0.0 {
        return;
    }
    let mut battles = world.resource_mut::<Battles>();
    let affordable = if worth > 0.0 { (battles.chest_left(d, Faction::Alliance) / worth).floor() } else { loaves };
    let loaves = loaves.min(affordable);
    battles.draw(d, Faction::Alliance, crate::fixed::Fixed::ceil(loaves * worth), today);
    let stores = battles.stores.entry(camp).or_default();
    stores.resize(content.goods.len(), 0.0);
    stores[bread] += loaves;
    drop(battles);
    world.resource_mut::<Ledger>().made[bread] += loaves;
}

/// Food loaded heartiest first out of `held` into `load`, up to `points` (whole items, the last one over); the points
/// loaded.
fn load_food(content: &Content, held: &mut [f64], load: &mut [f64], points: f64) -> f64 {
    let mut loaded = 0.0;
    for g in crate::army::heartiest_first(content, &Stock(held.to_vec())) {
        let left = points - loaded;
        if left <= 1e-9 {
            break;
        }
        let food = content.goods[g].food;
        let taken = (left / food - 1e-9).ceil().min(crate::kit::whole(held[g]));
        held[g] -= taken;
        load[g] += taken;
        loaded += taken * food;
    }
    loaded
}

/// Theramore buys what its marines take of an Alliance town's warehouse (John: "theramore should pay for the food"):
/// `taken` at the town's price for each (`Market::mark`), paid of its chest into the town's treasury, as much as it
/// has left.
fn pay_for(world: &mut World, content: &Content, b: &mut Battles, ledger: &mut Ledger, t: TownId, taken: &[f64], today: u64) {
    let market = world.resource::<crate::market::Market>();
    let worth: f64 = taken.iter().enumerate().filter(|(_, n)| **n > 0.0).map(|(g, n)| n * market.mark(content, t, g)).sum();
    if worth <= 0.0 {
        return;
    }
    let paid = b.draw(&content.dials, Faction::Alliance, crate::fixed::Fixed::ceil(worth), today);
    if paid <= crate::fixed::Fixed::ZERO {
        return;
    }
    let (now, day) = (world.resource::<Clock>().now, content.day_ticks.max(1));
    let e = world.resource::<Towns>().0[t];
    let mut cash = std::mem::take(&mut *world.resource_mut::<CashBook>());
    if let Some(mut money) = world.get_mut::<Money>(e) {
        post(ledger, &mut cash, Side::outside(), Side::treasury(t, &mut money.0), paid,
            Reason::Other(format!("{}'s war chest", Faction::Alliance.capital())), now, day);
    }
    *world.resource_mut::<CashBook>() = cash;
}

/// What a squad would bring to a fight on open ground: its soldiers' fundamentals summed (a council's reckoning).
fn strength(content: &Content, persons: &crate::persons::Persons, soldiers: &[Soldier]) -> f64 {
    let nobody = crate::war::stranger();
    soldiers.iter().map(|s| {
        let p = s.hand.as_ref().and_then(|h| persons.hands.get(&h.id)).unwrap_or(&nobody);
        crate::war::fundamentals(content, s, p, crate::war::Twist::Open)
    }).sum()
}

/// How much better the side holding a hex's ground fights there: on hills, in woods, behind a river.
pub fn ground_lift(d: &crate::dials::Dials, ground: crate::map::Ground) -> f64 {
    use crate::map::Land;
    1.0 + match ground.land { Land::Hills => d.hills_lift, Land::Woods | Land::Taiga => d.woods_lift, _ => 0.0 }
        + if ground.river { d.river_lift } else { 0.0 }
}

/// How good a hex's ground is to make a stand on: hills, then woods or a river.
fn ground_worth(ground: crate::map::Ground) -> i32 {
    use crate::map::Land;
    (match ground.land { Land::Hills => 3, Land::Woods | Land::Taiga => 2, _ => 0 }) + if ground.river { 2 } else { 0 }
}

/// The war councils, once a day after the campaign's (`docs/war-plan.md`, squads on the hexes; John: "strategies that
/// can be enacted against the deathball strategy of the marines"):
/// - **The Alliance's**: its marines not fighting go to a raid on its towns (the deathball turned back).
/// - **The Horde's**, for the Alliance's attack on one of its towns: the defenders from elsewhere mass ahead of the
///   marines on the best ground within `choke_reach` of the town on their road (a hill, woods, a river) while the
///   marines are on their way; once they're at the town, `surround_sides` squads take posts round it (its army cut off
///   from its camp) and the rest join the fight. And while the marines are `raid_hexes` or more from an Alliance town,
///   it raids it with its towns' squads within `battle_reach`.
pub fn council(world: &mut World) {
    let content = world.resource::<Content>().clone();
    let d = &content.dials;
    let now = world.resource::<Clock>().now;
    let Some(c) = world.resource::<Battles>().campaign.clone() else { return };
    if c.won_by.is_some() || !c.landed {
        return;
    }
    let map = world.resource::<crate::map::Map>().clone();
    let (horde, alliance) = (faction_towns(world, Faction::Horde), faction_towns(world, Faction::Alliance));
    // The Horde raids an Alliance town far from the marines (one raid at a time).
    let marines: Vec<Hex> = {
        let b = world.resource::<Battles>();
        b.squads.iter().filter(|q| q.capital && q.side == 1).map(|q| q.hex).collect()
    };
    let raiding = world.resource::<Battles>().battles.iter().any(|b| b.raid && b.won_by.is_none());
    let campaign_on = world.resource::<Battles>().battles.iter().any(|b| b.town.is_some() && !b.raid && b.won_by.is_none());
    if !raiding && campaign_on && !marines.is_empty() && d.raid_hexes > 0.0 {
        let far = |h: Hex| marines.iter().map(|m| m.distance(h)).min().unwrap_or(i32::MAX);
        let target = alliance.iter().copied().filter(|&(_, h)| far(h) as f64 >= d.raid_hexes)
            .filter(|&(_, h)| horde.iter().any(|x| x.1.distance(h) as f64 <= d.battle_reach))
            .max_by_key(|&(t, h)| (far(h), std::cmp::Reverse(t)));
        if let Some((t, hex)) = target {
            let camp = nearest(&horde, hex).expect("towns").0;
            let mut battle = town_battle(world, &content, t, hex, Faction::Horde, camp, now, false);
            // (A raid's Horde levy: its towns in reach only, `battle_levy` of each.)
            let towns = world.resource::<Towns>().0.clone();
            battle.levies[0] = horde.iter().filter(|x| x.1.distance(hex) as f64 <= d.battle_reach)
                .filter_map(|&(h, _)| world.get::<Roster>(towns[h]).map(|r| (h, levy_of(&content, r)))).collect();
            battle.raid = true;
            world.resource_mut::<Battles>().battles.push(battle);
        }
    }
    let base_hex = world.resource::<Battles>().campaign.as_ref().and_then(|c| c.base)
        .filter(|t| !world.resource::<Battles>().ruins.contains(t)).and_then(|t| town_hex_of(world, t));
    let mut battles = world.resource_mut::<Battles>();
    let b = &mut *battles;
    // Theramore's garrison back to its post at its base.
    for q in b.squads.iter_mut().filter(|q| q.capital && q.side == 1 && q.waits && q.battle.is_none()) {
        q.goal = base_hex;
    }
    // The Alliance's answer to a raid on its towns: a detachment of its marines not fighting, `overrun_odds` the
    // raiders, the rest marching on (those sallying round its assault, not).
    if let Some(ri) = b.battles.iter().position(|x| x.raid && x.won_by.is_none() && x.attacker == Some(0)) {
        let raid = b.battles[ri].hex;
        let raiders = b.battles[ri].armies[0].soldiers.len() + b.squads.iter()
            .filter(|q| q.side == 0 && q.battle.is_none() && !q.homeward && q.bound == Some(ri)).map(|q| q.soldiers.len()).sum::<usize>();
        let want = ((raiders.max(1) as f64 * d.overrun_odds).ceil() as usize).max(1);
        let have = b.battles[ri].armies[1].soldiers.iter().filter(|s| s.from_capital).count() + b.squads.iter()
            .filter(|q| q.capital && q.side == 1 && q.battle.is_none() && q.goal == Some(raid)).map(|q| q.soldiers.len()).sum::<usize>();
        let assault = b.battles.iter().rfind(|x| x.town.is_some() && !x.raid && x.won_by.is_none() && x.attacker == Some(1))
            .map(|x| x.hex);
        let from = b.squads.iter().enumerate().filter(|(_, q)| q.capital && q.side == 1 && q.battle.is_none() && !q.waits
                && q.goal != Some(raid) && !q.soldiers.is_empty() && !assault.is_some_and(|h| h.distance(q.hex) <= 2))
            .max_by_key(|(_, q)| (q.soldiers.len(), std::cmp::Reverse(q.id))).map(|(k, _)| k);
        if let (true, Some(k)) = (have < want, from) {
            let n = (want - have).min(b.squads[k].soldiers.len());
            if n == b.squads[k].soldiers.len() {
                b.squads[k].goal = Some(raid);
            } else {
                let keep = b.squads[k].soldiers.len() - n;
                let out = b.squads[k].soldiers.split_off(keep);
                let (camp, at) = (b.squads[k].home, b.squads[k].hex);
                b.next_squad += 1;
                let id = b.next_squad;
                b.squads.push(squad(id, 1, camp, true, out, at, Some(raid)));
            }
        }
    }
    // The Alliance's: its idle marines back to the battle going on; and cut off from its camp there, or its supply way
    // held, it sallies on a post that cuts it off, `overrun_odds` the post's size (which falls back before it).
    if let Some(i) = b.battles.iter().rposition(|x| x.town.is_some() && !x.raid && x.won_by.is_none()) {
        let hex = b.battles[i].hex;
        for q in b.squads.iter_mut().filter(|q| q.capital && q.side == 1 && q.battle.is_none() && !q.waits
                && (q.goal.is_none() || q.goal == Some(q.hex) && q.hex != hex)) {
            q.goal = Some(hex);
        }
        let camp = b.battles[i].camps[1];
        let camp_hex = alliance.iter().find(|x| x.0 == camp).map(|x| x.1);
        let posted: Vec<(usize, Hex)> = b.squads.iter().filter(|q| q.battle.is_none() && q.waits && !q.soldiers.is_empty())
            .map(|q| (q.side, q.hex)).collect();
        // (And it clears its supply way: a Horde post within a hex of it, the nearest its camp first.)
        let road = camp_hex.filter(|&c| c != hex).map(|c| supply_way(&map, crate::settlers::per_hex(&content), c, hex))
            .unwrap_or_default();
        let on_road = |h: Hex| road.iter().any(|r| r.distance(h) <= 1);
        let cut = cut_off(d, 1, hex, camp_hex, &posted);
        let blocked = b.squads.iter().any(|q| q.side == 0 && q.waits && q.battle.is_none() && !q.soldiers.is_empty() && on_road(q.hex));
        if !b.battles[i].armies[1].soldiers.is_empty() && (cut || blocked) {
            let post = if cut {
                b.squads.iter().filter(|q| q.side == 0 && q.waits && q.battle.is_none() && q.hex.distance(hex) <= 2)
                    .min_by_key(|q| (q.soldiers.len(), q.id)).map(|q| (q.hex, q.soldiers.len()))
            } else {
                b.squads.iter().filter(|q| q.side == 0 && q.waits && q.battle.is_none() && !q.soldiers.is_empty() && on_road(q.hex))
                    .min_by_key(|q| (camp_hex.map_or(0, |c| c.distance(q.hex)), q.id)).map(|q| (q.hex, q.soldiers.len()))
            };
            let marines = b.battles[i].armies[1].soldiers.iter().filter(|s| s.from_capital).count();
            let sallying = b.squads.iter().any(|q| q.capital && q.side == 1 && q.battle.is_none() && !q.waits
                && q.goal != Some(hex));
            if let (Some((at, n)), false) = (post, sallying) {
                let want = ((n as f64 * d.overrun_odds).ceil() as usize).max(1);
                if want * 2 <= marines {
                    b.next_squad += 1;
                    let id = b.next_squad;
                    let army = &mut b.battles[i].armies[1];
                    let mut out = Vec::new();
                    let mut j = 0;
                    while j < army.soldiers.len() && out.len() < want {
                        if army.soldiers[j].from_capital {
                            out.push(army.soldiers.remove(j));
                        } else {
                            j += 1;
                        }
                    }
                    b.squads.push(squad(id, 1, camp, true, out, hex, Some(at)));
                }
            }
        }
    }
    // The Horde's defence of a town the Alliance attacks.
    let Some(i) = b.battles.iter().rposition(|x| x.town.is_some() && !x.raid && x.won_by.is_none() && x.attacker == Some(1))
        else { return };
    let (hex, target) = (b.battles[i].hex, b.battles[i].town.expect("a town"));
    let at_town = !b.battles[i].armies[1].soldiers.is_empty();
    // (The marines' road: their way there, as they march it; else the line.)
    let road = b.squads.iter().filter(|q| q.capital && q.side == 1).max_by_key(|q| (q.soldiers.len(), q.id))
        .map(|q| if q.goal == Some(hex) && !q.way.is_empty() { q.way.clone() } else { Hex::line(q.hex, hex) });
    let choke = road.and_then(|road| {
        road.into_iter().filter(|h| *h != hex && (1..=d.choke_reach as i32).contains(&h.distance(hex))
            && map.ground(*h).water_share <= 0.5)
            .max_by_key(|h| (ground_worth(map.ground(*h)), -h.distance(hex), *h))
    });
    let mut posts: Vec<Hex> = if at_town {
        let mut round: Vec<Hex> = hex.neighbors().into_iter().filter(|h| map.ground(*h).water_share <= 0.5).collect();
        round.sort_by_key(|h| (std::cmp::Reverse(ground_worth(map.ground(*h))), *h));
        round.truncate(d.surround_sides.max(0.0) as usize);
        round
    } else {
        Vec::new()
    };
    // (Cut supply, John: "attacking the convoys should constitute a core strategy for the horde forces": the
    // largest `ambush_squads` of its squads wait on the marines' supply way, `ambush_reach` from the town and from
    // every marine, on the best ground.)
    let camp_hex = alliance.iter().find(|x| x.0 == b.battles[i].camps[1]).map(|x| x.1).filter(|&h| h != hex);
    let reach = d.ambush_reach.max(0.0) as i32;
    let ambush = camp_hex.and_then(|from| {
        supply_way(&map, crate::settlers::per_hex(&content), from, hex).into_iter()
            .filter(|h| map.ground(*h).water_share <= 0.5 && h.distance(hex) >= reach && h.distance(from) >= reach
                && marines.iter().all(|m| m.distance(*h) >= reach))
            .max_by_key(|h| (ground_worth(map.ground(*h)), *h))
    });
    let mut largest: Vec<(usize, u32)> = b.squads.iter().filter(|q| q.side == 0 && !q.capital && !q.homeward
        && q.battle.is_none() && q.bound == Some(i) && q.home != target).map(|q| (q.soldiers.len(), q.id)).collect();
    largest.sort_unstable_by(|x, y| y.cmp(x));
    let mut ambushers: Vec<u32> = largest.into_iter().take(if ambush.is_some() { d.ambush_squads.max(0.0) as usize } else { 0 })
        .map(|x| x.1).collect();
    ambushers.sort_unstable();
    for q in b.squads.iter_mut().filter(|q| q.side == 0 && !q.capital && !q.homeward && q.battle.is_none()
            && q.bound == Some(i) && q.home != target) {
        if let (Some(at), true) = (ambush, ambushers.binary_search(&q.id).is_ok()) {
            q.goal = Some(at);
            q.waits = true;
            continue;
        }
        if !at_town {
            if let Some(choke) = choke {
                q.goal = Some(choke);
                q.waits = true;
                continue;
            }
        }
        if let Some(post) = posts.pop() {
            q.goal = Some(post);
            q.waits = true;
        } else {
            q.goal = Some(hex);
            q.waits = false;
        }
    }
}

/// Is an army on `hex` cut off from its camp at `camp`: hostile squads on `surround_sides` of the hexes round it, or
/// one on the road between them (`posted`: each squad at its post, its side and hex)?
fn cut_off(d: &crate::dials::Dials, side: usize, hex: Hex, camp: Option<Hex>, posted: &[(usize, Hex)]) -> bool {
    let hostile: Vec<Hex> = posted.iter().filter(|p| p.0 != side).map(|p| p.1).collect();
    let round = hex.neighbors().into_iter().filter(|n| hostile.contains(n)).count();
    let sides = d.surround_sides.max(0.0) as usize;
    let road = camp.filter(|&c| c != hex).is_some_and(|c| {
        let line = Hex::line(c, hex);
        line[1..line.len() - 1].iter().any(|h| hostile.contains(h))
    });
    sides > 0 && round >= sides || road
}

/// A squad's soldiers into battle `i`'s army on its side, there.
fn join(b: &mut Battles, k: usize, i: usize) {
    let q = &mut b.squads[k];
    q.battle = Some(i);
    q.hex = b.battles[i].hex;
    let army = &mut b.battles[i].armies[q.side];
    for s in q.soldiers.drain(..) {
        army.records.entry(s.home).or_default().raised += 1;
        army.soldiers.push(s);
    }
}

/// A fight's dead on one side (John: "more realistic partial damage to the winning side"): `combat_losses` of its
/// soldiers by the enemy's share of the two scores (its last skirmish's), the winner fewer, a part of one by the draw;
/// who falls by each soldier's draw for fight `no`. Off the army, counted as fallen by their towns.
fn casualties(d: &crate::dials::Dials, army: &mut Army, no: u64) -> Vec<Soldier> {
    let Some(k) = army.skirmishes.last() else { return Vec::new() };
    let n = army.soldiers.len();
    let share = if k.score + k.target > 0.0 { k.target / (k.score + k.target) } else { 0.5 };
    let dead = ((n as f64 * d.combat_losses.max(0.0) * share + crate::persons::roll(no, n as u64, 79)).floor() as usize).min(n);
    let draw = |i: usize, s: &Soldier| crate::persons::roll(s.hand.as_ref().map_or(u64::MAX - i as u64, |h| h.id as u64), no, 80);
    let mut order: Vec<(f64, usize)> = army.soldiers.iter().enumerate().map(|(i, s)| (draw(i, s), i)).collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut picked: Vec<usize> = order[..dead].iter().map(|x| x.1).collect();
    picked.sort_unstable_by(|a, b| b.cmp(a));
    let fallen: Vec<Soldier> = picked.into_iter().map(|i| army.soldiers.remove(i)).collect();
    for s in &fallen {
        army.records.entry(s.home).or_default().fallen += 1;
    }
    fallen
}

/// The kit a town procures for its levy of `n` (John: "Towns should procure on behalf of the crown"): a sword, a
/// helmet and a chestplate each, and shields for half (the crown's stockpile's iron).
const LEVY_KIT: [(&str, f64); 4] = [("iron_sword", 1.0), ("iron_helmet", 1.0), ("iron_chestplate", 1.0), ("shield", 0.5)];

/// A battle's towns arm, once a day, on the crown's behalf as the crown does at its capital (`rules/crown.rs`; John:
/// "look at how the early war scenario involved plenty of incentives to get the production system in place early
/// using public spending"): each levied town requisitions its levy's kit beyond what it holds and has coming, at the
/// mobilising premium till its levies march and the war's after; its bids taken are awarded as the crown's are, a
/// third up front; and it tenders to train the master smiths its roads' towns lack. Its faction's war chest pays it
/// each contract's and tender's worth when let (from outside the society, as the crown's mint): the public spending
/// that pays the realm's towns to take up smithing.
pub fn procure(world: &mut World) {
    use crate::crown::{apply_awards, award, OFFTAKE, TRAINED, TRAINING_DAYS};
    use crate::sourcing::{route_facts, Env};
    let content = world.resource::<Content>().clone();
    let (now, day) = (world.resource::<Clock>().now, content.day_ticks.max(1));
    let d = &content.dials;
    let today = now / day;
    let mut battles = std::mem::take(&mut *world.resource_mut::<Battles>());
    let fresh = battles.procured != Some(today);
    battles.procured = Some(today);
    // Each levied town of a battle not won, its levy, its faction, and whether its levies have marched: the nearest
    // the fighting first.
    let towns = world.resource::<Towns>().0.clone();
    let mut buyers: Vec<(TownId, usize, Faction, bool)> = Vec::new();
    let mut far: Vec<i32> = Vec::new();
    for b in battles.battles.iter().filter(|b| b.won_by.is_none()) {
        for side in 0..2 {
            for &(t, n) in &b.levies[side] {
                if !buyers.iter().any(|x| x.0 == t) {
                    buyers.push((t, n, b.factions[side], now >= b.marches));
                    far.push(world.get::<Town>(towns[t]).and_then(|x| x.hex).map_or(i32::MAX, |h| h.distance(b.hex)));
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..buyers.len()).collect();
    order.sort_by_key(|&i| (far[i], buyers[i].0));
    let buyers: Vec<(TownId, usize, Faction, bool)> = order.into_iter().map(|i| buyers[i]).collect();
    *world.resource_mut::<Battles>() = battles;
    if !fresh {
        return;
    }
    let kit: Vec<(crate::content::GoodId, f64)> = LEVY_KIT.iter()
        .filter_map(|&(name, each)| crate::army::good_named(&content, name).map(|g| (g, each))).collect();
    let mut sov = std::mem::take(&mut *world.resource_mut::<Sovereign>());
    sov.lapse(now);
    let mut ordered: std::collections::BTreeMap<Faction, f64> = Default::default();
    for &(t, n, faction, marched) in &buyers {
        let premium = if marched { d.war_premium } else { d.mobilising_premium };
        // Bids in: the lots awarded.
        let routes = route_facts(world);
        let awards = award(&Env::gather(world, routes), &mut sov.requisitions, t, sov.theatre);
        sov.awarded += awards.len() as u32;
        apply_awards(world, &awards);
        // What its levy still lacks, while its capital doesn't owe its towns `chest_backlog_days` of its pace (what
        // it ordered today counted at its worth).
        let held = world.get::<Stock>(towns[t]).map_or_else(Vec::new, |s| s.0.clone());
        let needs: Vec<(crate::content::GoodId, f64)> = kit.iter()
            .map(|&(g, each)| (g, (n as f64 * each).ceil() - held.get(g).copied().unwrap_or(0.0).floor()))
            .filter(|x| x.1 > 0.0).collect();
        let worth: f64 = needs.iter().map(|&(g, need)| need * content.goods[g].value * premium).sum();
        let battles = world.resource::<Battles>();
        let behind = battles.backlog(faction) + ordered.get(&faction).copied().unwrap_or(0.0);
        if behind > 0.0 && behind + worth > battles.pace(d, faction) * d.chest_backlog_days {
            continue;
        }
        *ordered.entry(faction).or_default() += worth;
        let routes = route_facts(world);
        let env = Env::gather(world, routes);
        for (g, need) in needs {
            super::crown::requisition(&env, &mut sov, t, &[g], false, need, premium, now);
        }
    }
    *world.resource_mut::<Sovereign>() = sov;
    // The master smiths its roads' towns lack, tendered on its board.
    let mut tenders = std::mem::take(&mut *world.resource_mut::<Tenders>());
    for &(t, _, _, marched) in &buyers {
        let premium = if marched { d.war_premium } else { d.mobilising_premium };
        let realm = super::crown::realm(world, t);
        let book = world.resource::<Book>();
        for wanted in TRAINED.iter().filter(|w| !w.meat) {
            let Some(trade) = content.trade(wanted.trade) else { continue };
            let masters = realm.iter().filter_map(|&x| world.get::<Roster>(towns[x]))
                .flat_map(|r| r.hands.iter()).filter(|h| h.trade == trade && h.level >= wanted.level).count();
            let training = book.open.iter().map(|&id| &book.contracts[id])
                .filter(|c| c.buyer == t && matches!(c.state, crate::contracts::State::Drafted | crate::contracts::State::Active))
                .filter(|c| c.training.is_some_and(|x| x.trade == trade)).count();
            let short = wanted.count as i64 - (masters + training) as i64 - tenders.open_for(t, trade) as i64;
            if short > 0 {
                let bonus = d.training_bonus * wanted.level as f64 / 5.0 * premium;
                let work = crate::training::Training::new(trade, wanted.level, bonus, OFFTAKE);
                tenders.post(t, t, work, short as u32, now + TRAINING_DAYS * day, now, 0);
            }
        }
    }
    // What each has let of them since its faction's war chest owes it: each contract and tender once, its worth; paid
    // at its capital's pace, the nearest the fighting first.
    let mut battles = std::mem::take(&mut *world.resource_mut::<Battles>());
    let owed: Vec<f64> = {
        let book = world.resource::<Book>();
        buyers.iter().map(|&(t, ..)| {
            let mut sum = 0.0;
            for c in book.open.iter().map(|&id| &book.contracts[id]).filter(|c| c.buyer == t) {
                if matches!(c.state, crate::contracts::State::Drafted | crate::contracts::State::Active)
                        && battles.funded_contracts.insert(c.id) {
                    sum += c.worth();
                }
            }
            for x in tenders.on(t).iter().filter(|x| x.buyer == t) {
                if battles.funded_tenders.insert(x.id) {
                    sum += x.work.bonus.to_f64() * x.open as f64;
                }
            }
            sum
        }).collect()
    };
    *world.resource_mut::<Tenders>() = tenders;
    for (&(t, _, faction, _), &owes) in buyers.iter().zip(&owed) {
        if owes > 0.0 {
            match battles.owed.iter_mut().find(|o| o.0 == t && o.1 == faction) {
                Some(o) => o.2 += crate::fixed::Fixed::ceil(owes),
                None => battles.owed.push((t, faction, crate::fixed::Fixed::ceil(owes))),
            }
        }
    }
    let rank = |t: TownId| buyers.iter().position(|b| b.0 == t).unwrap_or(usize::MAX);
    battles.owed.sort_by_key(|o| (rank(o.0), o.0));
    // (A ruin is owed nothing.)
    let ruins = battles.ruins.clone();
    battles.owed.retain(|o| !ruins.contains(&o.0));
    let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
    let mut cash = std::mem::take(&mut *world.resource_mut::<CashBook>());
    for i in 0..battles.owed.len() {
        let (t, faction, owes) = battles.owed[i];
        if world.get::<Money>(towns[t]).is_none() {
            continue;
        }
        let grant = battles.spend_public(d, faction, owes, today);
        battles.owed[i].2 -= grant;
        let Some(mut money) = world.get_mut::<Money>(towns[t]) else { continue };
        if grant > crate::fixed::Fixed::ZERO {
            post(&mut ledger, &mut cash, Side::outside(), Side::treasury(t, &mut money.0), grant,
                Reason::Other(format!("{}'s war chest", faction.capital())), now, day);
        }
    }
    battles.owed.retain(|o| o.2 > crate::fixed::Fixed::ZERO);
    *world.resource_mut::<Battles>() = battles;
    *world.resource_mut::<Ledger>() = ledger;
    *world.resource_mut::<CashBook>() = cash;
}

/// Days a side regroups between one battle of the war and its next.
const REGROUP_DAYS: u64 = 3;

/// A town battle's attackers gather at their camp before they march on it (John: "muster, then march"): each town's
/// levy waits there till all have come, or this many days after they set off. (Not a dial: a dial is saved with the
/// world, and a new one is a new shape of save.)
const MUSTER_DAYS: u64 = 10;

/// A town's faction, if it's a faction's and not ruins.
fn faction_of(world: &World, t: TownId) -> Option<Faction> {
    if world.resource::<Battles>().ruins.contains(&t) {
        return None;
    }
    world.resource::<crate::taxes::Taxes>().peoples.get(&t).and_then(|p| Faction::of(p))
}

/// The region's towns of a faction (not ruins), with their hexes, in their order.
fn faction_towns(world: &World, f: Faction) -> Vec<(TownId, Hex)> {
    let towns = world.resource::<Towns>().0.clone();
    (0..towns.len()).filter(|&t| faction_of(world, t) == Some(f))
        .filter_map(|t| Some((t, world.get::<Town>(towns[t])?.hex?))).collect()
}

/// The nearest of `of` to the hex, the first on a tie.
fn nearest(of: &[(TownId, Hex)], to: Hex) -> Option<(TownId, Hex)> {
    of.iter().copied().min_by_key(|&(t, h)| (h.distance(to), t))
}

/// A battle of the war for a town: `attacker`'s towns near it against the town's own and theirs, the attackers' towns
/// within `battle_reach` of it levied `battle_levy` of their residents, every town of the defenders' `defence_levy`
/// (John: "using its local numerical advantage"), and the Horde's every town `pin_levy` when it attacks (the
/// Alliance's too when `whole`: Theramore gone, its towns march on alone); its attackers camped at `camp`, its
/// defenders in the town itself; they march at `marches`.
#[allow(clippy::too_many_arguments)]
fn town_battle(world: &World, content: &Content, target: TownId, hex: Hex, attacker: Faction, camp: TownId,
        marches: u64, whole: bool) -> Battle {
    let factions = [Faction::Horde, Faction::Alliance];
    let side = factions.iter().position(|&f| f == attacker).expect("a side");
    let mut camps = [target, target];
    camps[side] = camp;
    let towns = world.resource::<Towns>().0.clone();
    let mut levies: [Vec<(TownId, usize)>; 2] = Default::default();
    for (s, &f) in factions.iter().enumerate() {
        let (defends, pins) = (s != side, s == side && (f == Faction::Horde || whole));
        for (t, h) in faction_towns(world, f) {
            if defends || pins || h.distance(hex) as f64 <= content.dials.battle_reach {
                if let Some(roster) = world.get::<Roster>(towns[t]) {
                    let d = &content.dials;
                    let share = if defends { d.defence_levy } else if pins { d.pin_levy } else { d.battle_levy };
                    levies[s].push((t, levy_at(roster, share)));
                }
            }
        }
    }
    let now = world.resource::<Clock>().now;
    Battle { hex, factions, armies: Default::default(), camps, levies, marching: Vec::new(), holder: None,
        held_since: None, won_by: None, began: now, days: 0, fights: 0, reckoned: None, marches, marched: false,
        town: Some(target), attacker: Some(side), settled: false, field: false, raid: false, ground_side: None,
        depots: Default::default() }
}

/// Theramore's marines, Journeymen at arms kitted in Fine iron (John: "a larger reinforcement"), or Orgrimmar's
/// mercenaries (in what kit they find): `n` soldiers of its capital, at `camp`.
fn capital_soldiers(content: &Content, n: u32, camp: TownId, kitted: bool, now: u64) -> Vec<Soldier> {
    let piece = |name: &str| crate::army::good_named(content, name).filter(|_| kitted)
        .map(|good| crate::army::Worn { good, enchant: 0.0, unbreaking: 0, worn: 0.0,
            grade: Some(crate::persons::Grade::Fine) });
    (0..n).map(|_| Soldier { home: camp, enlisted: now, weapon: piece("iron_sword"),
        armour: [piece("iron_helmet"), piece("iron_chestplate"), None, None], shield: piece("shield"), from_capital: true,
        ..Default::default() }).collect()
}

/// `short` marines Theramore sends to make its expedition up, each costing it `marine_cost`, as many as its chest can
/// pay. Returns how many it sent.
fn send_marines(world: &mut World, d: &crate::dials::Dials, short: u32, today: u64) -> u32 {
    if short == 0 || d.marine_cost <= 0.0 {
        return short;
    }
    let mut battles = world.resource_mut::<Battles>();
    let sent = short.min((battles.chest_left(d, Faction::Alliance) / d.marine_cost).floor().max(0.0) as u32);
    battles.draw(d, Faction::Alliance, crate::fixed::Fixed::ceil(sent as f64 * d.marine_cost), today);
    sent
}

/// A town razed (`docs/war-plan.md` slice 4; John: "fight over a town's hex and raze it, which would create refugees
/// and frees up a hex for new settlement"): its people at home refugees to their faction's nearest town within
/// `refugee_reach` (none in reach: they leave the map), its households' purse with them; its treasury and goods looted
/// out of the world; its land no one's; no one's people any more, its hex the razers' to settle (`may_settle`).
/// Returns (refugees taken in, those with nowhere).
fn raze(world: &mut World, content: &Content, t: TownId) -> (u32, u32) {
    let (now, day) = (world.resource::<Clock>().now, content.day_ticks.max(1));
    let towns = world.resource::<Towns>().0.clone();
    let e = towns[t];
    let faction = faction_of(world, t);
    let hex = world.get::<Town>(e).and_then(|x| x.hex);
    let refuge = faction.zip(hex).and_then(|(f, hex)| {
        let friends: Vec<(TownId, Hex)> = faction_towns(world, f).into_iter().filter(|&(x, _)| x != t)
            .filter(|&(_, h)| h.distance(hex) as f64 <= content.dials.refugee_reach).collect();
        nearest(&friends, hex).map(|(x, _)| x)
    });
    world.resource_mut::<Battles>().ruins.insert(t);
    world.resource_mut::<crate::taxes::Taxes>().peoples.remove(&t);
    let moved = refugees(world, content, t, refuge);
    // Its households' purse goes with them; its treasury and goods are looted.
    let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
    let mut cash = std::mem::take(&mut *world.resource_mut::<CashBook>());
    let purse = world.get::<People>(e).map_or(crate::fixed::Fixed::ZERO, |p| p.purse);
    if let Some(r) = refuge {
        let mut dest = world.get::<People>(towns[r]).map(|p| p.purse).unwrap_or_default();
        let mut from = purse;
        post(&mut ledger, &mut cash, Side::households(t, &mut from), Side::households(r, &mut dest), purse,
            Reason::Other("refugees".into()), now, day);
        if let Some(mut p) = world.get_mut::<People>(towns[r]) {
            p.purse = dest;
        }
    } else {
        let mut from = purse;
        post(&mut ledger, &mut cash, Side::households(t, &mut from), Side::outside(), purse, Reason::Other("razed".into()), now, day);
    }
    if let Some(mut p) = world.get_mut::<People>(e) {
        p.purse = crate::fixed::Fixed::ZERO;
        p.beds = 0.0;
        p.land = 0.0;
    }
    if let Some(mut money) = world.get_mut::<Money>(e) {
        let all = money.0.at_least_zero();
        post(&mut ledger, &mut cash, Side::treasury(t, &mut money.0), Side::outside(), all, Reason::Other("razed".into()), now, day);
    }
    if let Some(mut stock) = world.get_mut::<Stock>(e) {
        for (g, n) in stock.0.iter_mut().enumerate() {
            if *n > 0.0 {
                ledger.used[g] += *n;
                *n = 0.0;
            }
        }
    }
    *world.resource_mut::<Ledger>() = ledger;
    *world.resource_mut::<CashBook>() = cash;
    // Its land no one's.
    let mut territory = world.resource_mut::<crate::territory::Territory>();
    let held: Vec<Hex> = territory.holding(t).map(|h| h.hexes.clone()).unwrap_or_default();
    for &h in &held {
        territory.cede(h);
    }
    // (Its culture with it: a ruin claims nothing again, `rules::territory::review`.)
    territory.holdings.remove(&t);
    drop(territory);
    world.resource_mut::<Battles>().ruined_land.insert(t, held);
    if let Some(stores) = world.resource_mut::<Battles>().stores.remove(&t) {
        let mut ledger = world.resource_mut::<Ledger>();
        for (g, n) in stores.iter().enumerate() {
            ledger.used[g] += n;
        }
    }
    moved
}

/// The ruins' people come home from the war to nowhere: refugees too.
fn ruins_home(world: &mut World, content: &Content, c: &mut Campaign) {
    let ruins: Vec<TownId> = world.resource::<Battles>().ruins.iter().copied().collect();
    let towns = world.resource::<Towns>().0.clone();
    for t in ruins {
        if world.get::<Roster>(towns[t]).is_some_and(|r| r.hands.iter().any(|h| !h.away)) {
            let (taken, gone) = refugees(world, content, t, None);
            c.refugees += taken;
            c.lost += gone;
        }
    }
}

/// A ruin's people at home go to `refuge` (none: they leave the map). Returns (taken in, gone).
fn refugees(world: &mut World, content: &Content, t: TownId, refuge: Option<TownId>) -> (u32, u32) {
    let towns = world.resource::<Towns>().0.clone();
    let Some(mut roster) = world.get::<Roster>(towns[t]).cloned() else { return (0, 0) };
    let (going, staying): (Vec<_>, Vec<_>) = roster.hands.into_iter().partition(|h| !h.away);
    roster.hands = staying;
    roster.merchant = None;
    let n = going.len() as u32;
    world.entity_mut(towns[t]).insert(roster);
    world.entity_mut(towns[t]).insert(crate::lockers::Lockers::default());
    if let Some(mut p) = world.get_mut::<People>(towns[t]) {
        p.villagers = 0.0;
    }
    let Some(r) = refuge else { return (0, n) };
    let Some(mut home) = world.get::<Roster>(towns[r]).cloned() else { return (0, n) };
    for hand in going {
        home.add(content, hand);
    }
    let residents = home.residents() as f64;
    world.entity_mut(towns[r]).insert(home);
    if let Some(mut p) = world.get_mut::<People>(towns[r]) {
        p.villagers = residents;
    }
    (n, 0)
}

/// The war for the region, once a day after the battles' day (`docs/war-plan.md`): declared on `war_day`, the base
/// the Alliance's port nearest the Horde, its first battle the Horde town nearest it (the run-up `battle_mobilise_days`);
/// Theramore's marines land when it's over. Each day the capitals pay their soldiers in the field (Theramore dry, it
/// withdraws: its marines sail home), its expedition made up again for each battle; Orgrimmar sends its mercenaries
/// at a last stand; a town its attackers held is
/// razed; a side with no towns left has lost (Theramore withdrawn, the war goes on: John, "the horde should also march
/// on and attempt to raze Pyrewood/remaining cities if the Alliance pulls out"); and with no battle on, the next: the
/// Horde strikes at the Alliance's camp (its base) when it beat off an attack, when its towns outnumber the Alliance
/// `pin_odds` to one and its own last strike wasn't beaten off, or whenever Theramore has gone; else the Alliance
/// strikes the Horde town nearest its base.
pub fn campaign(world: &mut World) {
    let content = world.resource::<Content>().clone();
    let d = &content.dials;
    let (now, day) = (world.resource::<Clock>().now, content.day_ticks.max(1));
    let today = now / day;
    if world.resource::<Battles>().campaign.is_none() {
        if d.war_day <= 0.0 || (today as f64) < d.war_day {
            return;
        }
        let (horde, alliance) = (faction_towns(world, Faction::Horde), faction_towns(world, Faction::Alliance));
        if horde.is_empty() || alliance.is_empty() {
            return;
        }
        let towns = world.resource::<Towns>().0.clone();
        let reach = |h: Hex| horde.iter().map(|x| x.1.distance(h)).min().unwrap_or(i32::MAX);
        let ports: Vec<(TownId, Hex)> = alliance.iter().copied()
            .filter(|&(t, _)| world.get::<crate::works::Works>(towns[t]).is_some_and(|w| w.port.is_some())).collect();
        let pool = if ports.is_empty() { &alliance } else { &ports };
        let Some(&(base, base_hex)) = pool.iter().min_by_key(|&&(t, h)| (reach(h), t)) else { return };
        let Some((target, hex)) = nearest(&horde, base_hex) else { return };
        let lands = now + (d.battle_mobilise_days.max(0.0) * day as f64) as u64;
        let battle = town_battle(world, &content, target, hex, Faction::Alliance, base, lands, false);
        let mut battles = world.resource_mut::<Battles>();
        battles.seeded = true;
        battles.battles.push(battle);
        battles.campaign = Some(Campaign { declared: now, lands, base: Some(base), ..Default::default() });
        return;
    }
    let mut c = world.resource::<Battles>().campaign.clone().expect("declared");
    if c.reckoned == Some(today) {
        return;
    }
    if c.won_by.is_some() {
        // (Won, its soldiers still come home: to their ruins, refugees.)
        c.reckoned = Some(today);
        ruins_home(world, &content, &mut c);
        world.resource_mut::<Battles>().campaign = Some(c);
        return;
    }
    c.reckoned = Some(today);
    let current = world.resource::<Battles>().battles.iter().rposition(|b| b.town.is_some() && !b.raid && b.won_by.is_none());
    // Landed: Theramore's marines at its camp.
    if !c.landed && now >= c.lands {
        c.landed = true;
        c.sailed = Some(now);
        c.marines = d.expedition_size.max(0.0) as u32;
        if let Some(i) = current {
            let (camp, goal) = { let b = &world.resource::<Battles>().battles[i]; (b.camps[1], b.hex) };
            let marines = capital_soldiers(&content, c.marines, camp, true, now);
            let at = town_hex_of(world, camp).unwrap_or(goal);
            land_marines(&mut world.resource_mut::<Battles>(), d, camp, marines, at, Some(goal));
            ship_rations(world, &content, camp, today);
        }
    }
    // The capitals' heroes, once, when Theramore lands (John: "a heroic unit leading things"; "Both sides get one"):
    // `hero_alliance` captains of renown, `hero_horde` warchiefs, each for `hero_cost` of its chest, leading its side's
    // largest forces (`fight`).
    if c.landed && c.withdrew.is_none() {
        for (side, f, n) in [(0, Faction::Horde, d.hero_horde), (1, Faction::Alliance, d.hero_alliance)] {
            let n = n.max(0.0).floor();
            if n < 1.0 || c.heroes[side].is_some() || world.resource::<Battles>().chest_left(d, f) < n * d.hero_cost {
                continue;
            }
            world.resource_mut::<Battles>().draw(d, f, crate::fixed::Fixed::ceil(n * d.hero_cost.max(0.0)), today);
            c.heroes[side] = Some(today);
            c.heroes_alive[side] = n as u32;
        }
    }
    // The capitals' soldiers in the field: counted, and paid of their chests; Theramore dry, it withdraws.
    if current.is_some() {
        let of = |side: usize| capital_count(world.resource::<Battles>(), side);
        let (marines, mercenaries) = (of(1), of(0));
        if c.landed {
            c.marines = marines;
            c.mercenaries = mercenaries;
        }
        let mut battles = world.resource_mut::<Battles>();
        for (side, f, n) in [(1, Faction::Alliance, marines), (0, Faction::Horde, mercenaries)] {
            if n == 0 {
                continue;
            }
            let want = crate::fixed::Fixed::ceil(n as f64 * d.capital_soldier_pay);
            if battles.draw(d, f, want, today) < want {
                dismiss_capital(&mut battles, side);
                if f == Faction::Alliance {
                    c.withdrew = Some(now);
                    c.marines = 0;
                } else {
                    c.mercenaries = 0;
                }
            }
        }
    }
    // The capitals' squads with nowhere to go, to the battle going on.
    if let Some(i) = current {
        let goal = world.resource::<Battles>().battles[i].hex;
        let mut battles = world.resource_mut::<Battles>();
        for q in battles.squads.iter_mut().filter(|q| q.capital && q.battle.is_none() && q.goal.is_none() && !q.waits) {
            q.goal = Some(goal);
        }
    }
    // Theramore makes its expedition up in the field too, when its ships come: every `convoy_days` at most (John: "pin
    // the Alliance down and attrit them": its losses cost its chest; "limit reinforcement frequency").
    let convoy = (d.convoy_days.max(1.0) * day as f64) as u64;
    let ships_due = |c: &Campaign| c.sailed.is_none_or(|s| now >= s + convoy);
    if let (Some(i), true, None) = (current, c.landed, c.withdrew) {
        let (marched, camp, goal) = { let b = &world.resource::<Battles>().battles[i]; (b.marched, b.camps[1], b.hex) };
        let there = capital_count(world.resource::<Battles>(), 1);
        let short = (d.expedition_size.max(0.0) as u32).saturating_sub(there);
        if marched && short > 0 && ships_due(&c) {
            let sent = send_marines(world, d, short, today);
            c.sailed = Some(now);
            let at = town_hex_of(world, camp).unwrap_or(goal);
            let marines = capital_soldiers(&content, sent, camp, true, now);
            land_marines(&mut world.resource_mut::<Battles>(), d, camp, marines, at, Some(goal));
            ship_rations(world, &content, camp, today);
            c.marines = there + sent;
        }
    }
    // Orgrimmar's mercenaries at a last stand, or its last `horde_losing_streak` battles lost, a band every
    // `convoy_days`.
    let horde_n = faction_towns(world, Faction::Horde).len();
    let streak = world.resource::<Battles>().battles.iter().rev().filter(|b| b.town.is_some() && !b.raid && b.won_by.is_some())
        .take_while(|b| b.won_by != Some(Faction::Horde)).count();
    let desperate = horde_n as f64 <= d.horde_last_stand
        || d.horde_losing_streak > 0.0 && streak as f64 >= d.horde_losing_streak;
    if let Some(i) = current {
        let gap = convoy;
        let band = d.mercenary_band.max(0.0) as u32;
        let cost = band as f64 * d.capital_soldier_pay * d.convoy_days.max(1.0);
        let left = world.resource::<Battles>().chest_left(d, Faction::Horde);
        if desperate && band > 0 && left >= cost && c.hired.is_none_or(|h| now >= h + gap) {
            let (camp, goal) = { let b = &world.resource::<Battles>().battles[i]; (b.camps[0], b.hex) };
            let mercs = capital_soldiers(&content, band, camp, false, now);
            let at = town_hex_of(world, camp).unwrap_or(goal);
            muster_capital(&mut world.resource_mut::<Battles>(), d, 0, camp, mercs, at, Some(goal));
            c.hired = Some(now);
            c.mercenaries += band;
        }
    }
    // What came of the battles won: a town its attackers held is razed.
    let won: Vec<usize> = world.resource::<Battles>().battles.iter().enumerate()
        .filter(|(_, b)| b.won_by.is_some() && !b.settled && b.town.is_some()).map(|(i, _)| i).collect();
    for i in won {
        let b = world.resource::<Battles>().battles[i].clone();
        world.resource_mut::<Battles>().battles[i].settled = true;
        let (Some(t), Some(a)) = (b.town, b.attacker) else { continue };
        if b.won_by == Some(b.factions[a]) && !world.resource::<Battles>().ruins.contains(&t) {
            let (taken, gone) = raze(world, &content, t);
            c.razed.push((today, t, b.factions[a]));
            c.refugees += taken;
            c.lost += gone;
        }
    }
    ruins_home(world, &content, &mut c);
    let towns = world.resource::<Towns>().0.clone();
    // Won: a side with no towns left.
    let (horde, alliance) = (faction_towns(world, Faction::Horde), faction_towns(world, Faction::Alliance));
    c.won_by = if horde.is_empty() {
        Some((today, Faction::Alliance))
    } else if alliance.is_empty() {
        Some((today, Faction::Horde))
    } else {
        None
    };
    // With no battle on, the next.
    let on = world.resource::<Battles>().battles.iter().any(|b| b.town.is_some() && !b.raid && b.won_by.is_none());
    if c.won_by.is_none() && !on {
        let last = world.resource::<Battles>().battles.iter().rev().find(|b| b.town.is_some() && !b.raid).cloned();
        let beaten_off = last.as_ref().and_then(|b| Some((b.factions[b.attacker?], b.won_by?, b.hex)))
            .filter(|(att, won, _)| att != won);
        let base = c.base.filter(|b| !world.resource::<Battles>().ruins.contains(b))
            .and_then(|b| Some((b, world.get::<Town>(towns[b])?.hex?)));
        // The Horde pins the Alliance at its camp (John: "pin the Alliance down and attrit them"): it strikes back at
        // an attack beaten off, and strikes whenever its towns' levy outnumbers the Alliance `pin_odds` to one, but
        // after its own strike beaten off, the Alliance strikes.
        let pin: usize = horde.iter().filter_map(|&(t, _)| world.get::<Roster>(towns[t]))
            .map(|r| levy_at(r, d.pin_levy)).sum();
        let theirs: usize = c.marines.max(if c.withdrew.is_none() { d.expedition_size.max(0.0) as u32 } else { 0 }) as usize
            + alliance.iter().filter_map(|&(t, _)| world.get::<Roster>(towns[t])).map(|r| levy_of(&content, r)).sum::<usize>();
        let outnumbers = pin as f64 >= d.pin_odds * theirs.max(1) as f64;
        let horde_beaten = beaten_off.is_some_and(|(att, ..)| att == Faction::Horde);
        let alliance_beaten = beaten_off.is_some_and(|(att, ..)| att == Faction::Alliance);
        // Theramore gone, the region's own towns fight it out (John: the Alliance "marches on alone"): a side's attack
        // beaten off, the other strikes; else the side whose towns' levy is the larger (the Horde's, even).
        let levy = |side: &[(TownId, Hex)]| -> usize { side.iter().filter_map(|&(t, _)| world.get::<Roster>(towns[t]))
            .map(|r| levy_of(&content, r)).sum() };
        let horde_strikes = if c.withdrew.is_some() {
            alliance_beaten || !horde_beaten && levy(&horde) >= levy(&alliance)
        } else {
            alliance_beaten || !horde_beaten && outnumbers
        };
        let (attacker, target) = if horde_strikes {
            // At the Alliance's camp, its base while it stands; else its town nearest.
            let from = beaten_off.map(|b| b.2).or(last.as_ref().map(|b| b.hex)).unwrap_or(horde[0].1);
            (Faction::Horde, base.or_else(|| nearest(&alliance, from)))
        } else {
            let from = base.map(|b| b.1).or(last.as_ref().map(|b| b.hex)).unwrap_or(alliance[0].1);
            (Faction::Alliance, nearest(&horde, from))
        };
        let own = if attacker == Faction::Alliance { &alliance } else { &horde };
        if let (Some((target, hex)), true) = (target, !own.is_empty()) {
            // (The Alliance's camp Theramore's base while it's in the war; then, as the Horde's, its town nearest.)
            let camp = if attacker == Faction::Alliance {
                c.base.filter(|b| c.withdrew.is_none() && !world.resource::<Battles>().ruins.contains(b))
                    .unwrap_or(nearest(own, hex).expect("towns").0)
            } else {
                nearest(own, hex).expect("towns").0
            };
            let battle = town_battle(world, &content, target, hex, attacker, camp, now + REGROUP_DAYS * day,
                c.withdrew.is_some());
            // The capitals' soldiers go with their side: Theramore's made up to its expedition again when its ships
            // come, while it can pay them.
            let goal = battle.hex;
            if c.landed && c.withdrew.is_none() {
                let short = (d.expedition_size.max(0.0) as u32).saturating_sub(c.marines);
                if short > 0 && ships_due(&c) {
                    let sent = send_marines(world, d, short, today);
                    c.marines += sent;
                    c.sailed = Some(now);
                    let camp = battle.camps[1];
                    let at = town_hex_of(world, camp).unwrap_or(goal);
                    let marines = capital_soldiers(&content, sent, camp, true, now);
                    land_marines(&mut world.resource_mut::<Battles>(), d, camp, marines, at, Some(goal));
                    ship_rations(world, &content, camp, today);
                }
            }
            let mut battles = world.resource_mut::<Battles>();
            send_capital(&mut battles, 1, goal);
            send_capital(&mut battles, 0, goal);
            battles.battles.push(battle);
        }
    }
    world.resource_mut::<Battles>().campaign = Some(c);
}
