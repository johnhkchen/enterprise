//! The harvest term's steps (`harvest.rs`): each look, rents issue their season's coupon and default (`Judge`, after
//! the day's judging); in a farm's plan, the capital for the rents it can afford to buy back, reckoned after the
//! plan's other purse uses (`PlanBuy`); at the buyer's market, after the sale, a farm town's wagon buys back the rents
//! its town owes there with what the sale brought (`Sell`), then the buyer offers it a forward sale or a rent of a crop
//! its lockers use more of than its own farms grow (the advance on a forward sale is paid on the wagon's next arrival,
//! `advances.rs`). The judging of the harvest branch itself is `Book::review_with` (`contracts.rs`).
//!
//! Java: `ContractBook.review`, `Rents.review`, `ForwardSales.offer`.

use bevy_ecs::prelude::*;

use super::farms::reserve;
use super::{in_order, Step, TownData};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::{self, Book, Incoterm};
use crate::farms::{self, Farm};
use crate::harvest::{bought_forward, could_grow, promised, Harvest, RentTerms, Rents};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        review_rents.in_set(Step::Judge).after(super::contracts::review),
        redeem.in_set(Step::Sell).after(super::arrive::sell),
        offer.in_set(Step::Sell).after(redeem),
        plan_redeem.in_set(Step::PlanBuy),
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

/// In the plan, last of what goes in the purse: the capital of a rent the town owes at the far end goes in the purse
/// when it can spare twice it beyond its reserve (`Territory.reserve`: trading capital and wage float), the road's
/// money and what the plan's purse holds already (`Caravans.plan`: bought back "with the capital it brought"). Money
/// only: the purse is no goods' worth.
#[allow(clippy::type_complexity)]
fn plan_redeem(content: Res<Content>, rents: Res<Rents>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Money, &Stock, &Trades, Option<&crate::roster::Roster>, Option<&Farm>)>) {
    if rents.rents.is_empty() {
        return;
    }
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let (money, stock, trades, roster, farm) = towns.get(route.from).expect("home");
        let keep_wheat = crate::road::keep_wheat(&content, trades, roster);
        let free = money.0 - reserve(&content, trades, farm) - crate::road::road_money(&content, route, &stock.0, keep_wheat);
        for id in rents.owed(route.from_id, route.to_id) {
            let capital = rents.rents[id].capital;
            let spare = free - plan.purse;
            if spare >= 2.0 * capital {
                plan.purse += capital;
            }
        }
    }
}

/// At the far end, the wagon's town buys back the rents it owes here, paying the capital from its purse beyond what it
/// keeps for the road home's food (`ForwardSales.offer`'s `keep`).
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
        let keep = crate::road::purse_keep(&content, &route);
        let wagon = state.wagon.as_mut().unwrap();
        for id in rents.owed(route.from_id, route.to_id) {
            let capital = rents.rents[id].capital;
            if wagon.purse - keep >= capital {
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                    capital, rent_reason(), clock.now, content.day_ticks);
                rents.redeem(id, &mut book);
            }
        }
    }
}

/// After the sale, the buyer offers the farm town whose wagon came a forward sale of each rotation crop it lacks: what its
/// people and lockers would buy (no more than its lockers use, and at least a lot a round trip, to start a trade),
/// less what its own fields grow and what it has bought forward already; no more than the farm could grow beyond what
/// it has promised. Only a town with a farm plan sells forward. A rent where the farm is short of money and the buyer has
/// twice the capital to put by (the capital rides home in the wagon's purse), else ten installments at today's price,
/// the first falling due a round trip and `forward_sow_days` on, the advance a third of it, paid when the farm's wagon
/// next arrives here. Both towns' money is reckoned beyond their reserve (trading capital and wage float). The buyer
/// holds the copy; the seller's rides home with the wagon.
#[allow(clippy::too_many_arguments)]
fn offer(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut rents: ResMut<Rents>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<TownData>, farms: Query<&Farm>) {
    if !content.dials.forward_sales {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    let terms = RentTerms::of(d, day);
    let crops = farms::rotation(&content);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) || book.trust(route.from_id, route.to_id, now, day) < contracts::MIN_TRUST {
            continue;
        }
        let route = *route;
        let Ok([farm_farm, buyer_farm]) = farms.get_many([route.from, route.to]) else { continue };
        // Only a town with farms sells its crop forward.
        if !farm_farm.set_up || farm_farm.per_day.is_empty() {
            continue;
        }
        let Ok([farm, buyer]) = towns.get_many_mut([route.from, route.to]) else { continue };
        let (_, farm_money, farm_trades, _, _) = farm;
        let (_, mut money, trades, _, _) = buyer;
        let farm_spare = farm_money.0 - reserve(&content, farm_trades, Some(farm_farm));
        let wants = farms::Wants::of(buyer_farm, &trades.uses);
        let every = day.max(2 * route.travel_ticks);
        let wagon = state.wagon.as_mut().unwrap();
        for &g in &crops {
            // What it lacks of what its lockers use (at least a lot a round trip, to start a trade it has no crop for),
            // not all its villagers could buy.
            let used = trades.uses[g].max(contracts::CONTRACT_MIN_ITEMS * day as f64 / every as f64);
            let grown = buyer_farm.per_day.get(&g).copied().unwrap_or(0.0);
            let mut lacking = wants.demand(g).min(used) - grown - bought_forward(&book, route.to_id, g, day);
            // No more than the farm could grow beyond what it has promised already.
            lacking = lacking.min(could_grow(&content, farm_farm, g) - promised(&book, route.from_id, day));
            if lacking <= 0.0 || book.covers(route.from_id, route.to_id, g) {
                continue;
            }
            let value = content.goods[g].value;
            // A rent, where the farm needs money and the town here has it to put by.
            let season = (d.forward_max_quantity * 2.0)
                .min((lacking * terms.season_ticks() as f64 / day as f64 / 16.0).floor() * 16.0);
            let capital = terms.capital(season, value);
            let savings = money.0 - reserve(&content, trades, Some(buyer_farm));
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
            book.contracts[id].advance_share = d.forward_advance_share;
            wagon.drafts.push(id);
        }
    }
}
