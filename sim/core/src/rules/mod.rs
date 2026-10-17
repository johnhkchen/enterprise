//! The rules, as the mod plays them (see `docs/decisions.md`), run as one look's systems in fixed steps, on one
//! thread, so the same world always comes out the same:
//!
//! 1. **Touch:** towns brought up to date (their people make and use goods, live, grow).
//! 2. **Work:** towns' people make goods: farms, herds, mines, gathering, the workshop.
//! 3. **Pay:** wages, guards' upkeep and rents' coupons paid out of a town's money.
//! 4. **Review:** a town's daily decisions: jobs, fields, tolls, buildings, commissions.
//! 5. **Judge:** contracts' installments and bills judged once a day.
//! 6. **Arrive:** wagons that got where they were going. Letters arrive (`Mail`) first. At the far end, in turn:
//!    `Papers` (bills presented), `Settle` (goods owed delivered), `Collect` (Ex Works), `Freight`, `Sell`, `Buy` (for
//!    home), `Draft` (standing orders), `Services`; then `Turn` (home again, or unloaded at home).
//! 7. **Plan:** each idle route's plan for a wagon, built up in turn: `PlanOwed`, `PlanCollect`, `PlanPapers`,
//!    `PlanFreight`, `PlanSale`, `PlanBuy`; then `Decide` (it goes, or waits).
//!
//! A module joins a step with its own systems in its own file (`register`), listed in `slices.rs`, so slices don't edit
//! each other's.

pub mod advances;
pub mod arrive;
pub mod atlas;
pub mod bank;
pub mod bills;
pub mod cargo;
pub mod chokepoints;
pub mod contracts;
pub mod desires;
pub mod farms;
pub mod harvest;
pub mod herds;
pub mod jobs;
pub mod lockers;
pub mod plan;
pub mod realms;
pub mod road;
pub mod roster;
pub mod settlers;
pub mod squad;
pub mod territory;
pub mod touch;
pub mod training;
pub mod workshop;
pub mod works;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SingleThreadedExecutor;

use crate::bills::Bills;
use crate::content::Content;
use crate::state::*;

/// The steps of a look, in order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    Touch,
    Work,
    Pay,
    Review,
    Judge,
    Arrive,
    Mail,
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
    // Two chains, joined at Sell, since a tuple of this length isn't an ordered set.
    schedule.configure_sets((Touch, Work, Pay, Review, Judge, Arrive, Mail, Papers, Settle, Collect, Freight, Sell).chain());
    schedule.configure_sets((Sell, Buy, Draft, Services, Turn, Plan, PlanOwed, PlanCollect, PlanPapers, PlanFreight, PlanSale, PlanBuy, Decide).chain());
    crate::slices::register(&mut schedule);
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

/// What a town's warehouse aims to hold of a good: `shelf_days` of what its lockers can take a day (`Trades.capacity`),
/// or of what its people use where it has no capacity.
pub fn shelf(content: &Content, trades: &Trades, good: usize) -> f64 {
    let capacity = trades.capacity.get(good).copied().unwrap_or(0.0);
    content.dials.shelf_days * if capacity > 0.0 { capacity } else { trades.uses[good] }
}

/// The town's trading capital: `reserve_days` of what its people use, at its worth. Spend it on anything else and it
/// couldn't buy the goods that earn its money.
pub fn trading_capital(content: &Content, trades: &Trades) -> f64 {
    trades.uses.iter().zip(&content.goods).map(|(u, g)| u * g.value).sum::<f64>() * content.dials.reserve_days
}

/// The town's free money: what it holds beyond its trading capital, less the bills on it falling due soon.
pub fn free(content: &Content, trades: &Trades, money: &Money, bills: &Bills, town: usize, now: u64) -> f64 {
    let days = content.dials.budget_days * if trades.counting_house { 2.0 } else { 1.0 };
    let ahead = now + (days * content.day_ticks as f64) as u64;
    (money.0 - trading_capital(content, trades) - bills.due_by(town, ahead)).max(0.0)
}

/// What's left of a town's day's budget for a seller who last came `days` ago: what the daily looks it missed would
/// have allowed, one after another (`1 - (1 - share)^days` of the free money), less what's spent today. The budget is
/// set at the day's first look, from the free money then.
pub fn left_for(content: &Content, now: u64, free: f64, budget: &mut Budget, days: f64) -> f64 {
    let share = if budget.counting_house { (content.dials.budget_share * 1.5).min(1.0) } else { content.dials.budget_share };
    let today = now / content.day_ticks;
    if !budget.set || budget.day != today {
        *budget = Budget { day: today, budget: share * free.max(0.0), spent: 0.0, set: true, counting_house: budget.counting_house };
    }
    let allowed = if days <= 1.0 || share <= 0.0 || share >= 1.0 {
        budget.budget
    } else {
        budget.budget / share * (1.0 - (1.0 - share).powf(days))
    };
    (allowed - budget.spent).max(0.0)
}
