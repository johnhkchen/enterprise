//! Freight hubs through the look (`freight.rs`; `Freight.onServerTick`, `considerHub`, `reorganise`; `Caravans.dispatch`,
//! `pickForwarded`, `setStop`, `callAtStop`, `atHub`, `arriveAtDestination`'s ticketed crates, `arriveHome`):
//!
//! - **Review**, once a day: a market the roads make a hub builds its storehouse ([`consider`]); then each hub's
//!   carriers' shops and draymen are restocked, its forwarder books the crates in its hands and buys its blocks of space,
//!   and the routes round it reorganise by price, one change a hub a day (a run of its own opened after the look, or a
//!   long run wound down: [`reorganise`]). Last in the step, after every other review.
//! - **Plan:** a wound-down route plans no more wagons ([`retire`]); a hub's run counts the crates booked on it (they
//!   load first, and their fares count toward its load's worth), and a carrier whose run home from a hub has a block of
//!   space sold goes on its day (`liner_days`) ([`plan_freight`], `PlanFreight`).
//! - **Decide**, as each wagon leaves: its crew and space kept for its freight's price; its calls, the far end and the
//!   stop on its way if its path crosses a hub's land, written first (`Itinerary`, `itinerary-drops`); from a hub, the crates booked on
//!   its run aboard first (two or more for one next stop as one consolidation, its superbill drawn at the hub:
//!   `freight::load_run`); to a hub, the goods its town owes markets the hub serves crated with a bill on each buyer
//!   ([`pick_forwarded`]) ([`depart`]); and, its stranded pallets
//!   aboard too, its manifest written, a superbill drawn at home for two lines or more ([`write_out`]).
//! - **Arrive**, before the arrivals are marked: a wagon reaching the hub on its way calls there ([`call_at_stop`]): the
//!   landing fee, food for the road home where the far end may not feed it, its crates bound for markets the hub serves
//!   handed over, its bills sold, the crates booked on its run loaded (consolidated as from a hub's own run), the block
//!   paid, the hub's contract copies for where it's bound carried on; then on along the road. Once the stranded sale is
//!   done too, a line still dropping there drops further on ([`close_call`]).
//!
//! **Drops** (`itinerary-drops`, `docs/pallets.md` section 3): every pallet loads with its line's drop, a call on its
//! itinerary: its consignee where a leg ends there, else the first hub ahead that forwards it ([`drop_for`]; no drop, no
//! load), a booked pallet its run's next stop, a hub's own wagon's pallets for its forwarder home. Each call drops by line
//! (`cargo::drop_off`): the forwarder takes the lines that drop at its hub (or that it has come to forward since they
//! loaded), freight and the far end's goods come off by line, and home takes all that's left; a line its call couldn't
//! take drops at the next that will, else home ([`drop_again`]). Each pickup fits every stretch to its drop
//! (`cargo::pick_up`, `room_to`).
//! - **Mail**, before the papers: crates aboard on freight tickets made out to the far end received there, the
//!   consolidations that come off there handed out whole first ([`deliver_freight`]).
//! - **Freight:** at a hub at the far end, the same dealings as at a stop, the run home's booked crates loaded
//!   ([`at_hub`]).
//! - **Turn:** home again, the crates on freight made out home received before the crew and cargo come home, the
//!   consolidations for home handed out whole first ([`deliver_home`]); a wagon turned for home writes its manifest (a
//!   superbill at its home for two lines or more) and sets the stop on its way back ([`turn_stop`]).

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::atlas::{Atlas, Whys, Why};
use crate::bank::Discounts;
use crate::bills::{BillState, Bills, Held as Paper};
use crate::cargo::{put_aboard, Paperwork, Shipments, Shipper};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::chokepoints::Takings;
use crate::content::Content;
use crate::contracts::{Book, State};
use crate::farms::Farm;
use crate::freight::*;
use crate::freight::Leg as RunLeg;
use crate::messages::{RouteId, TownId};
use crate::mint::Mint;
use crate::pallet::{At, Lading, Line, Pallet, PalletId, Pallets, Payment};
use crate::realms::OffMap;
use crate::roster::Roster;
use crate::fixed::Fixed;
use crate::state::*;
use crate::state::Leg;
use crate::territory::Territory;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        review.in_set(Step::Review).after(super::jobs::review).after(super::atlas::consider)
            .after(super::chokepoints::review).after(super::works::review).after(super::lockers::review)
            .after(super::farms::review).after(super::squad::train).after(super::settlers::review)
            .after(super::territory::review).after(super::training::lapse).after(super::works::culture),
        retire.in_set(Step::Plan).after(super::plan::start).before(super::squad::size_up)
            .before(super::atlas::want_books),
        plan_freight.in_set(Step::PlanFreight),
        depart.in_set(Step::Decide).after(super::cargo::ship).before(super::atlas::depart),
        write_out.in_set(Step::Decide).after(super::stranded::load).before(super::atlas::depart),
        call_at_stop.in_set(Step::Arrive).before(super::arrive::mark),
        close_call.in_set(Step::Arrive).after(super::stranded::sell_at_stop).before(super::arrive::mark),
        deliver_freight.in_set(Step::Mail),
        at_hub.in_set(Step::Freight),
        deliver_home.in_set(Step::Turn).after(super::atlas::hand_in).before(super::squad::come_home),
        turn_stop.in_set(Step::Turn).after(super::arrive::turn).after(super::road::feed),
    ));
}

fn today(content: &Content, now: u64) -> u64 {
    now / content.day_ticks.max(1)
}

/// What a town can spare beyond its reserve (trading capital and wage float: `Territory.reserve`).
fn spare(content: &Content, money: Fixed, trades: &Trades, farm: Option<&Farm>) -> Fixed {
    money - super::farms::reserve(content, trades, farm)
}

/// A town's passage fee at its posts, in spurs (none without a post: `Territory.passageFee`).
fn passage(territory: &Territory, town: TownId) -> f64 {
    territory.holding(town).filter(|h| !h.posts.is_empty()).map_or(0.0, |h| h.passage_fee)
}

/// Whether the hub at `at` hands goods made out to `to` on, for a wagon of `route` calling there (`handOver`'s offer):
/// a hub, `to` neither it nor one of the route's ends (those a wagon delivers itself), and a carrier's run from it that
/// way (the forwarding has a price).
pub(crate) fn forwards(net: &Network, route: &Route, to: TownId, at: TownId) -> bool {
    net.is_hub(at) && to != at && to != route.from_id && to != route.to_id && !net.legs(at, to).is_empty()
}

/// Where a pallet made out to `to` drops, loaded on the route's wagon riding from stretch `from` (`itinerary-drops`:
/// [`Itinerary::drop_for`], the hubs that forward it as [`forwards`] says, none without a network).
pub(crate) fn drop_for(net: Option<&Network>, route: &Route, wagon: &Wagon, to: TownId, from: usize) -> Option<TownId> {
    let out = wagon.leg == Leg::Out;
    let leg_end = if out { route.to_id } else { route.from_id };
    wagon.itinerary.drop_for(to, from, out, leg_end, route.from_id, |at| net.is_some_and(|n| forwards(n, route, to, at)))
}

/// A line whose call has passed without it coming off (a forwarder that couldn't take it, a leg turned) drops at the
/// next call ahead that takes it, riding from stretch `from`; with none, at the carrier's home (its own premises: it comes
/// back, as a pallet that wasn't forwarded always has).
pub(crate) fn drop_again(net: Option<&Network>, route: &Route, pallets: &Pallets, wagon: &mut Wagon, id: PalletId, from: usize) {
    let Some(to) = pallets.get(id).map(|p| p.lading.consignee) else { return };
    let drop = drop_for(net, route, wagon, to, from).unwrap_or(route.from_id);
    if let Some(l) = wagon.manifest.line_mut(id) {
        l.drop = Some(drop);
    }
}

/// The network as the routes and towns stand.
fn network<'a>(content: &Content, routes: impl Iterator<Item = (&'a Route, &'a RouteState, Option<&'a RoutePath>)>,
        towns: impl Iterator<Item = (&'a Town, &'a Hub)>) -> Network {
    Network::of(content, routes, towns.map(|(t, h)| (t.id, t.hex, h.storehouse)))
}

/// The world's routes and towns as the freight rules read them (built by each system from its own queries).
macro_rules! network_of {
    ($content:expr, $routes:expr, $towns:expr) => {{
        let runs: Vec<(&Route, &RouteState, Option<&RoutePath>)> = $routes;
        let towns: Vec<(&Town, &Hub)> = $towns;
        network($content, runs.into_iter(), towns.into_iter())
    }};
}

/// Once a day, last of the reviews: markets the roads make hubs build their storehouses; then every hub (in town order)
/// restocks its carriers' shops and its draymen, books what its forwarder holds, buys its blocks of space, and
/// reorganises the routes round it (`Freight.onServerTick`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn review(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, territory: Res<Territory>,
        takings: Res<Takings>, mut freight: ResMut<Freight>, mut whys: ResMut<Whys>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, pallets: Res<Pallets>,
        mut towns: Query<(Entity, &Town, &mut Hub, &mut Money, &Stock, &Trades, Option<&Farm>, Option<&Roster>,
            Option<&OffMap>)>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>) {
    let (now, day) = (clock.now, content.day_ticks);
    let today = today(&content, now);
    if freight.day == Some(today) {
        return;
    }
    freight.day = Some(today);
    let d = &content.dials;
    let rate = mint.rate(now, day);
    let mut order: Vec<(TownId, Entity)> = towns.iter().map(|(e, t, ..)| (t.id, e)).collect();
    order.sort();
    let names: BTreeMap<TownId, String> = towns.iter().map(|(_, t, ..)| (t.id, t.name.clone())).collect();
    let name = |t: TownId| names.get(&t).cloned().unwrap_or_default();
    // Markets the roads make hubs (`considerHub`): routes of others pass within a hex, enough pass or end there, the
    // Merchant is in, it can feed callers for their road home, and it has the storehouse's price beyond its reserve.
    macro_rules! build {
        () => {
            network_of!(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)).collect(),
                towns.iter().map(|(_, t, h, ..)| (t, h)).collect())
        };
    }
    let net = build!();
    let mut more = false;
    for &(_, e) in &order {
        let (_, town, mut hub, mut money, stock, trades, farm, roster, off) = towns.get_mut(e).unwrap();
        if hub.storehouse || off.is_some() {
            continue;
        }
        let passing = net.through(town.id).len();
        if passing == 0 {
            continue;
        }
        let calls = net.calls(town.id);
        if passing < 2 && passing + calls < d.hub_routes as usize {
            continue;
        }
        // (a market with a Merchant: one without a roster has its own)
        if roster.is_some_and(|r| crate::squad::crewed(Some(r)) && r.merchant.is_none()) {
            continue;
        }
        let spare = spare(&content, money.0, trades, farm);
        let storehouse = Fixed::ceil(d.storehouse_emeralds);
        if crate::road::held(&content, &stock.0) < crate::road::FOOD_PAR_POINTS || spare < storehouse {
            continue;
        }
        post(&mut ledger, &mut cash, Side::treasury(town.id, &mut money.0), Side::outside(), storehouse,
            crate::freight::storehouse_reason(), now, day);
        hub.storehouse = true;
        more = true;
        whys.why(town.id, None, None, Why::Note(format!("a freight hub now: {passing} routes of others pass through, {calls} call")),
            0.0, today);
    }
    let net = if more { build!() } else { net };
    // Each hub's day: its shops and draymen, its forwarder's bookings and blocks, and the routes round it.
    let mut wind: Vec<RouteId> = Vec::new();
    for &(id, e) in &order {
        let (_, _, mut hub, money, _, trades, farm, roster, _) = towns.get_mut(e).unwrap();
        if !hub.storehouse {
            continue;
        }
        let idle = roster.map_or(0, |r| r.idle(&content).len());
        hub.restock(&content, &net, id, idle);
        let mut can = spare(&content, money.0, trades, farm);
        hub.forward(&content, &net, id, &pallets, now, rate, &mut can);
        hub.buy_blocks(&content, &net, id);
        match reorganise(&content, &net, &territory, &takings, id, rate, &freight.pending, &wind) {
            Some(Reorganise::Open { end, caller }) => {
                freight.pending.push((id, end));
                whys.why(id, Some(end), None, Why::Note(format!("opened a run of its own to {}: {}'s route calls here",
                    name(end), name(caller))), 0.0, today);
            }
            Some(Reorganise::WindDown { route, owner, to, by_hub, direct, open }) => {
                if open {
                    freight.pending.push((owner, id));
                }
                wind.push(route);
                whys.why(owner, Some(to), None, Why::Note(format!("wound down its run to {}: by {} it costs {:.1} a trip, not {:.1}",
                    name(to), name(id), by_hub, direct)), 0.0, today);
            }
            None => {}
        }
    }
    for (_, route, mut state, _) in &mut routes {
        if wind.contains(&route.seq) {
            state.freight.wound_down = Some(now);
        }
    }
}

/// What a hub's routes do today (`reorganise`).
#[derive(Clone, Debug, PartialEq)]
pub enum Reorganise {
    /// The hub opens a run of its own to `end`, a far end of `caller`'s route that calls there.
    Open { end: TownId, caller: TownId },
    /// `owner` winds its run to `to` down (opening a short run to the hub if `open`): by the hub `by_hub` a trip,
    /// not `direct`.
    WindDown { route: RouteId, owner: TownId, to: TownId, by_hub: f64, direct: f64, open: bool },
}

/// The routes round a hub reorganise by price, one change a day (`Freight.reorganise`): the hub opens a run of its own
/// to a far end of an established route calling there (within a leg's food), where it has none either way; else the
/// owner of a long run calling there, where the hub runs on to its far end (and has made `ONWARD_TRIPS`), compares its
/// run's food, the tolls it pays and the landing against a short run to the hub, the landing and the freight on from
/// there for its usual load (carted, with the forwarder's margin), and winds the long run down at `SWITCH_SHARE` or
/// less. `pending` and `wound`: runs opened and wound down already today.
#[allow(clippy::too_many_arguments)]
pub fn reorganise(content: &Content, net: &Network, territory: &Territory, takings: &Takings, hub: TownId, rate: f64,
        pending: &[(TownId, TownId)], wound: &[RouteId]) -> Option<Reorganise> {
    let d = &content.dials;
    let calling: Vec<&Run> = net.runs.values()
        .filter(|r| r.stop.is_some_and(|(h, _)| h == hub) && !wound.contains(&r.route)).collect();
    let exists = |a: TownId, b: TownId| net.route_between(a, b).is_some() || pending.contains(&(a, b));
    for r in &calling {
        for end in [r.from, r.to] {
            let Some(distance) = net.distance(end, hub) else { continue };
            if r.trips >= ESTABLISHED_TRIPS && !exists(hub, end) && !exists(end, hub)
                    && crate::road::in_range(content, distance) {
                return Some(Reorganise::Open { end, caller: r.from });
            }
        }
    }
    for r in &calling {
        let Some(onward) = net.route_between(hub, r.to) else { continue };
        if onward.trips < ONWARD_TRIPS || r.trips < ESTABLISHED_TRIPS {
            continue; // the hub doesn't run there yet (or not reliably)
        }
        let Some(to_hub) = net.distance(r.from, hub) else { continue };
        let per_trip = r.traded / r.trips.max(1) as f64;
        let stacks = (per_trip / STACK_ITEMS).ceil().max(1.0);
        // The long run pays the posts on its way (a share of its coins, and the handling fee), as the mod reckons it
        // ahead: unrounded, every post's share of the whole (`Caravans.tollShare`, `passageFees`).
        let (share, fees) = toll_terms(territory, &[r.from, r.to], &r.path, d.default_toll_rate);
        let tolls = (share * takings.expected_value(r.route) + fees) / rate;
        let landing = stop_price(content, passage(territory, hub), rate) / rate; // both land at the hub: the long run calls there
        let direct = run_cost(content, r.blocks) + tolls + landing;
        let onward_leg = RunLeg { route: onward.route, home: onward.from, far: onward.to, outbound: true, at: onward.from };
        let cheapest = net.legs(hub, r.to).iter().map(|l| forwarding_price(content, net, l)).fold(f64::INFINITY, f64::min);
        let cheapest = if cheapest.is_finite() { cheapest } else { forwarding_price(content, net, &onward_leg) };
        let by_hub = run_cost(content, to_hub) + landing
            + stacks * (cheapest + cart_price(content) * 2.0 / CART as f64 * (1.0 + d.forward_margin));
        if by_hub <= direct * SWITCH_SHARE && crate::road::in_range(content, to_hub) {
            return Some(Reorganise::WindDown { route: r.route, owner: r.from, to: r.to, by_hub, direct,
                open: !exists(r.from, hub) });
        }
    }
    None
}

/// What a run pays the posts on its path, reckoned ahead (`Caravans.tollShare`, `passageFees`): the share of its coins
/// the posted towns take between them (one less what each leaves of the rest) and their passage fees summed, in spurs;
/// the ends' own posts free, each posted town once, in the order the path meets them.
fn toll_terms(territory: &Territory, ends: &[TownId], path: &[crate::hex::Hex], default_rate: f64) -> (f64, f64) {
    let mut posted: Vec<TownId> = Vec::new();
    for &hex in path {
        let owner = territory.owner(hex).filter(|o| !ends.contains(o) && !posted.contains(o)
            && territory.holding(*o).is_some_and(|h| h.posts.contains(&hex)));
        posted.extend(owner);
    }
    let kept: f64 = posted.iter().map(|&o| 1.0 - territory.holding(o).and_then(|h| h.toll_rate).unwrap_or(default_rate)).product();
    (1.0 - kept, posted.iter().map(|&o| passage(territory, o)).sum())
}

/// A wound-down route plans no more wagons (the mod's `removeRoute`): its wagon on the road comes home, and stays.
pub fn retire(mut routes: Query<&mut RouteState>) {
    for mut state in &mut routes {
        if state.freight.wound_down.is_some() && state.plan.is_some() {
            state.plan = None;
        }
    }
}

/// In the plan: from a hub, the crates booked on its run load first (`dispatch` 390), their fares toward the load's
/// worth (`booked * legPrice`). To a hub: the goods its town owes markets the hub serves, sent on through it, count in
/// its load as what's owed does (`owedOn`'s hub branch: owed `FORWARD_LEAD_DAYS` beyond its arrival, pressing if waiting
/// a day would miss an installment), their room kept for them; and with the forwarder's block of space on its way home,
/// the block's fares too, and it goes on its day, `liner_days` after the last (`space sold, on its day`).
#[allow(clippy::type_complexity)]
pub fn plan_freight(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, towns: Query<(&Town, &Hub, &Stock)>,
        pallets: Res<Pallets>, mut routes: Query<(&Route, &mut RouteState, Option<&RoutePath>)>) {
    if !towns.iter().any(|(_, h, _)| h.storehouse) {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let net = network_of!(&content, routes.iter().collect(), towns.iter().map(|(t, h, _)| (t, h)).collect());
    let hubs: BTreeMap<TownId, &Hub> = towns.iter().map(|(t, h, _)| (t.id, h)).collect();
    let liner = (content.dials.liner_days * day as f64) as u64;
    let grace = (crate::contracts::GRACE_DAYS * day as f64) as u64;
    let lead = (FORWARD_LEAD_DAYS * day as f64) as u64;
    for (route, mut state, _) in &mut routes {
        let last = state.last_departure;
        let Some(plan) = state.plan.as_mut() else { continue };
        if net.is_hub(route.from_id) {
            let leg = RunLeg::of(route, true);
            let booked = hubs.get(&route.from_id).map_or(0, |h| h.booked_on(&leg));
            if booked > 0 {
                plan.booked = booked;
                plan.worth += booked as f64 * leg_price(&content, &net, &leg);
                // Their weight, for the carrier's weight cap (`pallet-caps`).
                if let Some(h) = hubs.get(&route.from_id) {
                    plan.booked_weight = pallets.at_place(At::Custody(route.from_id))
                        .filter(|p| h.custody.get(&p.id).and_then(|c| c.booked).is_some_and(|b| b.leg.key() == leg.key()))
                        .map(|p| crate::pallet::Load::of_pallet(&content, p).weight).sum();
                }
            }
            continue;
        }
        if !net.is_hub(route.to_id) {
            continue;
        }
        // What its town owes markets beyond the hub, sent on through it.
        let Ok((_, _, stock)) = towns.get(route.from) else { continue };
        let there = now + route.travel_ticks + lead;
        for &id in &book.open {
            let c = &book.contracts[id];
            if !c.in_force() || c.seller != route.from_id || c.buyer == route.to_id || c.buyer == route.from_id
                    || !c.incoterm.seller_carries() || c.training.is_some() || net.legs(route.to_id, c.buyer).is_empty() {
                continue;
            }
            let going = (c.owed(there) - c.shipped).min(stock.0[c.good] - plan.cargo[c.good]).floor();
            if going <= 0.0 {
                continue;
            }
            plan.booked += crate::cargo::stacks_of(&content, c.good, going);
            plan.booked_weight += going * content.goods[c.good].weight;
            plan.worth += going * c.price;
            let counts = if c.incoterm.seller_bears_road() { there + day } else { now + day };
            plan.pressing |= c.next_deadline(c.wait_ticks(grace, day)).is_some_and(|deadline| counts > deadline);
        }
        let home = RunLeg::of(route, false);
        let Some(hub) = hubs.get(&route.to_id) else { continue };
        if hub.has_block(&home) {
            plan.worth += hub.block_fares(&home).to_f64();
            if last.is_none_or(|t| now - t >= liner) {
                plan.pressing = true; // space sold, on its day
            }
        }
    }
}

/// As each wagon leaves (`dispatch`): its crew with the team and its space kept for its freight's price (`lastCrew`,
/// `lastCapacity`); its calls, the far end and the hub on its way (`itinerary-drops`; `setStop`); from a hub, the crates booked on its run aboard first, as far as there's room (`loadBooked`; two or
/// more for one next stop as a consolidation: [`load_run`]); to a hub, its town's goods for markets the hub serves crated
/// and billed (`pickForwarded`). Its manifest is written once its stranded pallets are aboard
/// too ([`write_out`]).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn depart(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, territory: Res<Territory>,
        mut book: ResMut<Book>, mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        towns_list: Res<Towns>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        mut towns: Query<(&Town, &mut Hub, &mut Stock, &mut Money, &Trades, Option<&Farm>, &mut Shipments, &mut Paperwork)>) {
    let now = clock.now;
    let d = &content.dials;
    let left: Vec<Entity> = routes.iter().filter(|(_, _, s, _)| s.last_departure == Some(now)
        && s.wagon.as_ref().is_some_and(|w| w.leg == Leg::Out && w.freight.reckoned != Some(Leg::Out)))
        .map(|(e, ..)| e).collect();
    if left.is_empty() {
        return;
    }
    let net = network_of!(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)).collect(),
        towns.iter().map(|(t, h, ..)| (t, h)).collect());
    let rate = mint.rate(now, content.day_ticks);
    for e in in_order(routes.iter().filter(|(e, ..)| left.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let state = &mut *state;
        let wagon = state.wagon.as_mut().unwrap();
        let slots = if wagon.slots > 0 { wagon.slots } else { d.wagon_slots };
        let team = if wagon.porter { 0 } else { d.wagon_team };
        state.freight.crew = Some(crate::road::mouths(Some(wagon)) + team as f64);
        state.freight.capacity = Some(slots);
        // Its calls (`itinerary-drops`): the far end, and the hub on its way if its path crosses one's land (`setStop`),
        // so far along the road and the rest kept for after; written before anything else loads, each pallet's drop
        // one of them.
        wagon.itinerary = Itinerary::direct(route.to_id, wagon.arrives.saturating_sub(now));
        if let Some((hub, i)) = net.stop_on(route.seq) {
            let path = net.run(route.seq).map_or(1, |r| r.path.len().max(2) - 1);
            set_stop(wagon, now, hub, i as f64 / path as f64);
        }
        if net.is_hub(route.from_id) {
            // The crates booked on this run load first: they're paid for. Two or more for one next stop go as a
            // consolidation, its superbill drawn at the hub (`superbills`). Each drops at its run's next stop, as far as
            // every stretch to it has room.
            let (_, mut hub, _, _, _, _, _, mut paperwork) = towns.get_mut(towns_list.0[route.from_id]).unwrap();
            let leg = RunLeg::of(&route, true);
            let room = crate::cargo::room_to(&content, &pallets, route.seq, wagon, 0, Some(leg.to()));
            let load = hub.load_booked(&content, &pallets, route.from_id, &leg, room);
            load_run(&mut pallets, wagon, route.seq, route.from_id, &mut paperwork, load, now);
        }
        if net.is_hub(route.to_id) {
            let (_, _, mut stock, mut money, trades, farm, mut store, mut paperwork) =
                towns.get_mut(towns_list.0[route.from_id]).unwrap();
            let can = spare(&content, money.0, trades, farm);
            let mut shipper = Shipper { town: route.from_id, money: &mut money.0, spare: can, store: &mut store,
                paperwork: &mut paperwork };
            let fees = pick_forwarded(&content, &net, &mut book, &mut bills, &mut ledger, &mut cash, &mut pallets, now, rate,
                &route, &mut stock.0, &mut shipper, wagon);
            if fees.is_positive() {
                // The forwarding goes with the wagon, from what the town can spare, and its landing at the hub.
                let landing = stop_fee(&content, passage(&territory, route.to_id), rate);
                let purse = (fees + landing).min(money.0.at_least_zero());
                post(&mut ledger, &mut cash, Side::treasury(route.from_id, &mut money.0),
                    Side::wagon(route.seq, &mut wagon.purse), purse, Reason::WagonPurse, now, content.day_ticks);
            }
        }
        wagon.freight.reckoned = Some(Leg::Out);
    }
}

/// A wagon that left this look writes its manifest once everything has gone aboard (after `stranded::load`, whose
/// stranded pallets are lines too): two lines or more, a superbill drawn at its home (`superbills`).
pub fn write_out(clock: Res<Clock>, routes: Query<(&Route, &RouteState)>, mut papers: Query<&mut Paperwork>) {
    let now = clock.now;
    for (route, state) in &routes {
        let Some(wagon) = state.wagon.as_ref() else { continue };
        if state.last_departure != Some(now) || wagon.leg != Leg::Out || wagon.freight.reckoned != Some(Leg::Out) {
            continue;
        }
        if let Ok(mut paperwork) = papers.get_mut(route.from) {
            crate::cargo::write_manifest(&mut paperwork, &wagon.manifest, now);
        }
    }
}

/// The wagon calls at `hub` on its leg, `share` of the way along: a call on its itinerary before the leg's end, and it gets
/// there so far along, the rest kept for after (`setStop`).
fn set_stop(wagon: &mut Wagon, now: u64, hub: TownId, share: f64) {
    let to = wagon.itinerary.call_on_way(hub, share);
    wagon.arrives = now + to;
}

/// Goods the town owes under contracts to markets beyond the hub at the far end, where a carrier runs on there
/// (`Caravans.pickForwarded`): its crates made out to those buyers that came home unforwarded first (with the bills
/// on them it holds), then each contract's goods owed by the time they'd get there (`FORWARD_LEAD_DAYS` beyond the
/// wagon's arrival), crated with a bill of lading and a copy, made out to the buyer, and a bill of exchange drawn on the
/// buyer for them (the forwarder at the hub may buy it), as far as the wagon has room, the goods can be crated, and the
/// town can spare the forwarding for them. Returns the forwarding the wagon is to carry for them (prepaid, as a town
/// pays a hub's caravan: `collectForwarding`).
#[allow(clippy::too_many_arguments)]
fn pick_forwarded(content: &Content, net: &Network, book: &mut Book, bills: &mut Bills, ledger: &mut Ledger,
        cash: &mut CashBook, pallets: &mut Pallets, now: u64, rate: f64, route: &Route, stock: &mut [f64],
        shipper: &mut Shipper, wagon: &mut Wagon) -> Fixed {
    let (home, hub) = (route.from_id, route.to_id);
    let day = content.day_ticks;
    let serves = |buyer: TownId| buyer != hub && buyer != home && !net.legs(hub, buyer).is_empty();
    let mut fees = Fixed::ZERO;
    // Pallets it sent before that came home: out again, with their bills.
    let at_home: Vec<PalletId> = pallets.at(At::Town(home)).to_vec();
    for id in at_home {
        let s = pallets.get(id).unwrap();
        let to = s.lading.consignee;
        // It drops at the first hub ahead that forwards it (`itinerary-drops`), as far as every stretch to there has room
        // in slots, weight and pallets (`pallet-caps`).
        let drop = drop_for(Some(net), route, wagon, to, 0);
        let room = drop.is_some() && crate::cargo::fits_to(content, pallets, route.seq, wagon, s, 0, drop);
        let fee = Some(to).filter(|&b| serves(b)).and_then(|b| forwarding_fee(content, net, hub, b, s.slots(content), rate).map(|f| f.0));
        let Some(fee) = fee.filter(|f| s.lading.shipper == home && s.lading.contract.is_some() && room && shipper.spare >= *f) else {
            continue;
        };
        shipper.spare -= fee;
        fees += fee;
        let contract = s.lading.contract;
        for &b in &bills.open {
            let b = &mut bills.bills[b];
            if b.held == Paper::Town(home) && b.payee == home && b.drawee == to && b.contract == contract
                    && b.state == BillState::Outstanding {
                b.held = Paper::Wagon;
                wagon.bills.push(b.id);
            }
        }
        put_aboard(pallets, wagon, route.seq, Line { pallet: id, drop, ..Default::default() }, home, now);
    }
    let arrival = now + route.travel_ticks + (FORWARD_LEAD_DAYS * day as f64) as u64;
    let ids: Vec<usize> = book.open.clone();
    for id in ids {
        let c = &book.contracts[id];
        if !c.in_force() || c.seller != home || !c.incoterm.seller_carries() || !serves(c.buyer) || c.training.is_some() {
            continue;
        }
        let g = c.good;
        let owed = (c.owed(arrival) - c.shipped).floor();
        // Dropped at the first hub ahead that forwards it to the buyer (`itinerary-drops`: the hub at the far end does).
        let Some(drop) = drop_for(Some(net), route, wagon, c.buyer, 0) else { continue };
        // As far as the wagon has room for it packed on pallets, on every stretch to there: slots, weight and pallets
        // (`pallet-caps`).
        let room = crate::cargo::room_to(content, pallets, route.seq, wagon, 0, Some(drop)).palleted(content, g);
        let n = owed.min(room).min(stock[g].max(0.0).floor());
        if n <= 0.0 {
            continue;
        }
        let (buyer, price) = (c.buyer, c.price);
        // The forwarding as it will be charged: each pallet's, as packing will pallet it (each rounds its own carts up).
        let Some(fee) = crate::cargo::pallet_loads(content, g, n).iter()
            .map(|load| forwarding_fee(content, net, hub, buyer, load.iter().map(|pk| pk.slots(content)).sum::<u32>().max(1), rate)
                .map(|f| f.0))
            .sum::<Option<Fixed>>().filter(|f| shipper.spare >= *f) else { continue };
        // Only what can be crated goes: loose goods for a market beyond would be sold at the hub as the wagon's own.
        // A pallet each pallet's worth, a bill of lading each (`pallet-caps`); the forwarding each pallet's, as the hub
        // takes them one by one (`Hub::hand_over`).
        let lading = Lading { contract: Some(id), copy: true, ..Lading::new(home, buyer, now) };
        let ids = crate::cargo::pack_pallets(content, ledger, cash, now, shipper, pallets, &lading, g, n, At::Town(home));
        if ids.is_empty() {
            continue;
        }
        let count = crate::cargo::count_of(pallets, &ids);
        let fee = ids.iter().map(|p| pallets.get(*p).unwrap().slots(content))
            .map(|slots| forwarding_fee(content, net, hub, buyer, slots, rate).map(|f| f.0))
            .sum::<Option<Fixed>>().unwrap_or(fee);
        shipper.spare -= fee;
        fees += fee;
        stock[g] -= count;
        // Sent on with a bill of exchange drawn on the buyer for it (the mod's `billed` lading), the bill naming them.
        let bill = bills.draw_for(buyer, home, Fixed::ceil(count * price), now, day, Some(id));
        bills.bills[bill].pallets.extend(ids.iter().copied());
        wagon.bills.push(bill);
        book.ship(id, count);
        for p in ids {
            pallets.pallets.get_mut(&p).unwrap().lading.payment = Payment::ByBill(bill);
            put_aboard(pallets, wagon, route.seq, Line { pallet: p, drop: Some(drop), ..Default::default() }, home, now);
        }
    }
    fees
}

/// What the forwarder takes off a wagon at the hub, and what it pays the wagon, from the hub's money and to its
/// treasury: the landing fee first ([`land`]), then [`deal`].
struct AtHub<'a> {
    content: &'a Content,
    net: &'a Network,
    hub: TownId,
    route: Route,
    leg: RunLeg,
    now: u64,
    rate: f64,
}

/// The wagon's landing fee at the hub, from its purse as far as it goes, into the hub's treasury (`Freight.land`).
fn land(at: &AtHub, territory: &Territory, ledger: &mut Ledger, cash: &mut CashBook, hub: &mut Hub, money: &mut Money,
        wagon: &mut Wagon) {
    let fee = stop_fee(at.content, passage(territory, at.hub), at.rate).min(wagon.purse.at_least_zero());
    if fee > 0.0 {
        post(ledger, cash, Side::wagon(at.route.seq, &mut wagon.purse), Side::treasury(at.hub, &mut money.0), fee,
            stop_fee_reason(), at.now, at.content.day_ticks);
        hub.book.landing += fee;
    }
}

/// The wagon's pallets bound on (`Forwarder.handOver`), among `among` (those aboard it, in their order; not those for
/// `skip`, nor those on freight tickets or bought for its own forwarder, nor those `hold` keeps back): the forwarder looks
/// them over, and those whose line drops here (`itinerary-drops`: the first hub ahead that forwards them, as they loaded,
/// or this one if it has come to since) are handed to it, into its custody, the forwarding and drayage paid from its purse as far as it goes; those it couldn't
/// take drop at the next call ahead that forwards them, else back home ([`drop_again`]). Returns `among` less those it
/// took, in their new order: those it looked over and kept go behind the rest.
#[allow(clippy::too_many_arguments)]
pub(crate) fn hand_over(content: &Content, net: &Network, id: TownId, now: u64, rate: f64, route: &Route, skip: &[TownId],
        ledger: &mut Ledger, cash: &mut CashBook, pallets: &mut Pallets, hub: &mut Hub, money: &mut Money,
        wagon: &mut Wagon, among: Vec<PalletId>, hold: &dyn Fn(&Pallet) -> bool) -> Vec<PalletId> {
    let day = content.day_ticks;
    let seq = route.seq;
    let (offered, rest): (Vec<PalletId>, Vec<PalletId>) = among.into_iter().partition(|&p| {
        let s = pallets.get(p).unwrap();
        let c = s.lading.consignee;
        c != id && !skip.contains(&c) && !wagon.manifest.booked(p)
            && !wagon.manifest.for_forwarder(p) // (nor those it bought or collected for its own forwarder)
            && !hold(s)
    });
    if offered.is_empty() {
        return rest;
    }
    // A line this hub forwards now drops here, whatever drop it loaded with: a run that way opened since (the routes
    // reorganise each day), so the first hub ahead that forwards it is this one.
    for &p in &offered {
        let to = pallets.get(p).unwrap().lading.consignee;
        if forwards(net, route, to, id) {
            if let Some(l) = wagon.manifest.line_mut(p) {
                l.drop = Some(id);
            }
        }
    }
    // The lines that drop here: the forwarder's to take.
    let due: Vec<PalletId> = offered.iter().copied().filter(|&p| wagon.manifest.line(p).is_some_and(|l| l.drop == Some(id)))
        .collect();
    let crates: Vec<(TownId, u32, u32)> = due.iter().map(|&p| {
        let s = pallets.get(p).unwrap();
        (s.lading.consignee, s.slots(content), s.crates())
    }).collect();
    let held = crate::freight::custody_crates(pallets, id);
    let (taken, which) = hub.hand_over(content, net, id, held, &crates, wagon.purse.at_least_zero(), rate, skip);
    // Off the wagon by line, into custody in the order the forwarder took them.
    let off: Vec<PalletId> = which.iter().map(|&i| due[i]).collect();
    crate::cargo::drop_off(pallets, wagon, seq, id, false, now, |p, _| off.contains(&p.id));
    for &p in &off {
        pallets.move_to(p, At::Custody(id), now);
        hub.custody.insert(p, Custody { carted: true, booked: None });
    }
    // What it couldn't take drops further on.
    let from = wagon.itinerary.after(id);
    for (i, &p) in due.iter().enumerate() {
        if !which.contains(&i) {
            drop_again(Some(net), route, pallets, wagon, p, from);
        }
    }
    if taken.stacks > 0 {
        post(ledger, cash, Side::wagon(seq, &mut wagon.purse), Side::treasury(id, &mut money.0),
            taken.fee - taken.drayage, forwarding_reason(), now, day);
        post(ledger, cash, Side::wagon(seq, &mut wagon.purse), Side::treasury(id, &mut money.0), taken.drayage,
            drayage_reason(), now, day);
    }
    let kept = offered.iter().copied().filter(|p| !off.contains(p));
    rest.iter().copied().chain(kept).collect()
}

/// At the hub (`atHub`, `callAtStop`): the wagon hands the forwarder its crates bound for markets the hub serves (not
/// those for `skip`: where it's bound, or its route's ends), paying the forwarding and drayage from its purse; the
/// forwarder buys its bills on those markets; books what it holds; loads the crates booked on the wagon's run (their
/// fares paid it); and pays for its block on the run.
#[allow(clippy::too_many_arguments)]
fn deal(at: &AtHub, skip: &[TownId], book: &Book, bills: &mut Bills, ledger: &mut Ledger, cash: &mut CashBook,
        pallets: &mut Pallets, hub: &mut Hub, money: &mut Money, stock: &Stock, trades: &Trades, farm: Option<&Farm>,
        paperwork: &mut Paperwork, wagon: &mut Wagon, strand: Option<TownId>) {
    let (content, net, id) = (at.content, at.net, at.hub);
    let (now, day) = (at.now, content.day_ticks);
    let seq = at.route.seq;
    // Its crates bound on, handed over (those on freight tickets already are for its own run's end), but those a stranded
    // sale takes (`strand`: the town whose paper the hub buys, for a wagon that sells it): the mod sells those first
    // (`sellStrandedAtHub`), and the hub's bill for them is the forwarding, so they don't go to the forwarder for it.
    let counting = trades.counting_house;
    let hold = |s: &Pallet| strand.is_some_and(|town| crate::stranded::crate_ok(&crate::stranded::Terms {
        content, net, book, hub: id, town, counting, now, rate: at.rate }, s));
    let aboard = pallets.at(At::Wagon(seq)).to_vec();
    let left = hand_over(content, net, id, now, at.rate, &at.route, skip, ledger, cash, pallets, hub, money, wagon, aboard,
        &hold);
    pallets.arrange(At::Wagon(seq), left);
    // Its bills on markets the hub runs to, payable to the wagon's home and not yet due: bought for coin now.
    let mut can = spare(content, money.0, trades, farm);
    let fill = fill(content, stock, pallets, id);
    let means = can;
    let mut taking = Fixed::ZERO;
    let mut kept = Vec::new();
    for b in std::mem::take(&mut wagon.bills) {
        let bill = &bills.bills[b];
        if bill.state != BillState::Outstanding || bill.payee != at.route.from_id || bill.due <= now
                || net.legs(id, bill.drawee).is_empty() {
            kept.push(b);
            continue;
        }
        let q = Discounts::quote(bills, book, id, counting, bill, false, taking, fill, means, now, day, &content.dials);
        // Its face less the discount, rounded down to the thousandth (what the hub pays out: `Bank.deal`).
        let price = bill.amount.mul_floor(q.share);
        if !q.taken() || !price.is_positive() || can < price {
            kept.push(b);
            continue;
        }
        can -= price;
        taking += bill.amount;
        post(ledger, cash, Side::treasury(id, &mut money.0), Side::wagon(seq, &mut wagon.purse), price,
            crate::bank::discount_reason(), now, day);
        hub.book.bills += 1;
        hub.book.face += bill.amount;
        bills.endorse(b, id);
        bills.bills[b].held = Paper::Town(id);
    }
    wagon.bills = kept;
    // The forwarder books what it holds, then the crates booked on this run load, first.
    let mut can = spare(content, money.0, trades, farm);
    hub.forward(content, net, id, pallets, now, at.rate, &mut can);
    // As far as every stretch to the run's next stop has room: slots, weight and pallets (`pallet-caps`, `itinerary-drops`).
    let from = wagon.itinerary.after(id);
    let room = crate::cargo::room_to(content, pallets, seq, wagon, from, Some(at.leg.to()));
    let load = hub.load_booked(content, pallets, id, &at.leg, room);
    for (_, booking) in &load {
        // The fare the hub owes this carrier for the space, paid as its run loads (the mod's shop takings).
        let fare = booking.fare.min(money.0.at_least_zero());
        if fare > 0.0 {
            post(ledger, cash, Side::treasury(id, &mut money.0), Side::wagon(seq, &mut wagon.purse), fare, fares_reason(),
                now, day);
        }
    }
    // Aboard, two or more for one next stop as a consolidation, its superbill drawn here (`superbills`).
    load_run(pallets, wagon, seq, id, paperwork, load, now);
    // The forwarder's block on the run, paid, full or not.
    let cost = hub.run_block(content, &at.leg).min(money.0.at_least_zero());
    if cost > 0.0 {
        post(ledger, cash, Side::treasury(id, &mut money.0), Side::wagon(seq, &mut wagon.purse), cost, fares_reason(),
            now, day);
        hub.book.freight += cost;
    }
}

/// How full the hub's warehouse is, 0 to 1 (`Discounts.fill`): its stock's stacks and the crates in its forwarder's
/// custody against its slots.
pub(crate) fn fill(content: &Content, stock: &Stock, pallets: &Pallets, hub: TownId) -> f64 {
    let used: u32 = stock.0.iter().enumerate().map(|(g, n)| crate::cargo::stacks_of(content, g, *n)).sum::<u32>()
        + crate::freight::custody_crates(pallets, hub);
    let slots = content.dials.warehouse_slots;
    if slots == 0 { 1.0 } else { (used as f64 / slots as f64).clamp(0.0, 1.0) }
}

/// Food points wagons on their way out will eat at each far end, if they packed none (`foodPromisedAt`).
fn promised(content: &Content, routes: &[(&Route, &RouteState)]) -> BTreeMap<TownId, f64> {
    let mut out = BTreeMap::new();
    for (route, state) in routes {
        if let Some(w) = state.wagon.as_ref().filter(|w| w.leg == Leg::Out && w.provisions <= 0.0) {
            *out.entry(route.to_id).or_insert(0.0) += crate::road::food_needed(content, route.blocks)
                * crate::road::mouths(Some(w));
        }
    }
    out
}

/// A wagon reaching the hub on its way calls there (`callAtStop`): its landing fee; going out with no food packed for
/// the road home where the far end may not feed it (its food, less what wagons bound there will eat), it buys that food
/// here if the hub has it beyond its own par; it deals with the forwarder ([`deal`]); it carries the hub's copies of
/// contracts with where it's bound; both its home and the hub know each other; and it goes on.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn call_at_stop(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, territory: Res<Territory>,
        towns_list: Res<Towns>, mut book: ResMut<Book>, mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut atlas: ResMut<Atlas>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        mut towns: Query<(&Town, &mut Hub, &mut Money, &mut Stock, &Trades, Option<&Farm>, &mut Paperwork)>,
        realms: Query<&crate::realms::Realm>) {
    let (now, day) = (clock.now, content.day_ticks);
    let due: Vec<Entity> = routes.iter().filter(|(_, _, s, _)| s.wagon.as_ref().is_some_and(|w| w.arrives <= now
        && w.waiting_since.is_none() && w.itinerary.next_on_way().is_some())).map(|(e, ..)| e).collect();
    if due.is_empty() {
        return;
    }
    let rate = mint.rate(now, day);
    let net = network_of!(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)).collect(),
        towns.iter().map(|(t, h, ..)| (t, h)).collect());
    let food_mode = crate::road::road_food(&content);
    let all: Vec<(&Route, &RouteState)> = routes.iter().map(|(_, r, s, _)| (r, s)).collect();
    let mut promised = promised(&content, &all);
    let far_food: BTreeMap<TownId, f64> = towns.iter().map(|(t, _, _, s, ..)| (t.id, crate::road::held(&content, &s.0))).collect();
    for e in in_order(routes.iter().filter(|(e, ..)| due.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let state = &mut *state;
        let wagon = state.wagon.as_mut().unwrap();
        let stop = wagon.itinerary.next_on_way().unwrap().town;
        let outbound = wagon.leg == Leg::Out;
        let bound = if outbound { route.to_id } else { route.from_id };
        let at = AtHub { content: &content, net: &net, hub: stop, route, leg: RunLeg::calling(&route, outbound, stop),
            now, rate };
        if let Ok((_, mut hub, mut money, mut stock, trades, farm, mut paperwork)) = towns.get_mut(towns_list.0[stop]) {
            land(&at, &territory, &mut ledger, &mut cash, &mut hub, &mut money, wagon);
            // Food: going out, for the road home too where the far end may not feed it; bought here, where it can.
            if food_mode && outbound && wagon.provisions <= 0.0 {
                let food = crate::road::food_needed(&content, route.blocks) * crate::road::mouths(Some(wagon));
                let there = far_food.get(&route.to_id).copied().unwrap_or(0.0) - promised.get(&route.to_id).copied().unwrap_or(0.0);
                if there < food && crate::road::held(&content, &stock.0) >= food + crate::road::FOOD_PAR_POINTS {
                    let meal = crate::road::eat(&content, &mut ledger, &mut stock.0, food);
                    let cost: f64 = meal.iter().map(|&(g, n)| n * content.goods[g].value).sum();
                    let price = Fixed::ceil((cost * rate).ceil() / rate).min(wagon.purse.at_least_zero());
                    post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse),
                        Side::treasury(stop, &mut money.0), price, Reason::RoadFood, now, day);
                    wagon.provisions = food;
                    if let Some(p) = promised.get_mut(&route.to_id) {
                        *p -= food; // it won't eat there now
                    }
                }
            }
            let skip = [bound, route.from_id, route.to_id];
            let strand = (realms.get(towns_list.0[route.from_id]).is_err() && route.from_id != stop).then_some(route.from_id);
            deal(&at, &skip, &book, &mut bills, &mut ledger, &mut cash, &mut pallets, &mut hub, &mut money, &stock, trades,
                farm, &mut paperwork, wagon, strand);
        }
        // The hub's copies of its contracts with where the wagon's bound ride on.
        let copies: Vec<usize> = book.open.iter().copied().filter(|&id| {
            let c = &book.contracts[id];
            c.state == State::Drafted && ((c.seller == stop && c.buyer == bound && c.known_by_seller && !c.known_by_buyer)
                || (c.buyer == stop && c.seller == bound && c.known_by_buyer && !c.known_by_seller))
        }).collect();
        for id in copies {
            book.deliver_copy(id, bound);
        }
        atlas.learn_both(route.from_id, stop, now);
        // The call made: on along the road to the next.
        wagon.arrives = now + wagon.itinerary.made();
        state.freight.stops += 1;
    }
}

/// A call on the way closed, after everything done there (the forwarder's hand-over, the stranded sale): a line still
/// dropping there (a pallet the hub was to buy and didn't) drops at the next call ahead that takes it, else home
/// (`itinerary-drops`; it rides on as it always has). At the far end a leg's turn does the same (`turn_stop`).
#[allow(clippy::type_complexity)]
pub fn close_call(clock: Res<Clock>, content: Res<Content>, pallets: Res<Pallets>, towns: Query<(&Town, &Hub)>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>) {
    let now = clock.now;
    let called = |w: &Wagon| w.itinerary.called_on_way(now, w.arrives)
        .filter(|&at| w.manifest.lines.iter().any(|l| l.drop == Some(at)));
    let due: Vec<Entity> = routes.iter().filter(|(_, _, s, _)| s.wagon.as_ref().is_some_and(|w| called(w).is_some()))
        .map(|(e, ..)| e).collect();
    if due.is_empty() {
        return;
    }
    let net = network_of!(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)).collect(), towns.iter().collect());
    for e in in_order(routes.iter().filter(|(e, ..)| due.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let wagon = state.wagon.as_mut().unwrap();
        let at = called(wagon).unwrap();
        let from = wagon.itinerary.after(at);
        let stale: Vec<PalletId> = wagon.manifest.lines.iter().filter(|l| l.drop == Some(at)).map(|l| l.pallet).collect();
        for id in stale {
            drop_again(Some(&net), &route, &pallets, wagon, id, from);
        }
    }
}

/// At a hub at the far end (`atHub`, after `land`): the wagon deals with the forwarder ([`deal`]), its run home's
/// booked crates loaded.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn at_hub(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, territory: Res<Territory>,
        towns_list: Res<Towns>, book: Res<Book>, mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        mut towns: Query<(&Town, &mut Hub, &mut Money, &Stock, &Trades, Option<&Farm>, &mut Paperwork)>,
        realms: Query<&crate::realms::Realm>) {
    let (now, day) = (clock.now, content.day_ticks);
    let hubs: BTreeSet<TownId> = towns.iter().filter(|(_, h, ..)| h.storehouse).map(|(t, ..)| t.id).collect();
    let here: Vec<Entity> = routes.iter().filter(|(_, r, s, _)| s.arrived == Some(Leg::Out) && hubs.contains(&r.to_id))
        .map(|(e, ..)| e).collect();
    if here.is_empty() {
        return;
    }
    let rate = mint.rate(now, day);
    let net = network_of!(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)).collect(),
        towns.iter().map(|(t, h, ..)| (t, h)).collect());
    for e in in_order(routes.iter().filter(|(e, ..)| here.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let at = AtHub { content: &content, net: &net, hub: route.to_id, route, leg: RunLeg::of(&route, false), now, rate };
        let (_, mut hub, mut money, stock, trades, farm, mut paperwork) = towns.get_mut(towns_list.0[route.to_id]).unwrap();
        land(&at, &territory, &mut ledger, &mut cash, &mut hub, &mut money, wagon);
        let strand = realms.get(towns_list.0[route.from_id]).is_err().then_some(route.from_id);
        deal(&at, &[route.from_id], &book, &mut bills, &mut ledger, &mut cash, &mut pallets, &mut hub, &mut money, stock,
            trades, farm, &mut paperwork, wagon, strand);
    }
}

/// The pallets aboard on freight tickets made out to `town`, their lines dropping there, taken off the wagon and received
/// there ([`receive`]), in the order they loaded: by manifest line, the consolidations that come off here handed out whole
/// first, dissolved onto the wagon's floor behind the rest (`superbills`; the call's drop, `cargo::drop_off`).
#[allow(clippy::too_many_arguments)]
fn receive_aboard(content: &Content, book: &mut Book, pallets: &mut Pallets, now: u64, route: RouteId, town: TownId, wagon: &mut Wagon,
        stock: &mut Stock, store: &mut Shipments) {
    let here = crate::cargo::drop_off(pallets, wagon, route, town, false, now,
        |p, l| p.consigned_to(town) && l.booking.is_some());
    for id in here {
        let p = pallets.open(id).expect("aboard");
        receive(content, book, now, town, &mut stock.0, store, &p);
    }
}

/// Has the wagon pallets on freight tickets (loose, or in a consolidation)?
fn on_freight(wagon: &Wagon) -> bool {
    !wagon.manifest.containers.is_empty() || wagon.manifest.lines.iter().any(|l| l.booking.is_some())
}

/// At the far end, before anything else: the pallets it carried on freight tickets made out here are received
/// (`arriveAtDestination`'s ticketed crates, then `Crates.receive`).
pub fn deliver_freight(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<(&mut Stock, &mut Shipments)>) {
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut().filter(|w| on_freight(w)) else { continue };
        let Ok((mut stock, mut store)) = towns.get_mut(route.to) else { continue };
        receive_aboard(&content, &mut book, &mut pallets, clock.now, route.seq, route.to_id, wagon, &mut stock, &mut store);
    }
}

/// Home again, before the pallets are unloaded: those it carried home on freight tickets are received (`arriveHome`'s
/// `Crates.receive`).
pub fn deliver_home(clock: Res<Clock>, content: Res<Content>, mut book: ResMut<Book>, mut pallets: ResMut<Pallets>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut towns: Query<(&mut Stock, &mut Shipments)>) {
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut().filter(|w| on_freight(w)) else { continue };
        let Ok((mut stock, mut store)) = towns.get_mut(route.from) else { continue };
        receive_aboard(&content, &mut book, &mut pallets, clock.now, route.seq, route.from_id, wagon, &mut stock, &mut store);
    }
}

/// A wagon turned for home this look writes its manifest as it leaves the far end (two lines or more: a superbill drawn at
/// its home, the carrier's), and its calls home: home, and the hub on its way back too (`setStop` as it turns), the rest
/// of the way's share of the leg along. A line whose drop isn't one of them (a call it passed without coming off) drops
/// at the first call ahead that takes it, else home (`itinerary-drops`).
#[allow(clippy::type_complexity)]
pub fn turn_stop(clock: Res<Clock>, content: Res<Content>, pallets: Res<Pallets>, towns: Query<(&Town, &Hub)>,
        mut papers: Query<&mut Paperwork>, mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>) {
    let turned: Vec<Entity> = routes.iter().filter(|(_, _, s, _)| s.wagon.as_ref()
        .is_some_and(|w| w.leg == Leg::Home && w.freight.reckoned != Some(Leg::Home))).map(|(e, ..)| e).collect();
    if turned.is_empty() {
        return;
    }
    let hubs = towns.iter().any(|(_, h)| h.storehouse);
    let net = if hubs {
        Some(network_of!(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)).collect(), towns.iter().collect()))
    } else {
        None
    };
    for e in turned {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let (seq, home) = (route.seq, route.from);
        let wagon = state.wagon.as_mut().unwrap();
        wagon.itinerary = Itinerary::direct(route.from_id, wagon.arrives.saturating_sub(clock.now));
        if let Some((hub, i)) = net.as_ref().and_then(|n| n.stop_on(seq)) {
            let path = net.as_ref().and_then(|n| n.run(seq)).map_or(1, |r| r.path.len().max(2) - 1);
            set_stop(wagon, clock.now, hub, 1.0 - i as f64 / path as f64);
        }
        let stale: Vec<PalletId> = wagon.manifest.lines.iter()
            .filter(|l| !l.drop.is_some_and(|d| wagon.itinerary.calls.iter().any(|c| c.town == d))).map(|l| l.pallet).collect();
        for id in stale {
            drop_again(net.as_ref(), &route, &pallets, wagon, id, 0);
        }
        // Its manifest written as it leaves the far end, its drops chosen again first: two lines or more, a superbill
        // drawn at its home, the carrier's (`superbills`).
        if let Ok(mut paperwork) = papers.get_mut(home) {
            crate::cargo::write_manifest(&mut paperwork, &wagon.manifest, clock.now);
        }
        wagon.freight.reckoned = Some(Leg::Home);
    }
}
