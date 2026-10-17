//! Sourcing and procurement through the look (`sourcing.rs`; `Broker.source`, `Procurement`, `ContractBook.review`,
//! `Caravans.collectPressing`, `MetalExchange.day`):
//!
//! - **Work**, before the lockers restock: each town's Merchant switches on its managed lockers' selling offers for what
//!   its contracts want, and tells them what's due soon, to cap their runs ([`procure`]).
//! - **Review**, once a day: the capital's exchange makes its overseas market's metals, counts its own and sources the
//!   shortfall nearest holder first; each town with a master Merchant sources what its contracts want and its people
//!   can't make; what no market supplies is left for the desk's signals (`rules/orders.rs`) ([`review`]).
//! - **Judge**: a seller's contracts in force set what it needs to make what it still owes ([`inputs`]).
//! - **Papers**, at the far end: the copy of a contract sourcing drafted, held by one of the two towns, goes to the other
//!   with the wagon, or rides home with it ([`copies`]).
//! - **PlanCollect**: a trip that takes the far end inputs it is short of for what it makes this town can't wait, once an
//!   installment ([`plan_inputs`]); the sale load carries the inputs first (`rules/plan.rs`).

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::atlas::Stocktakes;
use crate::content::{Content, GoodId};
use crate::contracts::{Book, State};
use crate::lockers::Lockers;
use crate::messages::TownId;
use crate::realms::{Crown, OffMap};
use crate::roster::{Roster, TradeId};
use crate::sourcing::*;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // After the town's other work with its stock, and before its Merchant's lessons and its lockers' trading
        // (`LockerTrading.trade` switches on what's wanted first).
        procure_lockers.in_set(Step::Work).after(super::services::home).before(super::training::lessons)
            .before(super::lockers::restock),
        // (before the hubs' review and the shop, the last of the reviews: Bevy's free choice pinned as it ran)
        review.in_set(Step::Review).after(super::jobs::review).before(super::freight::review)
            .before(super::desires::keep_shops),
        // (as part of the contracts' review, before the rents' and so the bank's and training's judgements)
        inputs.in_set(Step::Judge).after(super::contracts::review).before(super::harvest::review_rents),
        // (once the wagon has fetched and paid what's owed there: `cargo::pay_owing` comes first)
        copies.in_set(Step::Papers).before(super::advances::advances_papers),
        plan_inputs.in_set(Step::PlanCollect).after(super::contracts::plan_collect),
    ));
}

/// Every town's Merchant switches on its managed lockers' selling offers for what its contracts want, in town order.
#[allow(clippy::type_complexity)]
pub fn procure_lockers(clock: Res<Clock>, content: Res<Content>, book: Res<Book>,
        mut towns: Query<(&Town, &Roster, &mut Lockers, &Stock)>) {
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(t, ..)| t.id);
    for (town, roster, mut lockers, stock) in order {
        // Nothing wanted, nothing switched on before, nothing to cap: leave the town as it is.
        if book.open.is_empty() && lockers.due.is_empty() && !lockers.hands.iter().any(|h| h.offers.iter().any(|o| o.procured)) {
            continue;
        }
        procure(&content, &book, town.id, clock.now, stock, roster, &mut lockers);
    }
}

/// Each seller's contracts in force set the inputs it needs to make what it still owes (`ContractBook.review` 301,
/// `Warehouse.setInputs`); a town that sells nothing needs none (`retainInputs`).
pub fn inputs(content: Res<Content>, book: Res<Book>, mut towns: Query<(&Town, &mut Trades)>) {
    let sellers: BTreeSet<TownId> = book.open.iter().filter_map(|&id| book.contracts.get(id))
        .filter(|c| c.in_force() && c.known_by_seller && c.training.is_none()).map(|c| c.seller).collect();
    for (town, mut trades) in &mut towns {
        let mut need = Vec::new();
        if sellers.contains(&town.id) {
            let needs = crate::services::contract_inputs(&content, &book, town.id);
            if !needs.is_empty() {
                need = vec![0.0; content.goods.len()];
                for (g, n) in needs {
                    need[g] = n;
                }
            }
        }
        if trades.inputs != need {
            trades.inputs = need;
        }
    }
}

/// At the far end the copy of a contract sourcing drafted goes to the town that doesn't hold it, or rides home with the
/// wagon if the far end is the one that does ("copies ride with any caravan between the parties, each way").
pub fn copies(mut sourcing: ResMut<Sourcing>, mut book: ResMut<Book>, mut routes: Query<(Entity, &Route, &mut RouteState)>) {
    if sourcing.copies.is_empty() {
        return;
    }
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        for &id in &sourcing.copies {
            let c = &book.contracts[id];
            if c.state != State::Drafted {
                continue;
            }
            let (a, b) = (route.from_id, route.to_id);
            if !(c.seller == a && c.buyer == b || c.seller == b && c.buyer == a) {
                continue;
            }
            let holds = |t: TownId| c.seller == t && c.known_by_seller || c.buyer == t && c.known_by_buyer;
            let (home, far) = (holds(a), holds(b));
            if home && !far {
                book.deliver_copy(id, b);
            } else if far && !home && !wagon.drafts.contains(&id) {
                wagon.drafts.push(id);
            }
        }
    }
    let book = &*book;
    sourcing.copies.retain(|&id| book.contracts[id].state == State::Drafted);
}

/// What can't wait on a trip (`Caravans.collectPressing`, `dispatch`'s business there): a trip that takes the far end
/// inputs it is short of for what it makes this town, once an installment: the far end's stock (as home knows it) holds
/// less of an input than its contracts need, home has some, and a contract in force from the far end to home was due
/// since the route last left. And, on the route's day, a contract sourcing drafted that waits for a copy to ride between
/// the two (the mod's copies also go by post; the core's post carries one only where no regular run joins the two, `post.rs`): business there.
#[allow(clippy::type_complexity)]
pub fn plan_inputs(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, stocktakes: Res<Stocktakes>,
        sourcing: Res<Sourcing>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Stock, &Trades, Option<&Roster>)>) {
    for (route, mut state) in &mut routes {
        let last = state.last_departure;
        let Some(plan) = state.plan.as_mut() else { continue };
        if plan.pressing {
            continue;
        }
        let (a, b) = (route.from_id, route.to_id);
        let on_its_day = last.is_none_or(|t| clock.now.saturating_sub(t) >= route.every_ticks);
        if on_its_day && sourcing.copies.iter().map(|&id| &book.contracts[id]).any(|c| {
            let holds = |t: TownId| c.seller == t && c.known_by_seller || c.buyer == t && c.known_by_buyer;
            c.state == State::Drafted && (c.seller == a && c.buyer == b || c.seller == b && c.buyer == a) && holds(a) != holds(b)
        }) {
            plan.pressing = true;
            continue;
        }
        let (Ok((home, ..)), Ok((far, far_trades, _))) = (towns.get(route.from), towns.get(route.to)) else { continue };
        if far_trades.inputs.iter().all(|n| *n <= 0.0) {
            continue;
        }
        let rostered = towns.get(route.from).is_ok_and(|(_, _, r)| crate::squad::crewed(r));
        let seen = if rostered { stocktakes.fresh(&content, route.to_id, clock.now) } else { None };
        let held = |g: usize| seen.map_or(far.0[g], |s| s.stock.get(g).copied().unwrap_or(0.0));
        let short = far_trades.inputs.iter().enumerate().any(|(g, &need)| need > 0.0 && held(g) < need && home.0[g] > 0.0);
        if short && book.in_force(route.to_id, route.from_id).any(|c| last.is_none_or(|t| t < c.previous_due(c.drafted))) {
            plan.pressing = true;
        }
    }
}

/// The trade that would supply a good: the first whose people sell it for emeralds, else the first that makes it
/// (`Procurement.suppliers`).
pub(crate) fn supplier_trade(content: &Content, good: GoodId) -> Option<TradeId> {
    (0..content.trades.len()).find(|&t| content.trades[t].offers.iter().any(|o| o.sells && o.good == good))
        .or_else(|| crate::services::recipes(content).iter().find(|r| r.output == good).map(|r| r.trade))
}

/// Is there anything to do today: the exchange to run, or a town with a master Merchant that hasn't sourced yet?
fn review_due(world: &World, today: u64) -> bool {
    let ex = world.resource::<MetalExchange>();
    if ex.desk.or(world.resource::<Crown>().capital).is_some() && !ex.last_day.is_some_and(|d| d >= today) {
        return true;
    }
    let sourcing = world.resource::<Sourcing>();
    world.resource::<Towns>().0.iter().enumerate().any(|(t, &e)| {
        !sourcing.reviewed.get(&t).is_some_and(|&d| d >= today)
            && world.get::<Roster>(e).and_then(|r| r.merchant_level()).is_some_and(|l| l >= MASTER)
            && !world.entity(e).contains::<OffMap>()
    })
}

/// Once a day: the exchange, then each town's sourcing. What no market with a road supplies is left in
/// [`Sourcing::unmet`] for the desk's signals (`rules/orders.rs`), the one stream to the jobs.
pub fn review(world: &mut World) {
    let (now, day) = {
        let c = world.resource::<Content>();
        (world.resource::<Clock>().now, c.day_ticks.max(1))
    };
    let today = now / day;
    if !review_due(world, today) {
        return;
    }
    world.resource_mut::<Sourcing>().ran = Some(now);
    let exchange_needs = exchange_day(world, today, now);
    let routes = route_facts(world);
    // What each part would do, read from the world as it stands.
    let mut drafts: Vec<(TownId, Vec<Draft>)> = Vec::new();
    let mut reviewed: Vec<TownId> = Vec::new();
    let mut unmet: BTreeMap<TownId, Vec<(GoodId, f64)>> = world.resource::<Sourcing>().unmet.clone();
    {
        let env = Env::gather(world, routes);
        if let Some((desk, needs)) = &exchange_needs {
            let mut ds = Vec::new();
            for &(good, need, premium) in needs {
                ds.extend(env.source(*desk, good, need, premium).drafts);
            }
            drafts.push((*desk, ds));
        }
        let sourcing = world.resource::<Sourcing>();
        for t in 0..env.towns.len() {
            let e = env.towns[t];
            let brokers = world.get::<Roster>(e).and_then(|r| r.merchant_level()).is_some_and(|l| l >= MASTER)
                && !world.entity(e).contains::<OffMap>();
            if !brokers {
                unmet.remove(&t); // (a town that no longer brokers posts nothing)
                continue;
            }
            if sourcing.reviewed.get(&t).is_some_and(|&d| d >= today) {
                continue;
            }
            reviewed.push(t);
            let stock = world.get::<Stock>(e).expect("a town");
            let mut ds = Vec::new();
            let mut left = Vec::new();
            // What its contracts want, and what a player's order at its desk still wants of what it can spare (the
            // shop's contract is one of the seller contracts `wanted` reads, but nets the whole stock, where the mod's
            // `fill` asks for what it can't spare beyond the par: the larger of the two).
            let mut wants = wanted(env.content, env.book, t, &stock.0);
            if let Some(open) = world.resource::<crate::orders::Orders>().wanted.get(&t) {
                for (&good, &n) in open {
                    let w = wants.entry(good).or_insert(0.0);
                    *w = w.max(n);
                }
            }
            for (good, need) in wants {
                // What its own people make or sell, its Merchant buys of them (`procure`).
                if env.makes(t, t, good) {
                    continue;
                }
                let sourced = env.source(t, good, need, 1.0);
                if !sourced.supplied {
                    left.push((good, need));
                }
                ds.extend(sourced.drafts);
            }
            if left.is_empty() {
                unmet.remove(&t);
            } else {
                unmet.insert(t, left);
            }
            drafts.push((t, ds));
        }
    }
    for (desk, ds) in &drafts {
        apply(world, *desk, ds);
    }
    let mut sourcing = world.resource_mut::<Sourcing>();
    sourcing.unmet = unmet;
    for t in reviewed {
        sourcing.reviewed.insert(t, today);
    }
}

/// A day at the exchange (`MetalExchange.day`): the market across the sea makes its metal and its travellers' bread, the
/// capital counts what it holds, and what it is short of it wants contracted, at the premium its shortage sets. Returns
/// the capital and each metal short: (good, how many, premium). Once a day.
fn exchange_day(world: &mut World, today: u64, now: u64) -> Option<(TownId, Vec<(GoodId, f64, f64)>)> {
    let ex = world.resource::<MetalExchange>().clone();
    let desk = ex.desk.or(world.resource::<Crown>().capital)?;
    if ex.last_day.is_some_and(|d| d >= today) {
        return None;
    }
    let metals = metals(world.resource::<Content>());
    let towns = world.resource::<Towns>().0.clone();
    if desk >= towns.len() {
        return None;
    }
    if let Some(far) = ex.overseas.filter(|&o| o < towns.len()) {
        let bread = crate::jobs::good_named(world.resource::<Content>(), "bread");
        let mut made: Vec<(GoodId, f64)> = Vec::new();
        {
            let stock = world.get::<Stock>(towns[far]).expect("a town");
            if let Some(b) = bread {
                made.push((b, OVERSEAS_BREAD - stock.0[b].floor()));
            }
            for m in &metals {
                made.push((m.good, m.overseas - stock.0[m.good].floor()));
            }
        }
        let mut ledger = std::mem::take(&mut *world.resource_mut::<Ledger>());
        {
            let mut stock = world.get_mut::<Stock>(towns[far]).expect("a town");
            for (g, n) in made.into_iter().filter(|(_, n)| *n > 0.0) {
                stock.0[g] += n;
                ledger.made[g] += n;
            }
        }
        *world.resource_mut::<Ledger>() = ledger;
        // What the capital's Merchant hears of it.
        let seen = world.get::<Stock>(towns[far]).expect("a town").0.clone();
        world.resource_mut::<Stocktakes>().record(far, &seen, Some(now));
    }
    let held: Vec<(GoodId, f64)> = {
        let stock = world.get::<Stock>(towns[desk]).expect("a town");
        metals.iter().map(|m| (m.good, stock.0[m.good].floor())).collect()
    };
    let mut ex = world.resource_mut::<MetalExchange>();
    ex.last_day = Some(today);
    for &(g, n) in &held {
        ex.held.insert(g, n);
    }
    let needs: Vec<(GoodId, f64, f64)> = metals.iter()
        .map(|m| (m.good, ex.target(m) - ex.held(m.good), ex.premium(m))).filter(|(_, need, _)| *need > 0.0).collect();
    Some((desk, needs))
}
