//! Services through the look (`services.rs`; `Services.home`, `wantedAbroad`, `abroad`; `Caravans.dispatch` 383–384 and
//! 563, `arriveHome` 1186 and 1199):
//!
//! - **Work:** after the workshop, each town's residents do the services at home it wants: a blessing, charting,
//!   what its trades make from its stock ([`home`]). What they learn of, the town knows at once.
//! - **Plan:** each idle route of a town with a roster asks the trades its hands at home have whether the far end
//!   would want their services, from the ledgers alone; those trades go along (`RouteState.services_wanted`, read by
//!   `atlas::trip_wants`) ([`plan`]).
//! - **Decide:** a wagon gone this look with a Shepherd aboard drives up to 8 of home's animals ([`drive`]).
//! - **Services:** at the far end, once it has sold, each of the crew's trades does what the far end wants and can pay
//!   for, the most skilled aboard; the fee from its money beyond its reserve into the wagon's purse, the two towns'
//!   trust up a little for each paid one ([`abroad`]).
//! - **Turn:** home again, the town hears of the markets its crew learned of, and the animals that didn't sell are
//!   home ([`home_again`]).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::atlas::{Atlas, Places, Stocktakes};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::Book;
use crate::farms::Farm;
use crate::herds::Herds;
use crate::lockers::Lockers;
use crate::realms::{OffMap, Realm};
use crate::roster::{Roster, TradeId};
use crate::services::*;
use crate::state::*;
use crate::territory::Territory;
use crate::works::Works;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // Home services in the touch, after the workshop (`Territory.touch`), so before the lockers' trading and the
        // lessons it pays for (`LockerTrading`).
        home.in_set(Step::Work).after(super::workshop::work).before(super::training::lessons)
            .before(super::lockers::restock),
        plan.in_set(Step::Plan).after(super::plan::start).after(super::atlas::want_books).before(super::squad::size_up),
        drive.in_set(Step::Decide).after(super::plan::decide).after(super::atlas::depart),
        abroad.in_set(Step::Services).after(super::atlas::copy_books),
        home_again.in_set(Step::Turn).after(super::atlas::hand_in).before(super::squad::come_home)
            .before(super::freight::deliver_home).before(super::stranded::home_forwarding),
    ));
}

/// A town's components as the services read and change them.
pub type ServedData<'a> = (&'a Town, &'a mut Money, &'a mut Stock, &'a Trades, &'a mut People, Option<&'a Roster>,
    Option<&'a mut Farm>, Option<&'a mut Herds>, Option<&'a Works>, Has<OffMap>, Has<Realm>);

/// What the atlas reads of the towns (`Society::places`): where each stands, how far word of it carries, the roads.
fn places<'a>(content: &Content, n: usize, towns: impl Iterator<Item = (&'a Town, Option<&'a Works>)>,
        routes: impl Iterator<Item = &'a Route>) -> Places {
    let mut places = Places { hexes: vec![None; n], reach: vec![content.dials.word_of_mouth; n], ..Default::default() };
    for (town, works) in towns {
        if town.id < n {
            places.hexes[town.id] = town.hex;
            places.reach[town.id] = crate::atlas::reach(content, works);
        }
    }
    for r in routes {
        places.roads.entry((r.from_id, r.to_id)).and_modify(|b| *b = (*b).min(r.blocks)).or_insert(r.blocks);
    }
    places
}

/// The services a system runs (those done `where_`, none passive) and the recipes, for the content as it stands: kept
/// between looks, worked out again only when the content changed (`all` and `recipes` match the mod's names to the
/// host's goods and trades: thousands of string compares, three times a look, for the same answer).
#[derive(Default)]
pub struct Listing(Option<(Vec<Service>, Vec<Recipe>)>);

fn listing<'a>(cache: &'a mut Listing, content: &Res<Content>, where_: Where) -> (&'a [Service], &'a [Recipe]) {
    if content.is_changed() {
        cache.0 = None;
    }
    let (list, recipes) = cache.0.get_or_insert_with(|| {
        (all(content).into_iter().filter(|s| s.where_ == where_ && !s.passive()).collect(), recipes(content))
    });
    (list, recipes)
}

fn guards(territory: &Territory, town: usize) -> usize {
    territory.holding(town).map_or(0, |h| h.posts.len())
}

fn routes_out<'a>(routes: impl Iterator<Item = &'a Route>, town: usize) -> usize {
    routes.filter(|r| r.from_id == town).count()
}

/// Every town, in its number's order, its residents doing the services at home it wants (`Services.home`, after
/// `Workshop.convert` in `Territory.touch`): what they make, its blessing, what they learn of (known at once).
#[allow(clippy::too_many_arguments)]
pub fn home(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, territory: Res<Territory>, order: Res<Towns>,
        mut atlas: ResMut<Atlas>, mut ledger: ResMut<Ledger>, mut services: ResMut<ServiceLedger>,
        routes: Query<&Route>, mut towns: Query<ServedData>, mut listed: Local<Listing>) {
    if !content.dials.services {
        return;
    }
    let (list, recipes) = listing(&mut listed, &content, Where::Home);
    if list.is_empty() {
        return;
    }
    let now = clock.now;
    let n = order.0.len();
    let places = places(&content, n, towns.iter().map(|t| (t.0, t.8)), routes.iter());
    for &e in &order.0 {
        let Ok(t) = towns.get(e) else { continue };
        if t.9 || t.10 || t.5.is_none() {
            continue;
        }
        let id = t.0.id;
        let skills = residents(t.5);
        if !list.iter().any(|s| skills.contains_key(&s.trade)) {
            continue;
        }
        let (guards, out) = (guards(&territory, id), routes_out(routes.iter(), id));
        for service in list {
            let Some(&skill) = skills.get(&service.trade) else { continue };
            let done = {
                let t = towns.get(e).unwrap();
                let town = Served { id, settlement: true, stock: &t.2.0, trades: t.3,
                    people: t.4, roster: t.5, farm: t.6, herds: t.7, guards, routes_out: out,
                    miners: crate::kit::miners(&content, &territory, id, t.5) };
                let visit = Visit { content: &content, from: id, at: id, skill, now, planning: false, budget: 0.0, town,
                    trip: None, book: &book, atlas: &atlas, places: &places, ledger: &services, recipes };
                if !wanted(service, &visit) {
                    continue;
                }
                perform(service, &visit)
            };
            let (_, _, mut stock, _, mut people, _, farm, ..) = towns.get_mut(e).unwrap();
            put(&mut ledger, &done, &mut stock.0, &mut people, farm.map(|f| f.into_inner()));
            for &m in &done.heard {
                atlas.learn(id, m, now);
            }
            services.record(&service.name, id, id, crate::fixed::Fixed::ZERO, now);
        }
    }
}

/// Each idle route of a town with a roster: the trades its hands at home have (the best of each, `Services.skills` of
/// those who could go) whose services abroad the far end would want, as far as the ledgers show
/// (`Services.wantedAbroad`, asked as `Caravans.dispatch` musters the squad): what its last stocktake holds, its
/// blessing, its untended fields, the maps both towns know, the road's map, home's animals.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn plan(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, territory: Res<Territory>, order: Res<Towns>,
        atlas: Res<Atlas>, stocktakes: Res<Stocktakes>, services: Res<ServiceLedger>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        towns: Query<(&Town, &Money, &Stock, &Trades, &People, Option<&Roster>, Option<&Farm>, Option<&Herds>,
            Option<&Works>, Has<OffMap>, Has<Realm>)>, lockers: Query<&Lockers>, mut listed: Local<Listing>) {
    if !content.dials.services {
        return;
    }
    let (list, recipes) = listing(&mut listed, &content, Where::Abroad);
    let now = clock.now;
    let n = order.0.len();
    let all_routes: Vec<Route> = routes.iter().map(|(_, r, _, _)| *r).collect();
    let places = places(&content, n, towns.iter().map(|t| (t.0, t.8)), all_routes.iter());
    for e in in_order(routes.iter().map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, path) = routes.get_mut(e).unwrap();
        if state.plan.is_none() {
            continue;
        }
        let route = *route;
        let (Ok(home), Ok(far)) = (towns.get(route.from), towns.get(route.to)) else { continue };
        if home.10 || !crate::squad::crewed(home.5) {
            continue;
        }
        let roster = home.5.unwrap();
        let available = crate::squad::available(&content, roster, lockers.get(route.from).ok());
        let skills = skills(available.iter().map(|&i| &roster.hands[i]));
        let held: Vec<f64> = (0..content.goods.len()).map(|g| stocktakes.on_hand(route.to_id, g)).collect();
        let town = Served { id: route.to_id, settlement: !far.9 && !far.10, stock: &held,
            trades: far.3, people: far.4, roster: far.5, farm: far.6, herds: far.7, guards: guards(&territory, route.to_id),
            miners: crate::kit::miners(&content, &territory, route.to_id, far.5),
            routes_out: routes_out(all_routes.iter(), route.to_id) };
        let trip = Trip {
            route: route.seq,
            mapped_at: state.mapped_at,
            path: path.map_or(&[][..], |p| &p.0[..]),
            home_spare_plots: (!home.9).then(|| home.6.map_or(0, |f| f.spare_plots)),
            home_animals: home.7.map_or(0, |h| drivable(&content, h).len()),
            herd: &[],
            seller_level: crate::bargain::merchant_level(home.5),
            buyer_level: crate::bargain::merchant_level(far.5),
        };
        let mut trades: Vec<TradeId> = Vec::new();
        for service in list {
            let Some(&skill) = skills.get(&service.trade) else { continue };
            if trades.contains(&service.trade) {
                continue;
            }
            let visit = Visit { content: &content, from: route.from_id, at: route.to_id, skill, now, planning: true,
                budget: 0.0, town, trip: Some(trip), book: &book, atlas: &atlas, places: &places, ledger: &services,
                recipes };
            if wanted(service, &visit) {
                trades.push(service.trade);
            }
        }
        if state.services_wanted != trades {
            state.services_wanted = trades;
        }
    }
}

/// A wagon gone this look with a Shepherd aboard (not a porter's run): she drives up to [`HERD`] of home's grown
/// animals (`Caravans.dispatch` 383–384, 563: `Squad.herd`, the market pen's).
pub fn drive(clock: Res<Clock>, content: Res<Content>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut herds: Query<&mut Herds>) {
    if !content.dials.services || content.animals.is_empty() {
        return;
    }
    let Some(shepherd) = content.trade(crate::squad::SHEPHERD) else { return };
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.last_departure != Some(clock.now) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut().filter(|w| w.leg == Leg::Out && !w.porter && w.herd.is_empty()) else {
            continue;
        };
        if !wagon.crew.iter().any(|h| h.trade == shepherd) {
            continue;
        }
        let Ok(mut home) = herds.get_mut(route.from) else { continue };
        let herd = drivable(&content, &home);
        for &k in &herd {
            home.count[k] = (home.count[k] - 1.0).max(0.0); // (a whole one forgives float dust)
        }
        wagon.herd = herd;
    }
}

/// Animals put in a town's herd, a kind each, as far as its pastures have room (the core has no pen to keep the rest
/// in: a herd that grew back while they were away lets them go). Returns how many it took.
fn turn_out(content: &Content, herds: &mut Herds, animals: &[usize]) -> usize {
    let kinds = content.animals.len();
    if herds.count.len() < kinds {
        herds.count.resize(kinds, 0.0);
        herds.owed.resize(kinds, 0.0);
        herds.seeded.resize(kinds, false);
    }
    let mut room = (herds.capacity(content) - herds.count.iter().sum::<f64>()).max(0.0);
    let mut took = 0;
    for &k in animals.iter().filter(|&&k| k < kinds) {
        if room < 1.0 {
            break;
        }
        herds.count[k] += 1.0;
        room -= 1.0;
        took += 1;
    }
    took
}

/// At the far end, once the wagon has sold (`Services.abroad`, from `arriveAtDestination` after the sale): each of the
/// crew's services abroad, in their order, the most skilled of the trade aboard, that the far end wants and can pay
/// for from its money beyond its reserve (read again before each). Its fee, at most that, goes into the wagon's purse
/// ("service: NAME"), and each paid one counts `trust_per_service` of a kept installment between the two towns. What
/// it did goes into the far end; what the crew learned of rides home (`Wagon.news`).
#[allow(clippy::too_many_arguments)]
pub fn abroad(clock: Res<Clock>, content: Res<Content>, territory: Res<Territory>, order: Res<Towns>,
        mut book: ResMut<Book>, mut atlas: ResMut<Atlas>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut services: ResMut<ServiceLedger>, mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        mut towns: Query<ServedData>, mut listed: Local<Listing>) {
    if !content.dials.services {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let (list, recipes) = listing(&mut listed, &content, Where::Abroad);
    let all_routes: Vec<Route> = routes.iter().map(|(_, r, _, _)| *r).collect();
    let mut places: Option<Places> = None;
    for e in in_order(routes.iter().map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, path) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let state = &mut *state;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        if wagon.crew.is_empty() {
            continue;
        }
        let skills: BTreeMap<TradeId, u8> = skills(wagon.crew.iter());
        if !list.iter().any(|s| skills.contains_key(&s.trade)) {
            continue;
        }
        let places = places.get_or_insert_with(|| self::places(&content, order.0.len(), towns.iter().map(|t| (t.0, t.8)),
            all_routes.iter()));
        let path: Vec<crate::hex::Hex> = path.map_or_else(Vec::new, |p| p.0.clone());
        let seller_level = towns.get(route.from).map_or(1, |t| crate::bargain::merchant_level(t.5));
        let (guards, out) = (guards(&territory, route.to_id), routes_out(all_routes.iter(), route.to_id));
        for service in list {
            let Some(&skill) = skills.get(&service.trade) else { continue };
            let (done, budget) = {
                let Ok(t) = towns.get(route.to) else { break };
                let reserve = super::farms::reserve(&content, t.3, t.6);
                let town = Served { id: route.to_id, settlement: !t.9 && !t.10, stock: &t.2.0,
                    trades: t.3, people: t.4, roster: t.5, farm: t.6, herds: t.7, guards, routes_out: out,
                    miners: crate::kit::miners(&content, &territory, route.to_id, t.5) };
                let trip = Trip { route: route.seq, mapped_at: state.mapped_at, path: &path, home_spare_plots: None,
                    home_animals: 0, herd: &wagon.herd, seller_level, buyer_level: crate::bargain::merchant_level(t.5) };
                let budget = (t.1.0 - reserve).at_least_zero();
                let visit = Visit { content: &content, from: route.from_id, at: route.to_id, skill, now, planning: false,
                    budget: budget.to_f64(), town, trip: Some(trip), book: &book, atlas: &atlas, places, ledger: &services,
                    recipes };
                if !wanted(service, &visit) {
                    continue;
                }
                (perform(service, &visit), budget)
            };
            // The fee rounded up to the thousandth (a price paid), at most the budget.
            let fee = crate::fixed::Fixed::ceil(done.fee.max(0.0)).min(budget);
            let (_, mut money, mut stock, _, mut people, _, farm, herds, ..) = towns.get_mut(route.to).unwrap();
            put(&mut ledger, &done, &mut stock.0, &mut people, farm.map(|f| f.into_inner()));
            if !done.bought.is_empty() {
                let bought: Vec<usize> = done.bought.iter().map(|&i| wagon.herd[i]).collect();
                if let Some(mut herds) = herds {
                    turn_out(&content, &mut herds, &bought); // (bought only as far as the room: `services::perform`)
                }
                let mut i = 0;
                wagon.herd.retain(|_| {
                    i += 1;
                    !done.bought.contains(&(i - 1))
                });
                state.animals_sold += bought.len() as u32;
            }
            for &m in &done.taught {
                atlas.learn(route.to_id, m, now);
            }
            for &m in &done.heard {
                if !wagon.news.contains(&m) {
                    wagon.news.push(m);
                }
            }
            if let Some(seen) = done.mapped {
                state.mapped_at = Some(now);
                state.towns_mapped = state.towns_mapped.max(seen);
            }
            if fee.is_positive() {
                post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    fee, Reason::Other(format!("service: {}", service.name)), now, day);
                book.kept(route.from_id, route.to_id, content.dials.trust_per_service, now, day);
                state.service_paid += fee;
            }
            services.record(&service.name, route.from_id, route.to_id, fee, now);
        }
    }
}

/// A wagon home (`arriveHome` 1186, 1199): its town hears of the markets the crew learned of, and the animals that
/// didn't sell are home in its herd.
pub fn home_again(clock: Res<Clock>, content: Res<Content>, mut atlas: ResMut<Atlas>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut herds: Query<&mut Herds>) {
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        if wagon.news.is_empty() && wagon.herd.is_empty() {
            continue;
        }
        for m in std::mem::take(&mut wagon.news) {
            atlas.learn(route.from_id, m, clock.now);
        }
        let herd = std::mem::take(&mut wagon.herd);
        if let Ok(mut home) = herds.get_mut(route.from) {
            turn_out(&content, &mut home, &herd);
        }
    }
}
