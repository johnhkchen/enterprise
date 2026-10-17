//! The contracts' steps: installments judged once a day (`Judge`); at the far end, goods owed delivered at the
//! locked price and old debts paid (`Settle`), and goods bought Ex Works collected (`Collect`); and in a route's
//! plan, what it owes (`PlanOwed`) and what it would collect (`PlanCollect`), with the deadlines that can't wait.

use bevy_ecs::prelude::*;

use super::{in_order, Step, TownData};
use crate::bills::Bills;
use crate::content::Content;
use crate::contracts::{self, Book, Contract};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        review.in_set(Step::Judge),
        settle.in_set(Step::Settle),
        collect.in_set(Step::Collect),
        plan_owed.in_set(Step::PlanOwed),
        plan_collect.in_set(Step::PlanCollect),
    ));
}

/// Once a day: installments whose grace has run out are judged.
fn review(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, towns: Res<Towns>, stocks: Query<&Stock>) {
    let day = content.day_ticks;
    if book.reviewed == Some(clock.now / day) {
        return;
    }
    let holds = |c: &Contract| stocks.get(towns.0[c.seller]).map_or(false, |s| s.0[c.good] >= c.quantity);
    book.review(clock.now, day, holds);
}

/// Goods owed under contract delivered first, whatever the buyer's shelf holds, at the locked price; what it can't
/// pay in coin it pays with a bill on itself if they trust each other, else owes. Then what it owes from before, as
/// far as it can.
fn settle(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<TownData>) {
    let now = clock.now;
    let day = content.day_ticks;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (mut stock, mut money, _, _, _) = towns.get_mut(route.to).expect("the far end");
        let wagon = state.wagon.as_mut().unwrap();
        for (id, carried) in std::mem::take(&mut wagon.contracted) {
            let c = &book.contracts[id];
            let n = carried.min(c.owed(now)).min(wagon.cargo[c.good]);
            let worth = n * c.price;
            let paid = worth.min(money.0.max(0.0));
            wagon.cargo[c.good] -= n;
            stock.0[c.good] += n;
            money.0 -= paid;
            wagon.purse += paid;
            let short = worth - paid;
            let by_bill = short > 1e-9 && book.trust(route.from_id, route.to_id, now, day) >= contracts::MIN_TRUST;
            if by_bill {
                wagon.bills.push(bills.draw(route.to_id, route.from_id, short, now, day));
            }
            book.settle(id, carried, n, paid + if by_bill { short } else { 0.0 }, if by_bill { 0.0 } else { short });
        }
        for id in book.open_between(route.from_id, route.to_id) {
            let c = &mut book.contracts[id];
            if c.owing <= 0.0 {
                continue;
            }
            let paid = c.owing.min(money.0.max(0.0));
            c.owing -= paid;
            c.paid += paid;
            money.0 -= paid;
            wagon.purse += paid;
        }
    }
}

/// Ex Works: what the buyer's wagon collects at the seller's, paid from the purse it brought.
fn collect(clock: Res<Clock>, mut book: ResMut<Book>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>) {
    let now = clock.now;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (mut stock, mut money, _, _, _) = towns.get_mut(route.to).expect("the seller");
        let wagon = state.wagon.as_mut().unwrap();
        for id in std::mem::take(&mut wagon.collecting) {
            let c = &book.contracts[id];
            let n = c.owed(now).min(stock.0[c.good]).min((wagon.purse / c.price.max(1e-9)).floor());
            if n <= 0.0 {
                continue;
            }
            let paid = n * c.price;
            stock.0[c.good] -= n;
            wagon.cargo[c.good] += n;
            money.0 += paid;
            wagon.purse -= paid;
            book.settle(id, 0.0, n, paid, 0.0);
        }
    }
}

/// In the plan: what's owed under contract the seller carries, by the time the wagon would arrive, as far as home
/// has it. Pressing if an installment would be judged before the goods count, were the wagon to wait a day.
fn plan_owed(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<&Stock>) {
    let now = clock.now;
    let day = content.day_ticks;
    let grace = (contracts::GRACE_DAYS * day as f64) as u64;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let home = towns.get(route.from).expect("home");
        let arrival = now + route.travel_ticks;
        for c in book.in_force(route.from_id, route.to_id).filter(|c| c.incoterm.seller_carries()) {
            let owed = (c.owed(arrival) - c.shipped).max(0.0);
            let going = owed.min(home.0[c.good] - plan.cargo[c.good]).floor();
            if going <= 0.0 {
                continue;
            }
            plan.cargo[c.good] += going;
            plan.contracted.push((c.id, going));
            plan.worth += going * c.price;
            let counts = if c.incoterm.seller_bears_road() { arrival + day } else { now + day };
            plan.pressing |= c.next_deadline(grace).is_some_and(|deadline| counts > deadline);
        }
    }
}

/// In the plan: what the town buys at the far end Ex Works, owed by the time it would arrive, with the money for it.
/// Pressing if waiting a day would miss an installment.
fn plan_collect(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>) {
    let now = clock.now;
    let day = content.day_ticks;
    let grace = (contracts::GRACE_DAYS * day as f64) as u64;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let arrival = now + route.travel_ticks;
        for c in book.in_force(route.to_id, route.from_id).filter(|c| !c.incoterm.seller_carries()) {
            let owed = c.owed(arrival);
            if owed <= 0.0 {
                continue;
            }
            plan.collecting.push(c.id);
            plan.purse += owed * c.price;
            plan.worth += owed * c.price;
            plan.pressing |= c.next_deadline(grace).is_some_and(|deadline| arrival + day > deadline);
        }
    }
}
