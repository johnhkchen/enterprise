//! Stranded paper through the look (`stranded.rs`; `market/Stranded.java`; `caravan/Caravans.java` `buyStrandedAtTown`,
//! `sellStrandedAtHub`, `collectForwarding`, `dispatch`'s stranded paper):
//!
//! - **Decide**, as a town's wagon leaves for a hub, or for a market whose road calls at one: its stranded crates (paid for
//!   by a bill on a market whose wagons don't call) and its stranded bills (on a market the hub runs to, past their day or
//!   whose wagons don't call) go aboard ([`load`]).
//! - **Arrive**, after a wagon's call at a hub on its way ([`sell_at_stop`]); **Freight**, after the hub at the far end
//!   ([`sell_at_hub`]; the forwarder's discount desk buys the bills not yet due first, so what's left is stranded): the
//!   hub buys what the wagon brought, coin from its treasury beyond its reserve (and its households'
//!   savings it may borrow), the rest in its own bills ([`crate::stranded::buy`]).
//! - **Papers**, a hub's own wagon at a town ([`buy_at_town`]): the town's stranded bills and crates bought with the
//!   wagon's purse beyond its road food, the hub's bill for the rest filed with the town; the bills ride home to the hub's
//!   files, the crates to its forwarder.
//! - **Freight**, a hub's own wagon at a town ([`collect_forwarding`]): the town's goods owed to markets the hub serves,
//!   crated and billed on their buyers, for the forwarder; the town pays the fare (freight, carting and the forwarder's
//!   margin) as far as it can spare, and the hub's wagon carries more on credit: what it can't pay is its bill to the
//!   hub, due in `freight_credit_days`.
//! - **Turn**, home again: the crates the wagon bought or collected are the forwarder's ([`home_forwarding`]).

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::bank::Discounts;
use crate::bills::{Bills, Held as Paper};
use crate::cargo::{pack, stacks, Paperwork, Shipment, Shipments, Shipper};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::contracts::Book;
use crate::farms::{borrow, on_call, Deposits, Farm};
use crate::freight::{cart_price, forwarding_price, Freight, Held, Hub, Leg as RunLeg, Network, CART, FORWARD_LEAD_DAYS};
use crate::messages::TownId;
use crate::mint::Mint;
use crate::realms::Realm;
use crate::fixed::Fixed;
use crate::state::*;
use crate::stranded::{bill_ok, buy, crate_ok, Terms};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        // Aboard before the books and tally count what went out (`Caravans.dispatch` 451–466, then the SENT lines at
        // 570): unpinned, the order fell to Bevy's tie-break, which wasm32 broke the other way.
        load.in_set(Step::Decide).after(super::freight::depart).before(super::atlas::depart),
        sell_at_stop.in_set(Step::Arrive).after(super::freight::call_at_stop).before(super::arrive::mark),
        sell_at_hub.in_set(Step::Freight).after(super::freight::at_hub),
        buy_at_town.in_set(Step::Papers).after(super::bank::deal).before(super::bills::present),
        collect_forwarding.in_set(Step::Freight).after(sell_at_hub),
        home_forwarding.in_set(Step::Turn).after(super::freight::deliver_home).before(super::squad::come_home),
    ));
}

fn stranded_reason() -> Reason {
    Reason::Other("stranded paper".into())
}

fn credit_reason() -> Reason {
    Reason::Other("forwarding".into())
}

/// What a town can spare beyond its reserve (trading capital and wage float).
fn spare(content: &Content, money: Fixed, trades: &Trades, farm: Option<&Farm>) -> Fixed {
    money - super::farms::reserve(content, trades, farm)
}

/// The network as the routes and towns stand.
fn network<'a>(content: &Content, routes: impl Iterator<Item = (&'a Route, &'a RouteState, Option<&'a RoutePath>)>,
        towns: impl Iterator<Item = (&'a Town, &'a Hub)>) -> Network {
    Network::of(content, routes, towns.map(|(t, h)| (t.id, t.hex, h.storehouse)))
}

/// The hub's side of a deal: its forwarder and treasury, and what it can lend.
struct HubSide<'a> {
    id: TownId,
    hub: &'a mut Hub,
    money: &'a mut Money,
    trades: &'a Trades,
    farm: Option<&'a Farm>,
    people: &'a mut People,
    deposits: &'a mut Deposits,
}

/// What a deal needs of the world.
struct Cx<'a> {
    content: &'a Content,
    net: &'a Network,
    book: &'a Book,
    now: u64,
    rate: f64,
}

/// As a town's wagon leaves for a hub, or one whose road calls at a hub on its way (`dispatch`): the town's stranded
/// crates, as many as the wagon has room for, and its stranded bills go aboard, to be sold there.
#[allow(clippy::type_complexity)]
pub fn load(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, book: Res<Book>, mut bills: ResMut<Bills>,
        towns_list: Res<Towns>, mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        places: Query<(&Town, &Hub)>, mut stores: Query<(&mut Shipments, Option<&Realm>)>) {
    let now = clock.now;
    let left: Vec<Entity> = routes.iter().filter(|(_, _, s, _)| s.last_departure == Some(now)
        && s.wagon.as_ref().is_some_and(|w| w.leg == Leg::Out && w.freight.reckoned == Some(Leg::Out)))
        .map(|(e, ..)| e).collect();
    if left.is_empty() || bills.open.is_empty() && stores.iter().all(|(s, _)| s.held.is_empty()) {
        return;
    }
    let net = network(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)), places.iter());
    if net.hubs.is_empty() {
        return;
    }
    let rate = mint.rate(now, content.day_ticks);
    for e in in_order(routes.iter().filter(|(e, ..)| left.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let wagon = state.wagon.as_mut().unwrap();
        // Its paper to sell at the hub this run goes to or calls at.
        let Some(hub) = (if net.is_hub(route.to_id) { Some(route.to_id) } else { wagon.freight.stop.map(|s| s.hub) }) else { continue };
        let Ok((mut store, realm)) = stores.get_mut(towns_list.0[route.from_id]) else { continue };
        if realm.is_some() {
            continue;
        }
        let t = Terms { content: &content, net: &net, book: &book, hub, town: route.from_id, counting: false, now, rate };
        let slots = if wagon.slots > 0 { wagon.slots } else { content.dials.wagon_slots };
        let mut room = slots as i64 - stacks(&content, &wagon.cargo, &wagon.shipments) as i64;
        let mut i = 0;
        while i < store.held.len() {
            if room > 0 && crate_ok(&t, &store.held[i]) {
                let s = store.held.remove(i);
                room -= s.stacks(&content) as i64;
                wagon.shipments.push(s);
            } else {
                i += 1;
            }
        }
        for &id in &bills.open.clone() {
            let b = &bills.bills[id];
            if b.held == Paper::Town(route.from_id) && bill_ok(&t, b) {
                bills.bills[id].held = Paper::Wagon;
                wagon.bills.push(id);
            }
        }
    }
}

/// A town's wagon sells the hub the stranded paper it brought: coin from the hub's treasury beyond its reserve (and its
/// households' savings it may borrow), the hub's bills for the rest, carried home. The bills go in the hub's files, the
/// crates to its forwarder. What the hub won't buy rides home again (`sellStrandedAtHub`).
#[allow(clippy::too_many_arguments)]
fn sell_aboard(cx: &Cx, bills: &mut Bills, ledger: &mut Ledger, cash: &mut CashBook, freight: &mut Freight, route: &Route,
        wagon: &mut Wagon, mut h: HubSide, skip: &[TownId]) {
    let (now, day, rate) = (cx.now, cx.content.day_ticks, cx.rate);
    let t = Terms { content: cx.content, net: cx.net, book: cx.book, hub: h.id, town: route.from_id,
        counting: h.trades.counting_house, now, rate };
    let (mut crates, rest): (Vec<Shipment>, Vec<Shipment>) = std::mem::take(&mut wagon.shipments).into_iter()
        .partition(|s| crate_ok(&t, s) && !wagon.freight.booked.contains(s));
    wagon.shipments = rest;
    let (mut offered, rest): (Vec<usize>, Vec<usize>) = std::mem::take(&mut wagon.bills).into_iter()
        .partition(|&id| bill_ok(&t, &bills.bills[id]));
    wagon.bills = rest;
    if crates.is_empty() && offered.is_empty() {
        return;
    }
    let cash_on_hand = spare(cx.content, h.money.0, h.trades, h.farm).at_least_zero();
    // Its households' savings count too: deposits the Merchant may lend on such paper.
    let means = cash_on_hand + on_call(cx.content, h.people.purse, h.deposits.owed);
    let deal = buy(&t, bills, &mut crates, &mut offered, means.to_f64(), means.to_f64());
    wagon.bills.extend(offered);
    let Some(deal) = deal else {
        hand_leftovers(cx, ledger, cash, route, wagon, &mut h, skip, crates);
        return;
    };
    let coin = deal.coin(rate);
    if coin > cash_on_hand {
        borrow(cx.content, ledger, cash, h.id, &mut h.money.0, &mut h.people.purse, h.deposits, coin - cash_on_hand, now);
    }
    let paid = coin.min(h.money.0.at_least_zero());
    // What the treasury couldn't find in coin the hub owes in a bill of its own, as the rest of the price is.
    let short = coin - paid;
    if short > Fixed::from_millis(10) {
        let term = (crate::bills::TERM_DAYS * day as f64) as u64;
        let id = bills.draw_term(h.id, route.from_id, Fixed::ceil(short.to_f64() / (1.0 + crate::bills::INTEREST)), now, day,
            term, None);
        bills.bills[id].held = Paper::Wagon;
        wagon.bills.push(id);
        freight.stranded(h.id).promised += bills.bills[id].amount;
    }
    post(ledger, cash, Side::treasury(h.id, &mut h.money.0), Side::wagon(route.seq, &mut wagon.purse), paid,
        stranded_reason(), now, day);
    for &id in &deal.hub_bills {
        bills.bills[id].held = Paper::Wagon; // the town's, riding home
        wagon.bills.push(id);
    }
    for &id in &deal.bills {
        bills.bills[id].held = Paper::Town(h.id); // filed with the hub
    }
    let book = freight.stranded(h.id);
    book.deals += 1;
    book.bills += deal.bills.len() as u32;
    book.face += deal.face;
    book.coin += paid;
    book.promised += deal.hub_bills.iter().map(|&id| bills.bills[id].amount).sum::<Fixed>();
    book.crates += deal.crates.len() as u32;
    for s in deal.crates {
        h.hub.held.push(Held { shipment: s, carted: false, booked: None });
    }
    hand_leftovers(cx, ledger, cash, route, wagon, &mut h, skip, crates);
}

/// The stranded crates the hub didn't buy go to its forwarder as any crate bound beyond, the forwarding from the wagon's
/// purse: the mod's `handOver` ran after the stranded sale. Not those for the wagon's own ends (`skip`).
#[allow(clippy::too_many_arguments)]
fn hand_leftovers(cx: &Cx, ledger: &mut Ledger, cash: &mut CashBook, route: &Route, wagon: &mut Wagon, h: &mut HubSide,
        skip: &[TownId], crates: Vec<Shipment>) {
    let rest = std::mem::replace(&mut wagon.shipments, crates);
    super::freight::hand_over(cx.content, cx.net, h.id, cx.now, cx.rate, route.seq, skip, ledger, cash, h.hub, h.money,
        wagon, &|_| false);
    wagon.shipments.extend(rest);
}

type HubQuery<'w, 's> = Query<'w, 's, (&'static Town, &'static mut Hub, &'static mut Money, &'static Trades,
    Option<&'static Farm>, &'static mut People, &'static mut Deposits)>;

/// A wagon that has called at a hub on its way this look sells it the stranded paper it brought (`callAtStop`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sell_at_stop(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, book: Res<Book>, towns_list: Res<Towns>,
        mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut freight: ResMut<Freight>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>, mut hubs: HubQuery, realms: Query<&Realm>) {
    let now = clock.now;
    // Wagons that called at their hub this look: `call_at_stop` set them on, so far along the road yet to go.
    let due: Vec<Entity> = routes.iter().filter(|(_, _, s, _)| s.wagon.as_ref().is_some_and(|w| w.waiting_since.is_none()
        && w.freight.stop.is_some_and(|s| s.called && w.arrives == now + s.remaining))).map(|(e, ..)| e).collect();
    if due.is_empty() || bills.open.is_empty() && routes.iter().all(|(_, _, s, _)| s.wagon.as_ref().is_none_or(|w| w.shipments.is_empty())) {
        return;
    }
    let net = Network::of(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)),
        hubs.iter().map(|(t, h, ..)| (t.id, t.hex, h.storehouse)));
    let rate = mint.rate(now, content.day_ticks);
    let cx = Cx { content: &content, net: &net, book: &book, now, rate };
    for e in in_order(routes.iter().filter(|(e, ..)| due.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let wagon = state.wagon.as_mut().unwrap();
        let hub = wagon.freight.stop.unwrap().hub;
        if realms.get(towns_list.0[route.from_id]).is_ok() {
            continue;
        }
        let Ok((_, mut hub_side, mut money, trades, farm, mut people, mut deposits)) = hubs.get_mut(towns_list.0[hub]) else { continue };
        let side = HubSide { id: hub, hub: &mut hub_side, money: &mut money, trades, farm, people: &mut people, deposits: &mut deposits };
        sell_aboard(&cx, &mut bills, &mut ledger, &mut cash, &mut freight, &route, wagon, side,
            &[route.from_id, route.to_id]);
    }
}

/// A town's wagon at the hub at its far end sells it the stranded paper it brought (`atHub`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sell_at_hub(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, book: Res<Book>, towns_list: Res<Towns>,
        mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut freight: ResMut<Freight>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>, mut hubs: HubQuery, realms: Query<&Realm>) {
    let now = clock.now;
    let here: Vec<Entity> = routes.iter().filter(|(_, r, s, _)| s.arrived == Some(Leg::Out)
        && s.wagon.as_ref().is_some_and(|w| !w.shipments.is_empty() || !w.bills.is_empty())
        && hubs.get(towns_list.0[r.to_id]).is_ok_and(|(_, h, ..)| h.storehouse)).map(|(e, ..)| e).collect();
    if here.is_empty() {
        return;
    }
    let net = Network::of(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)),
        hubs.iter().map(|(t, h, ..)| (t.id, t.hex, h.storehouse)));
    let rate = mint.rate(now, content.day_ticks);
    let cx = Cx { content: &content, net: &net, book: &book, now, rate };
    for e in in_order(routes.iter().filter(|(e, ..)| here.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        if realms.get(towns_list.0[route.from_id]).is_ok() {
            continue;
        }
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let Ok((_, mut hub_side, mut money, trades, farm, mut people, mut deposits)) = hubs.get_mut(towns_list.0[route.to_id]) else { continue };
        let side = HubSide { id: route.to_id, hub: &mut hub_side, money: &mut money, trades, farm, people: &mut people,
            deposits: &mut deposits };
        sell_aboard(&cx, &mut bills, &mut ledger, &mut cash, &mut freight, &route, wagon, side, &[route.from_id]);
    }
}

/// A hub's own wagon at a town buys its stranded paper (`buyStrandedAtTown`): the town's bills and crates, with coin from
/// the wagon's purse beyond its road food and the hub's bill for the rest, filed with the town; the bills and goods ride
/// home to the hub's files and its forwarder.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn buy_at_town(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, book: Res<Book>, towns_list: Res<Towns>,
        mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut freight: ResMut<Freight>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        places: Query<(&Town, &Hub)>,
        mut towns: Query<(&mut Money, &mut Shipments, &Trades, &People, &Deposits, Option<&Farm>)>) {
    let now = clock.now;
    let here: Vec<Entity> = routes.iter().filter(|(_, r, s, _)| s.arrived == Some(Leg::Out)
        && places.get(towns_list.0[r.from_id]).is_ok_and(|(_, h)| h.storehouse)).map(|(e, ..)| e).collect();
    if here.is_empty() {
        return;
    }
    let net = network(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)), places.iter());
    let rate = mint.rate(now, content.day_ticks);
    for e in in_order(routes.iter().filter(|(e, ..)| here.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let (hub, town) = (route.from_id, route.to_id);
        // What the hub could pay, as last counted at home: its treasury beyond its reserve, and its households' savings.
        let means = match towns.get(towns_list.0[hub]) {
            Ok((money, _, trades, people, deposits, farm)) => (spare(&content, money.0, trades, farm)
                + on_call(&content, people.purse, deposits.owed)).at_least_zero(),
            Err(_) => continue,
        };
        let counting = towns.get(towns_list.0[hub]).is_ok_and(|(_, _, t, ..)| t.counting_house);
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let Ok((mut money, mut store, ..)) = towns.get_mut(towns_list.0[town]) else { continue };
        let t = Terms { content: &content, net: &net, book: &book, hub, town, counting, now, rate };
        let mut crates = Vec::new();
        let mut i = 0;
        while i < store.held.len() {
            if crate_ok(&t, &store.held[i]) {
                crates.push(store.held.remove(i));
            } else {
                i += 1;
            }
        }
        let mut offered: Vec<usize> = bills.open.iter().copied()
            .filter(|&id| bills.bills[id].held == Paper::Town(town) && bill_ok(&t, &bills.bills[id])).collect();
        if crates.is_empty() && offered.is_empty() {
            continue;
        }
        let keep = crate::road::purse_keep(&content, &route, crate::road::mouths(Some(wagon)));
        let coin = (wagon.purse - keep).at_least_zero();
        let deal = buy(&t, &mut bills, &mut crates, &mut offered, coin.to_f64(), means.to_f64());
        store.held.extend(crates);
        let Some(deal) = deal else { continue };
        let paid = deal.coin(rate);
        post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(town, &mut money.0), paid,
            stranded_reason(), now, content.day_ticks);
        for &id in &deal.hub_bills {
            bills.bills[id].held = Paper::Town(town); // filed with the town
        }
        for &id in &deal.bills {
            bills.bills[id].held = Paper::Wagon; // home with the wagon, to the hub's files
            wagon.bills.push(id);
        }
        let book = freight.stranded(hub);
        book.deals += 1;
        book.bills += deal.bills.len() as u32;
        book.face += deal.face;
        book.coin += paid;
        book.promised += deal.hub_bills.iter().map(|&id| bills.bills[id].amount).sum::<Fixed>();
        book.crates += deal.crates.len() as u32;
        for s in deal.crates {
            wagon.freight.forwarding.push(s.clone());
            wagon.shipments.push(s);
        }
    }
}

/// The goods a town sends on through the hub, picked for its contracts: those owed by the time the hub's wagon would be home
/// (and `FORWARD_LEAD_DAYS` more) beyond what's shipped, crated and made out to the buyer with a copy of the contract, a
/// bill of exchange drawn on the buyer for each (`pickForwarded`'s contract loop), as many stacks as `room` for the
/// contracts `ok` passes. Returns the crates and the bills drawn.
#[allow(clippy::too_many_arguments)]
fn pick(content: &Content, net: &Network, now: u64, book: &mut Book, bills: &mut Bills, ledger: &mut Ledger, cash: &mut CashBook, route: &Route, stock: &mut [f64],
        shipper: &mut Shipper, room: i64, ok: impl Fn(&crate::contracts::Contract) -> bool) -> (Vec<Shipment>, Vec<usize>) {
    let (home, hub) = (route.to_id, route.from_id);
    let day = content.day_ticks;
    let arrival = now + route.travel_ticks + (FORWARD_LEAD_DAYS * day as f64) as u64;
    let (mut picked, mut drawn) = (Vec::<Shipment>::new(), Vec::new());
    for id in book.open.clone() {
        let c = &book.contracts[id];
        if !c.in_force() || c.seller != home || !c.incoterm.seller_carries() || c.training.is_some() || c.buyer == hub
                || c.buyer == home || net.legs(hub, c.buyer).is_empty() || !ok(c) {
            continue;
        }
        let g = c.good;
        let free = room - picked.iter().map(|s| s.stacks(content)).sum::<u32>() as i64;
        let owed = (c.owed(arrival) - c.shipped).floor();
        let n = owed.min(free.max(0) as f64 * content.goods[g].stack.max(1) as f64).min(stock[g].max(0.0).floor());
        if n <= 0.0 {
            continue;
        }
        let (buyer, price) = (c.buyer, c.price);
        let like = Shipment { good: g, count: 0.0, shipper: home, consignee: Some(buyer), contract: Some(id), settled: false,
            copy: true };
        // Only what can be crated goes: loose goods for a market beyond would be sold at the hub as the wagon's own.
        let Some(s) = pack(content, ledger, cash, now, shipper, g, n, &like) else { continue };
        stock[g] -= s.count;
        drawn.push(bills.draw_for(buyer, home, Fixed::ceil(s.count * price), now, day, Some(id)));
        book.ship(id, s.count);
        picked.push(s);
    }
    (picked, drawn)
}

/// A hub's own wagon at a town collects for its forwarder (`collectForwarding`): the town's goods owed to markets beyond the
/// hub, crated and made out to their buyers, the town paying the fare (the carrier's freight home as a backhaul, the
/// carting in and out, the forwarder's margin) from what it can spare, and the shipper's bills on those buyers bought by
/// the hub with the wagon's purse. The hub's own wagon carries more on credit: goods whose stack is worth the fare over
/// the lien's share, the fare they'd cost left as the town's bill to the hub, due in `freight_credit_days`.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn collect_forwarding(clock: Res<Clock>, content: Res<Content>, towns_list: Res<Towns>,
        mut book: ResMut<Book>, mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut freight: ResMut<Freight>, mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        mut towns: Query<(&Town, &mut Money, &mut Stock, &Trades, Option<&Farm>, &mut Shipments, &mut Paperwork, &mut Hub,
            &People, &Deposits)>) {
    let now = clock.now;
    let d = &content.dials;
    let here: Vec<Entity> = routes.iter().filter(|(_, r, s, _)| s.arrived == Some(Leg::Out)
        && towns.get(towns_list.0[r.from_id]).is_ok_and(|t| t.7.storehouse)).map(|(e, ..)| e).collect();
    if here.is_empty() || book.open.is_empty() {
        return;
    }
    let net = Network::of(&content, routes.iter().map(|(_, r, s, p)| (r, s, p)),
        towns.iter().map(|t| (t.0.id, t.0.hex, t.7.storehouse)));
    for e in in_order(routes.iter().filter(|(e, ..)| here.contains(e)).map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, _) = routes.get_mut(e).unwrap();
        let route = *route;
        let (hub, town) = (route.from_id, route.to_id);
        let Some(wagon) = state.wagon.as_mut() else { continue };
        // The fare a stack costs the town: the carrier's backhaul to the hub with the forwarder's margin, and the carting.
        let per_stack = forwarding_price(&content, &net, &RunLeg::of(&route, false))
            + cart_price(&content) * 2.0 / CART as f64 * (1.0 + d.forward_margin);
        if per_stack <= 0.0 {
            continue;
        }
        // The hub's means, as last counted, for its discount desk.
        let (hub_means, counting, fill) = match towns.get(towns_list.0[hub]) {
            Ok((_, money, stock, trades, farm, _, _, h, people, deposits)) => (
                (spare(&content, money.0, trades, farm) + on_call(&content, people.purse, deposits.owed)).at_least_zero(),
                trades.counting_house, super::freight::fill(&content, stock, h)),
            Err(_) => continue,
        };
        let Ok((_, mut money, mut stock, trades, farm, mut store, mut paperwork, ..)) = towns.get_mut(towns_list.0[town]) else { continue };
        let can = spare(&content, money.0, trades, farm).at_least_zero();
        let slots = if wagon.slots > 0 { wagon.slots } else { d.wagon_slots };
        let aboard = stacks(&content, &wagon.cargo, &wagon.shipments) as i64;
        let room = (slots as i64 - aboard).min((can.to_f64() / per_stack).floor() as i64);
        if slots as i64 - aboard <= 0 {
            continue;
        }
        let mut picked = Vec::new();
        let mut drawn = Vec::new();
        let mut credit = Fixed::ZERO;
        {
            let shipper_spare = can;
            let mut shipper = Shipper { town, money: &mut money.0, spare: shipper_spare, store: &mut store,
                paperwork: &mut paperwork };
            if room > 0 {
                let (p, b) = pick(&content, &net, now, &mut book, &mut bills, &mut ledger, &mut cash, &route, &mut stock.0, &mut shipper, room,
                    |_| true);
                picked = p;
                drawn = b;
            }
            // The rest on credit: the contracts whose stack is worth the fare over the lien's share.
            let more = slots as i64 - aboard - picked.iter().map(|s| s.stacks(&content)).sum::<u32>() as i64;
            if more > 0 {
                let (p, b) = pick(&content, &net, now, &mut book, &mut bills, &mut ledger, &mut cash, &route, &mut stock.0, &mut shipper, more,
                    |c| c.price * content.goods[c.good].stack as f64 * d.freight_lien >= per_stack);
                credit = Fixed::ceil(p.iter().map(|s| s.stacks(&content)).sum::<u32>() as f64 * per_stack);
                picked.extend(p);
                drawn.extend(b);
            }
        }
        if picked.is_empty() {
            continue;
        }
        let total: u32 = picked.iter().map(|s| s.stacks(&content)).sum();
        let fee = (Fixed::ceil(total as f64 * per_stack) - credit).at_least_zero().min(money.0.at_least_zero());
        post(&mut ledger, &mut cash, Side::treasury(town, &mut money.0), Side::wagon(route.seq, &mut wagon.purse), fee,
            credit_reason(), now, content.day_ticks);
        if credit > Fixed::from_millis(10) {
            // The rest on the town's bill to the hub, thirty days hence (it rides home in the wagon's papers).
            let term = (d.freight_credit_days * content.day_ticks as f64) as u64;
            let id = bills.draw_term(town, hub, credit, now, content.day_ticks, term, None);
            bills.bills[id].held = Paper::Wagon;
            wagon.bills.push(id);
        }
        // The shipper's bills on the buyers: bought now, from the purse, by the hub's forwarder; the rest it keeps.
        let mut taking = Fixed::ZERO;
        let hub_e = towns_list.0[hub];
        let mut tally = (0u32, Fixed::ZERO);
        for id in drawn {
            let b = &bills.bills[id];
            let q = Discounts::quote(&bills, &book, hub, counting, b, false, taking, fill, hub_means, now, content.day_ticks, d);
            let price = b.amount.mul_floor(q.share);
            if !q.taken() || !price.is_positive() || wagon.purse < price || b.due <= now || net.legs(hub, b.drawee).is_empty() {
                bills.bills[id].held = Paper::Town(town); // drawn at the town, filed by it
                continue;
            }
            taking += b.amount;
            tally.0 += 1;
            tally.1 += b.amount;
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(town, &mut money.0), price,
                crate::bank::discount_reason(), now, content.day_ticks);
            bills.endorse(id, hub);
            bills.bills[id].held = Paper::Wagon;
            wagon.bills.push(id);
        }
        // The goods ride home in the forwarder's name.
        for s in picked {
            wagon.freight.forwarding.push(s.clone());
            wagon.shipments.push(s);
        }
        let book_entry = freight.stranded(hub);
        book_entry.credit_stacks += if credit.is_positive() { (credit.to_f64() / per_stack).round() as u32 } else { 0 };
        book_entry.credit += credit;
        if let Ok((.., mut h, _, _)) = towns.get_mut(hub_e) {
            h.book.forwarded += total;
            h.book.fees += fee;
            h.book.bills += tally.0;
            h.book.face += tally.1;
        }
    }
}

/// Home again, before the crates are unloaded: those the wagon bought or collected for the forwarder are in its hands, to be
/// carted in and booked on (`Freight.forwarding`, then `insert`).
pub fn home_forwarding(mut routes: Query<(Entity, &Route, &mut RouteState)>, mut hubs: Query<&mut Hub>,
        towns_list: Res<Towns>) {
    // In the routes' order: crates from several runs reach one forwarder's hands in an order a load keeps.
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let Some(wagon) = state.wagon.as_mut().filter(|w| !w.freight.forwarding.is_empty()) else { continue };
        let Ok(mut hub) = hubs.get_mut(towns_list.0[route.from_id]) else { continue };
        for s in std::mem::take(&mut wagon.freight.forwarding) {
            let Some(i) = wagon.shipments.iter().position(|x| *x == s) else { continue };
            hub.held.push(Held { shipment: wagon.shipments.remove(i), carted: false, booked: None });
        }
    }
}
