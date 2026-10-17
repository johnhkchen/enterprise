//! The trade loop's rules, as the mod plays them (see `docs/decisions.md`): towns brought up to date when touched,
//! contracts judged once a day, a money manager's daily budget, wagons that go when something owed can't wait, when
//! full, or on their day, carrying what's owed under contract first and selling the rest within the buyer's means.
//! Each look runs the systems in order: towns caught up, contracts judged, wagons arrived, routes that might go.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SingleThreadedExecutor;

use crate::bills::{self, BillState, Bills};
use crate::content::Content;
use crate::contracts::{self, Book, Incoterm};
use crate::messages::Output;
use crate::state::*;

/// One look's systems, in order, on one thread: the same world always comes out the same.
pub fn look_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.add_systems((catch_up, review, arrivals, departures).chain());
    schedule
}

/// A town brought up to date: its people make and use goods for the time since, paying for what they use
/// (computed on touch, never per tick).
pub fn catch_up_town(now: u64, content: &Content, ledger: &mut Ledger, stock: &mut Stock, money: &mut Money,
        trades: &Trades, touched: &mut Touched) {
    let days = now.saturating_sub(touched.0) as f64 / content.day_ticks as f64;
    touched.0 = now;
    if days <= 0.0 {
        return;
    }
    for g in 0..stock.0.len() {
        let made = trades.makes[g] * days;
        stock.0[g] += made;
        ledger.made[g] += made;
        let used = (trades.uses[g] * days).min(stock.0[g]);
        stock.0[g] -= used;
        ledger.used[g] += used;
        let paid = used * content.goods[g].value;
        money.0 += paid;
        ledger.income += paid;
    }
}

fn catch_up(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>,
        mut towns: Query<(&mut Stock, &mut Money, &Trades, &mut Touched)>) {
    for (mut stock, mut money, trades, mut touched) in &mut towns {
        catch_up_town(clock.now, &content, &mut ledger, &mut stock, &mut money, trades, &mut touched);
    }
}

/// Once a day: installments whose grace has run out are judged.
fn review(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, towns: Res<Towns>, stocks: Query<&Stock>) {
    let day = content.day_ticks;
    if book.reviewed == Some(clock.now / day) {
        return;
    }
    let holds = |c: &contracts::Contract| stocks.get(towns.0[c.seller]).map_or(false, |s| s.0[c.good] >= c.quantity);
    book.review(clock.now, day, holds);
}

/// What a town's warehouse aims to hold of a good: `shelf_days` of what its people use.
pub fn shelf(content: &Content, trades: &Trades, good: usize) -> f64 {
    content.dials.shelf_days * trades.uses[good]
}

/// The town's trading capital: `reserve_days` of what its people use, at its worth. Spend it on anything else and it
/// couldn't buy the goods that earn its money.
pub fn trading_capital(content: &Content, trades: &Trades) -> f64 {
    trades.uses.iter().zip(&content.goods).map(|(u, g)| u * g.value).sum::<f64>() * content.dials.reserve_days
}

/// The town's free money: what it holds beyond its trading capital, less the bills on it falling due soon.
pub fn free(content: &Content, trades: &Trades, money: &Money, bills: &Bills, town: usize, now: u64) -> f64 {
    let ahead = now + (content.dials.budget_days * content.day_ticks as f64) as u64;
    (money.0 - trading_capital(content, trades) - bills.due_by(town, ahead)).max(0.0)
}

/// What's left of a town's day's budget for a seller who last came `days` ago: what the daily looks it missed would
/// have allowed, one after another (`1 - (1 - share)^days` of the free money), less what's spent today. The budget is
/// set at the day's first look, from the free money then.
pub fn left_for(content: &Content, now: u64, free: f64, budget: &mut Budget, days: f64) -> f64 {
    let share = content.dials.budget_share;
    let today = now / content.day_ticks;
    if !budget.set || budget.day != today {
        *budget = Budget { day: today, budget: share * free.max(0.0), spent: 0.0, set: true };
    }
    let allowed = if days <= 1.0 || share <= 0.0 || share >= 1.0 {
        budget.budget
    } else {
        budget.budget / share * (1.0 - (1.0 - share).powf(days))
    };
    (allowed - budget.spent).max(0.0)
}

/// Routes in their order, so every look takes them the same way.
fn in_order(routes: &Query<(Entity, &Route, &mut RouteState)>) -> Vec<Entity> {
    let mut order: Vec<(usize, Entity)> = routes.iter().map(|(e, r, _)| (r.seq, e)).collect();
    order.sort();
    order.into_iter().map(|(_, e)| e).collect()
}

/// Wagons that have got where they were going. At the far end: goods owed under contract delivered first, paid at
/// the locked price (what the buyer can't pay it owes); what the buyer owes from before, paid; goods bought Ex Works
/// collected; the rest sold up to the room on the buyer's shelf, as far as its budget goes; and a good that sold well
/// drafted as a standing order, its copy riding home. Home: what didn't sell goes back on the shelves, the purse into
/// the town's money, and the copies to the town (in force once both hold one).
#[allow(clippy::too_many_arguments)]
fn arrivals(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut outbox: ResMut<Outbox>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Stock, &mut Money, &Trades, &mut Budget)>) {
    let now = clock.now;
    let day = content.day_ticks;
    for entity in in_order(&routes) {
        let (_, route, mut state) = routes.get_mut(entity).expect("a route");
        let route = *route;
        let Some(leg) = state.wagon.as_ref().filter(|w| w.arrives <= now).map(|w| w.leg) else { continue };
        if leg == Leg::Home {
            let wagon = state.wagon.take().unwrap();
            let (mut stock, mut money, _, _) = towns.get_mut(route.from).expect("home");
            for (g, n) in wagon.cargo.iter().enumerate() {
                stock.0[g] += n;
            }
            money.0 += wagon.purse;
            for id in &wagon.drafts {
                book.deliver_copy(*id, route.from_id);
            }
            bills.home(&wagon.bills, route.from_id);
            state.trips += 1;
            outbox.0.push(Output::WagonHome { route: route.seq, takings: wagon.purse });
            continue;
        }
        let (mut stock, mut money, trades, mut budget) = towns.get_mut(route.to).expect("the far end");
        let mut wagon = state.wagon.take().unwrap();
        // Bills the wagon carries on this town, presented: it pays the due ones it can, whole.
        let carried_bills = std::mem::take(&mut wagon.bills);
        for id in carried_bills {
            let b = &mut bills.bills[id];
            if b.drawee != route.to_id || b.state == BillState::Paid {
                wagon.bills.push(id);
                continue;
            }
            if b.due <= now && money.0 >= b.amount {
                money.0 -= b.amount;
                wagon.purse += b.amount;
                if b.state == BillState::Outstanding {
                    book.kept(b.drawee, b.payee, bills::HONOURED_WEIGHT, now, day);
                }
                b.state = BillState::Paid;
                continue;
            }
            // Its grace runs from the day it's first put to it.
            let presented = *b.presented.get_or_insert(now);
            if b.state == BillState::Outstanding && now > b.due.max(presented) + Bills::grace(b, day) {
                b.state = BillState::Dishonoured;
                book.missed(b.drawee, b.payee, 1.0, now, day);
            }
            wagon.bills.push(id);
        }
        bills.tidy();
        // Under contract first: delivered whatever its shelf holds, at the locked price. What it can't pay in coin it
        // pays with a bill on itself, if they trust each other; else it owes it.
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
        // What it owes from before, as far as it can now.
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
        // Ex Works: what it collects here, paid from its purse.
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
        // Then for sale, within the buyer's budget for a seller away so long.
        let days = state.last_sold.map_or(1.0, |t| ((now - t) as f64 / day as f64).clamp(1.0, content.dials.max_budget_days));
        let (mut sold_all, mut takings) = (0.0, 0.0);
        let mut sold_each = vec![0.0; wagon.cargo.len()];
        for g in 0..wagon.cargo.len() {
            if wagon.cargo[g] <= 0.0 {
                continue;
            }
            let price = content.goods[g].value;
            let room = (shelf(&content, trades, g) - stock.0[g]).max(0.0);
            let free_money = free(&content, trades, &money, &bills, route.to_id, now);
            let can = left_for(&content, now, free_money, &mut budget, days).min(money.0.max(0.0)); // never beyond what it holds
            let n = wagon.cargo[g].min(room).min((can / price).floor()).max(0.0);
            if n <= 0.0 {
                continue;
            }
            wagon.cargo[g] -= n;
            stock.0[g] += n;
            let paid = n * price;
            money.0 -= paid;
            budget.spent += paid;
            wagon.purse += paid;
            sold_each[g] = n;
            sold_all += n;
            takings += paid;
        }
        // A good that sold well, the buyer's people using it steadily: a standing order, if they trust each other.
        let every = day.max(2 * route.travel_ticks);
        if content.dials.standing_orders && book.trust(route.from_id, route.to_id, now, day) >= contracts::MIN_TRUST {
            for (g, sold) in sold_each.iter().enumerate() {
                let used = trades.uses[g] * every as f64 / day as f64;
                let quantity = (sold.min((used / 16.0).round() * 16.0) / 16.0).floor() * 16.0;
                if quantity < contracts::CONTRACT_MIN_ITEMS || book.covers(route.from_id, route.to_id, g) {
                    continue;
                }
                let id = book.draft(route.from_id, route.to_id, g, quantity, content.goods[g].value, now + every,
                    every, contracts::CONTRACT_INSTALLMENTS, Incoterm::Ddp, false, false);
                wagon.drafts.push(id);
            }
        }
        wagon.leg = Leg::Home;
        wagon.arrives = now + route.travel_ticks;
        state.last_sold = Some(now);
        state.sold += sold_all;
        state.wagon = Some(wagon);
        outbox.0.push(Output::WagonSold { route: route.seq, sold: sold_all, takings });
    }
}

/// Idle routes. A wagon goes when something owed can't wait a day (an installment it carries, or one it collects Ex
/// Works, would be judged before it counts), when full, or on its day with a load worth its road food; if the town
/// can pay for the road (and what it collects). Its load: what's owed under contract first, then of each good the
/// far end's people use, what home can spare beyond its own shelf, up to the room on the far end's shelf, in the
/// wagon's slots. What's owed and not pressing rides along, and counts toward the load.
#[allow(clippy::too_many_arguments)]
fn departures(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>,
        mut outbox: ResMut<Outbox>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Stock, &mut Money, &Trades, &mut Budget)>) {
    let now = clock.now;
    let d = &content.dials;
    let day = content.day_ticks;
    let grace = (contracts::GRACE_DAYS * day as f64) as u64;
    for entity in in_order(&routes) {
        let (_, route, mut state) = routes.get_mut(entity).expect("a route");
        if state.wagon.is_some() {
            continue;
        }
        let route = *route;
        let Ok([(mut home_stock, mut home_money, home_trades, _), (far_stock, _, far_trades, _)]) =
            towns.get_many_mut([route.from, route.to]) else { continue };
        let arrival = now + route.travel_ticks;
        let n = content.goods.len();
        let mut cargo = vec![0.0; n];
        let mut contracted = Vec::new();
        let (mut stacks, mut worth, mut pressing) = (0u32, 0.0, false);
        let mut taken = vec![0.0; n]; // of home's stock, so far
        // Owed under contract, the seller carrying.
        for c in book.in_force(route.from_id, route.to_id).filter(|c| c.incoterm.seller_carries()) {
            let owed = (c.owed(arrival) - c.shipped).max(0.0);
            let going = owed.min(home_stock.0[c.good] - taken[c.good]).floor();
            if going <= 0.0 {
                continue;
            }
            taken[c.good] += going;
            contracted.push((c.id, going));
            worth += going * c.price;
            let counts = if c.incoterm.seller_bears_road() { arrival + day } else { now + day };
            pressing |= c.next_deadline(grace).is_some_and(|deadline| counts > deadline);
        }
        // To collect Ex Works there, with the money for it.
        let mut collecting = Vec::new();
        let mut purse = 0.0;
        for c in book.in_force(route.to_id, route.from_id).filter(|c| !c.incoterm.seller_carries()) {
            let owed = c.owed(arrival);
            if owed <= 0.0 {
                continue;
            }
            collecting.push(c.id);
            purse += owed * c.price;
            worth += owed * c.price;
            pressing |= c.next_deadline(grace).is_some_and(|deadline| arrival + day > deadline);
        }
        // For sale: what home can spare beyond its own shelf, up to the room on theirs.
        for g in 0..n {
            if taken[g] > 0.0 {
                cargo[g] += taken[g];
                stacks += (taken[g] / content.goods[g].stack as f64).ceil() as u32;
            }
        }
        for g in 0..n {
            if far_trades.uses[g] <= 0.0 || stacks >= d.wagon_slots {
                continue;
            }
            let spare = home_stock.0[g] - taken[g] - shelf(&content, home_trades, g);
            let room = shelf(&content, far_trades, g) - far_stock.0[g];
            let stack = content.goods[g].stack as f64;
            let going = spare.min(room).min((d.wagon_slots - stacks) as f64 * stack).floor();
            if going <= 0.0 {
                continue;
            }
            cargo[g] += going;
            stacks += (going / stack).ceil() as u32;
            worth += going * content.goods[g].value;
        }
        // Bills it holds on the far end, due by the time it's there: to present (collecting them is worth the trip).
        worth += bills.open.iter().map(|&id| &bills.bills[id])
            .filter(|b| b.held == bills::Held::Town(route.from_id) && b.payee == route.from_id && b.drawee == route.to_id
                && b.due <= arrival)
            .map(|b| b.amount).sum::<f64>();
        let full = stacks as f64 >= d.wagon_full * d.wagon_slots as f64;
        let on_its_day = state.last_departure.map_or(true, |t| now - t >= route.every_ticks);
        let pays = worth >= route.road_cost;
        if !(pressing || (full || on_its_day) && pays) || home_money.0 < route.road_cost + purse {
            continue;
        }
        for (g, n) in cargo.iter().enumerate() {
            home_stock.0[g] -= n;
        }
        for &(id, n) in &contracted {
            book.ship(id, n);
        }
        home_money.0 -= route.road_cost + purse;
        ledger.road += route.road_cost;
        state.last_departure = Some(now);
        state.carried += cargo.iter().sum::<f64>();
        let papers = bills.take_to_present(route.from_id, route.to_id, arrival);
        state.wagon = Some(Wagon { cargo, purse, leg: Leg::Out, arrives: arrival, contracted, collecting,
            drafts: Vec::new(), bills: papers });
        outbox.0.push(Output::WagonLeft { route: route.seq, stacks, worth });
    }
}
