//! The rules, as the mod plays them (see `docs/decisions.md`), run as one look's systems in fixed steps, on one
//! thread, so the same world always comes out the same:
//!
//! 1. **Touch:** towns brought up to date (their people make and use goods, live, grow).
//! 2. **Judge:** contracts' installments and bills judged once a day.
//! 3. **Arrive:** wagons that got where they were going. At the far end, in turn: `Papers` (bills presented),
//!    `Settle` (goods owed delivered), `Collect` (Ex Works), `Freight`, `Sell`, `Buy` (for home), `Draft` (standing
//!    orders), `Services`; then `Turn` (home again, or unloaded at home).
//! 4. **Plan:** each idle route's plan for a wagon, built up in turn: `PlanOwed`, `PlanCollect`, `PlanPapers`,
//!    `PlanFreight`, `PlanSale`, `PlanBuy`; then `Decide` (it goes, or waits).
//!
//! A module joins a step with its own systems in its own file (`register`), so slices don't edit each other's.

pub mod arrive;
pub mod bills;
pub mod contracts;
pub mod plan;
pub mod touch;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SingleThreadedExecutor;

use crate::bills::Bills;
use crate::content::Content;
use crate::state::*;

/// The steps of a look, in order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    Touch,
    Judge,
    Arrive,
    Papers,
    Settle,
    Collect,
    Freight,
    Sell,
    Buy,
    Draft,
    Services,
    Turn,
    Plan,
    PlanOwed,
    PlanCollect,
    PlanPapers,
    PlanFreight,
    PlanSale,
    PlanBuy,
    Decide,
}

/// One look's systems, every module's, in their steps.
pub fn look_schedule() -> Schedule {
    use Step::*;
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor::new());
    schedule.configure_sets(
        (Touch, Judge, Arrive, Papers, Settle, Collect, Freight, Sell, Buy, Draft, Services, Turn, Plan, PlanOwed,
            PlanCollect, PlanPapers, PlanFreight, PlanSale, PlanBuy, Decide)
            .chain(),
    );
    touch::register(&mut schedule);
    crate::people::register(&mut schedule);
    contracts::register(&mut schedule);
    bills::register(&mut schedule);
    arrive::register(&mut schedule);
    plan::register(&mut schedule);
    schedule
}

/// A town's components, as the arriving and planning steps read and change them.
pub type TownData<'a> = (&'a mut Stock, &'a mut Money, &'a Trades, &'a mut Budget, &'a People);

/// The routes' entities in their order, so every step takes them the same way.
pub fn in_order<'a>(routes: impl Iterator<Item = (Entity, &'a Route)>) -> Vec<Entity> {
    let mut order: Vec<(usize, Entity)> = routes.map(|(e, r)| (r.seq, e)).collect();
    order.sort();
    order.into_iter().map(|(_, e)| e).collect()
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
