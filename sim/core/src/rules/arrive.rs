//! Arrive: wagons that got where they were going this look are marked (`Arrive`); at the far end they sell (`Sell`)
//! and draft standing orders (`Draft`), with the papers, contracts and collections between (their own modules);
//! then each turns for home, or is unloaded at home (`Turn`).

use bevy_ecs::prelude::*;

use super::{free, in_order, left_for, shelf, Step, TownData};
use crate::bills::Bills;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::{self, Book, Incoterm};
use crate::messages::Output;
use crate::people::{comfort_held, comfort_par};
use crate::realms::Realm;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        mark.in_set(Step::Arrive),
        sell.in_set(Step::Sell),
        draft.in_set(Step::Draft),
        turn.in_set(Step::Turn),
    ));
}

/// The wagons that have got where they were going: the leg each finished, for the arriving steps.
pub fn mark(clock: Res<Clock>, mut routes: Query<&mut RouteState>) {
    for mut state in &mut routes {
        // A wagon waiting at the far end for its road food has done its business there (`road.rs`).
        let arrived = state.wagon.as_ref().filter(|w| w.arrives <= clock.now && w.waiting_since.is_none()).map(|w| w.leg);
        state.arrived = arrived;
        if arrived == Some(Leg::Out) {
            let w = state.wagon.as_mut().unwrap();
            w.sold_here = vec![0.0; w.cargo.len()];
            w.takings_here = 0.0;
            w.paid_here = vec![0.0; w.cargo.len()]; // each good's takings, for the books (`atlas.rs`)
        }
    }
}

/// At the far end, what's left for sale sold up to the room on the buyer's shelf (a comfort, up to the kind's par
/// in worth), within its budget for a seller away so long, and never beyond what it holds. Each good at the price
/// the two Merchants settle at the middle of the batch (`bargain.rs`; `Caravans.sell`), priced again once what the
/// buyer can pay has cut the batch; no deal, no sale.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sell(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, mut ledger: ResMut<Ledger>,
        mut book: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>, rosters: Query<&crate::roster::Roster>, realms: Query<&Realm>) {
    let now = clock.now;
    let day = content.day_ticks;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let days = state.last_sold.map_or(1.0, |t| ((now - t) as f64 / day as f64).clamp(1.0, content.dials.max_budget_days));
        // The travelling Merchant is its home's Merchant; the buyer, the far end's.
        // (A realm's are at its own fixed level, and a realm buys by par with the money it holds: `realms.rs`.)
        let seller = crate::realms::level(&content, realms.get(route.from).ok(), rosters.get(route.from).ok());
        let buyer = crate::realms::level(&content, realms.get(route.to).ok(), rosters.get(route.to).ok());
        let realm_buys = realms.get(route.to).is_ok();
        let (mut stock, mut money, trades, mut budget, people) = towns.get_mut(route.to).expect("the far end");
        let wagon = state.wagon.as_mut().unwrap();
        let mut sold_all = 0.0;
        for g in 0..wagon.cargo.len() {
            if wagon.cargo[g] <= 0.0 {
                continue;
            }
            let value = content.goods[g].value;
            let room = match content.goods[g].comfort.filter(|_| !realm_buys) {
                Some(kind) => ((comfort_par(&content, people) - comfort_held(&content, &stock, kind)) / value).floor(),
                None => shelf(&content, trades, g) - stock.0[g],
            }
            .max(0.0);
            let par = stock.0[g] + room;
            let free_money = free(&content, trades, &money, &bills, route.to_id, now);
            let can = if realm_buys { money.0.max(0.0) } else { left_for(&content, now, free_money, &mut budget, days).min(money.0.max(0.0)) };
            let mut n = wagon.cargo[g].min(room);
            let mut price = value;
            for _ in 0..2 {
                if n <= 0.0 {
                    break;
                }
                let each = crate::bargain::unit_price(&content, g, stock.0[g], n, par, seller, buyer);
                price = each.unwrap_or(0.0);
                n = n.min(each.map_or(0.0, |each| (can / each).floor()));
            }
            let n = n.max(0.0);
            if n <= 0.0 {
                continue;
            }
            wagon.cargo[g] -= n;
            stock.0[g] += n;
            let paid = n * price;
            post(&mut ledger, &mut book, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                paid, Reason::Sale, now, day);
            budget.spent += paid;
            wagon.sold_here[g] += n;
            wagon.takings_here += paid;
            crate::atlas::paid(wagon, g, paid);
            sold_all += n;
        }
        state.sold += sold_all;
    }
}

/// A good that sold well here, the buyer's people using it steadily: a standing order, if they trust each other (in
/// sixteens, ten installments, every round trip). The buyer holds the copy; the seller's rides home with the wagon.
pub(crate) fn draft(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, towns: Query<&Trades>) {
    if !content.dials.standing_orders {
        return;
    }
    let now = clock.now;
    let day = content.day_ticks;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) || book.trust(route.from_id, route.to_id, now, day) < contracts::MIN_TRUST {
            continue;
        }
        let route = *route;
        let trades = towns.get(route.to).expect("the far end");
        let every = day.max(2 * route.travel_ticks);
        let wagon = state.wagon.as_mut().unwrap();
        for g in 0..wagon.sold_here.len() {
            let used = trades.uses[g] * every as f64 / day as f64;
            let quantity = (wagon.sold_here[g].min((used / 16.0).round() * 16.0) / 16.0).floor() * 16.0;
            if quantity < contracts::CONTRACT_MIN_ITEMS || book.covers(route.from_id, route.to_id, g) {
                continue;
            }
            let id = book.draft(route.from_id, route.to_id, g, quantity, content.goods[g].value, now + every, every,
                contracts::CONTRACT_INSTALLMENTS, Incoterm::Ddp, false, false);
            wagon.drafts.push(id);
        }
    }
}

/// At the far end, the wagon turns for home (where content has road food, once fed: `rules/road.rs`). At home, what didn't sell goes back on the shelves, the purse into the
/// town's money, the copies to the town (in force once both hold one) and the bills into its keeping.
#[allow(clippy::too_many_arguments)]
pub fn turn(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut book: ResMut<Book>, mut bills: ResMut<Bills>, mut outbox: ResMut<Outbox>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<TownData>) {
    let now = clock.now;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        match state.arrived.take() {
            Some(Leg::Out) => {
                state.last_sold = Some(now);
                let w = state.wagon.as_mut().unwrap();
                // Where content has road food, its food turned it already, or it waits for food (`rules/road.rs`).
                if w.leg == Leg::Out && w.waiting_since.is_none() {
                    w.leg = Leg::Home;
                    w.arrives = now + crate::squad::leg_ticks(&content, route.travel_ticks, w.saddled, 1.0); // saddled: faster
                }
                let sold = w.sold_here.iter().sum();
                outbox.0.push(Output::WagonSold { route: route.seq, sold, takings: w.takings_here });
            }
            Some(Leg::Home) => {
                let mut wagon = state.wagon.take().unwrap();
                let (mut stock, mut money, _, _, _) = towns.get_mut(route.from).expect("home");
                for (g, n) in wagon.cargo.iter().enumerate() {
                    stock.0[g] += n;
                }
                let takings = wagon.purse;
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse),
                    Side::treasury(route.from_id, &mut money.0), takings, Reason::Takings, now, content.day_ticks);
                for id in &wagon.drafts {
                    book.deliver_copy(*id, route.from_id);
                }
                bills.home(&wagon.bills, route.from_id);
                state.trips += 1;
                outbox.0.push(Output::WagonHome { route: route.seq, takings });
            }
            None => {}
        }
    }
}
