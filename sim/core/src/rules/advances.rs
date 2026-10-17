//! The advance's steps (`advances.rs`): in a buyer's plan, the advance due on its contracts with the far end, carried in
//! the wagon's purse, and the advances home owes back to it (`PlanPapers`); at the far end, the copy handed over, the
//! advance paid (coin from the purse, the rest by a bill on the buyer where trusted), and refunds settled both ways
//! (`Papers`). The advance is worked off in `Settle` and `Collect` (`rules/contracts.rs`) and owed back in `Judge`
//! (`Book::review`).
//!
//! Java: `Caravans.collectPressing` (an advance due makes the trip pressing), `exWorksPurse`, `payAdvances`,
//! `settleOwing`.

use bevy_ecs::prelude::*;

use super::{in_order, trading_capital, Step, TownData};
use crate::bills::{Bills, Held};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::{self, Book, State, ADVANCE_EPS};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((advances_papers.in_set(Step::Papers), plan_advances.in_set(Step::PlanPapers)));
}

fn advance_reason() -> Reason {
    Reason::Other("advance".into())
}

fn refund_reason() -> Reason {
    Reason::Other("advance refund".into())
}

/// At the far end: the buyer's wagon hands over the copy it carries; advances due to the town here are paid from the
/// purse (and the rest by a bill on the buyer where trusted); advances the town here owes back are paid into the
/// purse from what it can spare beyond its trading capital; and advances home owes the town here are paid from the
/// purse.
#[allow(clippy::too_many_arguments)]
fn advances_papers(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>) {
    let now = clock.now;
    let day = content.day_ticks;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (_, mut money, trades, _, _) = towns.get_mut(route.to).expect("the far end");
        let wagon = state.wagon.as_mut().unwrap();
        // The copy the buyer's wagon carries is handed to the seller.
        for id in book.open_between(route.to_id, route.from_id) {
            let c = &book.contracts[id];
            if c.state == State::Drafted && c.advance_share > 0.0 && c.known_by_buyer && !c.known_by_seller {
                book.deliver_copy(id, route.to_id);
            }
        }
        // The advance the buyer (home) owes the town here.
        for id in book.advances_due(route.to_id, route.from_id) {
            let c = &book.contracts[id];
            if !c.known_by_seller {
                continue;
            }
            let due = c.advance_due();
            let coin = due.min(wagon.purse.max(0.0));
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                coin, advance_reason(), now, day);
            let mut paid = coin;
            if due - paid > ADVANCE_EPS && book.trust(route.from_id, route.to_id, now, day) >= contracts::MIN_TRUST {
                // A promise to pay the rest, filed with the seller.
                let bill = bills.draw(route.from_id, route.to_id, due - paid, now, day);
                bills.bills[bill].held = Held::Town(route.to_id);
                paid = due;
            }
            if paid > ADVANCE_EPS {
                book.advanced(id, paid);
            }
        }
        // The advance the town here owes back to the wagon's town: what it can spare beyond its trading capital.
        for id in book.refunds(route.to_id, route.from_id) {
            let can = (money.0 - trading_capital(&content, trades)).max(0.0);
            let pay = can.min(book.contracts[id].refund);
            if pay > ADVANCE_EPS {
                post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    pay, refund_reason(), now, day);
                book.refunded(id, pay);
            }
        }
        // The advance the wagon's town owes back to the town here: paid from the purse it brought.
        for id in book.refunds(route.from_id, route.to_id) {
            let pay = book.contracts[id].refund.min(wagon.purse.max(0.0));
            if pay > 0.0 {
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                    pay, refund_reason(), now, day);
                book.refunded(id, pay);
            }
        }
    }
}

/// In the plan: the advance home owes the far end under contracts it buys there, and the advances it owes back, go in
/// the purse, as far as the town can pay for the road and its purse; the trip is pressing while an advance can be paid
/// (in coin, or by a bill where trusted).
fn plan_advances(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>, towns: Query<&Money>) {
    let now = clock.now;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let money = towns.get(route.from).expect("home").0;
        let trusted = book.trust(route.from_id, route.to_id, now, content.day_ticks) >= contracts::MIN_TRUST;
        for id in book.advances_due(route.to_id, route.from_id) {
            let due = book.contracts[id].advance_due();
            let coin = due.min((money - route.road_cost - plan.purse).max(0.0));
            plan.purse += coin;
            plan.worth += due;
            plan.pressing |= coin > ADVANCE_EPS || trusted;
        }
        for id in book.refunds(route.from_id, route.to_id) {
            let owed = book.contracts[id].refund;
            plan.purse += owed.min((money - route.road_cost - plan.purse).max(0.0));
            plan.worth += owed;
        }
    }
}
