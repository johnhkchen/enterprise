//! The post and the forwarder through the look (`post.rs`; `Post.atDesk`, `read`, `write`; `Forwarder.review`,
//! `collect`, `consignUnsold`, `sold`; `Caravans`' run on call):
//!
//! - **Mail**, each look before the papers: every town reads the letters that have reached it ([`read`]): a copy puts
//!   its contract in force, bills are honoured and the coin goes back by post, a remittance is paid in, a call is booked
//!   or declined, a booking is the forwarder's.
//! - **Review**, once a day, last: each town writes: its booking with its hub's forwarder, its bills due on markets no
//!   wagon runs to, and the copies a day old of contracts it holds where no wagon runs; then each hub's forwarder
//!   collects from the towns whose bookings are full or have waited, and remits what its lots sold for ([`write`]).
//! - **Plan:** a run on call stays home but on a booked day or a collection's ([`stay_on_call`]); on one, it goes, with
//!   a purse for the call's goods ([`go_on_call`]); once it's gone, its calls are made ([`made`]).
//! - **Sell:** a hub's wagon's sales credited to the towns' lots ([`credit_sold`]); a town's wagon at a hub leaves what
//!   it couldn't sell on consignment ([`consign`]).
//! - **Buy:** the hub's wagon at a booked town collects its goods for sale, the town's lots, as pallets made out to the hub
//!   (`consignment-pallets`; [`collect`]); a wagon's
//!   purchases at a hub credited to the lots ([`credit_bought`]).

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;

use super::{free, in_order, shelf, Step};
use crate::bills::{self, BillState, Bills, Held};
use crate::cash_book::{post, Accountant, CashBook, Reason, Side};
use crate::pallet::{At, Lading, Line, Pallets, Payment};
use crate::content::{Content, GoodId};
use crate::contracts::{Book, State};
use crate::farms::{Deposits, Farm};
use crate::freight::Hub;
use crate::messages::TownId;
use crate::post::*;
use crate::realms::{Crown, OffMap, Realm};
use crate::fixed::Fixed;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        read.in_set(Step::Mail).after(super::freight::deliver_freight),
        write.in_set(Step::Review).after(super::freight::review),
        stay_on_call.in_set(Step::Plan).after(super::plan::start).after(super::freight::retire)
            .before(super::squad::size_up).before(super::atlas::want_books).before(super::services::plan),
        go_on_call.in_set(Step::PlanFreight).after(super::freight::plan_freight),
        // (before the wagon's loaded: it reads only who left; pinned as it ran natively)
        made.in_set(Step::Decide).after(super::plan::decide).before(super::cargo::ship),
        credit_sold.in_set(Step::Sell).after(super::arrive::sell).after(super::atlas::log_sales).before(super::cargo::relot),
        consign.in_set(Step::Sell).after(super::arrive::sell).after(super::atlas::log_sales).after(super::harvest::offer)
            .after(super::realms::customs).after(credit_sold).before(super::cargo::relot),
        collect.in_set(Step::Buy).after(super::squad::send_porters_home).before(super::desires::buy_for_home),
        credit_bought.in_set(Step::Buy).after(super::desires::buy_backhaul),
    ));
}

/// A town as the post reads and changes it.
type Place<'a> = (&'a Town, &'a mut Money, &'a mut People, &'a mut Stock, &'a Trades, Option<&'a Farm>,
    Option<&'a mut Deposits>, &'a Hub, Option<&'a OffMap>, Option<&'a Realm>);

/// What the post knows of each town at the look's start: where it stands, whether it's a hub, whether its riders go
/// there (not beyond the border: off the map or a foreign realm), its trades and its entity.
#[derive(Clone)]
struct Known {
    entity: Entity,
    hub: bool,
    outside: bool,
    trades: Trades,
}

fn today(content: &Content, now: u64) -> u64 {
    now / content.day_ticks.max(1)
}

/// The routes as the post sees them.
fn roads<'a>(routes: impl Iterator<Item = (&'a Route, &'a RouteState)>) -> Roads {
    let mut out: Vec<Road> = routes.map(|(r, s)| Road {
        seq: r.seq,
        from: r.from_id,
        to: r.to_id,
        running: s.freight.wound_down.is_none(),
        waiting: s.stayed == Some(crate::road::Stay::WaitingForALoad),
        on_call: r.on_call,
        trips: s.trips,
        blocks: r.blocks,
    }).collect();
    out.sort_by_key(|r| r.seq);
    Roads(out)
}

/// How far apart the towns are, from where they stand and their roads.
fn distances<'a>(towns: impl Iterator<Item = &'a Town>, routes: &Roads) -> Distances {
    let mut d = Distances::default();
    for t in towns {
        if let Some(h) = t.hex {
            d.hexes.insert(t.id, h);
        }
    }
    for r in &routes.0 {
        d.roads.entry((r.from, r.to)).or_insert(r.blocks);
    }
    d
}

/// The sender pays the postage, as far as its money goes (`Post.post`).
fn postage(content: &Content, ledger: &mut Ledger, cash: &mut CashBook, town: TownId, money: &mut Fixed, now: u64) {
    let fee = Fixed::ceil(content.dials.postage).min(money.at_least_zero());
    post(ledger, cash, Side::treasury(town, money), Side::outside(), fee, postage_reason(), now, content.day_ticks);
}

fn known(towns: &Query<Place>, list: &Towns) -> Vec<Known> {
    known_of(towns, list, true)
}

/// As [`known`], its trades left out when `trades` is false (the mail read each look needs none).
fn known_of(towns: &Query<Place>, list: &Towns, trades: bool) -> Vec<Known> {
    list.0.iter().map(|&e| {
        let (_, _, _, _, t, _, _, hub, off, realm) = towns.get(e).expect("a town");
        Known { entity: e, hub: hub.storehouse, outside: off.is_some() || realm.is_some(),
            trades: if trades { t.clone() } else { Trades::default() } }
    }).collect()
}

/// Each look, at every town: the letters that have reached it read (`Post.read`), in the order posted. And the contracts
/// still drafted noted with when they were drafted (since the last look, as the post first sees them), so their copies
/// go a day on ([`write`]).
#[allow(clippy::too_many_arguments)]
pub fn read(clock: Res<Clock>, content: Res<Content>, list: Res<Towns>, crown: Res<Crown>, mut mail: ResMut<Mail>,
        mut forwarder: ResMut<Forwarder>, mut book: ResMut<Book>, mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut towns: Query<Place>, routes: Query<(&Route, &RouteState)>) {
    let now = clock.now;
    let day = content.day_ticks;
    let drafted: BTreeSet<usize> = book.open.iter().copied().filter(|&id| book.contracts[id].state == State::Drafted).collect();
    if !mail.seen.is_empty() || !drafted.is_empty() {
        mail.seen.retain(|id, _| drafted.contains(id));
        let since = now.saturating_sub(content.dials.look_ticks);
        for &id in &drafted {
            mail.seen.entry(id).or_insert(since);
        }
    }
    if mail.letters.iter().all(|l| l.arrives > now) {
        return;
    }
    let roads = roads(routes.iter());
    let places = distances(towns.iter().map(|p| p.0), &roads);
    let known = known_of(&towns, &list, false);
    let n = known.len();
    for t in 0..n {
        if mail.letters.iter().all(|l| l.to != t || l.arrives > now) {
            continue;
        }
        for letter in mail.arrived(t, now) {
            match letter.kind {
                Kind::Copy => {
                    if let Some(id) = letter.contract.filter(|&id| id < book.contracts.len()) {
                        book.deliver_copy(id, t);
                    }
                }
                Kind::Bill => honour(&content, now, &letter, &mut mail, &mut book, &mut bills, &mut ledger, &mut cash,
                    &mut towns, &known, &places),
                Kind::Remit => {
                    let (_, mut money, ..) = towns.get_mut(known[t].entity).unwrap();
                    if letter.value.is_positive() {
                        post(&mut ledger, &mut cash, Side::outside(), Side::treasury(t, &mut money.0), letter.value,
                            remit_reason(), now, day);
                    }
                    let back: Vec<usize> = letter.bills.iter().copied()
                        .filter(|&id| bills.bills.get(id).is_some_and(|b| b.state != BillState::Paid)).collect();
                    bills.home(&back, t);
                }
                Kind::Call => call_in(&content, now, &letter, &crown, &roads, &places, &mut mail, &mut ledger, &mut cash,
                    &mut towns, &known),
                Kind::Reply => {} // the town knows when to have its goods ready
                Kind::Book => forwarder.booked(t, letter.from, letter.stacks, letter.value, now),
            }
        }
    }
    bills.tidy();
}

/// Once a day, at a town (`Post.mailCopies`): copies of contracts it holds that the other party hasn't seen, a day
/// after they were drafted, by post where no wagon runs between them and no copy's on its way.
#[allow(clippy::too_many_arguments)]
fn mail_copies(content: &Content, now: u64, town: TownId, book: &Book, roads: &Roads, places: &Distances,
        known: &[Known], mail: &mut Mail, ledger: &mut Ledger, cash: &mut CashBook, towns: &mut Query<Place>) {
    let n = known.len();
    let ripe: Vec<usize> = mail.seen.iter().filter(|(_, &at)| now.saturating_sub(at) >= content.day_ticks)
        .map(|(&id, _)| id).collect();
    for id in ripe {
        let c = &book.contracts[id];
        let to = match (c.known_by_seller, c.known_by_buyer) {
            (true, false) if c.seller == town => c.buyer,
            (false, true) if c.buyer == town => c.seller,
            _ => continue,
        };
        if c.state != State::Drafted || to == town || to >= n || known[to].outside || roads.regular_between(town, to)
            || mail.letters.iter().any(|l| l.kind == Kind::Copy && l.contract == Some(id) && l.to == to) {
            continue;
        }
        let (_, mut money, ..) = towns.get_mut(known[town].entity).unwrap();
        postage(content, ledger, cash, town, &mut money.0, now);
        mail.send(content, Letter { kind: Kind::Copy, from: town, to, contract: Some(id), ..Default::default() }, now,
            places.between(town, to));
    }
}

/// The drawee pays the bills it's sent as far as it can (`Post.honour`, `Bills.present`): whole bills due, from its
/// money beyond its reserve and its households' savings on call (borrowed as needed); the coin, and any bill it can't
/// pay yet (its grace running from the day it was first put to it), go back by post.
#[allow(clippy::too_many_arguments)]
fn honour(content: &Content, now: u64, letter: &Letter, mail: &mut Mail, book: &mut Book, bills: &mut Bills,
        ledger: &mut Ledger, cash: &mut CashBook, towns: &mut Query<Place>, known: &[Known], places: &Distances) {
    let day = content.day_ticks;
    let drawee = letter.to;
    let (_, mut money, mut people, _, trades, farm, deposits, ..) = towns.get_mut(known[drawee].entity).unwrap();
    let reserve = super::farms::reserve(content, trades, farm);
    let owed = deposits.as_ref().map_or(0.0, |d| d.owed);
    let cash_beyond = (money.0 - reserve).at_least_zero();
    let can = cash_beyond + crate::farms::on_call(content, people.purse, owed);
    let mut paid = Fixed::ZERO;
    let mut returned = Vec::new();
    for &id in &letter.bills {
        let Some(b) = bills.bills.get(id) else { continue };
        if b.state == BillState::Paid {
            continue;
        }
        if b.due <= now && paid + b.amount <= can {
            paid += b.amount;
            if b.state == BillState::Outstanding {
                book.kept(b.drawee, b.payee, bills::HONOURED_WEIGHT, now, day);
            }
            bills.bills[id].state = BillState::Paid;
            continue;
        }
        let presented = *bills.bills[id].presented.get_or_insert(now);
        let grace = bills.grace(&bills.bills[id], day, content.dials.bill_grace_days,
            content.dials.bill_grace_emeralds_per_day);
        let b = &mut bills.bills[id];
        if b.state == BillState::Outstanding && now > b.due.max(presented) + grace {
            b.state = BillState::Dishonoured;
            book.missed(b.drawee, b.payee, 1.0, now, day);
        }
        returned.push(id);
    }
    if paid > cash_beyond {
        if let Some(mut deposits) = deposits {
            crate::farms::borrow(content, ledger, cash, drawee, &mut money.0, &mut people.purse, &mut *deposits,
                paid - cash_beyond, now);
        }
    }
    let sent = paid.min(money.0.at_least_zero());
    post(ledger, cash, Side::treasury(drawee, &mut money.0), Side::outside(), sent, remit_reason(), now, day);
    postage(content, ledger, cash, drawee, &mut money.0, now);
    mail.send(content, Letter { kind: Kind::Remit, from: drawee, to: letter.from, value: sent, bills: returned,
        ..Default::default() }, now, places.between(drawee, letter.from));
}

/// A hub reads a town's letter to come call (`Post.callIn`): it books the call if what's offered pays for its trip
/// (`call_pays`; the crown's aid off a first visit, where there's a crown), on the first day the town has heard with no
/// other call booked, and runs there on call where it has no run; the crown pays the aid as far as its treasury holds
/// it. Either way it answers by post.
#[allow(clippy::too_many_arguments)]
fn call_in(content: &Content, now: u64, letter: &Letter, crown: &Crown, roads: &Roads, places: &Distances,
        mail: &mut Mail, ledger: &mut Ledger, cash: &mut CashBook, towns: &mut Query<Place>, known: &[Known]) {
    let day = content.day_ticks;
    let (hub, town) = (letter.to, letter.from);
    let route = roads.route(hub, town);
    let first = route.is_none_or(|r| r.trips == 0);
    let blocks = places.between(hub, town);
    let cost = blocks.map_or(f64::INFINITY, |b| trip_cost(content, b));
    let aid = if first && crown.capital.is_some() { content.dials.crown_aid * cost } else { 0.0 };
    let mut reply = Letter { kind: Kind::Reply, from: hub, to: town, ..Default::default() };
    if known[town].outside || !cost.is_finite() || !call_pays(content, letter.value.to_f64(), cost, aid) {
        let (_, mut money, ..) = towns.get_mut(known[hub].entity).unwrap();
        postage(content, ledger, cash, hub, &mut money.0, now);
        mail.send(content, reply, now, blocks); // declined
        return;
    }
    let booked = mail.day_for(content, hub, arrival(content, now, blocks));
    mail.calls.push(Call { hub, town, day: booked, value: letter.value });
    if route.is_none() && !mail.pending.contains(&(hub, town)) {
        mail.pending.push((hub, town));
    }
    // The crown's aid, out of its capital's treasury, as far as it holds it (`payAid`).
    if let Some(capital) = crown.capital.filter(|&c| c != hub && c < known.len() && aid > 0.0) {
        if let Ok([mut royal, mut at_hub]) = towns.get_many_mut([known[capital].entity, known[hub].entity]) {
            // Rounded down to the thousandth: aid paid out.
            let paid = Fixed::floor(aid).min(royal.1.0.at_least_zero());
            post(ledger, cash, Side::treasury(capital, &mut royal.1.0), Side::treasury(hub, &mut at_hub.1.0), paid,
                aid_reason(), now, day);
        }
    }
    reply.day = Some(booked);
    let (_, mut money, ..) = towns.get_mut(known[hub].entity).unwrap();
    postage(content, ledger, cash, hub, &mut money.0, now);
    mail.send(content, reply, now, blocks);
}

/// Once a day, last of the reviews: every town writes (`Post.write`): its booking with its hub's forwarder
/// (`considerBooking`), its bills due on markets no wagon of either goes to (`mailBills`) and its copies a day old
/// (`mailCopies`); then each hub's forwarder reviews its bookings and lots (`Forwarder.review`).
#[allow(clippy::too_many_arguments)]
pub fn write(clock: Res<Clock>, content: Res<Content>, list: Res<Towns>, crown: Res<Crown>, mut mail: ResMut<Mail>,
        mut forwarder: ResMut<Forwarder>, book: Res<Book>, mut bills: ResMut<Bills>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut towns: Query<Place>, routes: Query<(&Route, &RouteState)>, pallets: Res<Pallets>) {
    let now = clock.now;
    let today = today(&content, now);
    let n = list.0.len();
    if (0..n).all(|t| mail.last_wrote.get(&t) == Some(&today)) {
        return;
    }
    let roads = roads(routes.iter());
    let places = distances(towns.iter().map(|p| p.0), &roads);
    let known = known(&towns, &list);
    let mut wrote = Vec::new();
    for t in 0..n {
        if known[t].outside || mail.last_wrote.get(&t) == Some(&today) {
            continue;
        }
        mail.last_wrote.insert(t, today);
        wrote.push(t);
        consider_booking(&content, now, t, &crown, &book, &roads, &places, &known, &mut mail, &forwarder, &mut ledger,
            &mut cash, &mut towns, &pallets);
        mail_bills(&content, now, t, &roads, &places, &known, &mut mail, &mut bills, &mut ledger, &mut cash, &mut towns);
        mail_copies(&content, now, t, &book, &roads, &places, &known, &mut mail, &mut ledger, &mut cash, &mut towns);
    }
    for hub in wrote {
        if known[hub].hub {
            review_hub(&content, now, hub, &roads, &places, &known, &mut mail, &mut forwarder, &mut ledger, &mut cash,
                &mut towns);
        }
    }
}

/// A host's one-off forwarder review at a hub now ([`crate::society::Society::forwarder_review`]).
#[allow(clippy::too_many_arguments)]
pub fn review_one(In(hub): In<TownId>, clock: Res<Clock>, content: Res<Content>, list: Res<Towns>,
        mut mail: ResMut<Mail>, mut forwarder: ResMut<Forwarder>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut towns: Query<Place>, routes: Query<(&Route, &RouteState)>) {
    let roads = roads(routes.iter());
    let places = distances(towns.iter().map(|p| p.0), &roads);
    let known = known(&towns, &list);
    review_hub(&content, clock.now, hub, &roads, &places, &known, &mut mail, &mut forwarder, &mut ledger, &mut cash,
        &mut towns);
}

/// A host's letter, posted now ([`crate::society::Society::send_letter`]): the sender pays the postage. None goes to or
/// from a town beyond the border, or to itself.
pub fn send_now(In(letter): In<Letter>, clock: Res<Clock>, content: Res<Content>, list: Res<Towns>,
        mut mail: ResMut<Mail>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut towns: Query<Place>,
        routes: Query<(&Route, &RouteState)>) {
    let known = known_of(&towns, &list, false);
    if known[letter.from].outside || known[letter.to].outside || letter.from == letter.to {
        return; // the crown's riders don't go beyond its border
    }
    let roads = roads(routes.iter());
    let places = distances(towns.iter().map(|p| p.0), &roads);
    let (_, mut money, ..) = towns.get_mut(list.0[letter.from]).expect("a town");
    postage(&content, &mut ledger, &mut cash, letter.from, &mut money.0, clock.now);
    let blocks = places.between(letter.from, letter.to);
    mail.send(&content, letter, clock.now, blocks);
}

/// The goods a town would send the hub's network for sale (`Forwarder.forSale`): what the hub and the far ends of its
/// runs (but the town) want, beyond the town's own par and what it owes under harvest contracts, a wagon's worth.
#[allow(clippy::too_many_arguments)]
fn sale_goods(content: &Content, now: u64, town: TownId, hub: TownId, book: &Book, roads: &Roads, known: &[Known],
        stock: &[f64], pallets: &Pallets) -> (Vec<(GoodId, f64)>, u32) {
    let n = content.goods.len();
    let mut want = vec![false; n];
    let mut ends = roads.served_by(hub, town);
    ends.push(hub);
    for end in ends.into_iter().filter(|&e| e < known.len()) {
        for (g, w) in want.iter_mut().enumerate() {
            *w |= crate::post::wanted(&known[end].trades, g);
        }
    }
    let par: Vec<f64> = (0..n).map(|g| shelf(content, &known[town].trades, g)).collect();
    let aside = crate::cargo::set_aside_all(book, pallets, town, n, now, content.day_ticks);
    for_sale(content, stock, &par, &aside, &want)
}

/// The hub a town books with (`Forwarder.hubFor`, `Post.hubsFor`): the nearest hub its wagons could reach, not itself.
fn hub_for(content: &Content, town: TownId, known: &[Known], places: &Distances) -> Option<(TownId, u32)> {
    (0..known.len()).filter(|&h| h != town && known[h].hub && !known[h].outside)
        .filter_map(|h| places.between(town, h).map(|b| (h, b)))
        .filter(|&(_, b)| crate::road::in_range(content, b))
        .min_by_key(|&(h, b)| (b, h))
}

/// Once a day, at a town (`Forwarder.considerBooking`): if it doesn't ship for itself (a wagon of its own to the hub
/// that isn't waiting on a load) and has goods for sale worth twice their fare, it books with its hub by post; one
/// booking at a time. A hub, and the crown's capital, book with no one.
#[allow(clippy::too_many_arguments)]
fn consider_booking(content: &Content, now: u64, town: TownId, crown: &Crown, book: &Book, roads: &Roads,
        places: &Distances, known: &[Known], mail: &mut Mail, forwarder: &Forwarder, ledger: &mut Ledger,
        cash: &mut CashBook, towns: &mut Query<Place>, pallets: &Pallets) {
    if known[town].hub || crown.capital == Some(town) {
        return;
    }
    let Some((hub, blocks)) = hub_for(content, town, known, places) else { return };
    if roads.route(town, hub).is_some_and(|r| !r.waiting) || forwarder.bookings.iter().any(|b| b.town == town)
        || mail.on_the_road_from(hub, Kind::Book, town) > 0 {
        return;
    }
    let (_, mut money, _, stock, ..) = towns.get_mut(known[town].entity).unwrap();
    let (goods, stacks) = sale_goods(content, now, town, hub, book, roads, known, &stock.0, pallets);
    let worth: f64 = goods.iter().map(|&(g, n)| n * content.goods[g].value).sum();
    let value = Fixed::round(worth);
    let fare = fare(content, blocks, roads.route(hub, town).is_some_and(|r| !r.on_call));
    if stacks == 0 || worth < 2.0 * fare * stacks as f64 {
        return;
    }
    postage(content, ledger, cash, town, &mut money.0, now);
    mail.send(content, Letter { kind: Kind::Book, from: town, to: hub, stacks, value, ..Default::default() },
        now, Some(blocks));
}

/// Once a day, at a town (`Post.mailBills`): the bills it holds on markets no wagon of either goes to, once due, sent to
/// their drawee by post, a letter to each.
#[allow(clippy::too_many_arguments)]
fn mail_bills(content: &Content, now: u64, town: TownId, roads: &Roads, places: &Distances, known: &[Known],
        mail: &mut Mail, bills: &mut Bills, ledger: &mut Ledger, cash: &mut CashBook, towns: &mut Query<Place>) {
    let mut by_drawee: BTreeMap<TownId, Vec<usize>> = BTreeMap::new();
    for &id in &bills.open {
        let b = &bills.bills[id];
        if b.held == Held::Town(town) && b.payee == town && b.state == BillState::Outstanding && b.drawee != town
            && b.drawee < known.len() && !known[b.drawee].outside && b.due <= now && !roads.regular_between(town, b.drawee) {
            by_drawee.entry(b.drawee).or_default().push(id);
        }
    }
    for (drawee, ids) in by_drawee {
        for &id in &ids {
            bills.bills[id].held = Held::Wagon; // with the riders
        }
        let (_, mut money, ..) = towns.get_mut(known[town].entity).unwrap();
        postage(content, ledger, cash, town, &mut money.0, now);
        mail.send(content, Letter { kind: Kind::Bill, from: town, to: drawee, bills: ids, ..Default::default() }, now,
            places.between(town, drawee));
    }
}

/// A hub's forwarder's day (`Forwarder.review`): collections for the towns whose bookings fill a wagon's share or
/// have waited (a call booked today at each, its run there opened on call where it has none), and the proceeds of
/// lots sold remitted to their towns by post from the hub's money beyond its reserve.
#[allow(clippy::too_many_arguments)]
fn review_hub(content: &Content, now: u64, hub: TownId, roads: &Roads, places: &Distances, known: &[Known],
        mail: &mut Mail, forwarder: &mut Forwarder, ledger: &mut Ledger, cash: &mut CashBook, towns: &mut Query<Place>) {
    let today = today(content, now);
    for c in forwarder.collections(content, hub, now) {
        if roads.route(hub, c.town).is_none() && !mail.pending.contains(&(hub, c.town)) {
            mail.pending.push((hub, c.town));
        }
        mail.calls.push(Call { hub, town: c.town, day: today, value: c.worth });
    }
    let (_, mut money, _, _, trades, farm, ..) = towns.get_mut(known[hub].entity).unwrap();
    let mut spare = (money.0 - super::farms::reserve(content, trades, farm)).at_least_zero();
    for (town, net) in forwarder.remits(content, hub, now, &mut spare) {
        post(ledger, cash, Side::treasury(hub, &mut money.0), Side::outside(), net, remit_reason(), now,
            content.day_ticks);
        postage(content, ledger, cash, hub, &mut money.0, now);
        mail.send(content, Letter { kind: Kind::Remit, from: hub, to: town, value: net, ..Default::default() }, now,
            places.between(hub, town));
    }
}

/// A run on call stays home but on a day a call is booked or a collection's due (`Caravans` 174: "on call: waiting for
/// the town to write").
pub fn stay_on_call(clock: Res<Clock>, content: Res<Content>, mail: Res<Mail>, forwarder: Res<Forwarder>,
        mut routes: Query<(&Route, &mut RouteState)>) {
    let today = today(&content, clock.now);
    for (route, mut state) in &mut routes {
        if route.on_call && state.plan.is_some() && !mail.due(route.from_id, route.to_id, today)
            && !forwarder.is_collecting(route.from_id, route.to_id) {
            state.plan = None;
        }
    }
}

/// A hub's run on call with a call due, or any hub's run with a collection, goes (`Caravans.dispatch`'s "on call", "a
/// collection"); a run on call with a purse for the goods offered on the calls due, as far as the hub can spare beyond
/// its reserve and its day's budget allows ("a call's goods", `callPurse`: only `route.onCall`'s). A regular run's
/// calls (the forwarder books one with each collection) carry no purse and press it to go only through the collection.
#[allow(clippy::type_complexity)]
pub fn go_on_call(clock: Res<Clock>, content: Res<Content>, mail: Res<Mail>, forwarder: Res<Forwarder>,
        bills: Res<Bills>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&Money, &Trades, &mut Budget, Option<&Farm>)>) {
    let now = clock.now;
    let today = today(&content, now);
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        let Some(plan) = state.plan.as_mut() else { continue };
        let due = route.on_call && mail.due(route.from_id, route.to_id, today);
        if !due && !forwarder.is_collecting(route.from_id, route.to_id) {
            continue;
        }
        plan.pressing = true;
        if !route.on_call {
            continue;
        }
        let offered = mail.purse(route.from_id, route.to_id, today);
        if offered <= 0.0 {
            continue;
        }
        let Ok((money, trades, mut budget, farm)) = towns.get_mut(route.from) else { continue };
        let spare = (money.0 - super::farms::reserve(&content, trades, farm) - plan.purse).at_least_zero();
        let free_money = free(&content, trades, money, &bills, route.from_id, now);
        plan.purse += Accountant::allow(&content, &mut cash, route.from_id, now, free_money, &mut budget,
            Reason::Other("a call's goods".into()), offered.min(spare));
    }
}

/// A run on call has left: its calls due are made (`Post.made`, called only for `route.onCall`). A regular run's calls
/// (booked with the forwarder's collections) the mod never clears; nothing reads them past their day, so a regular run
/// leaving drops those of days gone, to keep the post's book from growing without end.
pub fn made(clock: Res<Clock>, content: Res<Content>, mut mail: ResMut<Mail>, routes: Query<(&Route, &RouteState)>) {
    let today = today(&content, clock.now);
    for (route, state) in &routes {
        if state.wagon.is_none() || state.last_departure != Some(clock.now) {
            continue;
        }
        if route.on_call {
            if mail.due(route.from_id, route.to_id, today) {
                mail.made(route.from_id, route.to_id, today);
            }
        } else if today > 0 && mail.due(route.from_id, route.to_id, today - 1) {
            mail.made(route.from_id, route.to_id, today - 1);
        }
    }
}

/// A hub's wagon's sales at the far end: some may be towns' goods on consignment, sold on their account (`Caravans`
/// 2557: `Forwarder.sold`).
pub fn credit_sold(mut forwarder: ResMut<Forwarder>, routes: Query<(Entity, &Route, &RouteState)>, hubs: Query<&Hub>) {
    if forwarder.lots.is_empty() {
        return;
    }
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(e).unwrap();
        if state.arrived != Some(Leg::Out) || !hubs.get(route.from).is_ok_and(|h| h.storehouse) {
            continue;
        }
        let w = state.wagon.as_ref().unwrap();
        for (g, &n) in w.sold_here.iter().enumerate() {
            if n > 0.0 {
                forwarder.sold(route.from_id, g, n, w.paid_here.get(g).copied().unwrap_or_default());
            }
        }
    }
}

/// What a hub's network has room for of each good (`consignUnsold`'s `roomFor`): the hub's par where it wants it, and
/// each far end's of its runs (but `town`) that wants it, less what the hub holds.
fn room_at(content: &Content, hub: TownId, town: TownId, roads: &Roads, trades: &BTreeMap<TownId, Trades>,
        held: &[f64], good: GoodId) -> f64 {
    let mut want = trades.get(&hub).map_or(0.0, |t| shelf(content, t, good));
    for end in roads.served_by(hub, town) {
        if let Some(t) = trades.get(&end).filter(|t| crate::post::wanted(t, good)) {
            want += shelf(content, t, good);
        }
    }
    want - held[good]
}

/// A town's wagon at a hub with goods it couldn't sell (`Caravans` 711): what the hub's network wants the forwarder
/// takes on consignment, into the hub's warehouse as the town's lots (`consignUnsold`); the rest rides home. Not a
/// hub's wagon, nor a foreign realm's, nor a market's off the map (no post goes there to remit its proceeds by).
#[allow(clippy::type_complexity)]
pub fn consign(clock: Res<Clock>, content: Res<Content>, mut forwarder: ResMut<Forwarder>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&Town, &mut Stock, &Trades, &Hub, Option<&Realm>, Option<&OffMap>)>) {
    let arriving: Vec<Entity> = in_order(routes.iter().map(|(e, r, _)| (e, r))).into_iter().filter(|&e| {
        let (_, r, s) = routes.get(e).unwrap();
        s.arrived == Some(Leg::Out) && towns.get(r.to).is_ok_and(|t| t.3.storehouse)
            && towns.get(r.from).is_ok_and(|t| !t.3.storehouse && t.4.is_none() && t.5.is_none())
    }).collect();
    if arriving.is_empty() {
        return;
    }
    let roads = roads(routes.iter().map(|(_, r, s)| (r, s)));
    let trades: BTreeMap<TownId, Trades> = towns.iter().map(|t| (t.0.id, t.2.clone())).collect();
    for e in arriving {
        let (_, route, mut state) = routes.get_mut(e).unwrap();
        let route = *route;
        let wagon = state.wagon.as_mut().unwrap();
        let (_, mut stock, ..) = towns.get_mut(route.to).unwrap();
        let held = stock.0.clone();
        let taken = forwarder.consign_unsold(&content, route.to_id, route.from_id, &mut wagon.cargo, clock.now,
            |g| room_at(&content, route.to_id, route.from_id, &roads, &trades, &held, g));
        for (g, n) in taken {
            stock.0[g] += n;
        }
    }
}

/// A host's goods offered to a hub's forwarder ([`crate::society::Society::consign_unsold`]): those it takes into the
/// hub's warehouse (come into the society), on consignment; returned.
#[allow(clippy::type_complexity)]
pub fn consign_host(In((hub, town, mut cargo)): In<(TownId, TownId, Vec<f64>)>, clock: Res<Clock>, content: Res<Content>,
        list: Res<Towns>, mut forwarder: ResMut<Forwarder>, mut ledger: ResMut<Ledger>,
        routes: Query<(&Route, &RouteState)>, mut towns: Query<(&Town, &mut Stock, &Trades)>) -> Vec<(GoodId, f64)> {
    let roads = roads(routes.iter());
    let trades: BTreeMap<TownId, Trades> = towns.iter().map(|t| (t.0.id, t.2.clone())).collect();
    let (_, mut stock, _) = towns.get_mut(list.0[hub]).expect("a hub");
    let held = stock.0.clone();
    let taken = forwarder.consign_unsold(&content, hub, town, &mut cargo, clock.now,
        |g| room_at(&content, hub, town, &roads, &trades, &held, g));
    for &(g, n) in &taken {
        stock.0[g] += n;
        ledger.delivered[g] += n;
    }
    taken
}

/// The hub's wagon at a booked town (`Forwarder.collect`): its goods for sale taken on consignment, as far as the wagon has
/// room, each lot owing its stacks' collection fare; the booking is done. Since `consignment-pallets` the goods go packed as
/// pallets made out to the hub (shipper the town, no contract, [`Payment::Paid`]: the goods are the town's and nobody
/// owes for them), crated at the town's cost where it can spare crates and loose on their pallet where it can't, sealed as
/// the wagon loads them, a superbill for two or more; the hub opens them at home into its stock (`rules/cargo.rs`
/// `unload`), where the lots they stand for are sold from. The lots are told as before, at collection.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn collect(clock: Res<Clock>, content: Res<Content>, list: Res<Towns>, book: Res<Book>,
        mut forwarder: ResMut<Forwarder>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<Place>, mut premises: Query<(&mut crate::cargo::Shipments, &mut crate::cargo::Paperwork)>,
        mut pallets: ResMut<Pallets>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>) {
    if forwarder.collecting.is_empty() {
        return;
    }
    let now = clock.now;
    let order = in_order(routes.iter().map(|(e, r, _)| (e, r)));
    for e in order {
        let (_, route, state) = routes.get(e).unwrap();
        if state.arrived != Some(Leg::Out) || !forwarder.is_collecting(route.from_id, route.to_id) {
            continue;
        }
        let route = *route;
        forwarder.collected(route.from_id, route.to_id);
        let roads = roads(routes.iter().map(|(_, r, s)| (r, s)));
        let known = known(&towns, &list);
        // The fare on the towns' distance, as the booking reckoned it (`Forwarder.fare`'s `distSqr`), not the road's.
        let blocks = distances(towns.iter().map(|p| p.0), &roads).between(route.from_id, route.to_id)
            .unwrap_or(route.blocks);
        let fare = fare(&content, blocks, !route.on_call);
        let (_, _, mut state) = routes.get_mut(e).unwrap();
        let wagon = state.wagon.as_mut().unwrap();
        let entity = route.to;
        let (_, mut money, _, mut stock, trades, farm, ..) = towns.get_mut(entity).unwrap();
        let Ok((mut store, mut paperwork)) = premises.get_mut(entity) else { continue };
        let (goods, _) = sale_goods(&content, now, route.to_id, route.from_id, &book, &roads, &known, &stock.0, &pallets);
        let spare = super::cargo::spare(&content, money.0, trades, farm);
        let mut shipper = crate::cargo::Shipper { town: route.to_id, money: &mut money.0, spare, store: &mut store,
            paperwork: &mut paperwork };
        let lading = Lading { payment: Payment::Paid, ..Lading::new(route.to_id, route.from_id, now) };
        for (g, n) in goods {
            let stack = content.goods[g].stack.max(1) as f64;
            // The room left: slots, weight and pallets (`pallet-caps`), as pallets of the good would take them.
            let room = crate::cargo::wagon_room(&content, &pallets, route.seq, wagon).palleted(&content, g);
            let going = n.min(room).min(stock.0[g]).floor();
            if going <= 0.0 {
                continue;
            }
            let packed = crate::cargo::pack_lines(&content, &mut ledger, &mut cash, now, &mut shipper, &mut pallets, &lading,
                &[(g, going)], At::Town(route.to_id), crate::cargo::How::OWED);
            let taken = packed.of(g);
            if taken <= 0.0 {
                continue;
            }
            stock.0[g] -= taken;
            // Made out to the hub, so they come off at its home, the leg's end (`itinerary-drops`: every line loads with
            // a drop).
            for id in packed.ids {
                let line = Line { pallet: id, drop: Some(route.from_id), ..Default::default() };
                crate::cargo::put_aboard(&mut pallets, wagon, route.seq, line, route.to_id, now);
            }
            forwarder.lot(route.from_id, route.to_id, g, taken, Fixed::ceil(fare * (taken / stack).ceil()), now);
        }
    }
}

/// A wagon's purchases at a hub: some may be towns' goods on consignment, sold on their account (`Caravans
/// .boughtThere`: `Forwarder.sold`).
pub fn credit_bought(clock: Res<Clock>, content: Res<Content>, mut forwarder: ResMut<Forwarder>,
        routes: Query<(Entity, &Route, &RouteState)>, hubs: Query<&Hub>) {
    if forwarder.lots.is_empty() {
        return;
    }
    let today = today(&content, clock.now);
    for e in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, state) = routes.get(e).unwrap();
        if state.arrived != Some(Leg::Out) || !hubs.get(route.to).is_ok_and(|h| h.storehouse) {
            continue;
        }
        let w = state.wagon.as_ref().unwrap();
        for e in w.log.iter().filter(|e| e.kind == crate::atlas::Kind::Bought && e.with == Some(route.to_id) && e.day == today) {
            if let Some(g) = e.good {
                forwarder.sold(route.to_id, g, e.count, e.money);
            }
        }
    }
}
