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
        fight.in_set(Step::Pay).after(super::army::campaign).before(super::territory::upkeep),
        procure.in_set(Step::Review).after(super::crown::review).after(super::market::review).before(super::training::lapse),
        campaign.in_set(Step::Pay).after(fight).before(super::territory::upkeep),
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
                    attacker: None, settled: false };
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
                    // (A town's defenders, `defence_lift` the better.)
                    let mut lift = [1.0; 2];
                    if let (Some(t), Some(a)) = (b.town, b.attacker) {
                        lift[1 - a] = defence_lift(d, entity(t).and_then(|e| works.get(e).ok()));
                    }
                    let (x_won, rx, ry) = crate::war::clash_lifted(&content, x, y, &mut persons, &mut charters, b.fights,
                        now, lift);
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
        // Held long enough, it's the holder's: the fighting ends, and both armies walk home. A town's hex its
        // attackers hold `raze_hold_days` (its razing is the campaign's: `campaign`).
        let hold = match (b.town, b.attacker) {
            (Some(_), Some(a)) if b.holder == Some(a) => d.raze_hold_days,
            _ => d.battle_hold_days,
        };
        if b.holder.is_some() && b.held_days(now, day) as f64 >= hold {
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
/// (John: "using its local numerical advantage"), and the Horde's every town `pin_levy` when it attacks; its attackers
/// camped at `camp`, its defenders in the town itself; they march at `marches`.
fn town_battle(world: &World, content: &Content, target: TownId, hex: Hex, attacker: Faction, camp: TownId,
        marches: u64) -> Battle {
    let factions = [Faction::Horde, Faction::Alliance];
    let side = factions.iter().position(|&f| f == attacker).expect("a side");
    let mut camps = [target, target];
    camps[side] = camp;
    let towns = world.resource::<Towns>().0.clone();
    let mut levies: [Vec<(TownId, usize)>; 2] = Default::default();
    for (s, &f) in factions.iter().enumerate() {
        let (defends, pins) = (s != side, s == side && f == Faction::Horde);
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
        town: Some(target), attacker: Some(side), settled: false }
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
    for h in held {
        territory.cede(h);
    }
    moved
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
/// razed; a side with no towns left has lost, as has the Alliance once Theramore withdrew; and with no battle on, the
/// next: the Horde strikes at the Alliance's camp (its base) when it beat off an attack, or when its towns outnumber
/// the Alliance `pin_odds` to one and its own last strike wasn't beaten off; else the Alliance strikes the Horde town
/// nearest its base.
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
        let battle = town_battle(world, &content, target, hex, Faction::Alliance, base, lands);
        let mut battles = world.resource_mut::<Battles>();
        battles.seeded = true;
        battles.battles.push(battle);
        battles.campaign = Some(Campaign { declared: now, lands, base: Some(base), ..Default::default() });
        return;
    }
    let mut c = world.resource::<Battles>().campaign.clone().expect("declared");
    if c.won_by.is_some() || c.reckoned == Some(today) {
        return;
    }
    c.reckoned = Some(today);
    let current = world.resource::<Battles>().battles.iter().rposition(|b| b.town.is_some() && b.won_by.is_none());
    // Landed: Theramore's marines at its camp.
    if !c.landed && now >= c.lands {
        c.landed = true;
        c.marines = d.expedition_size.max(0.0) as u32;
        if let Some(i) = current {
            let camp = world.resource::<Battles>().battles[i].camps[1];
            let marines = capital_soldiers(&content, c.marines, camp, true, now);
            world.resource_mut::<Battles>().battles[i].armies[1].soldiers.extend(marines);
        }
    }
    // The capitals' soldiers in the field: counted, and paid of their chests; Theramore dry, it withdraws.
    if let Some(i) = current {
        let b = world.resource::<Battles>().battles[i].clone();
        let of = |side: usize| b.armies[side].soldiers.iter().filter(|s| s.from_capital).count() as u32;
        if b.marched {
            c.marines = of(1);
            c.mercenaries = of(0);
        }
        let mut battles = world.resource_mut::<Battles>();
        for (side, f) in [(1, Faction::Alliance), (0, Faction::Horde)] {
            let n = of(side);
            if n == 0 {
                continue;
            }
            let want = crate::fixed::Fixed::ceil(n as f64 * d.capital_soldier_pay);
            if battles.draw(d, f, want, today) < want {
                battles.battles[i].armies[side].soldiers.retain(|s| !s.from_capital);
                if f == Faction::Alliance {
                    c.withdrew = Some(now);
                    c.marines = 0;
                } else {
                    c.mercenaries = 0;
                }
            }
        }
    }
    // Theramore makes its expedition up in the field too, as the battle's levies are, every `battle_reinforce_days`
    // (John: "pin the Alliance down and attrit them": its losses cost its chest).
    if let (Some(i), true, None) = (current, c.landed, c.withdrew) {
        let b = &world.resource::<Battles>().battles[i];
        let every = d.battle_reinforce_days.max(0.0) as u64;
        if b.marched && every > 0 && b.days > 0 && b.days % every == 0 {
            let there = b.armies[1].soldiers.iter().filter(|s| s.from_capital).count() as u32;
            let short = (d.expedition_size.max(0.0) as u32).saturating_sub(there);
            let sent = send_marines(world, d, short, today);
            let mut battles = world.resource_mut::<Battles>();
            let camp = battles.battles[i].camps[1];
            battles.battles[i].armies[1].soldiers.extend(capital_soldiers(&content, sent, camp, true, now));
            c.marines = there + sent;
        }
    }
    // Orgrimmar's mercenaries at a last stand, or its last `horde_losing_streak` battles lost, a band every
    // `battle_reinforce_days`.
    let horde_n = faction_towns(world, Faction::Horde).len();
    let streak = world.resource::<Battles>().battles.iter().rev().filter(|b| b.town.is_some() && b.won_by.is_some())
        .take_while(|b| b.won_by != Some(Faction::Horde)).count();
    let desperate = horde_n as f64 <= d.horde_last_stand
        || d.horde_losing_streak > 0.0 && streak as f64 >= d.horde_losing_streak;
    if let Some(i) = current {
        let gap = (d.battle_reinforce_days.max(1.0) as u64) * day;
        let band = d.mercenary_band.max(0.0) as u32;
        let cost = band as f64 * d.capital_soldier_pay * d.battle_reinforce_days.max(1.0);
        let left = world.resource::<Battles>().chest_left(d, Faction::Horde);
        if desperate && band > 0 && left >= cost && c.hired.is_none_or(|h| now >= h + gap) {
            let camp = world.resource::<Battles>().battles[i].camps[0];
            let mercs = capital_soldiers(&content, band, camp, false, now);
            world.resource_mut::<Battles>().battles[i].armies[0].soldiers.extend(mercs);
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
    // The ruins' people come home from the war to nowhere: refugees too.
    let ruins: Vec<TownId> = world.resource::<Battles>().ruins.iter().copied().collect();
    let towns = world.resource::<Towns>().0.clone();
    for t in ruins {
        if world.get::<Roster>(towns[t]).is_some_and(|r| r.hands.iter().any(|h| !h.away)) {
            let (taken, gone) = refugees(world, &content, t, None);
            c.refugees += taken;
            c.lost += gone;
        }
    }
    // Won: a side with no towns left; or Theramore withdrew.
    let (horde, alliance) = (faction_towns(world, Faction::Horde), faction_towns(world, Faction::Alliance));
    c.won_by = if horde.is_empty() {
        Some((today, Faction::Alliance))
    } else if alliance.is_empty() || c.withdrew.is_some() {
        Some((today, Faction::Horde))
    } else {
        None
    };
    // With no battle on, the next.
    let on = world.resource::<Battles>().battles.iter().any(|b| b.town.is_some() && b.won_by.is_none());
    if c.won_by.is_none() && !on {
        let last = world.resource::<Battles>().battles.iter().rev().find(|b| b.town.is_some()).cloned();
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
        let horde_strikes = beaten_off.is_some_and(|(att, ..)| att == Faction::Alliance) || !horde_beaten && outnumbers;
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
            let camp = if attacker == Faction::Alliance {
                c.base.filter(|b| !world.resource::<Battles>().ruins.contains(b)).unwrap_or(nearest(own, hex).expect("towns").0)
            } else {
                nearest(own, hex).expect("towns").0
            };
            let mut battle = town_battle(world, &content, target, hex, attacker, camp, now + REGROUP_DAYS * day);
            // The capitals' soldiers go with their side: Theramore's made up to its expedition again, while it can
            // pay them.
            if c.landed && c.withdrew.is_none() {
                let short = (d.expedition_size.max(0.0) as u32).saturating_sub(c.marines);
                c.marines += send_marines(world, d, short, today);
                battle.armies[1].soldiers.extend(capital_soldiers(&content, c.marines, battle.camps[1], true, now));
            }
            battle.armies[0].soldiers.extend(capital_soldiers(&content, c.mercenaries, battle.camps[0], false, now));
            world.resource_mut::<Battles>().battles.push(battle);
        }
    }
    world.resource_mut::<Battles>().campaign = Some(c);
}
