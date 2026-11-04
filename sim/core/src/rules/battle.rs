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
        fight.in_set(Step::Pay).after(super::army::campaign).before(super::territory::upkeep),
        procure.in_set(Step::Review).after(super::crown::review).after(super::market::review).before(super::training::lapse),
    ));
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
                    let grant = battles.draw(d, faction, cost, today);
                    if grant < cost {
                        continue; // its capital's chest dry: no more barracks of its paying
                    }
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
                    marches: now + (d.battle_mobilise_days.max(0.0) * day as f64) as u64, marched: false };
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

/// The kit a town procures for its levy of `n` (John: "Towns should procure on behalf of the crown"): a sword, a
/// helmet and a chestplate each, and shields for half (the crown's stockpile's iron).
const LEVY_KIT: [(&str, f64); 4] = [("iron_sword", 1.0), ("iron_helmet", 1.0), ("iron_chestplate", 1.0), ("shield", 0.5)];

/// A battle's towns arm, once a day, on the crown's behalf as the crown does at its capital (`rules/crown.rs`; John:
/// "look at how the early war scenario involved plenty of incentives to get the production system in place early
/// using public spending"): each levied town requisitions its levy's kit beyond what it holds and has coming, at the
/// mobilising premium till its levies march and the war's after; its bids taken are awarded as the crown's are, a
/// third up front; and it tenders to train the master smiths its roads' towns lack. Its faction's war chest keeps its
/// treasury up to what it owes on them (from outside the society, as the crown's mint): the public spending that pays
/// the realm's towns to take up smithing.
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
    // Each levied town of a battle not won, its levy, its faction, and whether its levies have marched.
    let mut buyers: Vec<(TownId, usize, Faction, bool)> = Vec::new();
    for b in battles.battles.iter().filter(|b| b.won_by.is_none()) {
        for side in 0..2 {
            for &(t, n) in &b.levies[side] {
                if !buyers.iter().any(|x| x.0 == t) {
                    buyers.push((t, n, b.factions[side], now >= b.marches));
                }
            }
        }
    }
    *world.resource_mut::<Battles>() = battles;
    if !fresh || buyers.is_empty() {
        return;
    }
    let towns = world.resource::<Towns>().0.clone();
    let kit: Vec<(crate::content::GoodId, f64)> = LEVY_KIT.iter()
        .filter_map(|&(name, each)| crate::army::good_named(&content, name).map(|g| (g, each))).collect();
    let mut sov = std::mem::take(&mut *world.resource_mut::<Sovereign>());
    sov.lapse(now);
    for &(t, n, _, marched) in &buyers {
        let premium = if marched { d.war_premium } else { d.mobilising_premium };
        // Bids in: the lots awarded.
        let routes = route_facts(world);
        let awards = award(&Env::gather(world, routes), &mut sov.requisitions, t, sov.theatre);
        sov.awarded += awards.len() as u32;
        apply_awards(world, &awards);
        // What its levy still lacks.
        let held = world.get::<Stock>(towns[t]).map_or_else(Vec::new, |s| s.0.clone());
        let routes = route_facts(world);
        let env = Env::gather(world, routes);
        for &(g, each) in &kit {
            let need = (n as f64 * each).ceil() - held.get(g).copied().unwrap_or(0.0).floor();
            if need > 0.0 {
                super::crown::requisition(&env, &mut sov, t, &[g], false, need, premium, now);
            }
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
    // What each owes on them, its faction's war chest makes good.
    let owed: Vec<(TownId, f64)> = {
        let book = world.resource::<Book>();
        buyers.iter().map(|&(t, ..)| {
            let contracts: f64 = book.open.iter().map(|&id| &book.contracts[id])
                .filter(|c| c.buyer == t && matches!(c.state, crate::contracts::State::Drafted | crate::contracts::State::Active))
                .map(|c| (c.worth() - c.paid.to_f64()).max(0.0)).sum();
            let tendered: f64 = tenders.on(t).iter().filter(|x| x.buyer == t).map(|x| x.work.bonus.to_f64() * x.open as f64).sum();
            (t, contracts + tendered)
        }).collect()
    };
    *world.resource_mut::<Tenders>() = tenders;
    let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
    let mut cash = std::mem::take(&mut *world.resource_mut::<CashBook>());
    let mut battles = std::mem::take(&mut *world.resource_mut::<Battles>());
    for (&(t, _, faction, _), &(_, owes)) in buyers.iter().zip(&owed) {
        let Some(mut money) = world.get_mut::<Money>(towns[t]) else { continue };
        let short = crate::fixed::Fixed::ceil(owes - money.0.to_f64());
        let grant = battles.draw(d, faction, short, today);
        if grant > crate::fixed::Fixed::ZERO {
            post(&mut ledger, &mut cash, Side::outside(), Side::treasury(t, &mut money.0), grant,
                Reason::Other(format!("{}'s war chest", faction.capital())), now, day);
        }
    }
    *world.resource_mut::<Battles>() = battles;
    *world.resource_mut::<Ledger>() = ledger;
    *world.resource_mut::<CashBook>() = cash;
}
