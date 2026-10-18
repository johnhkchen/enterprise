//! The crown's post and the forwarder's consignments (`docs/decisions.md` "Freight: carriers, forwarders, drayage";
//! `docs/core-plan.md` 4.1): riders carry letters between markets on the post days, every `post_days`, faster than any
//! wagon; a letter costs its sender the `postage`, and the crown keeps the riders. A letter waits at the market it's for
//! until the market reads its mail (each look).
//!
//! - **Copies by post:** a contract only one party holds goes to the other a day after it was drafted, where no wagon
//!   runs between them (a wagon that does carries it, as before) and no copy is on the road already.
//! - **Bills by post:** a town holding bills on a market none of its wagons go to (and none of theirs come from) sends
//!   them once due; the drawee pays what it can from its money beyond its reserve (borrowing its households' savings on
//!   call where that isn't enough), and the coin comes back by post, with any bill it couldn't pay yet.
//! - **Come call:** a town with goods a hub wants writes to say so (`Society::book_call`). The hub weighs what's offered
//!   against its trip there (`call_margin` of it against `call_stop` of the trip's road food, less the crown's aid on a
//!   first visit where there's a crown) and books the call on the first day the letter back reaches the town with no
//!   other call booked (`calls_per_day`), or declines. Its run there (on call, where it had none) leaves only on a
//!   booked day, with a purse for what's offered.
//! - **The forwarder:** a town whose own wagon wouldn't go books carriage with its hub by post: its goods for sale (beyond
//!   its own par, that the hub's network wants), when worth twice their fare. The forwarder collects once a town's
//!   bookings fill `collect_stacks` or have waited `collect_days`: the hub's run there goes that day (a run on call
//!   where it has none) and takes the goods as the town's lot, on consignment. What a town's wagon can't sell at the hub
//!   the forwarder takes on consignment too, as far as the hub's and its runs' pars have room. Lots sell with the hub's
//!   own goods (its wagons' sales and what visiting wagons buy there, oldest lot first); the proceeds go back to the town
//!   by post, less `consign_commission` and the collection's fare, once they're an emerald or the lot is done; a lot not
//!   sold in `consign_days` the forwarder buys at a trader's floor.
//!
//! Ported from `market/Post.java`: `arrival` (82), `post` (90), `atDesk` (100), `read` (112), `write` (133),
//! `mailCopies` (143), `book` (165), `remit` (179), `bookCall` (189), `onTheRoad`, `hubsFor` (200), `tripCost` (233),
//! `callIn` (241–282), `payAid` (285), `due` (299), `purse` (304), `made` (310), `mailBills` (320), `honour` (354);
//! `market/Forwarder.java`: `hubFor`, `fare` (88), `forSale` (96), `shipsItself` (134), `considerBooking` (143), `booked`
//! (170), `review` (186–208), `collecting` (211), `collect` (220), `consignUnsold` (264–316), `sold` (319), `remit`
//! (345–383); `market/Freight.java` `runCost` (437); `contract/Bills.java` `present` (266); `caravan/Caravans.java` the
//! on-call run (174, 357, 400) and the forwarder's hooks (120, 635, 711, 2557); `Dials.java` `POST_DAYS` to
//! `CONSIGN_DAYS`.
//!
//! **Where the core differs.** Money is emerald-worth, so the postage is a tenth of an emerald off the sender's money (the
//! mod's coin and change are the binding's) and the forwarder's sums are in emeralds. Coin in a letter is outside the
//! society's accounts while a rider carries it (posted out as "remit" and in again on arrival). A town reads its mail
//! every look, and writes once a day at the review; a contract was drafted, as far as the post knows, since the look
//! before the one that first saw it drafted. Where neither town is placed
//! and no road joins them, a rider takes its least time. A copy goes by post only where no wagon runs between the two
//! (the brief's rule: the mod posts one wherever the other party hasn't seen it a day on), and a bill only where no
//! wagon runs either way (the mod also posts one a post round past due that no wagon took); a run on call, which goes
//! only on booked days, doesn't count as one. The crown's aid counts
//! toward a call only where there's a crown to pay it. The forwarder's bookings are its towns' goods for sale (contract
//! crates bound on through the hub, `collectForwarding`, aren't collected yet), and a collection's fare is a run on
//! call's (half as much again) where the hub's run there is on call or not there yet. Bills in a letter are held
//! [`crate::bills::Held::Wagon`]: with a carrier, not at a town.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::hex::Hex;
use crate::messages::TownId;

/// What a letter is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// A town to a hub: come call, its goods offered worth `value`.
    Call,
    /// A hub's answer: the day booked, or declined.
    #[default]
    Reply,
    /// Bills sent to their drawee.
    Bill,
    /// Coin (`value`) and any bills not paid, back to their holder; or a lot's proceeds from a forwarder.
    Remit,
    /// A town books `stacks` with its hub's forwarder, its goods for sale worth `value`.
    Book,
    /// A contract's copy (`contract`) for the party that hasn't one.
    Copy,
}

/// A letter on the road: what it is, from and to, when posted and when it arrives, and what it carries.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Letter {
    pub kind: Kind,
    pub from: TownId,
    pub to: TownId,
    pub posted: u64,
    pub arrives: u64,
    /// A call: what the goods offered are worth; a booking: the goods for sale; a remittance: the emeralds it carries
    /// (in thousandths, `fixed.rs`).
    pub value: crate::fixed::Fixed,
    /// A reply: the day booked (none: declined).
    pub day: Option<u64>,
    /// A booking: stacks to collect.
    pub stacks: u32,
    /// A copy: the contract's.
    pub contract: Option<usize>,
    /// Bills sent, or returned unpaid.
    pub bills: Vec<usize>,
}

/// A call a hub has booked (`Post.Call`): the town, the day, and the worth of the goods offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub hub: TownId,
    pub town: TownId,
    pub day: u64,
    pub value: crate::fixed::Fixed,
}

/// The crown's post (`Post`): letters on the road, calls booked, the day each town last wrote, letters sent in all;
/// when the post first saw each contract still drafted; and the runs on call to open once the look is done.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mail {
    pub letters: Vec<Letter>,
    pub calls: Vec<Call>,
    pub last_wrote: BTreeMap<TownId, u64>,
    pub sent: u64,
    pub seen: BTreeMap<usize, u64>,
    pub pending: Vec<(TownId, TownId)>,
}

/// When a letter posted `now` arrives (`Post.arrival`): it leaves on the next post day, every `post_days`, and a rider
/// covers `post_speed` blocks a day, taking a tenth of a day at least (the mod's 2,400 ticks). `None` blocks: the
/// towns' distance isn't known, and the rider takes its least time.
pub fn arrival(content: &Content, now: u64, blocks: Option<u32>) -> u64 {
    let day = content.day_ticks.max(1);
    let every = content.dials.post_days.max(1) as u64 * day;
    let leaves = now.div_ceil(every) * every;
    let ride = blocks.map_or(0, |b| (b as f64 / content.dials.post_speed.max(1e-9) * day as f64) as u64);
    leaves + ride.max(day / 10)
}

impl Mail {
    /// Letters on the road to `to` of this kind (`onTheRoad`).
    pub fn on_the_road(&self, to: TownId, kind: Kind) -> usize {
        self.letters.iter().filter(|l| l.to == to && l.kind == kind).count()
    }

    /// Letters on the road to `to` of this kind from `from`.
    pub fn on_the_road_from(&self, to: TownId, kind: Kind, from: TownId) -> usize {
        self.letters.iter().filter(|l| l.to == to && l.kind == kind && l.from == from).count()
    }

    /// The calls booked at a hub.
    pub fn calls_at(&self, hub: TownId) -> Vec<Call> {
        self.calls.iter().filter(|c| c.hub == hub).copied().collect()
    }

    /// Is a call booked for the hub's run to the town by `today` (`Post.due`)?
    pub fn due(&self, hub: TownId, town: TownId, today: u64) -> bool {
        self.calls.iter().any(|c| c.hub == hub && c.town == town && c.day <= today)
    }

    /// The worth of the goods offered on the calls due by `today` for that run: its purse (`Post.purse`).
    pub fn purse(&self, hub: TownId, town: TownId, today: u64) -> crate::fixed::Fixed {
        self.calls.iter().filter(|c| c.hub == hub && c.town == town && c.day <= today).map(|c| c.value).sum()
    }

    /// The run has left: its calls due are made (`Post.made`).
    pub fn made(&mut self, hub: TownId, town: TownId, today: u64) {
        self.calls.retain(|c| !(c.hub == hub && c.town == town && c.day <= today));
    }

    /// The day a hub books a call at a town, the letter back reaching it `heard` (a tick): that day, or the first after
    /// it with fewer than `calls_per_day` calls at the hub (`callIn`'s day).
    pub fn day_for(&self, content: &Content, hub: TownId, heard: u64) -> u64 {
        let mut day = heard / content.day_ticks.max(1);
        let most = content.dials.calls_per_day.max(1) as usize;
        while self.calls.iter().filter(|c| c.hub == hub && c.day == day).count() >= most {
            day += 1;
        }
        day
    }

    /// A letter on the road, posted now (the postage paid by the caller), arriving as [`arrival`] says.
    pub fn send(&mut self, content: &Content, mut letter: Letter, now: u64, blocks: Option<u32>) {
        letter.posted = now;
        letter.arrives = arrival(content, now, blocks);
        self.letters.push(letter);
        self.sent += 1;
    }

    /// The letters for `to` that have arrived by `now`, taken off the road, in the order posted.
    pub fn arrived(&mut self, to: TownId, now: u64) -> Vec<Letter> {
        let (mine, rest): (Vec<Letter>, Vec<Letter>) =
            std::mem::take(&mut self.letters).into_iter().partition(|l| l.to == to && l.arrives <= now);
        self.letters = rest;
        mine
    }
}

/// How far apart towns are, in blocks, as the post reckons it: between their hexes' centres where both are placed,
/// else a road's length between them (`Post.arrival`'s `distSqr`; the atlas's `Places::distance`).
#[derive(Clone, Debug, Default)]
pub struct Distances {
    pub hexes: BTreeMap<TownId, Hex>,
    pub roads: BTreeMap<(TownId, TownId), u32>,
}

impl Distances {
    pub fn between(&self, a: TownId, b: TownId) -> Option<u32> {
        match (self.hexes.get(&a), self.hexes.get(&b)) {
            (Some(x), Some(y)) => {
                let (dx, dz) = (x.center_x() - y.center_x(), x.center_z() - y.center_z());
                Some((dx * dx + dz * dz).sqrt().ceil() as u32)
            }
            _ => self.roads.get(&(a, b)).or_else(|| self.roads.get(&(b, a))).copied(),
        }
    }
}

/// A route as the post and the forwarder see it: its ends, whether it still runs (not wound down for a run by a hub),
/// whether its wagon last stayed home waiting for a fuller load, whether it's on call, its trips and its length.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Road {
    pub seq: usize,
    pub from: TownId,
    pub to: TownId,
    pub running: bool,
    pub waiting: bool,
    pub on_call: bool,
    pub trips: u32,
    pub blocks: u32,
}

/// The routes, as [`Road`]s.
#[derive(Clone, Debug, Default)]
pub struct Roads(pub Vec<Road>);

impl Roads {
    /// The running route from `from` to `to`, if any (`CaravanLedger.route`).
    pub fn route(&self, from: TownId, to: TownId) -> Option<&Road> {
        self.0.iter().find(|r| r.running && r.from == from && r.to == to)
    }

    /// Does a wagon run between the two, either way?
    pub fn between(&self, a: TownId, b: TownId) -> bool {
        self.route(a, b).is_some() || self.route(b, a).is_some()
    }

    /// Does a wagon run between the two either way that isn't on call (one that goes only on a booked day carries no
    /// copy or bill in between)? The post's test for copies and bills.
    pub fn regular_between(&self, a: TownId, b: TownId) -> bool {
        self.0.iter().any(|r| r.running && !r.on_call && ((r.from == a && r.to == b) || (r.from == b && r.to == a)))
    }

    /// The far ends of the hub's running routes, but `but`.
    pub fn served_by(&self, hub: TownId, but: TownId) -> Vec<TownId> {
        let ends: BTreeSet<TownId> = self.0.iter().filter(|r| r.running && r.from == hub && r.to != but).map(|r| r.to).collect();
        ends.into_iter().collect()
    }
}

/// What a run's road food costs a hub a trip there, both ways (`Post.tripCost`: `Freight.runCost`).
pub fn trip_cost(content: &Content, blocks: u32) -> f64 {
    crate::freight::run_cost(content, blocks)
}

/// Does the hub weigh a call worth its trip (`callIn`)? What's offered, at `call_margin`, against `call_stop` of the
/// trip's cost less the crown's `aid`.
pub fn call_pays(content: &Content, value: f64, cost: f64, aid: f64) -> bool {
    value * content.dials.call_margin >= content.dials.call_stop * (cost - aid)
}

/// A stack's collection fare (`Forwarder.fare`): the hub's run's road food over a wagon's space, with the carriers'
/// margin; half as much again on a run on call (or none yet), where the run goes only for the collection.
pub fn fare(content: &Content, blocks: u32, regular: bool) -> f64 {
    let d = &content.dials;
    crate::freight::run_cost(content, blocks) / d.wagon_slots.max(1) as f64 * (1.0 + d.freight_margin)
        * if regular { 1.0 } else { 1.5 }
}

/// Does a town's warehouse want a good: its lockers take it, or its people use it (`Caravans.wantedAt`)?
pub fn wanted(trades: &crate::state::Trades, good: GoodId) -> bool {
    trades.capacity.get(good).is_some_and(|c| *c > 0.0) || trades.uses.get(good).is_some_and(|u| *u > 0.0)
}

/// What a town would consign (`Forwarder.forSale`): of the goods `wanted` (by the hub's network), what it holds beyond
/// its own par and what it owes under harvest contracts (`aside`), a wagon's worth at most, in the goods' order; and the
/// stacks it takes.
pub fn for_sale(content: &Content, held: &[f64], par: &[f64], aside: &[f64], wanted: &[bool]) -> (Vec<(GoodId, f64)>, u32) {
    let mut out = Vec::new();
    let mut stacks = 0u32;
    for g in 0..content.goods.len() {
        if !wanted.get(g).copied().unwrap_or(false) || content.goods[g].value <= 0.0 {
            continue;
        }
        let stack = content.goods[g].stack.max(1) as f64;
        let spare = (held[g] - par[g] - aside[g]).floor();
        let room = content.dials.wagon_slots.saturating_sub(stacks) as f64 * stack;
        let going = spare.min(room);
        if going <= 0.0 {
            continue;
        }
        out.push((g, going));
        stacks += (going / stack).ceil() as u32;
    }
    (out, stacks)
}

/// A town's booking with a hub (`Forwarder.Booking`): its stacks to collect, what its goods for sale are worth, and
/// since when.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Booking {
    pub hub: TownId,
    pub town: TownId,
    pub stacks: u32,
    pub worth: crate::fixed::Fixed,
    pub since: u64,
}

/// Goods on consignment at a hub (`Forwarder.Lot`): the town's, of a good, how many, how many sold and for what (in
/// emeralds, not yet remitted), the collection's fare still to come off, and since when.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lot {
    pub hub: TownId,
    pub town: TownId,
    pub good: GoodId,
    pub count: f64,
    pub sold: f64,
    pub proceeds: crate::fixed::Fixed,
    pub fare: crate::fixed::Fixed,
    pub since: u64,
}

/// The forwarders at every hub (`Forwarder`): towns' bookings, lots on consignment, the collections due (hub, town),
/// and what they've remitted, kept as commission and taken on consignment from towns' wagons, all told.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Forwarder {
    pub bookings: Vec<Booking>,
    pub lots: Vec<Lot>,
    pub collecting: Vec<(TownId, TownId)>,
    pub remitted: crate::fixed::Fixed,
    pub commission: crate::fixed::Fixed,
    pub consigned: f64,
}

/// A collection a hub's forwarder decided on: the town, and the worth of its goods for sale (the call's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Collection {
    pub town: TownId,
    pub worth: crate::fixed::Fixed,
}

impl Forwarder {
    /// The hub reads a booking (`booked`): the town's last one replaced.
    pub fn booked(&mut self, hub: TownId, town: TownId, stacks: u32, worth: crate::fixed::Fixed, now: u64) {
        self.bookings.retain(|b| b.town != town);
        self.bookings.push(Booking { hub, town, stacks, worth, since: now });
    }

    /// Is a collection due on the hub's run to the town (so the run goes)?
    pub fn is_collecting(&self, hub: TownId, town: TownId) -> bool {
        self.collecting.contains(&(hub, town))
    }

    /// Once a day at a hub (`review`'s first part): the towns whose bookings fill `collect_stacks` or have waited
    /// `collect_days` are collected from; returned, in booking order.
    pub fn collections(&mut self, content: &Content, hub: TownId, now: u64) -> Vec<Collection> {
        let d = &content.dials;
        let wait = (d.collect_days * content.day_ticks as f64) as u64;
        let mut out = Vec::new();
        for b in &self.bookings {
            if b.hub != hub || self.collecting.contains(&(hub, b.town)) {
                continue;
            }
            let enough = b.stacks >= d.collect_stacks;
            let waited = now.saturating_sub(b.since) >= wait;
            if enough || waited {
                out.push(Collection { town: b.town, worth: b.worth });
            }
        }
        for c in &out {
            self.collecting.push((hub, c.town));
        }
        out
    }

    /// The hub's wagon at a booked town (`collect`): the collection's done and the booking with it; true if one was due.
    pub fn collected(&mut self, hub: TownId, town: TownId) -> bool {
        let due = self.is_collecting(hub, town);
        self.collecting.retain(|&c| c != (hub, town));
        self.bookings.retain(|b| !(b.town == town && b.hub == hub));
        due
    }

    /// A collection that can't be made (no run there can be opened): dropped, and its booking.
    pub fn drop_collection(&mut self, hub: TownId, town: TownId) {
        self.collected(hub, town);
    }

    /// A lot of the town's goods on consignment at the hub, its collection's fare to come off its proceeds.
    pub fn lot(&mut self, hub: TownId, town: TownId, good: GoodId, count: f64, fare: crate::fixed::Fixed, now: u64) {
        self.lots.push(Lot { hub, town, good, count, sold: 0.0, proceeds: crate::fixed::Fixed::ZERO, fare, since: now });
    }

    /// Of a good, what's on consignment at the hub and not yet sold.
    pub fn on_consignment(&self, hub: TownId, good: GoodId) -> f64 {
        self.lots.iter().filter(|l| l.hub == hub && l.good == good).map(|l| l.count - l.sold).sum()
    }

    /// Goods of `good` the hub sold (its wagons', or to a wagon calling there) for `emeralds`: credited to the towns'
    /// lots of it, oldest first (`sold`), each lot's share to the nearest thousandth.
    pub fn sold(&mut self, hub: TownId, good: GoodId, count: f64, emeralds: crate::fixed::Fixed) {
        if count <= 0.0 {
            return;
        }
        let each = emeralds.to_f64() / count;
        let mut left = count;
        for lot in self.lots.iter_mut() {
            if left <= 0.0 {
                break;
            }
            if lot.hub != hub || lot.good != good || lot.sold >= lot.count {
                continue;
            }
            let n = left.min(lot.count - lot.sold);
            lot.sold += n;
            lot.proceeds += crate::fixed::Fixed::round(n * each);
            left -= n;
        }
    }

    /// What a town's wagon couldn't sell at the hub, the forwarder takes on consignment (`consignUnsold`): of each good
    /// in `cargo` worth something, as much as `room(good)` holds (the hub's and its runs' pars, less what the hub holds;
    /// less what's on consignment already, here), all of it or whole stacks. Taken out of `cargo` and returned as
    /// (good, count); the lots are the town's from now.
    pub fn consign_unsold(&mut self, content: &Content, hub: TownId, town: TownId, cargo: &mut [f64], now: u64,
            room: impl Fn(GoodId) -> f64) -> Vec<(GoodId, f64)> {
        let mut taken = Vec::new();
        for (g, n) in cargo.iter_mut().enumerate() {
            if *n <= 0.0 || content.goods[g].value <= 0.0 {
                continue;
            }
            let can = (room(g) - self.on_consignment(hub, g)).max(0.0);
            let stack = content.goods[g].stack.max(1) as f64;
            let take = if *n <= can { *n } else { ((can / stack).floor() * stack).min(*n) };
            if take <= 0.0 {
                continue;
            }
            *n -= take;
            taken.push((g, take));
        }
        for &(g, count) in &taken {
            self.lot(hub, town, g, count, crate::fixed::Fixed::ZERO, now);
            self.consigned += count;
        }
        taken
    }

    /// Once a day at a hub (`remit`): each lot's proceeds less the commission and the collection's fare, owed its town,
    /// once they're an emerald or the lot is done (a lot not sold in `consign_days` the forwarder buys at a trader's
    /// floor), as far as the hub can `spare` (beyond its reserve; counted down): returned as (town, emeralds), to be sent
    /// by post. A lot done and its fare paid is cleared. The commission is a share kept, rounded down to the thousandth.
    pub fn remits(&mut self, content: &Content, hub: TownId, now: u64, spare: &mut crate::fixed::Fixed)
            -> Vec<(TownId, crate::fixed::Fixed)> {
        let d = &content.dials;
        let wait = (d.consign_days * content.day_ticks as f64) as u64;
        let mut out = Vec::new();
        let mut keep = Vec::new();
        for mut lot in std::mem::take(&mut self.lots) {
            if lot.hub != hub {
                keep.push(lot);
                continue;
            }
            let mut done = lot.sold >= lot.count;
            if !done && now.saturating_sub(lot.since) >= wait {
                let rest = lot.count - lot.sold;
                // bought: it's the hub's
                lot.proceeds += crate::fixed::Fixed::round(rest * crate::bargain::seller_floor(content.goods[lot.good].value));
                lot.sold = lot.count;
                done = true;
            }
            let take = lot.proceeds.mul_floor(d.consign_commission);
            let net = lot.proceeds - take - lot.fare;
            if net < crate::fixed::Fixed::ONE && !done {
                keep.push(lot); // less than an emerald yet: it waits for more to sell
                continue;
            }
            if net.is_positive() {
                if *spare < net {
                    keep.push(lot); // the hub's short just now: next time (the postage is paid as far as it goes)
                    continue;
                }
                *spare -= net + crate::fixed::Fixed::ceil(d.postage);
                out.push((lot.town, net));
                self.remitted += net;
            }
            self.commission += take;
            // what the fare left owing, from the next of its sales
            lot.fare = if net.is_negative() { -net } else { crate::fixed::Fixed::ZERO };
            lot.proceeds = crate::fixed::Fixed::ZERO;
            if !(done && !lot.fare.is_positive()) {
                keep.push(lot);
            }
        }
        self.lots = keep;
        out
    }
}

/// The post's reasons in the cash book.
pub fn postage_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("postage".into())
}

pub fn remit_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("remit".into())
}

pub fn aid_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("crown aid".into())
}

use crate::society::Society;
use bevy_ecs::system::RunSystemOnce;

impl Society {
    /// The crown's post as it stands: letters on the road, calls booked.
    pub fn mail(&self) -> &Mail {
        self.world.resource::<Mail>()
    }

    /// The forwarders as they stand: bookings, lots, collections due, totals.
    pub fn forwarder(&self) -> &Forwarder {
        self.world.resource::<Forwarder>()
    }

    /// A letter of `kind` from one town to another, posted now, the sender paying the postage as far as its money goes
    /// (`Post.sendForTest`; a host's own letter). The riders don't go beyond the border (off the map, a foreign realm).
    pub fn send_letter(&mut self, kind: Kind, from: TownId, to: TownId, value: impl Into<crate::fixed::Fixed>) {
        let letter = Letter { kind, from, to, value: value.into(), ..Default::default() };
        self.world.run_system_once_with(crate::rules::post::send_now, letter).expect("the post");
    }

    /// A town writes to a hub to come call: its goods offered worth `value` (`Kind::Call`).
    pub fn book_call(&mut self, town: TownId, hub: TownId, value: impl Into<crate::fixed::Fixed>) {
        self.send_letter(Kind::Call, town, hub, value);
    }

    /// Every letter on the road arrives now (`Post.deliverNowForTest`): read at the next look.
    pub fn deliver_mail_now(&mut self) {
        let now = self.now();
        for l in self.world.resource_mut::<Mail>().letters.iter_mut() {
            l.arrives = l.arrives.min(now);
        }
    }

    /// The calls booked at a hub.
    pub fn calls(&self, hub: TownId) -> Vec<Call> {
        self.mail().calls_at(hub)
    }

    /// Is a call booked for the hub's run to the town by the day of `at` (`Post.due`)?
    pub fn call_due(&self, hub: TownId, town: TownId, at: u64) -> bool {
        self.mail().due(hub, town, at / self.content().day_ticks.max(1))
    }

    /// How far apart two towns are as the post reckons it (between their hexes, else a road's length), if known.
    pub fn mail_distance(&self, a: TownId, b: TownId) -> Option<u32> {
        let mut d = Distances::default();
        for (t, &e) in self.towns.iter().enumerate() {
            if let Some(h) = self.world.get::<crate::state::Town>(e).and_then(|t| t.hex) {
                d.hexes.insert(t, h);
            }
        }
        for &e in &self.routes {
            if let Some(r) = self.world.get::<crate::state::Route>(e) {
                d.roads.entry((r.from_id, r.to_id)).or_insert(r.blocks);
            }
        }
        d.between(a, b)
    }

    /// A route's length in blocks.
    pub fn route_road_blocks(&self, route: crate::messages::RouteId) -> u32 {
        self.world.get::<crate::state::Route>(self.routes[route]).map_or(0, |r| r.blocks)
    }

    /// Is the route a hub's run on call (it leaves only on a booked day)?
    pub fn on_call(&self, route: crate::messages::RouteId) -> bool {
        self.world.get::<crate::state::Route>(self.routes[route]).is_some_and(|r| r.on_call)
    }

    /// A booking read at the hub now (`Forwarder.bookedForTest`).
    pub fn book_with_forwarder(&mut self, hub: TownId, town: TownId, stacks: u32, worth: impl Into<crate::fixed::Fixed>) {
        let now = self.now();
        self.world.resource_mut::<Forwarder>().booked(hub, town, stacks, worth.into(), now);
    }

    /// A lot of the town's goods on consignment at the hub, its collection's fare in emeralds (`Forwarder.lotForTest`).
    pub fn consign_lot(&mut self, hub: TownId, town: TownId, good: GoodId, count: f64, fare: impl Into<crate::fixed::Fixed>) {
        let now = self.now();
        self.world.resource_mut::<Forwarder>().lot(hub, town, good, count, fare.into(), now);
    }

    /// The hub sold `count` of a good for `emeralds`: credited to the towns' lots of it (`Forwarder.sold`).
    pub fn consign_sold(&mut self, hub: TownId, good: GoodId, count: f64, emeralds: impl Into<crate::fixed::Fixed>) {
        self.world.resource_mut::<Forwarder>().sold(hub, good, count, emeralds.into());
    }

    /// Is a collection due on the hub's run to the town (`Forwarder.collecting`)?
    pub fn collecting(&self, hub: TownId, town: TownId) -> bool {
        self.forwarder().is_collecting(hub, town)
    }

    /// The hub's forwarder's review now, as its day's review does (`Forwarder.review`): its collections (a run on call
    /// opened where it has none) and the lots' proceeds remitted by post.
    pub fn forwarder_review(&mut self, hub: TownId) {
        self.world.run_system_once_with(crate::rules::post::review_one, hub).expect("the forwarder");
        self.open_call_runs();
    }

    /// A host's wagon's goods it couldn't sell at the hub, offered to its forwarder (`Forwarder.consignUnsold`): what it
    /// takes comes off `cargo` into the hub's warehouse, on consignment for `town`. Returns how many it took.
    pub fn consign_unsold(&mut self, hub: TownId, town: TownId, cargo: &mut [f64]) -> f64 {
        let taken = self.world.run_system_once_with(crate::rules::post::consign_host, (hub, town, cargo.to_vec()))
            .expect("the forwarder");
        let mut n = 0.0;
        for (g, count) in taken {
            cargo[g] -= count;
            n += count;
        }
        n
    }

    /// The runs on call the post's reviews opened this look (`ledger.addRoute(hub, town).setOnCall(true)`), opened as
    /// [`Society::open_route`] opens one; a run that can't be opened (no way, nothing known of the distance) drops its
    /// calls and its collection.
    pub(crate) fn open_call_runs(&mut self) {
        let pending = std::mem::take(&mut self.world.resource_mut::<Mail>().pending);
        for (hub, town) in pending {
            let there = self.routes.iter().any(|&e| {
                self.world.get::<crate::state::Route>(e).is_some_and(|r| r.from_id == hub && r.to_id == town)
                    && self.world.get::<crate::state::RouteState>(e).is_some_and(|s| s.freight.wound_down.is_none())
            });
            if there {
                continue;
            }
            match self.open_route(hub, town) {
                Some(id) => self.world.get_mut::<crate::state::Route>(self.routes[id]).unwrap().on_call = true,
                None => {
                    self.world.resource_mut::<Mail>().calls.retain(|c| !(c.hub == hub && c.town == town));
                    self.world.resource_mut::<Forwarder>().drop_collection(hub, town);
                }
            }
        }
    }
}
