//! Plan: each idle route starts a plan this look (`Plan`); the modules add to it (owed goods, collections, papers,
//! freight); the sale load fills the rest (`PlanSale`); then the wagon goes or waits (`Decide`).

use bevy_ecs::prelude::*;

use super::{free, in_order, shelf, Step, TownData};
use super::road::provision;
use crate::road::Stay;
use crate::bills::Bills;
use crate::cash_book::{post_legs, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::Book;
use crate::messages::Output;
use crate::people::{comfort_held, comfort_keep, comfort_par};
use crate::state::*;
use crate::squad::crewed;
use std::collections::BTreeMap;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        start.in_set(Step::Plan),
        plan_sale.in_set(Step::PlanSale),
        decide.in_set(Step::Decide),
    ));
}

/// An idle route starts a plan; a route with its wagon out has none.
pub fn start(content: Res<Content>, mut routes: Query<&mut RouteState>) {
    let n = content.goods.len();
    for mut state in &mut routes {
        state.plan = state.wagon.is_none().then(|| Plan { cargo: vec![0.0; n], ..Default::default() });
    }
}

/// For sale, on top of what's planned already: comforts by kind, the cheapest first, up to the far end's par in worth
/// from what home holds beyond its keep; any other good the far end's people use, what home can spare beyond its own
/// shelf up to the room on theirs; all in the wagon's slots. The load is worth what the far end is expected to pay:
/// the bargained price at the middle of each good's batch (`bargain.rs`), and a good it won't deal in stays home.
fn plan_sale(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Stock, &Trades, &People)>, rosters: Query<&crate::roster::Roster>,
        stores: Query<&crate::cargo::Shipments>) {
    let d = &content.dials;
    let n = content.goods.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        // The squad's slots (`squad.rs`: packs and porters), a wagon's without one.
        let slots = if plan.slots > 0 { plan.slots } else { d.wagon_slots };
        let Ok([(home_stock, home_trades, home_people), (far_stock, far_trades, far_people)]) =
            towns.get_many([route.from, route.to]) else { continue };
        let seller = crate::bargain::merchant_level(rosters.get(route.from).ok());
        let buyer = crate::bargain::merchant_level(rosters.get(route.to).ok());
        // What the town owes under harvest contracts by the end of the next window, beyond what this plan already
        // carries to the contract's buyer and what's crated for them, isn't for sale (`ForwardSales.setAside`).
        let mut aside = vec![0.0; n];
        for g in 0..n {
            aside[g] = crate::cargo::set_aside(&book, stores.get(route.from).ok(), route.from_id, g, clock.now, content.day_ticks);
        }
        for &(id, taken) in &plan.contracted {
            let c = &book.contracts[id];
            if c.harvest.is_some() {
                aside[c.good] = (aside[c.good] - taken).max(0.0);
            }
        }
        let mut stacks: u32 = (0..n).map(|g| (plan.cargo[g] / content.goods[g].stack as f64).ceil() as u32).sum();
        stacks += plan.shipments.iter().map(|s| s.stacks(&content)).sum::<u32>(); // home's crates (`rules/cargo.rs`)
        let mut loaded_kind = vec![0.0; content.comforts.len()];
        for &g in &order {
            if stacks >= slots {
                break;
            }
            let value = content.goods[g].value.max(1e-9);
            let planned = plan.cargo[g];
            let (spare, room, par) = match content.goods[g].comfort {
                Some(kind) => {
                    let room = comfort_par(&content, far_people) - comfort_held(&content, far_stock, kind) - loaded_kind[kind];
                    let spare = comfort_held(&content, home_stock, kind) - comfort_keep(&content, home_people) - loaded_kind[kind];
                    ((spare / value).min(home_stock.0[g] - planned - aside[g]), room / value, far_stock.0[g] + (room / value).floor())
                }
                None if far_trades.uses[g] > 0.0 => (
                    home_stock.0[g] - planned - aside[g] - shelf(&content, home_trades, g),
                    shelf(&content, far_trades, g) - far_stock.0[g],
                    shelf(&content, far_trades, g),
                ),
                None => continue,
            };
            let stack = content.goods[g].stack as f64;
            let going = spare.min(room).min((slots - stacks) as f64 * stack).floor();
            if going <= 0.0 {
                continue;
            }
            let Some(price) = crate::bargain::unit_price(&content, g, far_stock.0[g], going, par, seller, buyer) else {
                continue;
            };
            plan.cargo[g] += going;
            stacks += (going / stack).ceil() as u32;
            plan.worth += going * price;
            plan.sale.push((g, going, price));
            if let Some(kind) = content.goods[g].comfort {
                loaded_kind[kind] += going * value;
            }
        }
        plan.stacks = stacks;
    }
}

/// The wagon goes when something can't wait, or when full or on its day with a load worth its road food, if the town
/// can pay for the road and what it collects (where content has food, the road's food: `rules/road.rs`). Leaving, it
/// takes its cargo, the money for its collections and the bills home holds on the far end.
#[allow(clippy::too_many_arguments)]
pub(crate) fn decide(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut outbox: ResMut<Outbox>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>, mut rosters: Query<&mut crate::roster::Roster>, mut lockers: Query<&mut crate::lockers::Lockers>) {
    let now = clock.now;
    let d = &content.dials;
    // Squads (`squad.rs`): each town's routes, and those of them without a wagon (`dispatch`'s `waiting`).
    let (mut routes_at, mut waiting): (BTreeMap<usize, usize>, BTreeMap<usize, usize>) = Default::default();
    for (_, route, state) in routes.iter() {
        *routes_at.entry(route.from_id).or_default() += 1;
        *waiting.entry(route.from_id).or_default() += state.wagon.is_none() as usize;
    }
    // Road food (`road.rs`): what wagons on their way out will eat at each far end, if they packed none.
    let food_mode = crate::road::road_food(&content);
    let mut promised: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
    if food_mode {
        for (_, route, state) in routes.iter() {
            if let Some(w) = state.wagon.as_ref().filter(|w| w.leg == Leg::Out && w.provisions <= 0.0) {
                *promised.entry(route.to_id).or_insert(0.0) += crate::road::food_needed(&content, route.blocks)
                    * crate::road::mouths(Some(w));
            }
        }
    }
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        let Some(mut plan) = state.plan.take() else { continue };
        let mut roster = rosters.get_mut(route.from).ok();
        let rostered = roster.as_deref().is_some_and(|r| crewed(Some(r)));
        // A town with a roster sends no wagon with no Merchant at its desk (`Caravans.dispatch` 210).
        if rostered && roster.as_ref().is_some_and(|r| r.merchant.is_none()) {
            state.stayed = Some(Stay::NoMerchant);
            continue;
        }
        // The stacks of what the far end wants that home holds now (`wantedStacks`), for the squad's packs and porters.
        let stacks_wanted = match (rostered, towns.get(route.from), towns.get(route.to)) {
            (true, Ok((home, ..)), Ok((_, _, far, ..))) => crate::squad::wanted_stacks(&content, &home.0, far),
            _ => 0,
        };
        let far_food = if food_mode {
            towns.get(route.to).map_or(0.0, |(far, ..)| crate::road::held(&content, &far.0))
                - promised.get(&route.to_id).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        let (mut stock, mut money, trades, _, _) = towns.get_mut(route.from).expect("home");
        // Every route planned on the warehouse as it was at the look's start: another wagon from here may have left
        // with some of it since. What's owed first, then the rest, as far as the shelves still hold.
        let mut left = stock.0.clone();
        let mut owed = vec![0.0; left.len()];
        for (id, n) in plan.contracted.iter_mut() {
            let g = book.contracts[*id].good;
            *n = n.min(left[g].max(0.0)).floor();
            left[g] -= *n;
            owed[g] += *n;
        }
        plan.contracted.retain(|&(_, n)| n > 0.0);
        let mut stacks = 0;
        for g in 0..plan.cargo.len() {
            plan.cargo[g] = owed[g] + (plan.cargo[g] - owed[g]).min(left[g].max(0.0)).max(0.0).floor();
            stacks += (plan.cargo[g] / content.goods[g].stack as f64).ceil() as u32;
        }
        // Home's crates it takes (`rules/cargo.rs`).
        stacks += plan.shipments.iter().map(|s| s.stacks(&content)).sum::<u32>();
        plan.stacks = stacks;
        let on_its_day = state.last_departure.map_or(true, |t| now - t >= route.every_ticks);
        let waiting_here = waiting.get(&route.from_id).copied().unwrap_or(0);
        let mut porter = false;
        if let Some(roster) = roster.as_deref_mut().filter(|_| rostered) {
            // The squad, mustered of the hands still at home (`squad.rs`); nobody to lead it, a hand taken on, or none.
            let wanted_trades = state.wanted_trades.clone();
            let routes_here = routes_at.get(&route.from_id).copied().unwrap_or(0);
            let mustered = crate::rules::squad::crew(&content, &mut ledger, &mut cash, now, route.from_id, &mut plan,
                roster, lockers.get_mut(route.from).ok().map(|l| l.into_inner()), stacks_wanted, &wanted_trades,
                waiting_here, routes_here, &mut money, |m| free(&content, trades, m, &bills, route.from_id, now));
            if let Err(stay) = mustered {
                state.stayed = Some(stay);
                continue;
            }
            // A small load goes on a porter's back: one hand, one pack, no team to feed.
            let owed_stacks: u32 = owed.iter().enumerate()
                .map(|(g, n)| (n / content.goods[g].stack as f64).ceil() as u32).sum();
            porter = crate::rules::squad::porter_run(&content, &state, &plan, owed_stacks, on_its_day, Some(roster));
            if porter {
                crate::rules::squad::to_porter(&content, &mut plan, roster);
            }
        }
        if food_mode && !crate::road::in_range(&content, route.blocks) {
            state.stayed = Some(Stay::OutOfRange);
            continue;
        }
        if let Some(roster) = roster.as_deref().filter(|_| rostered) {
            // Each member eats: one more only while home's food (what the wagons gone before left) feeds it.
            let home_food = food_mode.then(|| crate::road::held(&content, &stock.0));
            crate::rules::squad::trim(&content, roster, &mut plan, home_food, route.blocks, porter);
        }
        let slots = if plan.slots > 0 { plan.slots } else { d.wagon_slots };
        if plan.stacks > slots {
            // Fewer slots than the plan was made for: the last of the sale load stays home, never what's owed.
            clip(&content, &mut plan, &owed, slots);
        }
        let full = plan.stacks as f64 >= d.wagon_full * slots as f64;
        // Each of the crew eats (`squad.rs`); a town without a roster sends one.
        let mouths = plan.crew.len().max(1) as f64;
        // The road's cost: its food where content has food (eaten from home, bought on the way, a purse for the road
        // home's), else its money; a load must be worth both legs' food, and home must pay for the road and the purse.
        let keep_wheat = food_mode && crate::road::keep_wheat(&content, trades, roster.as_deref());
        let provision = food_mode.then(|| {
            let after: Vec<f64> = stock.0.iter().zip(&plan.cargo).map(|(s, c)| s - c).collect();
            let home_free = free(&content, trades, &money, &bills, route.from_id, now);
            let mut p = provision(&content, route.blocks, mouths, crate::road::held(&content, &after), far_food,
                home_free, |feed| crate::road::fodder(&content, &after, feed, keep_wheat).2);
            if porter {
                crate::rules::squad::no_team(&content, &mut p);
            }
            p
        });
        let (road_money, wagon_purse, cost) = match provision {
            Some(p) => (p.food_coin + p.team_coin, p.road_purse + plan.purse, p.cost),
            None => (route.road_cost, plan.purse, route.road_cost),
        };
        let pays = plan.worth >= cost;
        // (Where content has food, a purse the plan filled to all home has beyond the road may sum an ulp over it.)
        let slack = if provision.is_some() { 1e-9 } else { 0.0 };
        if money.0 + slack < road_money + wagon_purse {
            state.stayed = Some(Stay::CantFund);
            continue;
        }
        let wagon_purse = if provision.is_some() { wagon_purse.min(money.0 - road_money).max(0.0) } else { wagon_purse };
        if !(plan.pressing || (full || on_its_day) && pays) {
            state.stayed = Some(Stay::WaitingForALoad);
            continue;
        }
        state.stayed = None;
        for (g, n) in plan.cargo.iter().enumerate() {
            stock.0[g] -= n;
        }
        for &(id, n) in &plan.contracted {
            book.ship(id, n);
        }
        if let Some(p) = provision {
            // The team's fodder first, as it was found (the mod eats it after the food: the same, unless wheat is food).
            crate::road::eat_fodder(&content, &mut ledger, &mut stock.0, p.team_feed, keep_wheat);
            // The way out's food and any packed for the road home, from home's warehouse: already paid for.
            crate::road::eat(&content, &mut ledger, &mut stock.0, p.from_home + p.provisions);
            state.food_eaten += p.food; // the way out's (`foodEaten`, as the mod counts it at dispatch)
            if p.provisions <= 0.0 {
                *promised.entry(route.to_id).or_insert(0.0) += p.food;
            }
        }
        // Road food (what it lacks of it, and the team's fodder, bought on the way) and the wagon's purse go out of the
        // treasury as one sum.
        let mut purse = 0.0;
        post_legs(&mut ledger, &mut cash, Side::treasury(route.from_id, &mut money.0), vec![
            (Side::outside(), road_money, Reason::RoadFood),
            (Side::wagon(route.seq, &mut purse), wagon_purse, Reason::WagonPurse),
        ], now, content.day_ticks);
        // The crew go away from home (`squad.rs`); a saddle in the warehouse, it rides a third faster.
        let crew = crate::rules::squad::depart(roster.as_deref_mut(), &plan.crew);
        if let Some(w) = waiting.get_mut(&route.from_id) {
            *w -= 1;
        }
        let saddled = crate::rules::squad::saddle(&content, &mut ledger, &mut state, &mut stock.0);
        let arrival = now + crate::squad::leg_ticks(&content, route.travel_ticks, saddled, 1.0);
        state.last_departure = Some(now);
        state.carried += plan.cargo.iter().sum::<f64>();
        let papers = bills.take_to_present(route.from_id, route.to_id, arrival);
        state.wagon = Some(Wagon {
            cargo: plan.cargo,
            purse,
            leg: Leg::Out,
            arrives: arrival,
            contracted: plan.contracted,
            collecting: plan.collecting,
            bills: papers,
            provisions: provision.map_or(0.0, |p| p.provisions),
            shipments: plan.shipments,
            crew,
            porter,
            slots,
            saddled,
            ..Default::default()
        });
        outbox.0.push(Output::WagonLeft { route: route.seq, stacks: plan.stacks, worth: plan.worth });
    }
}

/// The sale load cut to `slots` (`plan_sale` filled the slots the squad sized up at the look's start): the last of what
/// it put aboard first, whole stacks, never below what's `owed`; its worth goes with it.
fn clip(content: &Content, plan: &mut Plan, owed: &[f64], slots: u32) {
    let stacks_of = |plan: &Plan| -> u32 {
        (0..plan.cargo.len()).map(|g| (plan.cargo[g] / content.goods[g].stack as f64).ceil() as u32).sum()
    };
    let sale = std::mem::take(&mut plan.sale);
    for &(g, n, price) in sale.iter().rev() {
        let over = plan.stacks.saturating_sub(slots);
        if over == 0 {
            break;
        }
        let stack = content.goods[g].stack as f64;
        let had = (plan.cargo[g] / stack).ceil() as u32;
        let floor = (owed[g] / stack).ceil() as u32;
        let keep = had.saturating_sub(over).max(floor);
        let kept = plan.cargo[g].min(keep as f64 * stack).max(owed[g]);
        let dropped = plan.cargo[g] - kept;
        plan.cargo[g] = kept;
        plan.worth -= dropped.min(n) * price;
        plan.stacks = stacks_of(plan);
    }
    plan.sale = sale;
}
