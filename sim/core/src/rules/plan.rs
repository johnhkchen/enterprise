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

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        start.in_set(Step::Plan),
        plan_sale.in_set(Step::PlanSale),
        decide.in_set(Step::Decide),
    ));
}

/// An idle route starts a plan; a route with its wagon out has none.
fn start(content: Res<Content>, mut routes: Query<&mut RouteState>) {
    let n = content.goods.len();
    for mut state in &mut routes {
        state.plan = state.wagon.is_none().then(|| Plan { cargo: vec![0.0; n], ..Default::default() });
    }
}

/// For sale, on top of what's planned already: comforts by kind, the cheapest first, up to the far end's par in worth
/// from what home holds beyond its keep; any other good the far end's people use, what home can spare beyond its own
/// shelf up to the room on theirs; all in the wagon's slots. The load is worth what the far end is expected to pay:
/// the bargained price at the middle of each good's batch (`bargain.rs`), and a good it won't deal in stays home.
fn plan_sale(content: Res<Content>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Stock, &Trades, &People)>, rosters: Query<&crate::roster::Roster>) {
    let d = &content.dials;
    let n = content.goods.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let Ok([(home_stock, home_trades, home_people), (far_stock, far_trades, far_people)]) =
            towns.get_many([route.from, route.to]) else { continue };
        let seller = crate::bargain::merchant_level(rosters.get(route.from).ok());
        let buyer = crate::bargain::merchant_level(rosters.get(route.to).ok());
        let mut stacks: u32 = (0..n).map(|g| (plan.cargo[g] / content.goods[g].stack as f64).ceil() as u32).sum();
        let mut loaded_kind = vec![0.0; content.comforts.len()];
        for &g in &order {
            if stacks >= d.wagon_slots {
                break;
            }
            let value = content.goods[g].value.max(1e-9);
            let planned = plan.cargo[g];
            let (spare, room, par) = match content.goods[g].comfort {
                Some(kind) => {
                    let room = comfort_par(&content, far_people) - comfort_held(&content, far_stock, kind) - loaded_kind[kind];
                    let spare = comfort_held(&content, home_stock, kind) - comfort_keep(&content, home_people) - loaded_kind[kind];
                    ((spare / value).min(home_stock.0[g] - planned), room / value, far_stock.0[g] + (room / value).floor())
                }
                None if far_trades.uses[g] > 0.0 => (
                    home_stock.0[g] - planned - shelf(&content, home_trades, g),
                    shelf(&content, far_trades, g) - far_stock.0[g],
                    shelf(&content, far_trades, g),
                ),
                None => continue,
            };
            let stack = content.goods[g].stack as f64;
            let going = spare.min(room).min((d.wagon_slots - stacks) as f64 * stack).floor();
            if going <= 0.0 {
                continue;
            }
            let Some(price) = crate::bargain::unit_price(&content, g, far_stock.0[g], going, par, seller, buyer) else {
                continue;
            };
            plan.cargo[g] += going;
            stacks += (going / stack).ceil() as u32;
            plan.worth += going * price;
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
fn decide(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut outbox: ResMut<Outbox>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>, rosters: Query<&crate::roster::Roster>) {
    let now = clock.now;
    let d = &content.dials;
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
        if food_mode && !crate::road::in_range(&content, route.blocks) {
            state.stayed = Some(Stay::OutOfRange);
            continue;
        }
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
        plan.stacks = stacks;
        let full = plan.stacks as f64 >= d.wagon_full * d.wagon_slots as f64;
        let on_its_day = state.last_departure.map_or(true, |t| now - t >= route.every_ticks);
        // The road's cost: its food where content has food (eaten from home, bought on the way, a purse for the road
        // home's), else its money; a load must be worth both legs' food, and home must pay for the road and the purse.
        let keep_wheat = food_mode && crate::road::keep_wheat(&content, trades, rosters.get(route.from).ok());
        let provision = food_mode.then(|| {
            let after: Vec<f64> = stock.0.iter().zip(&plan.cargo).map(|(s, c)| s - c).collect();
            let home_free = free(&content, trades, &money, &bills, route.from_id, now);
            provision(&content, route.blocks, crate::road::mouths(None), crate::road::held(&content, &after), far_food,
                home_free, |feed| crate::road::fodder(&content, &after, feed, keep_wheat).2)
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
        let arrival = now + route.travel_ticks;
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
            ..Default::default()
        });
        outbox.0.push(Output::WagonLeft { route: route.seq, stacks: plan.stacks, worth: plan.worth });
    }
}
