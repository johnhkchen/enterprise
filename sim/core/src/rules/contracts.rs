//! The contracts' steps: installments judged once a day (`Judge`); at the far end, goods owed delivered at the
//! locked price and old debts paid (`Settle`), and goods bought Ex Works collected (`Collect`); and in a route's
//! plan, what it owes (`PlanOwed`) and what it would collect (`PlanCollect`), with the deadlines that can't wait.

use bevy_ecs::prelude::*;

use super::{in_order, Step, TownData};
use crate::bills::Bills;
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::{self, Book, Contract};
use crate::fixed::Fixed;
use crate::pallet::At;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        review.in_set(Step::Judge),
        settle.in_set(Step::Settle),
        collect.in_set(Step::Collect),
        plan_owed.in_set(Step::PlanOwed),
        plan_collect.in_set(Step::PlanCollect),
    ));
}

/// Once a day: installments whose grace has run out are judged.
pub(crate) fn review(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, towns: Res<Towns>, stocks: Query<&Stock>) {
    let day = content.day_ticks;
    if book.reviewed == Some(clock.now / day) {
        return;
    }
    let holds = |c: &Contract| stocks.get(towns.0[c.seller]).map_or(false, |s| s.0[c.good] >= c.quantity);
    book.review_with(clock.now, day, &crate::harvest::HarvestDials::of(&content.dials), holds);
}

/// Goods owed under contract delivered first, whatever the buyer's shelf holds, at the locked price; what it can't
/// pay in coin it pays with a bill on itself if they trust each other, else owes. Then what it owes from before, as
/// far as it can.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settle(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut bills: ResMut<Bills>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<TownData>) {
    let now = clock.now;
    let day = content.day_ticks;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (mut stock, mut money, _, _, _) = towns.get_mut(route.to).expect("the far end");
        let wagon = state.wagon.as_mut().unwrap();
        let mut delivered = 0.0;
        for (id, carried) in std::mem::take(&mut wagon.contracted) {
            let c = &book.contracts[id];
            let n = carried.min(c.owed(now)).min(wagon.cargo[c.good]);
            // The delivery's price, rounded up to the thousandth (the buyer's to pay: `Treasury.withdraw`).
            let worth = Fixed::ceil(n * c.price);
            // What was paid up front is worked off: its share of this delivery's price.
            let recoup = c.recoup_from(worth, n);
            let owed_money = worth - recoup;
            let paid = owed_money.min(money.0.at_least_zero());
            wagon.cargo[c.good] -= n;
            stock.0[c.good] += n;
            post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                paid, Reason::Contract, now, day);
            let short = owed_money - paid;
            let by_bill = short.is_positive() && book.trust(route.from_id, route.to_id, now, day) >= contracts::MIN_TRUST;
            if by_bill {
                wagon.bills.push(bills.draw(route.to_id, route.from_id, short, now, day));
            }
            book.recouped(id, recoup);
            book.settle(id, carried, n, paid + if by_bill { short } else { Fixed::ZERO } + recoup,
                if by_bill { Fixed::ZERO } else { short });
            delivered += n;
        }
        for id in book.open_between(route.from_id, route.to_id) {
            let c = &mut book.contracts[id];
            if !c.owing.is_positive() {
                continue;
            }
            let paid = c.owing.min(money.0.at_least_zero());
            c.owing -= paid;
            c.paid += paid;
            post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                paid, Reason::Contract, now, day);
        }
        state.delivered += delivered; // `itemsDelivered`
    }
}

/// Ex Works: what the buyer's wagon collects at the seller's, paid from the purse it brought: what the seller crated
/// for it first, then loose; the seller crates it, made out to the buyer and paid for (`rules/cargo.rs`). A loan the
/// wagon's bank made the seller on the contract comes out of the payment first (`Caravans.collectExWorks`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn collect(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut pallets: ResMut<crate::pallet::Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<TownData>,
        mut stores: Query<(&mut crate::cargo::Shipments, &mut crate::cargo::Paperwork)>, farms: Query<&crate::farms::Farm>,
        mut banks: ResMut<crate::bank::Banks>) {
    let now = clock.now;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (mut stock, mut money, trades, _, _) = towns.get_mut(route.to).expect("the seller");
        let (mut store, mut paperwork) = stores.get_mut(route.to).expect("the seller");
        let farm = farms.get(route.to).ok();
        let wagon = state.wagon.as_mut().unwrap();
        let mut collected = 0.0;
        for id in std::mem::take(&mut wagon.collecting) {
            let c = &book.contracts[id];
            let crated: f64 = pallets.at_place(crate::pallet::At::Town(route.to_id))
                .filter(|p| p.lading.contract == Some(id) && p.consigned_to(c.buyer)).map(|p| p.count()).sum();
            // (What its bank lent on the contract is paid out of the goods, not the purse.)
            let lent = banks.owed_to(route.from_id, id);
            let n = c.owed(now).min(stock.0[c.good] + crated).min(((wagon.purse + lent).to_f64() / c.price.max(1e-9)).floor());
            if n <= 0.0 {
                continue;
            }
            let worth = Fixed::ceil(n * c.price);
            let recoup = c.recoup_from(worth, n);
            let paid = worth - recoup;
            let from_crates = super::cargo::take_earmarked(&content, &mut pallets, &mut store, route.to_id, id, c.buyer, n);
            stock.0[c.good] -= n - from_crates;
            let packed = super::cargo::pack_collected(&content, &mut ledger, &mut cash, now, route.to_id, &mut money.0,
                trades, farm, &mut store, &mut paperwork, c, n);
            let in_crates = match packed {
                Some(packages) => {
                    // Paid for as it's collected, sealed as it leaves the seller's.
                    let lading = crate::pallet::Lading { contract: Some(id), payment: crate::pallet::Payment::Paid, copy: true,
                        ..crate::pallet::Lading::new(route.to_id, c.buyer, now) };
                    let p = pallets.pack(lading, packages, crate::pallet::At::Town(route.to_id), now);
                    let count = pallets.get(p).unwrap().count();
                    crate::cargo::put_aboard(&mut pallets, wagon, route.seq, crate::pallet::Line { pallet: p, ..Default::default() },
                        route.to_id, now);
                    count
                }
                None => 0.0,
            };
            wagon.cargo[c.good] += n - in_crates;
            // A loan from this wagon's bank on the contract comes out of the payment first (`bank.rs`).
            let repaid = banks.repay_from(route.from_id, id, paid);
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                paid - repaid, Reason::Contract, now, content.day_ticks);
            book.recouped(id, recoup);
            book.settle(id, 0.0, n, paid + recoup, Fixed::ZERO);
            collected += n;
        }
        state.bought += collected; // `itemsBought`, as `collectExWorks` counts it
    }
}

/// In the plan: what's owed under contract the seller carries, by the time the wagon would arrive, as far as home
/// has it. Pressing if an installment would be judged before the goods count, were the wagon to wait a day.
pub(crate) fn plan_owed(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, pallets: Res<crate::pallet::Pallets>,
        mut routes: Query<(&Route, &mut RouteState)>, towns: Query<&Stock>) {
    let now = clock.now;
    let day = content.day_ticks;
    let grace = (contracts::GRACE_DAYS * day as f64) as u64;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let home = towns.get(route.from).expect("home");
        let arrival = now + route.travel_ticks;
        for c in book.in_force(route.from_id, route.to_id).filter(|c| c.incoterm.seller_carries()) {
            // Less what home's pallets for it carry already (`rules/cargo.rs`): those this plan counts, and those another
            // route's plan has claimed this look (`pallet-claims`: a pallet is counted once, not sent twice).
            let crated: f64 = pallets.at_place(At::Town(route.from_id))
                .filter(|p| p.claimed.is_some() && p.consigned_to(route.to_id) && p.lading.contract == Some(c.id)).map(|p| p.count()).sum();
            let owed = (c.owed(arrival) - c.shipped - crated).max(0.0);
            let going = owed.min(home.0[c.good] - plan.cargo[c.good]).floor();
            if going <= 0.0 {
                continue;
            }
            plan.cargo[c.good] += going;
            plan.contracted.push((c.id, going));
            plan.worth += going * c.price;
            let counts = if c.incoterm.seller_bears_road() { arrival + day } else { now + day };
            plan.pressing |= c.next_deadline(c.wait_ticks(grace, day)).is_some_and(|deadline| counts > deadline);
        }
    }
}

/// In the plan: what the town buys at the far end Ex Works, owed by the time it would arrive, with the money for it.
/// Pressing if waiting a day would miss an installment.
pub(crate) fn plan_collect(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>) {
    let now = clock.now;
    let day = content.day_ticks;
    let grace = (contracts::GRACE_DAYS * day as f64) as u64;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let arrival = now + route.travel_ticks;
        for c in book.in_force(route.to_id, route.from_id).filter(|c| !c.incoterm.seller_carries()) {
            let owed = c.owed(arrival);
            if owed <= 0.0 {
                continue;
            }
            plan.collecting.push(c.id);
            let worth = Fixed::ceil(owed * c.price);
            plan.purse += worth - c.recoup_from(worth, 0.0);
            plan.worth += owed * c.price;
            plan.pressing |= c.next_deadline(c.wait_ticks(grace, day)).is_some_and(|deadline| arrival + day > deadline);
        }
    }
}
