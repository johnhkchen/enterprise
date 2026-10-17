//! The trade loop's rules, as the mod plays them (see `docs/decisions.md`): towns brought up to date when touched,
//! a money manager's daily budget, wagons on a timetable or full, sold within the buyer's means. Each look runs the
//! systems in order: towns caught up, wagons that have arrived, routes that might go.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SingleThreadedExecutor;

use crate::content::Content;
use crate::messages::Output;
use crate::state::*;

/// One look's systems, in order, on one thread: the same world always comes out the same.
pub fn look_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.add_systems((catch_up, arrivals, departures).chain());
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

/// What a town's warehouse aims to hold of a good: `shelf_days` of what its people use.
pub fn shelf(content: &Content, trades: &Trades, good: usize) -> f64 {
    content.dials.shelf_days * trades.uses[good]
}

/// What's left of a town's day's budget for a seller who last came `days` ago: what the daily looks it missed would
/// have allowed, one after another (`1 - (1 - share)^days` of the free money), less what's spent today.
pub fn left_for(content: &Content, now: u64, money: &Money, budget: &mut Budget, days: f64) -> f64 {
    let share = content.dials.budget_share;
    let today = now / content.day_ticks;
    if !budget.set || budget.day != today {
        *budget = Budget { day: today, budget: share * money.0.max(0.0), spent: 0.0, set: true };
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

/// Wagons that have got where they were going: at the far end they sell each good up to the room on the buyer's
/// shelf, as far as the buyer's budget for them goes, and turn for home with the rest; home, what didn't sell goes
/// back on the shelves and the takings into the town's money.
fn arrivals(clock: Res<Clock>, content: Res<Content>, mut outbox: ResMut<Outbox>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Stock, &mut Money, &Trades, &mut Budget)>) {
    let now = clock.now;
    for entity in in_order(&routes) {
        let (_, route, mut state) = routes.get_mut(entity).expect("a route");
        let seq = route.seq;
        let Some(wagon) = state.wagon.as_mut().filter(|w| w.arrives <= now) else { continue };
        match wagon.leg {
            Leg::Out => {
                let (mut stock, mut money, trades, mut budget) = towns.get_mut(route.to).expect("the far end");
                let days = state.last_sold.map_or(1.0, |t| {
                    ((now - t) as f64 / content.day_ticks as f64).clamp(1.0, content.dials.max_budget_days)
                });
                let wagon = state.wagon.as_mut().unwrap();
                let (mut sold, mut takings) = (0.0, 0.0);
                for g in 0..wagon.cargo.len() {
                    if wagon.cargo[g] <= 0.0 {
                        continue;
                    }
                    let price = content.goods[g].value;
                    let room = (shelf(&content, trades, g) - stock.0[g]).max(0.0);
                    let can = left_for(&content, now, &money, &mut budget, days);
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
                    sold += n;
                    takings += paid;
                }
                wagon.leg = Leg::Home;
                wagon.arrives = now + route.travel_ticks;
                state.last_sold = Some(now);
                state.sold += sold;
                outbox.0.push(Output::WagonSold { route: seq, sold, takings });
            }
            Leg::Home => {
                let wagon = state.wagon.take().unwrap();
                let (mut stock, mut money, _, _) = towns.get_mut(route.from).expect("home");
                for (g, n) in wagon.cargo.iter().enumerate() {
                    stock.0[g] += n;
                }
                money.0 += wagon.purse;
                state.trips += 1;
                outbox.0.push(Output::WagonHome { route: seq, takings: wagon.purse });
            }
        }
    }
}

/// Idle routes: a wagon goes when full, or on its day, with a load worth its road food, if the town can pay for the
/// road. Its load: of each good the far end's people use, what home can spare beyond its own shelf, up to the room
/// on the far end's shelf, in the wagon's slots.
fn departures(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut outbox: ResMut<Outbox>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Stock, &mut Money, &Trades, &mut Budget)>) {
    let now = clock.now;
    let d = &content.dials;
    for entity in in_order(&routes) {
        let (_, route, mut state) = routes.get_mut(entity).expect("a route");
        if state.wagon.is_some() {
            continue;
        }
        let Ok([(mut home_stock, mut home_money, home_trades, _), (far_stock, _, far_trades, _)]) =
            towns.get_many_mut([route.from, route.to]) else { continue };
        let mut cargo = vec![0.0; content.goods.len()];
        let (mut stacks, mut worth) = (0u32, 0.0);
        for g in 0..cargo.len() {
            if far_trades.uses[g] <= 0.0 || stacks >= d.wagon_slots {
                continue;
            }
            let spare = home_stock.0[g] - shelf(&content, home_trades, g);
            let room = shelf(&content, far_trades, g) - far_stock.0[g];
            let stack = content.goods[g].stack as f64;
            let going = spare.min(room).min((d.wagon_slots - stacks) as f64 * stack).floor();
            if going <= 0.0 {
                continue;
            }
            cargo[g] = going;
            stacks += (going / stack).ceil() as u32;
            worth += going * content.goods[g].value;
        }
        let full = stacks as f64 >= d.wagon_full * d.wagon_slots as f64;
        let on_its_day = state.last_departure.map_or(true, |t| now - t >= route.every_ticks);
        if !(full || on_its_day) || worth < route.road_cost || home_money.0 < route.road_cost {
            continue;
        }
        for (g, n) in cargo.iter().enumerate() {
            home_stock.0[g] -= n;
        }
        home_money.0 -= route.road_cost;
        ledger.road += route.road_cost;
        state.last_departure = Some(now);
        state.carried += cargo.iter().sum::<f64>();
        state.wagon = Some(Wagon { cargo, purse: 0.0, leg: Leg::Out, arrives: now + route.travel_ticks });
        outbox.0.push(Output::WagonLeft { route: route.seq, stacks, worth });
    }
}
