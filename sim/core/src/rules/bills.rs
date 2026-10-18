//! The bills' steps: at the far end, the bills a wagon carries on the town presented (`Papers`); in a route's plan,
//! the bills home holds on the far end, due by the time it would be there (`PlanPapers`): collecting them is worth the
//! trip. The papers themselves go with the wagon when it leaves (`plan::decide`).

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::bills::{self, BillState, Bills, Held};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::Book;
use crate::farms::{borrow, on_call, Deposits};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    // `plan_papers` after what's owed for goods a player brought, as it runs unpinned on main (both add to the plan's
    // worth: an ulp's difference).
    schedule.add_systems((present.in_set(Step::Papers), plan_papers.in_set(Step::PlanPapers)
        .after(super::cargo::plan_owing)));
}

/// Bills the wagon carries on this town, presented: it pays whole the due ones it can, from its money (what it owes
/// isn't its money manager's to hold back), short of it from its households' savings, as far as the treasury may borrow
/// them (`Caravans.presentBills`: `depositsOnCall`, `borrowDeposits`). Its grace runs from the day a bill is first put to it; unpaid past that,
/// the bill is dishonoured and trust falls a whole miss; honoured, it counts half a kept installment.
#[allow(clippy::too_many_arguments)]
pub(super) fn present(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Money, &mut People, &mut Deposits)>) {
    let now = clock.now;
    let day = content.day_ticks;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (mut money, mut people, mut deposits) = towns.get_mut(route.to).expect("the far end");
        let wagon = state.wagon.as_mut().unwrap();
        for id in std::mem::take(&mut wagon.bills) {
            let b = &mut bills.bills[id];
            if b.drawee != route.to_id || b.state == BillState::Paid {
                wagon.bills.push(id);
                continue;
            }
            // Short of coin, the Merchant honours its bills with its households' savings, as far as it may borrow them.
            if b.due <= now && money.0 < b.amount && money.0 + on_call(&content, people.purse, deposits.owed) >= b.amount {
                let short = b.amount - money.0;
                borrow(&content, &mut ledger, &mut cash, route.to_id, &mut money.0, &mut people.purse, &mut deposits, short, now);
            }
            if b.due <= now && money.0 >= b.amount {
                post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0),
                    Side::wagon(route.seq, &mut wagon.purse), b.amount, Reason::Bill, now, day);
                if b.state == BillState::Outstanding {
                    book.kept(b.drawee, b.payee, bills::HONOURED_WEIGHT, now, day);
                }
                b.state = BillState::Paid;
                continue;
            }
            let presented = *b.presented.get_or_insert(now);
            let grace = bills.grace(&bills.bills[id], day, content.dials.bill_grace_days, content.dials.bill_grace_emeralds_per_day);
            let b = &mut bills.bills[id];
            if b.state == BillState::Outstanding && now > b.due.max(presented) + grace {
                b.state = BillState::Dishonoured;
                book.missed(b.drawee, b.payee, 1.0, now, day);
            }
            wagon.bills.push(id);
        }
    }
    bills.tidy();
}

/// In the plan: bills home holds on the far end, due by the time it'd be there, count toward the trip's worth.
fn plan_papers(clock: Res<Clock>, bills: Res<Bills>, mut routes: Query<(&Route, &mut RouteState)>) {
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let arrival = clock.now + route.travel_ticks;
        plan.worth += bills.open.iter().map(|&id| &bills.bills[id])
            .filter(|b| b.held == Held::Town(route.from_id) && b.payee == route.from_id && b.drawee == route.to_id
                && b.due <= arrival)
            .map(|b| b.amount.to_f64()).sum::<f64>();
    }
}
