//! Crates and their papers' steps (`cargo.rs`):
//!
//! - `Touch`: a town's paperwork reckoned: stationery from its stock, a scribe for what it lacks (`Paperwork.settle`).
//! - `Work`: a farm crates what falls due under its forward sales and coupons by the end of the next harvest window,
//!   in lots of at least a stack (or all that's owed), when it holds the whole lot and has room for the crates; a crate
//!   whose contract is no longer in force, or has gone to another buyer, goes back on the shelf (`ForwardSales.earmark`).
//! - `PlanOwed`: home's crates for a contract the seller carries load first, as they are (`Caravans.pickContracted`,
//!   `ForwardSales.takeEarmarked`); the loose goods owed are planned after them (`rules/contracts.rs`).
//! - `PlanPapers`: the buyer's wagon takes what its town owes the far end for goods a player brought (`exWorksPurse`).
//! - `Decide`, as the wagon leaves: the crates planned taken from home's; the loose goods owed packed with a bill of
//!   lading and a copy of the contract, made out to the far end; then each good for sale of two stacks or more packed
//!   in crates of its own, with no papers (`Caravans.lots`); what can't be crated travels loose.
//! - `Papers`, first: the far end's empty crates beyond the stack it keeps go back, and crates there made out to the
//!   wagon's home are fetched, as far as there's room, before anything is delivered, paid or collected (the mod's
//!   order in `arriveAtDestination`: `spareEmpties` and the consigned crates before `settleOwing`). Then the buyer's
//!   wagon pays what its town owes the far end for goods delivered without a wagon (`Caravans.settleOwing`; the
//!   seller's wagon collecting it is `rules/contracts.rs`'s `settle`).
//! - `Settle`: crates made out to the far end are opened by its Merchant: their copies filed, their goods delivered
//!   under the contract (`rules/contracts.rs`) or for sale, the empty crates kept (`Crates.store`, `open`).
//! - `Sell`: lots for sale opened to sell; a lot bought whole leaves its crate with the buyer, the rest stays crated.
//! - `Turn`, at home: crates made out home received (`Crates.receive`), lots that didn't sell back on the shelves, the
//!   empty crates into home's.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::cargo::{self, pack, stacks, stacks_of, Paperwork, Shipment, Shipments, Shipper, CRATE_STACKS};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::{self, Book};
use crate::farms::Farm;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        reckon.in_set(Step::Touch).after(crate::people::live),
        earmark.in_set(Step::Work).after(super::farms::work).after(crate::herds::tend_towns)
            .before(super::workshop::work).before(super::lockers::restock),
        plan_crated.in_set(Step::PlanOwed).before(super::contracts::plan_owed),
        plan_owing.in_set(Step::PlanPapers).before(super::advances::plan_advances),
        ship.in_set(Step::Decide).after(super::plan::decide),
        fetch.in_set(Step::Papers).before(pay_owing),
        pay_owing.in_set(Step::Papers).before(super::advances::advances_papers).before(super::bills::present),
        open.in_set(Step::Settle).before(super::contracts::settle),
        unlot.in_set(Step::Sell).before(super::arrive::sell),
        relot.in_set(Step::Sell).after(super::arrive::sell),
        unload.in_set(Step::Turn).after(super::road::feed).after(super::squad::come_home).before(super::arrive::turn),
    ));
}

/// Each town's paperwork reckoned as it's touched, in town order: stationery from its stock, a scribe for the rest.
fn reckon(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut towns: Query<(&Town, &mut Money, &mut Stock, &mut Paperwork, Option<&crate::works::Works>)>) {
    let mut order: Vec<_> = towns.iter_mut().filter(|(_, _, _, p, _)| p.owed >= 1.0).collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (town, mut money, mut stock, mut paperwork, works) in order {
        // With a counting house, its clerks keep the papers: half the stationery, a clerk's fee for what's lacking.
        let (stationery, fee) = if works.is_some_and(|w| w.counting()) {
            (cargo::COUNTING_STATIONERY, cargo::CLERK_EMERALDS)
        } else {
            (1.0, content.dials.scribe_emeralds)
        };
        cargo::reckon(&content, &mut ledger, &mut cash, clock.now, town.id, &mut money.0, &mut stock.0, &mut paperwork,
            stationery, fee);
    }
}

/// What a town can spare beyond its reserve (trading capital and wage float), for crates.
fn spare(content: &Content, money: f64, trades: &Trades, farm: Option<&Farm>) -> f64 {
    money - super::farms::reserve(content, trades, farm)
}

/// The farm earmarks its harvest for what it owes, in town order: stale crates back on the shelf, then for each
/// forward sale and coupon in force, what falls due by the end of the next harvest window beyond what's on the road
/// and crated already, crated in one go once the stock holds a lot of it (a stack, or all that's owed), if there's room
/// for the crates; one bill of lading for it, made out to the buyer, with a copy of the contract.
#[allow(clippy::type_complexity)]
pub(crate) fn earmark(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut towns: Query<(&Town, &mut Stock, &mut Money, &Trades, Option<&Farm>, &mut Shipments, &mut Paperwork)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let harvests: Vec<usize> = book.open.iter().copied()
        .filter(|&id| book.contracts[id].harvest.is_some() && book.contracts[id].in_force()).collect();
    let mut order: Vec<_> = towns.iter_mut().filter(|(town, .., store, _)| !store.held.is_empty()
        || harvests.iter().any(|&id| book.contracts[id].seller == town.id)).collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (town, mut stock, mut money, trades, farm, mut store, mut paperwork) in order {
        // A crate it shipped whose contract is no longer in force, or has gone to another buyer: its own again.
        let mut i = 0;
        while i < store.held.len() {
            let s = &store.held[i];
            let stale = s.shipper == town.id && !s.consigned_to(town.id) && s.contract.is_some_and(|id| {
                let c = &book.contracts[id];
                // (A pupil's work crated for a training contract's buyer waits for its wagon after the contract is done.)
                (!c.in_force() && c.training.is_none()) || (c.harvest.is_some() && c.seller == town.id && s.consignee != Some(c.buyer))
            });
            if stale {
                let s = store.held.remove(i);
                stock.0[s.good] += s.count;
                store.empties += s.crates(&content);
            } else {
                i += 1;
            }
        }
        for &id in &harvests {
            let c = &book.contracts[id];
            if c.seller != town.id {
                continue;
            }
            let g = c.good;
            let window = c.harvest.map_or(0, |h| h.window_ticks(day));
            let want = (c.owed(now + window) - c.shipped - store.earmarked(id)).floor();
            if want <= 0.0 {
                continue;
            }
            // In lots: a stack, or all that's still owed, at a time.
            let lot = want.min(content.goods[g].stack as f64);
            if stock.0[g] < lot {
                continue;
            }
            let taking = want.min(stock.0[g].floor());
            let crates = stacks_of(&content, g, taking).div_ceil(CRATE_STACKS);
            if store.crates(&content) + crates > content.dials.crate_room {
                continue; // no room for the crates: the crop stays loose (still set aside), and no papers are drawn
            }
            let like = Shipment { good: g, count: 0.0, shipper: town.id, consignee: Some(c.buyer), contract: Some(id),
                settled: false, copy: true };
            let spare = spare(&content, money.0, trades, farm);
            let mut shipper = Shipper { town: town.id, money: &mut money.0, spare, store: &mut *store, paperwork: &mut *paperwork };
            if let Some(s) = pack(&content, &mut ledger, &mut cash, now, &mut shipper, g, taking, &like) {
                stock.0[g] -= s.count;
                store.held.push(s);
            }
        }
    }
}

/// In the plan, before the loose goods owed: home's crates for each contract it carries to the far end, as they are,
/// as many as cover what's owed by the time it would arrive (within the wagon's slots); pressing as the loose goods'
/// would be (`rules/contracts.rs` `plan_owed`).
fn plan_crated(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>,
        stores: Query<&Shipments>) {
    let (now, day) = (clock.now, content.day_ticks);
    let grace = (contracts::GRACE_DAYS * day as f64) as u64;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let Ok(store) = stores.get(route.from) else { continue };
        if store.held.is_empty() {
            continue;
        }
        let arrival = now + route.travel_ticks;
        for c in book.in_force(route.from_id, route.to_id).filter(|c| c.incoterm.seller_carries()) {
            let owed = (c.owed(arrival) - c.shipped).max(0.0);
            let free = content.dials.wagon_slots.saturating_sub(stacks(&content, &plan.cargo, &plan.shipments));
            let mut left = owed.min(free as f64 * content.goods[c.good].stack as f64);
            let mut taken = 0.0;
            for s in store.held.iter().filter(|s| s.contract == Some(c.id) && s.consigned_to(route.to_id)) {
                if left <= 0.0 {
                    break;
                }
                left -= s.count;
                taken += s.count;
                plan.shipments.push(s.clone());
            }
            if taken <= 0.0 {
                continue;
            }
            plan.worth += taken * c.price;
            let counts = if c.incoterm.seller_bears_road() { arrival + day } else { now + day };
            plan.pressing |= c.next_deadline(c.wait_ticks(grace, day)).is_some_and(|deadline| counts > deadline);
        }
    }
}

/// In the plan: what home owes the far end for goods delivered without a wagon (a player brought them) goes in the
/// purse, as far as the town can pay for the road and its purse (`Caravans.exWorksPurse`).
pub(crate) fn plan_owing(content: Res<Content>, book: Res<Book>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Money, &Stock, &Trades, Option<&crate::roster::Roster>)>) {
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let owing = book.owing(route.to_id, route.from_id);
        if owing.is_empty() {
            continue;
        }
        let (money, stock, trades, roster) = towns.get(route.from).expect("home");
        let keep_wheat = crate::road::keep_wheat(&content, trades, roster);
        let mouths = plan.crew.len().max(1) as f64; // the crew it has mustered (`squad.rs`), one without
        let money = money.0 - crate::road::road_money(&content, route, &stock.0, keep_wheat, mouths);
        for id in owing {
            let owed = book.contracts[id].owing;
            plan.purse += owed.min((money - plan.purse).max(0.0));
            plan.worth += owed;
        }
    }
}

/// As each wagon leaves: the crates it planned taken from home's (another wagon may have taken one since: it's not
/// aboard), counted on the road under their contracts; the loose goods owed packed, a shipment a contract, made out to
/// the far end with its bill of lading and copy; then each good for sale of two stacks or more packed in crates of its
/// own, with no papers. Crates are home's empties first, then new ones as far as it can spare; the rest goes loose.
#[allow(clippy::type_complexity)]
pub(crate) fn ship(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Money, &Trades, Option<&Farm>, &mut Shipments, &mut Paperwork)>) {
    let now = clock.now;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let route = *route;
        if state.last_departure != Some(now) {
            continue;
        }
        // Just left (a saddled wagon rides a third faster: `squad.rs`).
        let Some(wagon) = state.wagon.as_mut().filter(|w| w.leg == Leg::Out
            && w.arrives == now + crate::squad::leg_ticks(&content, route.travel_ticks, w.saddled, 1.0))
            else { continue };
        let Ok((mut money, trades, farm, mut store, mut paperwork)) = towns.get_mut(route.from) else { continue };
        // The crates planned, from home's.
        let mut crated: Vec<(usize, f64)> = Vec::new();
        for s in std::mem::take(&mut wagon.shipments) {
            let Some(i) = store.held.iter().position(|h| *h == s) else { continue };
            let s = store.held.remove(i);
            if let Some(id) = s.contract {
                book.ship(id, s.count);
                crated.push((id, s.count));
            }
            wagon.shipments.push(s);
        }
        let spare = spare(&content, money.0, trades, farm);
        let mut shipper = Shipper { town: route.from_id, money: &mut money.0, spare, store: &mut *store, paperwork: &mut *paperwork };
        // The loose goods owed, crated with their papers.
        let mut owed_loose = vec![0.0; wagon.cargo.len()];
        for &(id, n) in &wagon.contracted {
            let g = book.contracts[id].good;
            let like = Shipment { good: g, count: 0.0, shipper: route.from_id, consignee: Some(route.to_id),
                contract: Some(id), settled: false, copy: true };
            let packed = pack(&content, &mut ledger, &mut cash, now, &mut shipper, g, n, &like);
            let in_crates = packed.as_ref().map_or(0.0, |s| s.count);
            wagon.cargo[g] -= in_crates;
            owed_loose[g] += n - in_crates;
            wagon.shipments.extend(packed);
        }
        for (id, n) in crated {
            match wagon.contracted.iter_mut().find(|(c, _)| *c == id) {
                Some(entry) => entry.1 += n,
                None => wagon.contracted.push((id, n)),
            }
        }
        // Goods for sale, a good to a crate where there are two stacks or more of it.
        for g in 0..wagon.cargo.len() {
            let for_sale = (wagon.cargo[g] - owed_loose[g]).max(0.0);
            if stacks_of(&content, g, for_sale) < 2 {
                continue;
            }
            let like = Shipment { good: g, count: 0.0, shipper: route.from_id, consignee: None, contract: None,
                settled: true, copy: false };
            if let Some(s) = pack(&content, &mut ledger, &mut cash, now, &mut shipper, g, for_sale, &like) {
                wagon.cargo[g] -= s.count;
                wagon.shipments.push(s);
            }
        }
    }
}

/// At the far end: the buyer's wagon pays what its town owes there for goods delivered without a wagon, from its
/// purse beyond the road home's food (`Caravans.settleOwing`); a loan its bank made the seller on the contract comes out
/// first.
fn pay_owing(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<&mut Money>,
        mut banks: ResMut<crate::bank::Banks>) {
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let owing = book.owing(route.to_id, route.from_id);
        if owing.is_empty() {
            continue;
        }
        let mut money = towns.get_mut(route.to).expect("the far end");
        let wagon = state.wagon.as_mut().unwrap();
        let keep = crate::road::purse_keep(&content, &route, crate::road::mouths(Some(wagon)));
        for id in owing {
            // A loan from this wagon's bank on the contract comes out first (`bank.rs`).
            let repaid = banks.repay_from(route.from_id, id, book.contracts[id].owing);
            if repaid > 0.0 {
                book.paid_owing(id, repaid);
            }
            let pay = book.contracts[id].owing.min((wagon.purse - keep).max(0.0));
            if pay > 0.0 {
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                    pay, Reason::Contract, clock.now, content.day_ticks);
                book.paid_owing(id, pay);
            }
        }
    }
}

/// At the far end, before the goods owed are delivered: its Merchant opens the crates made out to it: a copy of the
/// contract is now its copy, the goods are delivered under the contract or sold like any (`rules/contracts.rs`,
/// `rules/arrive.rs`), the empty crates kept to ship with.
fn open(content: Res<Content>, mut book: ResMut<Book>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut stores: Query<&mut Shipments>) {
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let wagon = state.wagon.as_mut().unwrap();
        if !wagon.shipments.iter().any(|s| s.consigned_to(route.to_id)) {
            continue;
        }
        let Ok(mut store) = stores.get_mut(route.to) else { continue };
        let (here, rest): (Vec<Shipment>, Vec<Shipment>) =
            std::mem::take(&mut wagon.shipments).into_iter().partition(|s| s.consigned_to(route.to_id));
        wagon.shipments = rest;
        for s in here {
            if let Some(id) = s.contract.filter(|_| s.copy) {
                book.deliver_copy(id, route.to_id);
            }
            wagon.cargo[s.good] += s.count;
            store.empties += s.crates(&content);
        }
    }
}

/// At the far end, first of all (before the debts, the deliveries and the Ex Works collections, as the mod's
/// `arriveAtDestination`): its empty crates beyond the stack it keeps go back with the wagon, and the crates there made
/// out to the wagon's home are fetched, as far as the wagon, still loaded with what it brought, has room. Not a farm's
/// unpaid earmark for an Ex Works contract: that's for the Ex Works collection to take first, paid for (as the mod's
/// `collectExWorks` says it does; its fetch, coming first, would take it unpaid and the collection the loose goods too).
fn fetch(content: Res<Content>, book: Res<Book>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut stores: Query<&mut Shipments>) {
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let Ok(mut store) = stores.get_mut(route.to) else { continue };
        let wagon = state.wagon.as_mut().unwrap();
        // As far as its crew carries (`caravan.capacity()`: a squad's packs, a porter's one; 0, a plain wagon's).
        let slots = if wagon.slots > 0 { wagon.slots } else { content.dials.wagon_slots };
        wagon.empties += store.spare_empties(content.dials.empties_kept);
        let mut i = 0;
        while i < store.held.len() {
            let h = &store.held[i];
            let collected = !h.settled && h.contract.is_some_and(|id| !book.contracts[id].incoterm.seller_carries());
            if h.consigned_to(route.from_id) && !collected && stacks(&content, &wagon.cargo, &wagon.shipments)
                    + h.stacks(&content) <= slots {
                wagon.shipments.push(store.held.remove(i));
            } else {
                i += 1;
            }
        }
    }
}

/// At the far end, before the sale: lots for sale opened, the goods to sell from.
fn unlot(mut routes: Query<&mut RouteState>) {
    for mut state in &mut routes {
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let wagon = state.wagon.as_mut().unwrap();
        let (lots, rest): (Vec<Shipment>, Vec<Shipment>) =
            std::mem::take(&mut wagon.shipments).into_iter().partition(|s| s.consignee.is_none());
        wagon.shipments = rest;
        for s in lots {
            wagon.cargo[s.good] += s.count;
            wagon.lots.push((s.good, s.count));
        }
    }
}

/// After the sale: a lot bought whole leaves its crate with the buyer; what's left of one stays crated aboard.
fn relot(content: Res<Content>, mut routes: Query<(&Route, &mut RouteState)>, mut stores: Query<&mut Shipments>) {
    for (route, mut state) in &mut routes {
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let wagon = state.wagon.as_mut().unwrap();
        for (g, count) in std::mem::take(&mut wagon.lots) {
            let left = wagon.cargo[g].min(count).max(0.0);
            let lot = Shipment { good: g, count, shipper: route.from_id, consignee: None, contract: None, settled: true,
                copy: false };
            if left <= 0.0 {
                if let Ok(mut store) = stores.get_mut(route.to) {
                    store.empties += lot.crates(&content);
                }
            } else {
                wagon.cargo[g] -= left;
                wagon.shipments.push(Shipment { count: left, ..lot });
            }
        }
    }
}

/// Home again, before the cargo goes back on the shelves: crates made out home received (`Crates.receive`), lots that
/// didn't sell back in stock, any other crate kept as it is; the empty crates it brought into home's.
#[allow(clippy::type_complexity)]
fn unload(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<(&mut Stock, &mut Shipments)>) {
    // In route order: receiving books deliveries and debts under contracts two wagons may share.
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let wagon = state.wagon.as_mut().unwrap();
        if wagon.shipments.is_empty() && wagon.empties == 0 {
            continue;
        }
        let Ok((mut stock, mut store)) = towns.get_mut(route.from) else { continue };
        store.empties += std::mem::take(&mut wagon.empties);
        for s in std::mem::take(&mut wagon.shipments) {
            if s.consigned_to(route.from_id) {
                cargo::receive(&content, &mut book, clock.now, route.from_id, &mut stock.0, &mut store, s);
            } else if s.consignee.is_none() {
                stock.0[s.good] += s.count;
                store.empties += s.crates(&content);
            } else {
                store.held.push(s);
            }
        }
    }
}

/// The Ex Works goods a buyer's wagon collects, from the seller's crates for the contract first (opened, the crates
/// kept), up to `most`: what it took from them.
pub(crate) fn take_earmarked(content: &Content, store: &mut Shipments, contract: usize, buyer: crate::messages::TownId,
        most: f64) -> f64 {
    let mut taken = 0.0;
    let mut i = 0;
    while i < store.held.len() && taken < most {
        let s = &mut store.held[i];
        if s.contract != Some(contract) || !s.consigned_to(buyer) {
            i += 1;
            continue;
        }
        let n = s.count.min(most - taken);
        taken += n;
        s.count -= n;
        if s.count <= 0.0 {
            let s = store.held.remove(i);
            store.empties += s.crates(content);
        } else {
            i += 1;
        }
    }
    taken
}

/// The seller packs what a buyer's wagon collects Ex Works, made out to the buyer, paid for (`Caravans.collectExWorks`):
/// its own empties first, else new crates as far as it can spare. Returns the shipment, if any of it was crated.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pack_collected(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64,
        seller: crate::messages::TownId, money: &mut f64, trades: &Trades, farm: Option<&Farm>, store: &mut Shipments,
        paperwork: &mut Paperwork, contract: &crate::contracts::Contract, count: f64) -> Option<Shipment> {
    let spare = spare(content, *money, trades, farm);
    let like = Shipment { good: contract.good, count: 0.0, shipper: seller, consignee: Some(contract.buyer),
        contract: Some(contract.id), settled: true, copy: true };
    let mut shipper = Shipper { town: seller, money, spare, store, paperwork };
    pack(content, ledger, cash, now, &mut shipper, contract.good, count, &like)
}
