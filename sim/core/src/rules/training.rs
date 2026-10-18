//! Training's steps (`training.rs`):
//!
//! - `Work`, before the lockers restock: each town's lockers are told what its training contracts ask of its Merchant
//!   (`plan`); after them, the pupil's paid work crated for the buyer under an offtake, with a bill drawn on it (`finish`).
//! - `Review`, once a day: tenders that lapse are taken off the boards, and posted again for more where the buyer reposts.
//! - `Judge`: training contracts in force are judged on the sellers' rosters (`Book::review_training`).
//! - `Draft` at the far end, and `Turn` at home, before the wagon turns: a wagon between two towns lets each one's
//!   Merchant read the other's board and take what's worth it (`Tenders::read`); then, at the far end, a copy of a
//!   training contract the wagon's home holds goes to the far end, and one the far end holds rides home (`Services`).

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::bills::Bills;
use crate::cargo::{Paperwork, Shipments};
use crate::cash_book::CashBook;
use crate::content::Content;
use crate::contracts::{Book, State};
use crate::farms::Farm;
use crate::lockers::Lockers;
use crate::roster::Roster;
use crate::state::*;
use crate::training::{finish, mastered, plan, Apprenticeships, Desk, Env, Tenders};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        lessons.in_set(Step::Work).after(super::cargo::earmark).after(super::workshop::work).before(super::lockers::restock),
        crate_work.in_set(Step::Work).after(super::lockers::restock),
        // Pinned as main ran it before atlas-and-books' `consider` moved Bevy's ambiguous order.
        lapse.in_set(Step::Review).after(super::works::culture).before(super::farms::review),
        judge.in_set(Step::Judge).after(super::harvest::review_rents),
        read_out.in_set(Step::Draft).after(super::arrive::draft),
        read_home.in_set(Step::Turn).after(super::road::feed).before(super::squad::come_home).before(super::arrive::turn),
        copies.in_set(Step::Services).after(super::chokepoints::takings),
    ));
}

/// Every town's lockers told what its training contracts ask, in town order.
#[allow(clippy::type_complexity)]
pub(crate) fn lessons(content: Res<Content>, book: Res<Book>, mut apprenticeships: ResMut<Apprenticeships>,
        mut towns: Query<(&Town, &mut Lockers, &Stock, &Trades, Option<&Farm>, Option<&Shipments>)>) {
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(t, ..)| t.id);
    for (town, mut lockers, stock, trades, farm, store) in order {
        // Nothing to say, nothing said before: leave the town as it is.
        let held = book.training_for(town.id).next().is_some();
        if !held && lockers.lessons.targets.is_empty() && lockers.lessons.made.is_empty() && !apprenticeships.added.contains_key(&town.id) {
            continue;
        }
        plan(&content, &book, &mut apprenticeships, town.id, &mut lockers, stock, trades, farm, store);
    }
}

/// The pupil's paid work crated for the buyer, in town order, where the buyer can collect it (a wagon of the buyer's calls,
/// or a hub the town's wagons reach runs to the buyer: `stranded.rs`).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn crate_work(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, routes: Query<(&Route, &RouteState, Option<&RoutePath>)>,
        places: Query<(&Town, &crate::freight::Hub)>,
        mut towns: Query<(&Town, &mut Lockers, &mut Stock, &mut Money, &Trades, Option<&Farm>, &mut Shipments, &mut Paperwork)>) {
    let mut order: Vec<_> = towns.iter_mut().filter(|(_, l, ..)| !l.lessons.made.is_empty()).collect();
    if order.is_empty() {
        return;
    }
    let net = crate::freight::Network::of(&content, routes.iter(),
        places.iter().map(|(t, h)| (t.id, t.hex, h.storehouse)).collect::<Vec<_>>());
    order.sort_by_key(|(t, ..)| t.id);
    for (town, mut lockers, mut stock, mut money, trades, farm, mut store, mut paperwork) in order {
        finish(&content, &book, &mut bills, &mut ledger, &mut cash, &net, clock.now, town.id, &mut lockers, &mut stock,
            &mut money, trades, farm, &mut store, &mut paperwork);
    }
}

/// Tenders past their day lapse, those reposted raised.
pub(crate) fn lapse(clock: Res<Clock>, content: Res<Content>, mut tenders: ResMut<Tenders>) {
    if tenders.tenders.is_empty() {
        return;
    }
    tenders.lapse(&content, clock.now);
}

/// Training contracts in force judged on the sellers' rosters.
pub(crate) fn judge(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, order: Res<Towns>, rosters: Query<&Roster>) {
    if book.contracts.iter().all(|c| c.training.is_none()) {
        return;
    }
    book.review_training(clock.now, content.day_ticks, |c, t| mastered(rosters.get(order.0[c.seller]).ok(), t.trade, t.level));
}

type Desks<'w, 's> = Query<'w, 's, (&'static Money, &'static Stock, &'static Trades, Option<&'static Farm>, Option<&'static Roster>,
    Option<&'static Shipments>)>;

#[allow(clippy::too_many_arguments)]
/// Reads `board`'s board for `reader` over the world as the systems see it.
fn read(clock: &Clock, content: &Content, book: &mut Book, tenders: &mut Tenders, routes: &[Route], order: &Towns,
        desks: &Desks, reader: usize, board: usize) {
    if tenders.tenders.is_empty() {
        return;
    }
    let desk = |t: usize| {
        let (money, stock, trades, farm, roster, store) = desks.get(order.0[t]).ok()?;
        Some(Desk { roster, money: money.0, stock, trades, farm, store })
    };
    let env = Env { content, now: clock.now, routes, desk };
    tenders.read(book, &env, reader, board);
}

/// A wagon at the far end: the Merchant there reads its home's board.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_out(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut tenders: ResMut<Tenders>, order: Res<Towns>,
        desks: Desks, routes: Query<(Entity, &Route, &RouteState)>) {
    if tenders.tenders.is_empty() {
        return;
    }
    let all: Vec<Route> = routes.iter().map(|(_, r, _)| *r).collect();
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(entity).unwrap();
        if state.arrived == Some(Leg::Out) {
            read(&clock, &content, &mut book, &mut tenders, &all, &order, &desks, route.to_id, route.from_id);
        }
    }
}

/// A wagon home: the Merchant there reads the far end's board.
#[allow(clippy::too_many_arguments)]
pub(crate) fn read_home(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut tenders: ResMut<Tenders>, order: Res<Towns>,
        desks: Desks, routes: Query<(Entity, &Route, &RouteState)>) {
    if tenders.tenders.is_empty() {
        return;
    }
    let all: Vec<Route> = routes.iter().map(|(_, r, _)| *r).collect();
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(entity).unwrap();
        if state.arrived == Some(Leg::Home) {
            read(&clock, &content, &mut book, &mut tenders, &all, &order, &desks, route.from_id, route.to_id);
        }
    }
}

/// A wagon at the far end carries the copies of drafted training contracts between its home and the far end: the home's
/// is handed over, the far end's rides home with the wagon (and is its home's once it is there).
pub(crate) fn copies(mut book: ResMut<Book>, mut routes: Query<(Entity, &Route, &mut RouteState)>) {
    if book.contracts.iter().all(|c| c.training.is_none()) {
        return;
    }
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let (home, far) = (route.from_id, route.to_id);
        let wagon = state.wagon.as_mut().unwrap();
        for id in book.open.clone() {
            let c = &book.contracts[id];
            if c.state != State::Drafted || c.training.is_none() {
                continue;
            }
            let held = |t: usize| (c.seller == t && c.known_by_seller) || (c.buyer == t && c.known_by_buyer);
            let between = (c.seller == home && c.buyer == far) || (c.seller == far && c.buyer == home);
            if !between {
                continue;
            }
            if held(home) && !held(far) {
                book.deliver_copy(id, far);
            } else if held(far) && !held(home) && !wagon.drafts.contains(&id) {
                wagon.drafts.push(id);
            }
        }
    }
}
