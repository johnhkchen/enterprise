//! The bank's steps (`bank.rs`):
//!
//! - `Judge`, after the day's judging and the rents' (as `Caravans.poll`: `review`, `Rents.review`, then
//!   `Bills.lapse`): a bill outstanding `lapse_days` past its day that no one can carry to its drawee (no route between
//!   the two either way, and no hub that buys it) lapses, the drawee's miss.
//! - `PlanCollect`, after the Ex Works plan: a bank's wagon collecting goods its bank lent on carries that much less
//!   (the loan comes out of the payment first; the mod carries the whole, and draws a bill for any shortfall).
//! - `PlanBuy`, before the rents' buy-back (`Caravans.dispatch`: `bankFloat` after the shopping and Ex Works money,
//!   before `buyBack`): a bank's wagon takes up to `bank_float` more in its purse, as far as its market can spare beyond
//!   its reserve, the road and what every plan from it carries already, and its Accountant allows today.
//! - `Decide`, as the wagons leave: the float a bank's wagon took (its Accountant allowed it at the plan) is counted spent
//!   off its day's budget.
//! - `Papers`, after the advances and before the bills are presented (`Caravans.arrive`: `settleOwing`, `payAdvances`,
//!   `Bank.deal`, `presentBills`): a bank's wagon collects loans on contracts done or broken, buys the town's bills not
//!   yet due, and lends against its contracts in force (`Bank.deal`: `collect`, `discount`, `lend`).
//!
//! The loan coming out of the payment first is in `rules/contracts.rs` (`collect`: `Caravans.collectExWorks`) and
//! `rules/cargo.rs` (`pay_owing`: `Caravans.settleOwing`).

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

use super::farms::reserve;
use super::{free, in_order, Step};
use crate::bank::{self, discount_price, loan_cap, to_come, Banks, COLLECT_EPS, LOAN_EPS, MIN_LOAN};
use crate::bills::{BillState, Bills, Held};
use crate::cash_book::{post, Accountant, CashBook, Side};
use crate::content::Content;
use crate::contracts::{self, Book};
use crate::farms::Farm;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        lapse.in_set(Step::Judge).after(super::contracts::review).after(super::harvest::review_rents),
        plan_loans.in_set(Step::PlanCollect).after(super::contracts::plan_collect),
        plan_float.in_set(Step::PlanBuy).before(super::harvest::plan_redeem),
        spend_float.in_set(Step::Decide).after(super::plan::decide).before(super::cargo::ship),
        deal.in_set(Step::Papers).after(super::advances::advances_papers).before(super::bills::present),
    ));
}

/// Bills no one can carry to their drawee lapse `lapse_days` past their day (`Bills.lapse`).
fn lapse(clock: Res<Clock>, content: Res<Content>, mut bills: ResMut<Bills>, mut book: ResMut<Book>, routes: Query<&Route>) {
    if bills.open.is_empty() {
        return;
    }
    let roads: BTreeSet<(usize, usize)> = routes.iter().map(|r| (r.from_id, r.to_id)).collect();
    let carried = |payee: usize, drawee: usize| roads.contains(&(payee, drawee)) || roads.contains(&(drawee, payee))
        || bank::hub_collects(payee, drawee);
    bills.lapse(&mut book, clock.now, content.day_ticks, content.dials.lapse_days, carried);
}

/// A bank's wagon collecting goods Ex Works under a contract its bank lent on carries that much less: the loan comes
/// out of the payment first (`Caravans.collectExWorks`; the mod carries the whole and draws a bill for any shortfall).
fn plan_loans(clock: Res<Clock>, book: Res<Book>, banks: Res<Banks>,
        mut routes: Query<(&Route, &mut RouteState)>) {
    if banks.loans.is_empty() {
        return;
    }
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        if !banks.is(route.from_id) {
            continue;
        }
        let arrival = clock.now + route.travel_ticks;
        for &id in &plan.collecting {
            let c = &book.contracts[id];
            let owed = c.owed(arrival);
            let payment = owed * c.price - c.recoup_from(owed * c.price, 0.0);
            plan.purse -= banks.owed_to(route.from_id, id).min(payment.max(0.0));
        }
        plan.purse = plan.purse.max(0.0);
    }
}

/// A bank's wagon's float: up to `bank_float`, as far as its market can spare beyond its reserve, the road and the
/// purses of every plan from it this look (and floats planned already), and as far as its Accountant allows today.
#[allow(clippy::type_complexity)]
pub(crate) fn plan_float(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, mut banks: ResMut<Banks>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>,
        towns: Query<(&Money, &Stock, &Trades, &Budget, Option<&crate::roster::Roster>, Option<&Farm>)>) {
    banks.floats.clear();
    if banks.banks.is_empty() {
        return;
    }
    let now = clock.now;
    // What every plan from a bank's market carries and its road costs, this look.
    let mut committed: BTreeMap<usize, f64> = BTreeMap::new();
    for (_, route, state) in routes.iter() {
        let Some(plan) = state.plan.as_ref() else { continue };
        if !banks.is(route.from_id) {
            continue;
        }
        let (_, stock, trades, _, roster, _) = towns.get(route.from).expect("home");
        let keep_wheat = crate::road::keep_wheat(&content, trades, roster);
        // (for the crew it has mustered, `squad.rs`: one without)
        let mouths = plan.crew.len().max(1) as f64;
        *committed.entry(route.from_id).or_insert(0.0) += plan.purse
            + crate::road::road_money(&content, route, &stock.0, keep_wheat, mouths);
    }
    let mut budgets: BTreeMap<usize, Budget> = BTreeMap::new();
    let mut planned: BTreeMap<usize, f64> = BTreeMap::new();
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        if !banks.is(route.from_id) {
            continue;
        }
        let Some(plan) = state.plan.as_mut() else { continue };
        let (money, _, trades, budget, _, farm) = towns.get(route.from).expect("home");
        let already = planned.get(&route.from_id).copied().unwrap_or(0.0);
        let spare = money.0 - reserve(&content, trades, farm) - committed.get(&route.from_id).copied().unwrap_or(0.0)
            - already;
        let wanted = content.dials.bank_float.min(spare).max(0.0);
        if wanted <= 0.0 {
            continue;
        }
        let budget = budgets.entry(route.from_id).or_insert(*budget);
        let free_money = free(&content, trades, money, &bills, route.from_id, now);
        let float = wanted.min(Accountant::left(&content, now, free_money, budget));
        if float <= 0.0 {
            continue;
        }
        budget.spent += float;
        plan.purse += float;
        *planned.entry(route.from_id).or_insert(0.0) += float;
        banks.floats.insert(route.seq, float);
    }
}

/// The floats of the banks' wagons that left this look, spent off their market's day's budget: the Accountant allowed
/// them when they were planned (`Caravans.dispatch`: `accountant.allow` on the `bankFloat`), so they're only counted
/// spent here, after the wagons are sent and their purses taken (`cargo::ship` after it: neither reads the other).
fn spend_float(clock: Res<Clock>, mut banks: ResMut<Banks>, routes: Query<(Entity, &Route, &RouteState)>,
        mut budgets: Query<&mut Budget>) {
    if banks.floats.is_empty() {
        return;
    }
    let now = clock.now;
    let floats = std::mem::take(&mut banks.floats);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(entity).unwrap();
        let Some(&float) = floats.get(&route.seq) else { continue };
        if state.last_departure != Some(now) || state.wagon.is_none() {
            continue;
        }
        budgets.get_mut(route.from).expect("home").spent += float;
    }
}

/// A bank's wagon at a town (`Bank.deal`): loans on contracts done or broken collected from what the town can spare;
/// its bills not yet due bought for coin now; loans made against its contracts in force where it's short. All from
/// the purse beyond what the wagon keeps for the road home's food.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn deal(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut bills: ResMut<Bills>, mut banks: ResMut<Banks>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Money, &Trades, Option<&Farm>)>) {
    if banks.banks.is_empty() {
        return;
    }
    let now = clock.now;
    let day = content.day_ticks;
    let d = &content.dials;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        if !banks.is(route.from_id) {
            continue;
        }
        let (bank_id, town) = (route.from_id, route.to_id);
        let (mut money, trades, farm) = towns.get_mut(route.to).expect("the far end");
        let reserve = reserve(&content, trades, farm);
        let wagon = state.wagon.as_mut().unwrap();
        let keep = crate::road::purse_keep(&content, &route, crate::road::mouths(Some(wagon)));

        // Collect: loans on contracts no longer in force, from what the town can spare beyond its reserve.
        for i in 0..banks.loans.len() {
            let loan = &banks.loans[i];
            if loan.bank != bank_id || loan.borrower != town || loan.owed <= LOAN_EPS
                || book.contracts.get(loan.contract).is_some_and(|c| c.in_force()) {
                continue; // repaid when its goods are paid for
            }
            let back = (money.0 - reserve).max(0.0).min(loan.owed);
            if back > COLLECT_EPS {
                post(&mut ledger, &mut cash, Side::treasury(town, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    back, bank::repaid_reason(), now, day);
                banks.loans[i].owed -= back;
                banks.book(bank_id).repaid += back;
            }
        }
        banks.loans.retain(|l| l.owed > LOAN_EPS);

        // Discount: the town's own bills, outstanding and not yet due, bought for coin now.
        for id in bills.open.clone() {
            let b = &bills.bills[id];
            if b.state != BillState::Outstanding || b.held != Held::Town(town) || b.payee != town || b.due <= now {
                continue; // not its to sell, settled, or due anyway: presented in the ordinary way
            }
            let price = discount_price(b.amount, b.due, now, day, d);
            if price <= 0.0 || wagon.purse - keep < price {
                continue;
            }
            let (face, drawee) = (b.amount, b.drawee);
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(town, &mut money.0),
                price, bank::discount_reason(), now, day);
            let tally = banks.book(bank_id);
            tally.bills_bought += 1;
            tally.bills_paid += price;
            tally.bills_face += face;
            if drawee == bank_id {
                bills.redeem(id); // its own market's paper: settled now
            } else {
                bills.endorse(id, bank_id); // home with the wagon, to present when due
                bills.bills[id].held = Held::Wagon;
                wagon.bills.push(id);
            }
        }
        bills.tidy();

        // Lend: against the town's contracts in force as seller, where its money is short of what's to come.
        if book.trust(town, bank_id, now, day) < contracts::MIN_TRUST {
            continue;
        }
        let mut spare = money.0 - reserve;
        for &id in &book.open {
            let c = &book.contracts[id];
            if !c.in_force() || c.seller != town {
                continue;
            }
            let coming = to_come(c);
            let cap = loan_cap(c, banks.owed_on(id), d);
            if cap < MIN_LOAN || spare >= coming * d.loan_share {
                continue; // little to lend on, or it has the money
            }
            let principal = cap.min(wagon.purse - keep);
            if principal < MIN_LOAN {
                continue;
            }
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(town, &mut money.0),
                principal, bank::loan_reason(), now, day);
            banks.lend(bank_id, town, id, principal, d.loan_interest, now);
            spare += principal;
        }
    }
}
