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
    // As the mod: refunds and advances settled before the bills are presented (`Caravans.arrive`: `settleOwing`,
    // `payAdvances`, then `presentBills`), so the money an advance brings can meet a bill.
    schedule.add_systems((advances_papers.in_set(Step::Papers).before(super::bills::present),
        plan_advances.in_set(Step::PlanPapers)));
}

fn advance_reason() -> Reason {
    Reason::Other("advance".into())
}

fn refund_reason() -> Reason {
    Reason::Other("advance refund".into())
}

/// At the far end: the buyer's wagon hands over the copy it carries; then, in the mod's order (`settleOwing`, then
/// `payAdvances`), advances the town here owes back are paid into the purse from what it can spare beyond its trading
/// capital, and advances home owes back the town here from the purse; advances due to the town here are paid from the
/// purse (the rest by a bill on the buyer where trusted); and advances the town here owes home are paid into the purse
/// from what it can spare.
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
        // The advance the buyer (home) owes the town here.
        for id in book.advances_due(route.to_id, route.from_id) {
            let c = &book.contracts[id];
            if !c.known_by_seller {
                continue;
            }
            let due = c.advance_due();
            // Keeping its road food: the purse for the road home's (`payAdvances`'s `keep`).
            let coin = due.min((wagon.purse - crate::road::purse_keep(&content, &route)).max(0.0));
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
        // The advance the town here owes the wagon's town, under contracts it buys from it whose copy it holds: paid
        // into the purse from what it can spare beyond its trading capital.
        for id in book.advances_due(route.from_id, route.to_id) {
            if !book.contracts[id].known_by_buyer {
                continue;
            }
            let can = (money.0 - trading_capital(&content, trades)).max(0.0);
            let pay = can.min(book.contracts[id].advance_due());
            if pay > ADVANCE_EPS {
                post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    pay, advance_reason(), now, day);
                book.advanced(id, pay);
            }
        }
    }
}

/// In the plan: the advance home owes the far end under contracts it buys there, and the advances it owes back, go in
/// the purse, as far as the town can pay for the road and its purse; the trip is pressing while an advance can be paid
/// (in coin, or by a bill where trusted).
pub(crate) fn plan_advances(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Money, &Stock, &Trades, Option<&crate::roster::Roster>)>) {
    let now = clock.now;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let (money, stock, trades, roster) = towns.get(route.from).expect("home");
        // Room left for the road: its food, fodder and purse where content has food (`road.rs`), else its money.
        let keep_wheat = crate::road::keep_wheat(&content, trades, roster);
        let money = money.0 - crate::road::road_money(&content, route, &stock.0, keep_wheat);
        let trusted = book.trust(route.from_id, route.to_id, now, content.day_ticks) >= contracts::MIN_TRUST;
        for id in book.advances_due(route.to_id, route.from_id) {
            let due = book.contracts[id].advance_due();
            let coin = due.min((money - plan.purse).max(0.0));
            plan.purse += coin;
            plan.worth += due;
            plan.pressing |= coin > ADVANCE_EPS || trusted;
        }
        for id in book.refunds(route.from_id, route.to_id) {
            let owed = book.contracts[id].refund;
            plan.purse += owed.min((money - plan.purse).max(0.0));
            plan.worth += owed;
        }
    }
}
