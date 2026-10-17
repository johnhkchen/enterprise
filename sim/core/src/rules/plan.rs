//! Plan: each idle route starts a plan this look (`Plan`); the modules add to it (owed goods, collections, papers,
//! freight); the sale load fills the rest (`PlanSale`); then the wagon goes or waits (`Decide`).

use bevy_ecs::prelude::*;

use super::{in_order, shelf, Step, TownData};
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
/// shelf up to the room on theirs; all in the wagon's slots.
fn plan_sale(content: Res<Content>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Stock, &Trades, &People)>) {
    let d = &content.dials;
    let n = content.goods.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let Ok([(home_stock, home_trades, home_people), (far_stock, far_trades, far_people)]) =
            towns.get_many([route.from, route.to]) else { continue };
        let mut stacks: u32 = (0..n).map(|g| (plan.cargo[g] / content.goods[g].stack as f64).ceil() as u32).sum();
        let mut loaded_kind = vec![0.0; content.comforts.len()];
        for &g in &order {
            if stacks >= d.wagon_slots {
                break;
            }
            let value = content.goods[g].value.max(1e-9);
            let planned = plan.cargo[g];
            let (spare, room) = match content.goods[g].comfort {
                Some(kind) => {
                    let room = comfort_par(&content, far_people) - comfort_held(&content, far_stock, kind) - loaded_kind[kind];
                    let spare = comfort_held(&content, home_stock, kind) - comfort_keep(&content, home_people) - loaded_kind[kind];
                    ((spare / value).min(home_stock.0[g] - planned), room / value)
                }
                None if far_trades.uses[g] > 0.0 => (
                    home_stock.0[g] - planned - shelf(&content, home_trades, g),
                    shelf(&content, far_trades, g) - far_stock.0[g],
                ),
                None => continue,
            };
            let stack = content.goods[g].stack as f64;
            let going = spare.min(room).min((d.wagon_slots - stacks) as f64 * stack).floor();
            if going <= 0.0 {
                continue;
            }
            plan.cargo[g] += going;
            stacks += (going / stack).ceil() as u32;
            plan.worth += going * content.goods[g].value;
            if let Some(kind) = content.goods[g].comfort {
                loaded_kind[kind] += going * value;
            }
        }
        plan.stacks = stacks;
    }
}

/// The wagon goes when something can't wait, or when full or on its day with a load worth its road food, if the town
/// can pay for the road and what it collects. Leaving, it takes its cargo, the money for its collections and the bills
/// home holds on the far end.
#[allow(clippy::too_many_arguments)]
fn decide(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut outbox: ResMut<Outbox>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>) {
    let now = clock.now;
    let d = &content.dials;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        let Some(mut plan) = state.plan.take() else { continue };
        let (mut stock, mut money, _, _, _) = towns.get_mut(route.from).expect("home");
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
        let pays = plan.worth >= route.road_cost;
        if !(plan.pressing || (full || on_its_day) && pays) || money.0 < route.road_cost + plan.purse {
            continue;
        }
        for (g, n) in plan.cargo.iter().enumerate() {
            stock.0[g] -= n;
        }
        for &(id, n) in &plan.contracted {
            book.ship(id, n);
        }
        // Road food and the wagon's purse go out of the treasury as one sum.
        let mut purse = 0.0;
        post_legs(&mut ledger, &mut cash, Side::treasury(route.from_id, &mut money.0), vec![
            (Side::outside(), route.road_cost, Reason::RoadFood),
            (Side::wagon(route.seq, &mut purse), plan.purse, Reason::WagonPurse),
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
            ..Default::default()
        });
        outbox.0.push(Output::WagonLeft { route: route.seq, stacks: plan.stacks, worth: plan.worth });
    }
}
