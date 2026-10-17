//! The atlas and books through the look (`atlas.rs`; `Caravans.dispatch`, `arriveAtDestination`, `arriveHome`,
//! `CaravanServices.copyBooks`, `Commissions.consider`):
//!
//! - **Review:** at each town's reviews (every `toll_review_days`, the first look only starting the clock), a town
//!   short of comforts with money to spare commissions a route of its own ([`consider`]); it opens after the look.
//! - **Sell:** at the far end, what it bought goes in its Merchant's books (`Bought`), and in the wagon's log what it
//!   sold there and why the rest didn't sell ([`log_sales`]).
//! - **Services:** a Librarian aboard copies the far end's books, its stocktake; a wagon from a home without a roster
//!   counts the far end at every visit ([`copy_books`]).
//! - **Turn:** home again, what came home goes in its books (`Home`), the wagon's log is handed in, and each town whose
//!   land its road crossed hears of both its ends ([`hand_in`]).
//! - **Plan:** an idle route's trip wants a Librarian while the far end's books are stale ([`want_books`]); the sale
//!   load reads the far end's stocktake (`plan::plan_sale`).
//! - **Decide:** a wagon gone, home's stocktake is taken, both ends know each other, what went out goes in its books
//!   (`Sent`) and its reasons in the tally; a route that stayed tells why, once a day ([`depart`]).

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::atlas::*;
use crate::contracts::Book;
use crate::content::Content;
use crate::people::{comfort_held, comfort_par};
use crate::roster::Roster;
use crate::state::*;
use crate::territory::Territory;
use crate::works::Works;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        consider.in_set(Step::Review).after(super::works::review).after(super::squad::train).after(super::training::lapse)
            .before(super::jobs::review),
        log_sales.in_set(Step::Sell).after(super::arrive::sell).after(super::harvest::offer).before(super::cargo::relot),
        copy_books.in_set(Step::Services).after(super::territory::tolls).after(super::training::copies),
        // Before the crew are home and the crates unloaded (`rules/cargo.rs` unloads after the crew).
        hand_in.in_set(Step::Turn).after(super::road::feed).after(super::training::read_home).before(super::squad::come_home),
        want_books.in_set(Step::Plan).after(super::plan::start).before(super::squad::size_up),
        depart.in_set(Step::Decide).after(super::plan::decide).after(super::cargo::ship),
    ));
}

fn today(content: &Content, now: u64) -> u64 {
    now / content.day_ticks.max(1)
}

/// An idle route's trip wants a Librarian while the far end's books are stale (`CaravanServices.booksStale`, through
/// `Services.wantedAbroad`): never copied, or over `stale_books_days` old.
pub fn want_books(clock: Res<Clock>, content: Res<Content>, stocktakes: Res<Stocktakes>,
        mut routes: Query<(&Route, &mut RouteState)>) {
    for (route, mut state) in &mut routes {
        let stale = state.plan.is_some() && stocktakes.stale(&content, route.to_id, clock.now);
        if state.books_stale != stale {
            state.books_stale = stale;
        }
    }
}

/// A wagon gone this look (`Caravans.dispatch` 498, 510, 562, 570): home's stocktake taken as it leaves (no time: it
/// makes no books fresh), both ends know each other, what went out (crates opened) in home's books, the plan's
/// reasons and why it went in the tally. A route that stayed home tells why (and its plan's reasons), once a day.
#[allow(clippy::too_many_arguments)]
pub fn depart(clock: Res<Clock>, content: Res<Content>, mut atlas: ResMut<Atlas>, mut stocktakes: ResMut<Stocktakes>,
        mut books: ResMut<Books>, mut whys: ResMut<Whys>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        towns: Query<(&Stock, Option<&Roster>, Option<&Works>)>) {
    let now = clock.now;
    let today = today(&content, now);
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        let route = *route;
        let state = &mut *state;
        let reasons = std::mem::take(&mut state.whys);
        let left = state.last_departure == Some(now) && state.wagon.as_ref().is_some_and(|w| w.leg == Leg::Out);
        if left {
            let w = state.wagon.as_ref().unwrap();
            let Ok((stock, roster, works)) = towns.get(route.from) else { continue };
            stocktakes.record(route.from_id, &stock.0, None);
            atlas.learn_both(route.from_id, route.to_id, now);
            let mut sent = w.cargo.clone();
            for s in &w.shipments {
                if s.good < sent.len() {
                    sent[s.good] += s.count;
                }
            }
            let lines: Vec<Entry> = sent.iter().enumerate().filter(|(_, n)| **n > 0.0)
                .map(|(g, &n)| Entry::new(today, Kind::Sent, route.to_id, g, n, 0.0)).collect();
            books.file(&mut whys, route.from_id, today, keeps(&content, roster, works), lines);
            for (good, why, n) in reasons {
                whys.why(route.from_id, Some(route.to_id), good, why, n, today);
            }
            let stacks = crate::cargo::stacks(&content, &w.cargo, &w.shipments);
            let slots = if w.slots > 0 { w.slots } else { content.dials.wagon_slots };
            let full = stacks as f64 >= content.dials.wagon_full * slots as f64;
            whys.why(route.from_id, Some(route.to_id), None, Why::Went { full, porter: w.porter }, stacks as f64, today);
        } else if let Some(stay) = state.stayed.filter(|_| state.wagon.is_none()) {
            if whys.noted.get(&route.seq) == Some(&today) {
                continue;
            }
            whys.noted.insert(route.seq, today);
            // Why each good would have stayed; what would have gone didn't.
            for (good, why, n) in reasons.into_iter().filter(|(_, w, _)| !matches!(w, Why::Sent(_) | Why::SentComfort(_))) {
                whys.why(route.from_id, Some(route.to_id), good, why, n, today);
            }
            whys.why(route.from_id, Some(route.to_id), None, Why::NoTrip(stay_label(stay).to_string()), 0.0, today);
        }
    }
}

/// At the far end, once the wagon has sold (`arriveAtDestination` 696–703): the far end's Merchant books what it
/// bought, the wagon's log what it sold there; and why the rest didn't sell (`sell`, `logUnsold`): they hold their
/// fill (of a kind of comfort), they can't pay, they've no use for it. What it collected under Ex Works is for home.
#[allow(clippy::type_complexity)]
pub fn log_sales(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut books: ResMut<Books>,
        mut whys: ResMut<Whys>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        towns: Query<(&Stock, &Trades, &People, Option<&Roster>, Option<&Works>)>) {
    let today = today(&content, clock.now);
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let Ok((stock, trades, people, roster, works)) = towns.get(route.to) else { continue };
        let mut bought = Vec::new();
        for g in 0..wagon.sold_here.len() {
            let n = wagon.sold_here[g];
            if n <= 0.0 {
                continue;
            }
            let money = wagon.paid_here.get(g).copied().unwrap_or(0.0);
            bought.push(Entry::new(today, Kind::Bought, route.from_id, g, n, money));
            wagon.log.push(Entry::new(today, Kind::Sold, route.to_id, g, n, money));
        }
        books.file(&mut whys, route.to_id, today, keeps(&content, roster, works), bought);
        let collected: Vec<usize> = wagon.collecting.iter().filter_map(|&id| book.contracts.get(id).map(|c| c.good)).collect();
        for g in 0..wagon.cargo.len().min(content.goods.len()) {
            let left = wagon.cargo[g];
            if left <= 0.0 || collected.contains(&g) {
                continue;
            }
            let value = content.goods[g].value.max(1e-9);
            let why = match content.goods[g].comfort {
                Some(kind) if (comfort_par(&content, people) - comfort_held(&content, stock, kind)) / value <= 0.0 =>
                    Why::UnsoldFull(Some(kind)),
                Some(_) => Why::UnsoldCantPay,
                None if trades.uses[g] > 0.0 || trades.capacity.get(g).is_some_and(|c| *c > 0.0)
                    || crate::road::points(&content, g) > 0.0 => {
                    if super::shelf(&content, trades, g) - stock.0[g] < 1.0 { Why::UnsoldFull(None) } else { Why::UnsoldCantPay }
                }
                None => Why::UnsoldNoUse,
            };
            wagon.log.push(Entry::why(today, route.to_id, Some(g), why, left));
        }
    }
}

/// At the far end, the squad's Librarian copies the books: the far end's stocktake, taken now (`CaravanServices
/// .copyBooks`, done whenever one is aboard). A wagon from a home without a roster counts the far end at every visit.
pub fn copy_books(clock: Res<Clock>, mut stocktakes: ResMut<Stocktakes>, content: Res<Content>,
        routes: Query<(Entity, &Route, &RouteState)>, towns: Query<&Stock>, rosters: Query<&Roster>) {
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(e).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let Some(wagon) = state.wagon.as_ref() else { continue };
        let rostered = crate::squad::crewed(rosters.get(route.from).ok());
        if rostered && !wagon.crew.iter().any(|h| copies_books(&content, h.trade)) {
            continue;
        }
        if let Ok(stock) = towns.get(route.to) {
            stocktakes.record(route.to_id, &stock.0, Some(clock.now));
        }
    }
}

/// Home again (`arriveHome` 1160–1192): what it brought home (crates opened) in home's books, its log handed in (its
/// reasons tallied), and every town whose land its road crossed hears of both its ends.
#[allow(clippy::too_many_arguments)]
pub fn hand_in(clock: Res<Clock>, content: Res<Content>, territory: Res<Territory>, mut atlas: ResMut<Atlas>,
        mut books: ResMut<Books>, mut whys: ResMut<Whys>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        towns: Query<(Option<&Roster>, Option<&Works>)>) {
    let now = clock.now;
    let today = today(&content, now);
    for e in in_order(routes.iter().map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, path) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let mut home = wagon.cargo.clone();
        for s in &wagon.shipments {
            if s.good < home.len() {
                home[s.good] += s.count;
            }
        }
        let mut lines: Vec<Entry> = home.iter().enumerate().filter(|(_, n)| **n > 0.0)
            .map(|(g, &n)| Entry::new(today, Kind::Home, route.to_id, g, n, 0.0)).collect();
        lines.append(&mut wagon.log);
        let (roster, works) = towns.get(route.from).unwrap_or((None, None));
        books.file(&mut whys, route.from_id, today, keeps(&content, roster, works), lines);
        if let Some(path) = path {
            for &hex in &path.0 {
                if let Some(town) = territory.owner(hex) {
                    atlas.learn(town, route.from_id, now);
                    atlas.learn(town, route.to_id, now);
                }
            }
        }
    }
}

/// At its reviews (every `toll_review_days`, as the mod's toll reviews; the first look starts the clock), each town
/// considers a caravan of its own (`Commissions.consider`): the market it'd send to, opened once the look is done.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn consider(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, stocktakes: Res<Stocktakes>,
        mut atlas: ResMut<Atlas>, routes: Query<&Route>,
        q: Query<(&Town, &People, &Money, &Trades, Option<&crate::farms::Farm>, Option<&Works>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let every = (content.dials.toll_review_days * day as f64) as u64;
    let mut due = Vec::new();
    for &e in &towns.0 {
        let Ok((town, ..)) = q.get(e) else { continue };
        match atlas.considered.get(&town.id) {
            Some(&last) if now < last + every => {}
            Some(_) => {
                atlas.considered.insert(town.id, now);
                due.push(e);
            }
            None => {
                atlas.considered.insert(town.id, now);
            }
        }
    }
    if due.is_empty() {
        return;
    }
    let n = towns.0.len();
    let mut places = Places { hexes: vec![None; n], reach: vec![content.dials.word_of_mouth; n], ..Default::default() };
    for &e in &towns.0 {
        if let Ok((town, _, _, _, _, works)) = q.get(e) {
            if town.id < n {
                places.hexes[town.id] = town.hex;
                places.reach[town.id] = reach(&content, works);
            }
        }
    }
    for r in routes.iter() {
        places.roads.entry((r.from_id, r.to_id)).and_modify(|b| *b = (*b).min(r.blocks)).or_insert(r.blocks);
    }
    for e in due {
        let Ok((town, people, money, trades, farm, _)) = q.get(e) else { continue };
        let has_route = routes.iter().any(|r| r.from_id == town.id) || atlas.pending.iter().any(|&(f, _)| f == town.id);
        let spare = money.0 - super::farms::reserve(&content, trades, farm);
        let considering = Considering { town: town.id, has_route, comfort_met: &people.comfort_met, spare };
        if let Some(to) = choose(&content, &atlas, &stocktakes, &places, &considering) {
            atlas.pending.push((town.id, to));
        }
    }
}
