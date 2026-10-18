//! Crates and their papers' steps (`cargo.rs`):
//!
//! - `Touch`: a town's paperwork reckoned: stationery from its stock, a scribe for what it lacks (`Paperwork.settle`).
//! - `Work`: a farm crates what falls due under its forward sales and coupons by the end of the next harvest window,
//!   in lots of at least a stack (or all that's owed), when it holds the whole lot and has room for the crates; a crate
//!   whose contract is no longer in force, or has gone to another buyer, goes back on the shelf (`ForwardSales.earmark`).
//! - `PlanOwed`: home's crates for a contract the seller carries load first, as they are, a whole pallet only where it
//!   fits a wagon's slots, weight and pallets (`Caravans.pickContracted`, `ForwardSales.takeEarmarked`; `pallet-caps`);
//!   the loose goods owed are planned after them (`rules/contracts.rs`).
//! - `PlanPapers`: the buyer's wagon takes what its town owes the far end for goods a player brought (`exWorksPurse`).
//! - `Decide`, as the wagon leaves: the pallets planned taken from home's, sealed; the loose goods owed packed on pallets
//!   within the pallet kind's caps (`pallet-caps`: nine stacks and nine weight, a crate a pallet in the base content), a
//!   bill of lading and a copy of the contract each, made out to the far end; then each good for sale of two stacks or
//!   more packed in crates of its own, an open lot with no papers (`Caravans.lots`); what can't be crated travels loose.
//! - `Papers`, first: the far end's empty crates beyond the stack it keeps go back, and crates there made out to the
//!   wagon's home are fetched, as far as there's room, before anything is delivered, paid or collected (the mod's
//!   order in `arriveAtDestination`: `spareEmpties` and the consigned crates before `settleOwing`), a pallet only if it
//!   fits the wagon's slots, weight and pallets (`pallet-caps`). Then the buyer's
//!   wagon pays what its town owes the far end for goods delivered without a wagon (`Caravans.settleOwing`; the
//!   seller's wagon collecting it is `rules/contracts.rs`'s `settle`).
//! - `Settle`: crates made out to the far end are opened by its Merchant: their copies filed, their goods delivered
//!   under the contract (`rules/contracts.rs`) or for sale, the empty crates kept (`Crates.store`, `open`).
//! - `Sell`: lots for sale opened to sell; a lot bought whole leaves its crate with the buyer, the rest stays crated.
//! - `Turn`, at home: crates made out home received (`Crates.receive`), lots that didn't sell back on the shelves, the
//!   empty crates into home's.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::cargo::{self, crated_aboard, put_aboard, pack, pack_pallets, stacks_of, Lot, Paperwork, Shipments, Shipper};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::{self, Book};
use crate::farms::Farm;
use crate::messages::TownId;
use crate::road::Stay;
use crate::pallet::{At, Caps, Lading, Line, Load, Package, Payment, PalletId, Pallets};
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
        pay_owing.in_set(Step::Papers).before(super::advances::advances_papers).before(super::bills::present)
            .before(super::sourcing::copies),
        // (after the shop's sighting, as main ran them before sourcing's systems joined the look)
        open.in_set(Step::Settle).after(super::desires::sight).before(super::contracts::settle),
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
fn spare(content: &Content, money: crate::fixed::Fixed, trades: &Trades, farm: Option<&Farm>) -> crate::fixed::Fixed {
    money - super::farms::reserve(content, trades, farm)
}

/// The farm earmarks its harvest for what it owes, in town order: stale crates back on the shelf, then for each
/// forward sale and coupon in force, what falls due by the end of the next harvest window beyond what's on the road
/// and crated already, crated in one go once the stock holds a lot of it (a stack, or all that's owed), if there's room
/// for the crates; one bill of lading for it, made out to the buyer, with a copy of the contract.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn earmark(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, bills: Res<crate::bills::Bills>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut pallets: ResMut<Pallets>,
        mut towns: Query<(&Town, &mut Stock, &mut Money, &Trades, Option<&Farm>, &mut Shipments, &mut Paperwork)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let harvests: Vec<usize> = book.open.iter().copied()
        .filter(|&id| book.contracts[id].harvest.is_some() && book.contracts[id].in_force()).collect();
    let mut order: Vec<_> = towns.iter_mut().filter(|(town, ..)| !pallets.at(At::Town(town.id)).is_empty()
        || harvests.iter().any(|&id| book.contracts[id].seller == town.id)).collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (town, mut stock, mut money, trades, farm, mut store, mut paperwork) in order {
        // A crate it shipped whose contract is no longer in force, or has gone to another buyer: its own again (the
        // shipper opens a void pallet at its premises).
        let stale: Vec<PalletId> = pallets.at_place(At::Town(town.id)).filter(|p| {
            let l = &p.lading;
            l.shipper == town.id && !p.consigned_to(town.id) && l.contract.is_some_and(|id| {
                let c = &book.contracts[id];
                // A harvest crate whose contract has closed or gone to another buyer; any crate shipped under a contract no
                // longer in force, unless a bill it was paid for by is still to be collected (then it's the buyer's, and goes
                // with the bill: `ForwardSales.earmark`, `market.Stranded`).
                (c.harvest.is_some() && c.seller == town.id && (!c.in_force() || l.consignee != c.buyer))
                    || (!c.in_force() && !(l.prepaid() && bills.open.iter().map(|&b| &bills.bills[b]).any(|b|
                        b.state == crate::bills::BillState::Outstanding && b.payee == town.id && l.consignee == b.drawee)))
            })
        }).map(|p| p.id).collect();
        for id in stale {
            let p = pallets.unpack(id).expect("a pallet at the town");
            stock.0[p.good()] += p.count();
            store.empties += p.crates();
        }
        for &id in &harvests {
            let c = &book.contracts[id];
            if c.seller != town.id {
                continue;
            }
            let g = c.good;
            let window = c.harvest.map_or(0, |h| h.window_ticks(day));
            let want = (c.owed(now + window) - c.shipped - cargo::earmarked(&pallets, town.id, id)).floor();
            if want <= 0.0 {
                continue;
            }
            // In lots: a stack, or all that's still owed, at a time.
            let lot = want.min(content.goods[g].stack as f64);
            if stock.0[g] < lot {
                continue;
            }
            let taking = want.min(stock.0[g].floor());
            // (Its crates as packing puts them on pallets: a crate a pallet in the base content, nine stacks each.)
            let crates: u32 = cargo::pallet_loads(&content, g, taking).iter().map(|l| l.len() as u32).sum();
            if cargo::crates_held(&pallets, town.id) + crates > content.dials.crate_room {
                continue; // no room for the crates: the crop stays loose (still set aside), and no papers are drawn
            }
            let spare = spare(&content, money.0, trades, farm);
            let mut shipper = Shipper { town: town.id, money: &mut money.0, spare, store: &mut *store, paperwork: &mut *paperwork };
            // A pallet each pallet's worth, a bill each (`pallet-caps`).
            let lading = Lading { contract: Some(id), payment: Payment::Owed, copy: true,
                ..Lading::new(town.id, c.buyer, now) };
            let ids = pack_pallets(&content, &mut ledger, &mut cash, now, &mut shipper, &mut pallets, &lading, g, taking,
                At::Town(town.id));
            stock.0[g] -= cargo::count_of(&pallets, &ids);
        }
    }
}

/// In the plan, before the loose goods owed: home's pallets for each contract it carries to the far end, as they are, in
/// the order they came, as many as cover what's owed by the time it would arrive (within a wagon's slots, weight and
/// pallets, each whole pallet only if it fits: `pallet-caps`); pressing as the loose goods' would be
/// (`rules/contracts.rs` `plan_owed`).
fn plan_crated(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let grace = (contracts::GRACE_DAYS * day as f64) as u64;
    // In route order, in two passes: the first route to plan a pallet claims it, and the routes after it go without
    // (`pallet-claims`). A route whose last try ended in a stay a pallet can't mend (it can't fund the road, has no
    // Merchant or squad, is out of range) goes in the second pass, so it doesn't hold a pallet a route that can go could take.
    let order = in_order(routes.iter().map(|(e, r, _)| (e, r)));
    let stuck = |routes: &Query<(Entity, &Route, &mut RouteState)>, e: Entity| routes.get(e).is_ok_and(|(_, _, s)| matches!(
        s.stayed, Some(Stay::CantFund | Stay::NoMerchant | Stay::OutOfRange | Stay::NoSquad)));
    let (second, first): (Vec<Entity>, Vec<Entity>) = order.into_iter().partition(|e| stuck(&routes, *e));
    for entity in first.into_iter().chain(second) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        let Some(plan) = state.plan.as_mut() else { continue };
        if pallets.at(At::Town(route.from_id)).is_empty() {
            continue;
        }
        let arrival = now + route.travel_ticks;
        let wagon = Caps::wagon(&content);
        for c in book.in_force(route.from_id, route.to_id).filter(|c| c.incoterm.seller_carries()) {
            let owed = (c.owed(arrival) - c.shipped).max(0.0);
            // The wagon's room (a plain wagon's caps: slots, weight, pallets) beside the loose cargo and pallets planned.
            let mut load = Load::of(&content, &plan.cargo, plan.pallets.iter().filter_map(|id| pallets.get(*id)));
            let mut left = owed.min(wagon.less(&load).loose(&content, c.good));
            let mut taken = 0.0;
            let mut counted = Vec::new();
            for p in pallets.at_place(At::Town(route.from_id)).filter(|p| p.claimed.is_none()
                    && p.lading.contract == Some(c.id) && p.consigned_to(route.to_id)) {
                if left <= 0.0 {
                    break;
                }
                // Only a pallet that fits (`pallet-caps`): a whole pallet, never past the wagon's caps.
                let with = load.and(Load::of_pallet(&content, p));
                if !wagon.holds(&with) {
                    continue;
                }
                load = with;
                let count = p.count();
                left -= count;
                taken += count;
                counted.push(p.id);
            }
            for id in counted {
                pallets.claim(id, route.seq);
                plan.pallets.push(id);
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
            plan.purse += owed.min((money - plan.purse).at_least_zero());
            plan.worth += owed.to_f64();
        }
    }
}

/// As each wagon leaves: the pallets its plan claimed, taken from home's by id and released, sealed, counted on the road
/// under their contracts; the loose goods owed packed, a pallet a contract, made out to the far end with its bill of
/// lading and copy; then each good for sale of two stacks or more packed in crates of its own, an open lot with no papers.
/// Crates are home's empties first, then new ones as far as it can spare; the rest goes loose.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn ship(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut pallets: ResMut<Pallets>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
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
        let (home, seq) = (route.from_id, route.seq);
        // The pallets planned (their lines written as the plan went), from home's.
        let mut crated: Vec<(usize, f64)> = Vec::new();
        for line in std::mem::take(&mut wagon.manifest.lines) {
            // Claimed by this route's plan at the look's start, so no other wagon's plan took it (`pallet-claims`).
            let p = pallets.get(line.pallet).expect("a pallet on a manifest is in the registry");
            assert!(p.at == At::Town(home) && p.claimed == Some(seq), "a pallet on a manifest is home's, claimed by its plan");
            pallets.release(line.pallet);
            let p = pallets.get(line.pallet).unwrap();
            if let Some(id) = p.lading.contract {
                let count = p.count();
                book.ship(id, count);
                crated.push((id, count));
            }
            put_aboard(&mut pallets, wagon, seq, line, home, now);
        }
        let spare = spare(&content, money.0, trades, farm);
        let mut shipper = Shipper { town: home, money: &mut money.0, spare, store: &mut *store, paperwork: &mut *paperwork };
        // The loose goods owed, crated with their papers.
        let mut owed_loose = vec![0.0; wagon.cargo.len()];
        for (id, n) in wagon.contracted.clone() {
            let g = book.contracts[id].good;
            // A pallet each pallet's worth, a bill each (`pallet-caps`).
            let lading = Lading { contract: Some(id), payment: Payment::Owed, copy: true,
                ..Lading::new(home, route.to_id, now) };
            let mut in_crates = 0.0;
            let packed = pack_pallets(&content, &mut ledger, &mut cash, now, &mut shipper, &mut pallets, &lading, g, n,
                At::Town(home));
            for p in packed {
                in_crates += pallets.get(p).unwrap().count();
                put_aboard(&mut pallets, wagon, seq, Line { pallet: p, ..Default::default() }, home, now);
            }
            wagon.cargo[g] -= in_crates;
            owed_loose[g] += n - in_crates;
        }
        for (id, n) in crated {
            match wagon.contracted.iter_mut().find(|(c, _)| *c == id) {
                Some(entry) => entry.1 += n,
                None => wagon.contracted.push((id, n)),
            }
        }
        // Goods for sale, a good to a lot where there are two stacks or more of it.
        for g in 0..wagon.cargo.len() {
            let for_sale = (wagon.cargo[g] - owed_loose[g]).max(0.0);
            if stacks_of(&content, g, for_sale) < 2 {
                continue;
            }
            if let Some(packed) = pack(&content, &mut ledger, &mut cash, now, &mut shipper, g, for_sale, false) {
                let lot = Lot { packages: packed.into_iter().flatten().collect() };
                wagon.cargo[g] -= lot.count();
                wagon.for_sale.push(lot);
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
            let pay = book.contracts[id].owing.min((wagon.purse - keep).at_least_zero());
            if pay > 0.0 {
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut money.0),
                    pay, Reason::Contract, clock.now, content.day_ticks);
                book.paid_owing(id, pay);
            }
        }
    }
}

/// At the far end, before the goods owed are delivered: its Merchant opens the pallets made out to it, at its premises: a
/// copy of the contract is now its copy, the goods are delivered under the contract or sold like any
/// (`rules/contracts.rs`, `rules/arrive.rs`), the empty crates kept to ship with.
pub(crate) fn open(mut book: ResMut<Book>, mut pallets: ResMut<Pallets>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut stores: Query<&mut Shipments>) {
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let wagon = state.wagon.as_mut().unwrap();
        let here: Vec<PalletId> = pallets.at_place(At::Wagon(route.seq)).filter(|p| p.consigned_to(route.to_id))
            .map(|p| p.id).collect();
        if here.is_empty() {
            continue;
        }
        let Ok(mut store) = stores.get_mut(route.to) else { continue };
        for id in here {
            let p = pallets.open(id).expect("aboard");
            wagon.manifest.take(id);
            if let Some(id) = p.lading.contract.filter(|_| p.lading.copy) {
                book.deliver_copy(id, route.to_id);
            }
            wagon.cargo[p.good()] += p.count();
            store.empties += p.crates();
        }
    }
}

/// At the far end, first of all (before the debts, the deliveries and the Ex Works collections, as the mod's
/// `arriveAtDestination`): its empty crates beyond the stack it keeps go back with the wagon, and the crates there made
/// out to the wagon's home are fetched, as far as the wagon, still loaded with what it brought, has room. Not a farm's
/// unpaid earmark for an Ex Works contract: that's for the Ex Works collection to take first, paid for (as the mod's
/// `collectExWorks` says it does; its fetch, coming first, would take it unpaid and the collection the loose goods too).
fn fetch(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut stores: Query<&mut Shipments>) {
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let Ok(mut store) = stores.get_mut(route.to) else { continue };
        let wagon = state.wagon.as_mut().unwrap();
        // As far as its crew carries (`caravan.capacity()`: a squad's packs, a porter's one; 0, a plain wagon's), in
        // slots, weight and pallets (`pallet-caps`).
        wagon.empties += store.spare_empties(content.dials.empties_kept);
        let there: Vec<PalletId> = pallets.at(At::Town(route.to_id)).to_vec();
        for id in there {
            let h = pallets.get(id).unwrap();
            let collected = !h.lading.prepaid() && h.lading.contract.is_some_and(|id| !book.contracts[id].incoterm.seller_carries());
            if h.consigned_to(route.from_id) && !collected && cargo::fits(&content, &pallets, route.seq, wagon, h) {
                put_aboard(&mut pallets, wagon, route.seq, Line { pallet: id, ..Default::default() }, route.to_id, clock.now);
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
        for lot in std::mem::take(&mut wagon.for_sale) {
            let (g, count) = (lot.good(), lot.count());
            wagon.cargo[g] += count;
            wagon.lots.push((g, count));
        }
    }
}

/// After the sale: a lot bought whole leaves its crates with the buyer; what's left of one stays crated aboard.
pub(crate) fn relot(content: Res<Content>, mut routes: Query<(&Route, &mut RouteState)>, mut stores: Query<&mut Shipments>) {
    for (route, mut state) in &mut routes {
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let wagon = state.wagon.as_mut().unwrap();
        for (g, count) in std::mem::take(&mut wagon.lots) {
            let left = wagon.cargo[g].min(count).max(0.0);
            if left <= 0.0 {
                if let Ok(mut store) = stores.get_mut(route.to) {
                    store.empties += Lot { packages: cargo::crated(&content, g, count) }.crates();
                }
            } else {
                wagon.cargo[g] -= left;
                wagon.for_sale.push(Lot { packages: cargo::crated(&content, g, left) });
            }
        }
    }
}

/// Home again, before the cargo goes back on the shelves: pallets made out home received (`Crates.receive`), any other
/// kept as it is, still sealed; lots that didn't sell back in stock; the empty crates it brought into home's.
#[allow(clippy::type_complexity)]
pub(crate) fn unload(clock: Res<Clock>, mut book: ResMut<Book>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<(&mut Stock, &mut Shipments)>) {
    // In route order: receiving books deliveries and debts under contracts two wagons may share.
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let wagon = state.wagon.as_mut().unwrap();
        let aboard: Vec<PalletId> = pallets.at(At::Wagon(route.seq)).to_vec();
        if aboard.is_empty() && wagon.for_sale.is_empty() && wagon.empties == 0 {
            continue;
        }
        let Ok((mut stock, mut store)) = towns.get_mut(route.from) else { continue };
        store.empties += std::mem::take(&mut wagon.empties);
        // (Its lots for sale went back aboard after the sale, last of all: home's pallets came before them.)
        for id in aboard {
            wagon.manifest.take(id);
            if pallets.get(id).unwrap().consigned_to(route.from_id) {
                let p = pallets.open(id).unwrap();
                cargo::receive(&mut book, clock.now, route.from_id, &mut stock.0, &mut store, &p);
            } else {
                pallets.move_to(id, At::Town(route.from_id), clock.now);
            }
        }
        for lot in std::mem::take(&mut wagon.for_sale) {
            stock.0[lot.good()] += lot.count();
            store.empties += lot.crates();
        }
    }
}

/// The Ex Works goods a buyer's wagon collects, from the seller's pallets for the contract first (opened at its
/// premises, the crates kept; the last part taken, what's left re-crated on its pallet), up to `most`: what it took from
/// them.
pub(crate) fn take_earmarked(content: &Content, pallets: &mut Pallets, store: &mut Shipments, seller: TownId,
        contract: usize, buyer: TownId, most: f64) -> f64 {
    let mut taken = 0.0;
    let there: Vec<PalletId> = pallets.at(At::Town(seller)).to_vec();
    for id in there {
        if taken >= most {
            break;
        }
        let p = pallets.get(id).unwrap();
        if p.lading.contract != Some(contract) || !p.consigned_to(buyer) {
            continue;
        }
        let (good, count) = (p.good(), p.count());
        let n = count.min(most - taken);
        taken += n;
        let left = count - n;
        // What's left is crated again on its pallet: none left, the pallet's taken apart, and its crates kept are those
        // what's left would take (one, empty: the core has always kept one of an emptied pallet's crates).
        let left_in = cargo::crated(content, good, left);
        if left <= 0.0 {
            pallets.unpack(id);
            store.empties += left_in.len() as u32;
        } else {
            pallets.repack(id, left_in);
        }
    }
    taken
}

/// The seller packs what a buyer's wagon collects Ex Works, made out to the buyer, paid for (`Caravans.collectExWorks`):
/// its own empties first, else new crates as far as it can spare. Returns the crates, a group a pallet, if any of it was
/// crated.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pack_collected(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, now: u64,
        seller: TownId, money: &mut crate::fixed::Fixed, trades: &Trades, farm: Option<&Farm>, store: &mut Shipments,
        paperwork: &mut Paperwork, contract: &crate::contracts::Contract, count: f64) -> Option<Vec<Vec<Package>>> {
    let spare = spare(content, *money, trades, farm);
    let mut shipper = Shipper { town: seller, money, spare, store, paperwork };
    pack(content, ledger, cash, now, &mut shipper, contract.good, count, true)
}

/// What a wagon carries of a good, loose and crated (`crated_aboard`).
pub(crate) fn aboard_of(pallets: &Pallets, route: crate::messages::RouteId, wagon: &Wagon, good: usize) -> f64 {
    wagon.cargo[good] + crated_aboard(pallets, route, wagon, good)
}
