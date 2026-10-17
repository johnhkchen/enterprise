//! Squads through the look (`squad.rs`; `Caravans.dispatch`, `sendPortersHome`, `arriveHome`, `poll`'s walkers,
//! `Commissions.train`):
//!
//! - **Review:** once a day, a town with routes, a desk's Merchant and a squad short of leads trains one ([`train`]).
//! - **Arrive:** porters walking home get there ([`walk_home`]).
//! - **Buy:** at the far end, once the cargo fits without them, porters leave and walk home ([`send_porters_home`]),
//!   before the wagon buys for home or eats there (`rules/road.rs`).
//! - **Turn:** home again, the crew are home ([`come_home`]).
//! - **Plan:** each idle route of a town with a roster sizes up its squad, as the roster stands at the look's start
//!   ([`size_up`]): the slots its load fills, the mouths its road feeds. Nobody's hired, nobody's kept back.
//! - **Decide** (`plan::decide`, route by route as `Caravans.dispatch` goes): a town with no Merchant at its desk sends
//!   none; the route musters its squad of the hands still at home, keeping a lead for every route here still without
//!   a wagon; nobody to lead it, it takes on a hand, or gets no wagon ([`crew`]); a small load goes by porter
//!   ([`porter_run`]); the crew is trimmed to the mouths home's food feeds ([`trim`]); the crew go away and eat the
//!   road's food ([`depart`]); a saddle makes the wagon ride faster ([`saddle`]).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use super::{free, in_order, Step};
use super::road::Provision;
use crate::bills::Bills;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::lockers::Lockers;
use crate::road::{food_needed, held, road_food, Stay, FOOD_PAR_POINTS};
use crate::roster::{Hand, Roster, TradeId, MERCHANT};
use crate::squad::*;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        train.in_set(Step::Review).after(super::settlers::review).after(super::lockers::review)
            .after(super::farms::review).before(super::jobs::review),
        walk_home.in_set(Step::Arrive).after(super::arrive::mark),
        send_porters_home.in_set(Step::Buy),
        come_home.in_set(Step::Turn).after(super::road::feed).before(super::arrive::turn),
        size_up.in_set(Step::Plan).after(super::plan::start),
    ));
}

fn signing() -> Reason {
    Reason::Other("signing".into())
}

fn training() -> Reason {
    Reason::Other("training".into())
}

/// What a trip calls for of a town's hands at home (`dispatch` 249–270): the trades wanted there that a hand at home
/// has (`Services.wantedAbroad`, a hook); a Leatherworker, and porters, for more stacks of what the far end wants than
/// a wagon holds; and how many may go beyond a lead for every route `waiting` here (this one too).
pub fn call(content: &Content, roster: &Roster, home: &[usize], stacks: u32, wanted_trades: &[TradeId], waiting: usize)
        -> (Needs, usize) {
    let mut wanted: Vec<TradeId> = Vec::new();
    for &t in wanted_trades {
        if !wanted.contains(&t) && home.iter().any(|&i| roster.hands[i].trade == t) {
            wanted.push(t);
        }
    }
    let leatherworker = content.trade(LEATHERWORKER);
    let mut porters = 0;
    if stacks > content.dials.wagon_slots {
        if let Some(lw) = leatherworker {
            if !wanted.contains(&lw) {
                wanted.push(lw);
            }
        }
        let packs = home.iter().any(|&i| Some(roster.hands[i].trade) == leatherworker);
        porters = porters_needed(content, stacks, packs);
    }
    (Needs { wanted, water: false, porters }, home.len().saturating_sub(waiting))
}

/// Plan: each idle route of a town with a roster sizes up the squad it would muster (in the routes' order, on the
/// roster as it stands at the look's start, each keeping a lead for every route here without a wagon), for the slots
/// its load fills (`plan_sale`) and the mouths its road feeds (`plan_advances`), trimmed to the mouths home's food
/// feeds. Nobody's hired and nobody's kept back: the route musters its squad for good as it leaves ([`crew`]).
pub fn size_up(content: Res<Content>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        homes: Query<(&Roster, Option<&Lockers>, &Stock)>, fars: Query<&Trades>) {
    let food_mode = road_food(&content);
    let mut idle: BTreeMap<usize, usize> = BTreeMap::new();
    for (_, route, state) in routes.iter() {
        *idle.entry(route.from_id).or_default() += state.wagon.is_none() as usize;
    }
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        let route = *route;
        let state = &mut *state;
        let Some(plan) = state.plan.as_mut() else { continue };
        let Ok((roster, lockers, stock)) = homes.get(route.from) else { continue };
        if !crewed(Some(roster)) {
            continue;
        }
        let Ok(far) = fars.get(route.to) else { continue };
        let home = available(&content, roster, lockers);
        let stacks = wanted_stacks(&content, &stock.0, far);
        let waiting = idle.get(&route.from_id).copied().unwrap_or(0);
        let (needs, extras) = call(&content, roster, &home, stacks, &state.wanted_trades, waiting);
        plan.crew = crate::squad::muster(&content, roster, &home, &needs, extras);
        plan.wanted = needs.wanted;
        plan.porters = needs.porters;
        plan.slots = 0;
        if !plan.crew.is_empty() {
            trim(&content, roster, plan, food_mode.then(|| held(&content, &stock.0)), route.blocks, false);
        }
    }
}

/// Decide, a crewed town's route as it leaves (`Caravans.dispatch` 249–270): its squad mustered of the hands still at
/// home (those of the routes gone before it this look are away), keeping a lead for each of the `waiting` routes here
/// without a wagon (those that stayed too); nobody to lead it, the Merchant takes on a hand waiting for work as a
/// travelling Merchant while the leads at home and on the road are fewer than its `routes`, if the town can pay one
/// (`free`), and musters again; still nobody, "no squad". The crew, the trades wanted and the porters go on the plan.
#[allow(clippy::too_many_arguments)]
pub fn crew(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, town: usize, plan: &mut Plan,
        roster: &mut Roster, mut lockers: Option<&mut Lockers>, stacks: u32, wanted_trades: &[TradeId],
        waiting: usize, routes: usize, money: &mut Money, free: impl Fn(&Money) -> f64) -> Result<(), Stay> {
    let d = &content.dials;
    let mut home = available(content, roster, lockers.as_deref());
    let (needs, extras) = call(content, roster, &home, stacks, wanted_trades, waiting);
    let mut squad = crate::squad::muster(content, roster, &home, &needs, extras);
    if squad.is_empty() {
        let out = routes.saturating_sub(waiting);
        if let (Some(i), Some(merchant)) = (to_hire(content, roster, &home, routes, out), content.trade(MERCHANT)) {
            if free(money) >= d.signing_emeralds {
                post(ledger, cash, Side::treasury(town, &mut money.0), Side::outside(), d.signing_emeralds, signing(),
                    now, content.day_ticks);
                sign_on(content, roster, lockers.as_deref_mut(), i, merchant);
                home = available(content, roster, lockers.as_deref());
                squad = crate::squad::muster(content, roster, &home, &needs, extras);
            }
        }
    }
    if squad.is_empty() {
        return Err(Stay::NoSquad);
    }
    plan.crew = squad;
    plan.wanted = needs.wanted;
    plan.porters = needs.porters;
    plan.slots = capacity(content, plan.crew.iter().map(|&i| roster.hands[i].trade), false);
    Ok(())
}

/// Each member eats (`dispatch` 296–301): one more goes only while `home_food` (none: content has no food) beyond the
/// caravans' par feeds the whole crew a leg; the crew's slots, as it goes (a porter run's, one pack).
pub fn trim(content: &Content, roster: &Roster, plan: &mut Plan, home_food: Option<f64>, blocks: u32, porter: bool) {
    if let Some(home_food) = home_food {
        let per_mouth = food_needed(content, blocks);
        while plan.crew.len() > 1 && home_food - FOOD_PAR_POINTS < per_mouth * plan.crew.len() as f64 {
            plan.crew.pop();
        }
    }
    plan.slots = capacity(content, plan.crew.iter().map(|&i| roster.hands[i].trade), porter);
}

/// A small load goes on a porter's back (`Caravans.porterRun`, as `dispatch` asks it): a town with a roster whose
/// trip wants at most one trade's work and no porters nor a Shepherd's droving, whose wagon came home last time with
/// a pack's worth or less (a run never seen home goes by wagon), with a team to spare (`wagon_team`), what's owed and
/// what's planned within one pack, and something that can't wait or its day come.
pub fn porter_run(content: &Content, state: &RouteState, plan: &Plan, owed_stacks: u32, on_its_day: bool,
        roster: Option<&Roster>) -> bool {
    let d = &content.dials;
    let Some(roster) = roster else { return false };
    let shepherd = content.trade(SHEPHERD);
    !plan.crew.is_empty() && d.wagon_team > 0
        && state.last_home.is_some_and(|h| h <= d.pack_stacks)
        && plan.wanted.len() <= 1 && plan.porters == 0
        && !plan.crew.iter().any(|&i| Some(roster.hands[i].trade) == shepherd)
        && owed_stacks <= d.pack_stacks && plan.stacks <= d.pack_stacks
        && (plan.pressing || on_its_day)
}

/// A porter run's crew: the one hand whose trade is wanted there, if any, else the lead; one pack.
pub fn to_porter(content: &Content, plan: &mut Plan, roster: &Roster) {
    let pick = plan.crew.iter().copied().find(|&i| plan.wanted.contains(&roster.hands[i].trade)).unwrap_or(plan.crew[0]);
    plan.crew = vec![pick];
    plan.slots = capacity(content, [roster.hands[pick].trade], true);
}

/// A porter run has no team to feed: its road is only the crew's food.
pub fn no_team(content: &Content, p: &mut Provision) {
    p.team_feed = 0.0;
    p.fed = 0.0;
    p.team_coin = 0.0;
    p.cost = 2.0 * p.food * content.dials.purse_per_food_point;
}

/// The crew leave: each marked away in home's roster, with an id to know it by ([`stamp`]), and the hands as they
/// left, for the wagon.
pub fn depart(roster: Option<&mut Roster>, crew: &[usize]) -> Vec<Hand> {
    let Some(roster) = roster else { return Vec::new() };
    crew.iter().filter_map(|&i| stamp(roster, i)).collect()
}

/// A saddle in home's warehouse: the wagon rides a third faster, and the trip wears a tenth of it (used up at a
/// whole one: `Caravans.dispatch` 528–537). Returns whether it rides saddled.
pub fn saddle(content: &Content, ledger: &mut Ledger, state: &mut RouteState, stock: &mut [f64]) -> bool {
    let Some(g) = content.good(SADDLE) else { return false };
    if stock[g] < 1.0 {
        return false;
    }
    state.saddle_wear += content.dials.saddle_wear;
    if state.saddle_wear >= 1.0 {
        stock[g] -= 1.0;
        ledger.used[g] += 1.0;
        state.saddle_wear -= 1.0;
    }
    true
}

/// At the far end, once the wagon has sold: its porters have delivered, and walk home on their own (a leg's time),
/// unless what's left aboard still needs their backs, or they're all its crew (`Caravans.sendPortersHome`).
pub fn send_porters_home(clock: Res<Clock>, content: Res<Content>, mut routes: Query<(Entity, &Route, &mut RouteState)>) {
    let d = &content.dials;
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let state = &mut *state;
        let wagon = state.wagon.as_mut().unwrap();
        let porters = wagon.crew.iter().filter(|h| porter(&content, h.trade)).count();
        if porters == 0 || porters == wagon.crew.len() {
            continue;
        }
        let capacity = capacity(&content, wagon.crew.iter().map(|h| h.trade), wagon.porter);
        let stacks: u32 = wagon.cargo.iter().enumerate()
            .map(|(g, n)| (n.max(0.0) / content.goods[g].stack.max(1) as f64).ceil() as u32).sum();
        if stacks as i64 > capacity as i64 - (d.pack_stacks as usize * porters) as i64 {
            continue;
        }
        let (walking, staying): (Vec<Hand>, Vec<Hand>) = wagon.crew.drain(..).partition(|h| porter(&content, h.trade));
        wagon.crew = staying;
        wagon.slots = crate::squad::capacity(&content, wagon.crew.iter().map(|h| h.trade), wagon.porter);
        let arrives = clock.now + route.travel_ticks;
        state.porters_sent += walking.len() as u32;
        state.walkers.extend(walking.into_iter().map(|hand| Walker { hand, arrives }));
    }
}

/// Porters walking home who get there by now are home (`CaravanLedger.walkersHome`).
pub fn walk_home(clock: Res<Clock>, mut routes: Query<(Entity, &Route, &mut RouteState)>, mut rosters: Query<&mut Roster>) {
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if !state.walkers.iter().any(|w| w.arrives <= clock.now) {
            continue;
        }
        let (home, walking): (Vec<Walker>, Vec<Walker>) = state.walkers.drain(..).partition(|w| w.arrives <= clock.now);
        state.walkers = walking;
        if let Ok(mut roster) = rosters.get_mut(route.from) {
            release(&mut roster, &home.into_iter().map(|w| w.hand).collect::<Vec<_>>());
        }
    }
}

/// A wagon home: its crew are home again (`arriveHome`: the squad released, no longer away), and the stacks it
/// brought home are noted (`lastHome`, for the porter run).
pub fn come_home(content: Res<Content>, mut routes: Query<(Entity, &Route, &mut RouteState)>, mut rosters: Query<&mut Roster>) {
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let Some(wagon) = state.wagon.as_ref() else { continue };
        let stacks: u32 = wagon.cargo.iter().enumerate()
            .map(|(g, n)| (n.max(0.0) / content.goods[g].stack.max(1) as f64).ceil() as u32).sum();
        if let Ok(mut roster) = rosters.get_mut(route.from) {
            release(&mut roster, &wagon.crew);
        }
        state.last_home = Some(stacks);
    }
}

/// Once a day: every town with a roster and routes of its own trains a hand for its squad, while it's short of leads;
/// before the idle find work (`rules/jobs.rs`), which waits on it (`Territory.grow`: squads before anything).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn train(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, towns_order: Res<Towns>, mut squads: ResMut<Squads>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, routes: Query<&Route>,
        mut towns: Query<(&Town, &mut Roster, Option<&mut Lockers>, &mut Money, &Trades)>) {
    let now = clock.now;
    let today = now / content.day_ticks.max(1);
    if squads.trained == Some(today) {
        return;
    }
    squads.trained = Some(today);
    let mut count: BTreeMap<usize, usize> = BTreeMap::new();
    for r in routes.iter() {
        *count.entry(r.from_id).or_default() += 1;
    }
    for &e in towns_order.0.iter() {
        let Ok((town, mut roster, lockers, mut money, trades)) = towns.get_mut(e) else { continue };
        let routes = count.get(&town.id).copied().unwrap_or(0);
        if routes == 0 || !crewed(Some(&roster)) {
            continue;
        }
        let spare = free(&content, trades, &money, &bills, town.id, now);
        if train_town(&content, &mut ledger, &mut cash, now, town.id, routes, spare, &mut roster,
            lockers.map(|l| l.into_inner()), &mut money).is_some() {
            squads.trained_hand.insert(town.id, today);
        }
    }
}

/// A town with `routes`, a Merchant at its desk and `free` money trains a hand for its squad (`Commissions.train`): the
/// one [`to_train`] picks, if it can pay `training_emeralds` from its free money. Returns the hand trained.
#[allow(clippy::too_many_arguments)]
pub fn train_town(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64, town: usize, routes: usize,
        free: f64, roster: &mut Roster, lockers: Option<&mut Lockers>, money: &mut Money) -> Option<usize> {
    // No Merchant at the desk, nobody to commission it (`Caravans.dispatch` 210: the town sends no caravan either).
    roster.merchant?;
    let cost = content.dials.training_emeralds;
    let (i, trade) = to_train(content, roster, lockers.as_deref(), routes)?;
    if free < cost {
        return None;
    }
    post(ledger, cash, Side::treasury(town, &mut money.0), Side::outside(), cost, training(), now, content.day_ticks);
    sign_on(content, roster, lockers, i, trade);
    Some(i)
}
