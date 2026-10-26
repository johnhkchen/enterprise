//! Public works' steps (`works.rs`):
//!
//! - **Pay:** the masons of a town's project are paid out of its treasury for the days since they last worked, and lay
//!   blocks in proportion (`Works.work`); a project finished marks the world (a bridge on the map, a store on the farm,
//!   a counting house on the budget) once.
//! - **Review:** the culture its works add is set on its holding before the census counts it; then each day the town
//!   tells the job draw what its halls would cost (`BuildServices.work`), and, with nothing under way, at its toll
//!   reviews starts a bridge that repays its wages, else a wall (`Works.consider`); else the next hall it wants and can
//!   pay twice over, each day by its own Mason, at a toll review by hired builders (`BuildServices.next`, `hire`); a
//!   master Farmer plants the square's flower beds.

use bevy_ecs::prelude::*;

use super::Step;
use crate::bills::Bills;
use crate::cash_book::{post, Accountant, CashBook, Reason, Side};
use crate::cargo::Paperwork;
use crate::chokepoints::{bridge_value, guard_cost_per_day, RouteEnds, Takings};
use crate::content::Content;
use crate::farms::Farm;
use crate::jobs::Signals;
use crate::map::{Map, CLIFF, IMPASSABLE};
use crate::mint::Mint;
use crate::roster::Roster;
use crate::state::*;
use crate::territory::Territory;
use crate::works::*;

/// The XP a town's Masons earn a day they lay its works: a level-1 Mason's trade's (clay for an emerald, 2).
pub const MASON_WORK_XP: u32 = 2;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        pay.in_set(Step::Pay).after(super::territory::upkeep).before(super::farms::pay),
        // Pinned as main ran it before atlas-and-books' `consider` moved Bevy's ambiguous order.
        culture.in_set(Step::Review).after(super::settlers::review).before(super::farms::review)
            .before(super::territory::review),
        review.in_set(Step::Review).after(super::territory::review).after(super::farms::review)
            .after(super::chokepoints::review).before(super::jobs::review),
    ));
}

/// The masons are paid and lay their blocks, towns in their order, once a day each (`Works.work`, at each touch in the
/// mod: after the guards' upkeep, before the farm's wages). The first look at a project only starts its clock. A day
/// they lay blocks, the town's Masons at home learn from it as from a trade ([`MASON_WORK_XP`]): with no clay to trade
/// for, a Mason would never reach the level a square needs (not as the mod, whose Masons learn only at their lockers).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn pay(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, territory: Res<Territory>, mut map: ResMut<Map>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut q: Query<(&Town, &mut Works, &mut Money, &mut Trades, &mut Budget, Option<&mut Farm>,
            Option<&mut crate::roster::Roster>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let mason = content.trade("mason");
    for &e in &towns.0 {
        let Ok((town, mut works, mut money, mut trades, mut budget, mut farm, roster)) = q.get_mut(e) else { continue };
        if let Some(i) = works.underway() {
            let hexes = territory.holding(town.id).map_or(0, |h| h.hexes.len());
            let p = &mut works.projects[i];
            if p.started.is_none_or(|t| now >= t + day) {
                let before = p.laid;
                let paid = work(p, &content.dials, hexes, now, day, money.0);
                if let (true, Some(mut roster), Some(mason)) = (p.laid > before, roster, mason) {
                    for h in roster.hands.iter_mut().filter(|h| h.trade == mason && !h.away && !h.is_baby()) {
                        h.reward_trade_xp(MASON_WORK_XP);
                    }
                }
                post(&mut ledger, &mut book, Side::treasury(town.id, &mut money.0), Side::outside(), paid,
                    Reason::Other("masons".into()), now, day);
            }
        }
        apply(&mut works, &mut map, farm.as_deref_mut());
        flags(&works, &mut trades, &mut budget);
    }
}

/// What a town's works add to its culture a day is set on its holding before the census counts it: the change since
/// last time, so a host's own figure (`Society::set_culture_extras`) stays.
pub fn culture(towns: Res<Towns>, mut territory: ResMut<Territory>, mut q: Query<(&Town, &mut Works)>) {
    for &e in &towns.0 {
        let Ok((town, mut works)) = q.get_mut(e) else { continue };
        let now = works.culture();
        if now == works.culture_applied {
            continue;
        }
        let h = territory.holdings.entry(town.id).or_default();
        h.extra_culture = (h.extra_culture + now - works.culture_applied).max(0.0);
        works.culture_applied = now;
    }
}

/// Is the hall one the town wants (`BuildServices.wanted`): a store where its crop piles up, a counting house where the
/// papers do, any other whenever its partners are there.
fn wanted(content: &Content, farm: Option<&Farm>, paperwork: &Paperwork, now: u64, kind: Kind) -> bool {
    if kind == Kind::CountingHouse {
        return paperwork.per_day(now, content.day_ticks) >= content.dials.counting_house_documents;
    }
    match kind.store() {
        Some(store) => farm.is_some_and(|f| f.stores_wanted.contains(&store)),
        None => true,
    }
}

/// What a town's money paid a day for goods brought in, over its last `imports_days` days: what it bought off
/// wagons, and under its contracts.
pub fn imports_per_day(content: &Content, book: &CashBook, town: usize, now: u64) -> f64 {
    let days = content.dials.imports_days.max(1.0);
    let today = now / content.day_ticks.max(1);
    let from = today.saturating_sub(days as u64);
    let paid: f64 = book.entries(crate::cash_book::Account::Treasury(town), from, today).iter()
        .filter(|(_, r, v)| matches!(r, Reason::Sale | Reason::Contract) && v.to_f64() < 0.0)
        .map(|(_, _, v)| -v.to_f64()).sum();
    paid / days
}

/// The halls a town could build now: not started, its partners at home, wanted (and a pier for stone docks).
#[allow(clippy::too_many_arguments)]
fn buildable(content: &Content, works: &Works, roster: Option<&Roster>, farm: Option<&Farm>, paperwork: &Paperwork,
        now: u64, kind: Kind) -> bool {
    if works.has_started(kind) {
        return false;
    }
    let skills = skills(content, roster);
    let merchant = crate::bargain::merchant_level(roster);
    kind.partnered(&skills, merchant) && (kind != Kind::StoneDocks || works.dock) && wanted(content, farm, paperwork, now, kind)
}

/// Each town reviews what to build, once a day: its own Mason's next hall; at its toll reviews a bridge, a wall, or
/// hired builders' hall.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn review(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, bills: Res<Bills>, map: Res<Map>,
        mint: Res<Mint>, takings: Res<Takings>, territory: Res<Territory>, mut signals: ResMut<Signals>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, routes: Query<&Route>, places: Query<&Town>,
        mut q: Query<(&Town, &mut Works, &mut Money, &mut Stock, &Trades, &mut Budget, &People, Option<&Farm>,
            Option<&Roster>, &Paperwork)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    // The bridges every town has started or finished: a river is bridged once.
    let mut underway: std::collections::BTreeSet<crate::hex::Hex> = q.iter()
        .flat_map(|(_, w, ..)| w.projects.iter().filter_map(|p| match p.what {
            What::Bridge { hex, .. } => Some(hex),
            _ => None,
        }).collect::<Vec<_>>()).collect();
    for &e in &towns.0 {
        let Ok((town, mut works, mut money, mut stock, trades, mut budget, people, farm, roster, paperwork)) = q.get_mut(e)
            else { continue };
        if works.reviewed.is_some_and(|t| now < t + day) {
            continue;
        }
        works.reviewed = Some(now);
        let t = town.id;
        let spare = money.0 - super::farms::reserve(&content, trades, farm);
        // What its halls would cost, for the Mason's marbles in the job draw (`BuildServices.work`).
        let mut work_wanted = 0.0;
        for kind in KINDS {
            if !buildable(&content, &works, roster, farm, paperwork, now, kind) {
                continue;
            }
            let c = cost(&content, &stock.0, kind);
            if work_wanted + c > spare.to_f64() {
                break;
            }
            work_wanted += c;
        }
        if work_wanted > 0.0 {
            signals.build_work.insert(t, work_wanted);
        } else {
            signals.build_work.remove(&t);
        }
        // A master Farmer plants the square's flower beds (`MasterServices.plantFlowers`).
        plant_flowers(&content, &mut works, &mut stock, &mut ledger, roster);
        // A bridge, a wall and hired builders are weighed at the town's toll reviews, every `toll_review_days`, the
        // first look only starting the clock (`Territory.touch`: `consider` and `hire` when `reviewTolls` reviewed); its
        // own Mason's building service each day.
        let toll_review = match works.bridges_looked {
            None => {
                works.bridges_looked = Some(now);
                false
            }
            Some(l) if now >= l + (d.toll_review_days * day as f64) as u64 => {
                works.bridges_looked = Some(now);
                true
            }
            Some(_) => false,
        };
        let hexes = territory.holding(t).map_or(0, |h| h.hexes.len());
        // A town without land has no stone: it starts nothing.
        if works.busy() || hexes == 0 {
            continue;
        }
        let spurs = mint.rate(now, day);
        let per_day = per_day(d, hexes);
        // A bridge over the river that keeps one of its roads to llamas (`map.rs`: a wagon has no way over a river
        // without one): the first river on the wagon's way that nobody's bridging, if the town can pay all its masons'
        // wages and keep `river_bridge_buffer` beyond. Its road takes wagons once it's built.
        if toll_review {
            let mut in_seq: Vec<&Route> = routes.iter()
                .filter(|r| r.from_id == t && r.carrier == crate::map::Carrier::Llama).collect();
            in_seq.sort_by_key(|r| r.seq);
            let hex_of = |id: usize| towns.0.get(id).and_then(|&x| places.get(x).ok()).and_then(|p| p.hex);
            let per_hex = crate::map::per_hex_at_bread(&content);
            let crossing = in_seq.iter().filter_map(|r| Some((hex_of(r.from_id)?, hex_of(r.to_id)?)))
                .flat_map(|(a, b)| map.river_crossings(a, b, per_hex))
                .find(|(hex, ..)| !underway.contains(hex));
            if let Some((hex, from, to)) = crossing {
                let deck = deck_size(hex, from, to);
                let wages = deck as f64 / per_day * d.masons as f64 * d.mason_pay;
                if spare >= wages + d.river_bridge_buffer {
                    underway.insert(hex);
                    works.start(What::Bridge { hex, from, to }, deck);
                    continue;
                }
            }
        }
        // A bridge whose tolls repay its wages soonest, within `payback_days`, if the town can pay half of them now.
        if toll_review {
            // In the routes' order: a load (or a component added mid-run) reorders the query, and the first best
            // crossing found is the one built.
            let mut in_seq: Vec<&Route> = routes.iter().collect();
            in_seq.sort_by_key(|r| r.seq);
            let ends: Vec<RouteEnds> = in_seq.into_iter().filter_map(|r| {
                let hex = |id: usize| towns.0.get(id).and_then(|&x| places.get(x).ok()).and_then(|p| p.hex);
                Some(RouteEnds { ends: [r.from_id, r.to_id], start: hex(r.from_id)?, goal: hex(r.to_id)?,
                    value: takings.expected_value(r.seq) })
            }).collect();
            let posts = territory.toll_posts(d.default_toll_rate);
            let per_hex = crate::map::per_hex_at_bread(&content) * spurs;
            let mut best: Option<(Hex2, f64)> = None;
            let mut best_payback = d.payback_days;
            for &hex in territory.holding(t).map_or(&[][..], |h| &h.hexes[..]) {
                let g = map.ground(hex);
                if works.has_bridge(hex) || g.bridged || (g.water_share <= IMPASSABLE && !g.river) || g.relief >= CLIFF {
                    continue;
                }
                let value = bridge_value(&map, &posts, &ends, t, hex, per_hex, d.max_toll_rate, d.detour_share);
                let net = value.tolls_per_day / spurs - guard_cost_per_day(d, spurs) / spurs;
                let (Some(from), Some(to)) = (value.from, value.to) else { continue };
                if net <= 0.0 {
                    continue;
                }
                let deck = deck_size(hex, from, to);
                let wages = deck as f64 / per_day * d.masons as f64 * d.mason_pay;
                let payback = wages / net;
                if payback < best_payback && spare >= wages / 2.0 {
                    best = Some(((hex, from, to, deck), payback));
                    best_payback = payback;
                }
            }
            if let Some(((hex, from, to, deck), _)) = best {
                works.start(What::Bridge { hex, from, to }, deck);
                continue;
            }
        }
        // Nothing that pays: a content town with money to spare walls itself in, if it can pay all the masons'
        // wages, keep `wall_buffer` beyond, keep buying what it brings in while the wall goes up, and keep its roads'
        // round trips (`Trades::road_keep`).
        if toll_review && works.wall().is_none() && people.mood >= MOOD_TO_START && spare >= d.wall_start_emeralds {
            let days = d.wall_blocks as f64 / per_day;
            let wages = days * d.masons as f64 * d.mason_pay;
            let roads = trades.road_keep.to_f64() * d.wall_road_trips;
            if spare >= wages + d.wall_buffer + imports_per_day(&content, &book, t, now) * days + roads {
                works.start(What::Wall, d.wall_blocks);
                continue;
            }
        }
        // Else the next hall it wants, with its Mason's skill each day or hired builders' at a toll review
        // (`BuildServices.next`, `hire`).
        let skill = match mason(&content, roster) {
            None if toll_review => d.hired_skill as u8,
            None => continue,
            Some(None) => continue, // its Masons are away
            Some(Some(level)) => level,
        };
        let Some(kind) = next_hall(&content, &works, roster, farm, paperwork, &stock.0, people.mood, spare.to_f64(), now, skill)
            else { continue };
        // Rounded up to the thousandth: a cost.
        let to_buy = crate::fixed::Fixed::ceil(materials(&content, &stock.0, kind));
        let what = Reason::Other(format!("a {}", kind.id().replace('_', " ")));
        let free = super::free(&content, trades, &money, &bills, t, now);
        if !Accountant::approve(&content, &mut book, t, now, free, &mut budget, what.clone(), to_buy) {
            continue; // not today: the accountant holds the money back
        }
        take_logs(&content, &mut stock, &mut ledger, (kind.emeralds() / EMERALDS_PER_LOG).ceil());
        post(&mut ledger, &mut book, Side::treasury(t, &mut money.0), Side::outside(), to_buy, what, now, day);
        works.start(What::Hall(kind), kind.blocks());
    }
}

type Hex2 = (crate::hex::Hex, crate::hex::Hex, crate::hex::Hex, u32);

/// The first kind a town lacks whose partners it has, if it can pay twice over (`BuildServices.next`): a town that
/// isn't content or can't yet pay for the first it wants saves up, and builds in order.
#[allow(clippy::too_many_arguments)]
fn next_hall(content: &Content, works: &Works, roster: Option<&Roster>, farm: Option<&Farm>, paperwork: &Paperwork,
        stock: &[f64], mood: f64, spare: f64, now: u64, skill: u8) -> Option<Kind> {
    for kind in KINDS {
        if works.has_started(kind) || skill < kind.mason_level() || !buildable(content, works, roster, farm, paperwork, now, kind) {
            continue;
        }
        if mood < MOOD_TO_START || spare < content.dials.build_savings * cost(content, stock, kind) {
            return None;
        }
        return Some(kind);
    }
    None
}

/// Takes up to `count` logs from the stock, in the order of `LOGS` (`BuildServices.takeLogs`).
fn take_logs(content: &Content, stock: &mut Stock, ledger: &mut Ledger, mut count: f64) {
    for name in LOGS {
        let Some(g) = crate::jobs::good_named(content, name) else { continue };
        if count <= 0.0 {
            return;
        }
        let n = stock.0[g].floor().max(0.0).min(count);
        stock.0[g] -= n;
        ledger.used[g] += n;
        count -= n;
    }
}

/// A master Farmer with a square to plant and eight flowers in the warehouse plants its beds
/// (`MasterServices.flowersWanted`, `plantFlowers`).
fn plant_flowers(content: &Content, works: &mut Works, stock: &mut Stock, ledger: &mut Ledger, roster: Option<&Roster>) {
    if works.flowers || !works.built(Kind::Square) {
        return;
    }
    let master = content.trade("farmer").zip(roster).is_some_and(|(trade, r)| {
        r.hands.iter().any(|h| h.trade == trade && !h.is_baby() && !h.away && h.level >= FLOWER_FARMER_LEVEL)
    });
    let flowers: Vec<usize> = FLOWER_ITEMS.iter().filter_map(|n| crate::jobs::good_named(content, n)).collect();
    if !master || flowers.iter().map(|&g| stock.0[g].floor().max(0.0)).sum::<f64>() < FLOWERS {
        return;
    }
    let mut left = FLOWERS;
    for g in flowers {
        let n = stock.0[g].floor().max(0.0).min(left);
        stock.0[g] -= n;
        ledger.used[g] += n;
        left -= n;
    }
    works.flowers = true;
}
