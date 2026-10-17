//! The harvest term's steps (`harvest.rs`): each look, rents issue their season's coupon and default (`Judge`, after
//! the day's judging); in a farm's plan, the capital for the rents it can afford to buy back (`PlanPapers`); at the
//! buyer's market, a farm town's wagon first buys back the rents its town owes there (`Sell`, before the sale), then,
//! after the sale, the buyer offers it a forward sale or a rent of a crop its lockers use more of than its own farms
//! grow, a third up front. The judging of the harvest branch itself is `Book::review_with` (`contracts.rs`).
//!
//! Java: `ContractBook.review`, `Rents.review`, `ForwardSales.offer`.

use bevy_ecs::prelude::*;

use super::{in_order, trading_capital, Step, TownData};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::{self, Book, Incoterm};
use crate::harvest::{bought_forward, promised, Harvest, RentTerms, Rents};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        review_rents.in_set(Step::Judge).after(super::contracts::review),
        redeem.in_set(Step::Sell).before(super::arrive::sell),
        offer.in_set(Step::Sell).after(super::arrive::sell),
        plan_redeem.in_set(Step::PlanPapers).after(super::advances::plan_advances),
    ));
}

fn rent_reason() -> Reason {
    Reason::Other("rent".into())
}

/// Every look: a rent whose season is out issues its coupon, or defaults.
fn review_rents(clock: Res<Clock>, content: Res<Content>, mut rents: ResMut<Rents>, mut book: ResMut<Book>) {
    if rents.rents.is_empty() {
        return;
    }
    rents.review(&mut book, clock.now, &RentTerms::of(&content.dials, content.day_ticks));
}

/// In the plan: the capital of a rent the town owes at the far end goes in the purse when it can spare twice it beyond
/// its trading capital and the road (`ForwardSales.offer`: bought back "with the capital it brought").
fn plan_redeem(content: Res<Content>, rents: Res<Rents>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Money, &Trades)>) {
    if rents.rents.is_empty() {
        return;
    }
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let (money, trades) = towns.get(route.from).expect("home");
        for id in rents.owed(route.from_id, route.to_id) {
            let capital = rents.rents[id].capital;
            let spare = money.0 - trading_capital(&content, trades) - route.road_cost - plan.purse;
            if spare >= 2.0 * capital {
                plan.purse += capital;
                plan.worth += capital;
            }
        }
    }
}

/// At the far end, the wagon's town buys back the rents it owes here, paying the capital from its purse.
fn redeem(clock: Res<Clock>, content: Res<Content>, mut rents: ResMut<Rents>, mut book: ResMut<Book>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>) {
    if rents.rents.is_empty() {
        return;
    }
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (_, mut money, _, _, _) = towns.get_mut(route.to).expect("the far end");
        let wagon = state.wagon.as_mut().unwrap();
        for id in rents.owed(route.from_id, route.to_id) {
            let capital = rents.rents[id].capital;
            if wagon.purse >= capital {
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                    capital, rent_reason(), clock.now, content.day_ticks);
                rents.redeem(id, &mut book);
            }
        }
    }
}

/// After the sale, the buyer offers the farm town whose wagon came a forward sale of each crop its lockers use more of
/// than its own farms grow, in lots of 16: a rent where the farm is short of money and the buyer has twice the capital
/// to put by (the capital rides home in the wagon's purse), else ten installments at today's price, a third paid up
/// front into the purse, the first falling due a round trip and `forward_sow_days` on. No more than the farm could add,
/// less what it has promised already. The buyer holds the copy; the seller's rides home with the wagon.
#[allow(clippy::too_many_arguments)]
fn offer(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut rents: ResMut<Rents>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>) {
    if rents.crops.is_empty() {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    let terms = RentTerms::of(d, day);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) || book.trust(route.from_id, route.to_id, now, day) < contracts::MIN_TRUST {
            continue;
        }
        let route = *route;
        let Ok([farm, buyer]) = towns.get_many_mut([route.from, route.to]) else { continue };
        let (_, farm_money, farm_trades, _, _) = farm;
        let (_, mut money, trades, _, _) = buyer;
        let farm_spare = farm_money.0 - trading_capital(&content, farm_trades);
        let farm_makes = farm_trades.makes.clone();
        let every = day.max(2 * route.travel_ticks);
        let wagon = state.wagon.as_mut().unwrap();
        let crops: Vec<usize> = rents.crops.iter().copied().collect();
        for g in crops {
            // A farm town sells only what it grows.
            if farm_makes[g] <= 0.0 {
                continue;
            }
            // What the lockers use (at least a lot a round trip, to start a trade), less what the town's own fields grow
            // and what it has bought forward already; no more than the farm could grow beyond what it has promised.
            let used = trades.uses[g].max(contracts::CONTRACT_MIN_ITEMS * day as f64 / every as f64);
            let mut lacking = used - trades.makes[g] - bought_forward(&book, route.to_id, g, day);
            lacking = lacking.min(d.could_grow_own_share * farm_makes[g] - promised(&book, route.from_id, day));
            if lacking <= 0.0 || book.covers(route.from_id, route.to_id, g) {
                continue;
            }
            let value = content.goods[g].value;
            // A rent, where the farm needs money and the town here has it to put by.
            let season = (d.forward_max_quantity * 2.0)
                .min((lacking * terms.season_ticks() as f64 / day as f64 / 16.0).floor() * 16.0);
            let capital = terms.capital(season, value);
            let savings = money.0 - trading_capital(&content, trades);
            if season >= contracts::CONTRACT_MIN_ITEMS && farm_spare < capital && savings >= 2.0 * capital {
                post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    capital, rent_reason(), now, day);
                rents.sell(route.from_id, route.to_id, g, season, capital, &mut book, now, &terms);
                continue;
            }
            let quantity = d.forward_max_quantity.min((lacking * every as f64 / day as f64 / 16.0).floor() * 16.0);
            if quantity < contracts::CONTRACT_MIN_ITEMS {
                continue;
            }
            let start = now + every + (d.forward_sow_days * day as f64) as u64;
            let id = book.draft_harvest(route.from_id, route.to_id, g, quantity, value, start, every, d.forward_installments,
                Incoterm::Ddp, false, false, Harvest { window_days: d.harvest_window_days, carried: false });
            book.contracts[id].advance_share = d.advance_share;
            wagon.drafts.push(id);
            // A third up front, as far as the buyer can spare it: into the purse, to ride home with the copy.
            let due = book.contracts[id].advance_due();
            let pay = due.min((money.0 - trading_capital(&content, trades)).max(0.0));
            if pay > contracts::ADVANCE_EPS {
                post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    pay, Reason::Other("advance".into()), now, day);
                book.advanced(id, pay);
            }
        }
    }
}
