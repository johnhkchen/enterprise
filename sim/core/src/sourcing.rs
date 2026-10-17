//! Sourcing and procurement (`docs/decisions.md` "Supply chains, back to back", "Inputs follow contracts", "Pacing",
//! "The metal exchange buys by contract", "Trust never freezes trade"): a town short of a good contracts a supplier;
//! its Merchant buys from its own lockers what its contracts want soon; what a contract to make needs raises the par
//! so wagons bring it; and the capital's metal exchange buys by contract, nearest holder first.
//!
//! - **Sourcing** ([`Env::source`], `Broker.source` 287–349): the markets the town knows (the atlas, nearest first) that
//!   held the good at their last stocktake and have a road between them (a route either way), then (failing those) those
//!   whose people make or sell it. Ex Works if the town's own wagons go there (they collect), else Delivered At Place
//!   (the supplier's wagons bring it). The price is the worth plus `supplier_margin`, then carriage and tolls by the
//!   terms ([`Env::landed`], `Broker.landed` 450). One open contract per good per supplier. A supplier can spare what it
//!   holds beyond its par, half its kit, and the food its wagons need ([`Env::means`], 379–432). A lot to make is
//!   `maker_lot` (a stack of food), in up to `maker_installments` a day apart after `maker_lead_days`, a third paid up
//!   front; below `MIN_TRUST` the two still deal, on trial terms: `trial_lot`, one installment, nothing up front. What a
//!   town has under contract and not yet received counts ([`Env::coming`], 460). With no road the want stays a signal:
//!   the trade that would supply it is a marble in the job draw of the towns that know the market ([`Signals`]).
//!   Drafted at the buyer's: in force once a wagon between the two has carried the copy each way ([`Sourcing::copies`]).
//! - **Procurement** ([`wanted`], [`soon`], [`due`], [`procure`], `Procurement.java` 74, 101, 123–129): what a town's
//!   contracts still have it deliver, beyond its stock; and what is due within `pace_days`. At a restock its Merchant
//!   switches on its managed lockers' selling offers for the goods wanted (and off again those it switched on that no
//!   longer are), and a procured offer runs no more than what's due soon (`LockerTrading.trade` 182–235:
//!   [`crate::lockers::Lockers::due`]).
//! - **Inputs** (`ContractBook.review` 301, `Crafts.contractInputs` 247, `Warehouse.par` 138, `Caravans.wantsAt` 2267,
//!   `collectPressing` 1273): each look a seller's contracts in force set `Trades.inputs`, what its recipes need for what
//!   it still owes; its shelf is at least that, so a wagon carries it (inputs first, `rules/plan.rs`), and a trip with
//!   inputs the far end is short of, once an installment, can't wait.
//! - **The metal exchange** ([`MetalExchange`], `MetalExchange.java`): the capital's Merchant keeps a stock of iron, coal,
//!   flint, diamonds and gold. Each day it counts what it holds and, for each metal short of its target (twice as much
//!   while the sovereign mobilises, three times at war), contracts for the shortfall at a premium that rises as it runs
//!   short, nearest holder first: a town with a mine, then the market across the sea.
//!
//! **Where the core differs.** No way by a freight hub (`Broker.means(.., viaHub)`, `Freight.via`), no armoury (a town
//! keeps half its kit always), no crown shortfall (`Sovereign.shortfall`) among a town's wants: that slice calls
//! [`Society::source`] and adds to [`wanted`] when it lands. A player's order is a seller contract of the desk's own
//! (`orders.rs`), which [`wanted`] reads like any other, so the core sources it as the mod's `Broker.fill` does; the
//! mod asks for `remaining - spare` (what it can't spare beyond its par and what is set aside), where [`wanted`] nets
//! the whole stock, so a desk's open orders (`Orders::wanted`) are added to the town's wants, the larger of the two.
//! The mod only sources for orders, the crown and the exchange; the core's towns with a master Merchant also source
//! what their contracts want and can't be made at home, once a day (and skip, as for any contract, a good its own
//! people make or sell: its Merchant buys that of them, even for an order). What none of the markets with a road can
//! supply is [`Sourcing::unmet`]; it is not posted from here but folded into the desk's one list of signals
//! (`Broker.signals`: its open orders, its lockers' wants and these, where the mod has the war's), and that list is the
//! only thing that reaches the jobs' marbles (`rules/orders.rs`), so a good is never counted by two streams. The shops of the
//! exchange (Numismatics vendors) and the players' sales to it are the binding's. Overseas, the exchange's market makes
//! its metals and bread up to what it keeps, once a day.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::atlas::{Atlas, Places, Stocktakes, Why};
use crate::content::{Content, GoodId};
use crate::contracts::{self, Book, Incoterm, State};
use crate::hex::Hex;
use crate::jobs::good_named;
use crate::lockers::Lockers;
use crate::messages::TownId;
use crate::realms::OffMap;
use crate::roster::{Roster, TradeId};
use crate::society::Society;
use crate::state::*;
use crate::territory::Territory;
use crate::works::Works;

/// The Merchant level that brokers (`Broker.MASTER`).
pub const MASTER: u8 = 5;
/// What a want nobody supplies is posted at, over the good's worth (`Broker.SIGNAL_PREMIUM`).
pub const SIGNAL_PREMIUM: f64 = 1.5;
/// How far the exchange's price moves with its stock: half again when empty, down to three quarters when over
/// (`MetalExchange.PRESSURE`).
pub const PRESSURE: f64 = 0.5;
/// The market across the sea lies this far west of the capital (`MetalExchange.OVERSEAS_DISTANCE`).
pub const OVERSEAS_DISTANCE: f64 = 2400.0;
/// The bread the market across the sea keeps for travellers (`MetalExchange.OVERSEAS_BREAD`).
pub const OVERSEAS_BREAD: f64 = 192.0;
/// The metals the exchange keeps, with the stock it aims to hold in peace and what the market across the sea keeps of it
/// (0: none): iron, coal, flint, diamonds and (the war chest) gold (`MetalExchange.METALS`).
pub const METALS: [(&str, f64, f64); 5] = [
    ("iron_ingot", 192.0, 512.0),
    ("coal", 192.0, 512.0),
    ("flint", 96.0, 256.0),
    ("diamond", 12.0, 32.0),
    ("gold_ingot", 32.0, 0.0),
];

// ---------------------------------------------------------------------------------------------------------------------
// State

/// Where the sovereign stands (`Sovereign.Phase`): the exchange aims to hold more as it mobilises, and at war. The crown
/// slice sets it ([`Society::set_phase`] until then).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    #[default]
    Peace,
    Mobilising,
    War,
}

impl Phase {
    /// How many times its peacetime stock the exchange aims to hold (`MetalExchange.target`).
    pub fn factor(self) -> f64 {
        match self {
            Phase::Peace => 1.0,
            Phase::Mobilising => 2.0,
            Phase::War => 3.0,
        }
    }
}

/// A metal the exchange keeps, by the host's good.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metal {
    pub good: GoodId,
    /// The stock it aims to hold in peace, and what the market across the sea keeps of it.
    pub target: f64,
    pub overseas: f64,
}

/// The metals the exchange keeps that the host has goods for, in the mod's order (`MetalExchange.METALS`).
pub fn metals(content: &Content) -> Vec<Metal> {
    METALS.iter().filter_map(|&(name, target, overseas)| good_named(content, name).map(|good| Metal { good, target, overseas }))
        .collect()
}

/// The capital's metal exchange (`MetalExchange`): its desk (the capital the crown names, until then the town
/// [`Society::open_exchange`] named), the market across the sea, the phase it aims by, the day it last ran, and what it
/// held of each metal at its last count.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MetalExchange {
    pub desk: Option<TownId>,
    pub overseas: Option<TownId>,
    pub phase: Phase,
    pub last_day: Option<u64>,
    pub held: BTreeMap<GoodId, f64>,
}

impl MetalExchange {
    /// The stock it aims to hold now: more while the sovereign mobilises, more again at war (`MetalExchange.target`).
    pub fn target(&self, metal: &Metal) -> f64 {
        metal.target * self.phase.factor()
    }

    /// What it held of a metal at its last count.
    pub fn held(&self, good: GoodId) -> f64 {
        self.held.get(&good).copied().unwrap_or(0.0)
    }

    /// How much over its worth it pays for a metal: more the shorter it is, half again with none, three quarters over
    /// its target (`MetalExchange.premium`).
    pub fn premium(&self, metal: &Metal) -> f64 {
        let target = self.target(metal);
        let gap = if target <= 0.0 { 0.0 } else { (target - self.held(metal.good)) / target };
        (1.0 + PRESSURE * gap).clamp(0.75, 1.0 + PRESSURE)
    }
}

/// Sourcing's books: the contracts it drafted whose copies still ride with the wagons between the two towns, what the
/// desks' signals have added to the jobs' (by town that reads it and trade: `rules/orders.rs` posts and takes it off
/// again) and what each town wanted that no market with a road supplies, the day each town last sourced.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sourcing {
    pub copies: BTreeSet<usize>,
    pub signalled: BTreeMap<(TownId, TradeId), f64>,
    pub unmet: BTreeMap<TownId, Vec<(GoodId, f64)>>,
    pub reviewed: BTreeMap<TownId, u64>,
    /// The look at which the review last ran (the desks' signals are made again then).
    #[serde(default)]
    pub ran: Option<u64>,
}

/// Puts the trades the desks' signals call for onto the jobs' signals (by town that reads it and trade), taking off
/// what `old` had put there.
pub fn post_trades(signals: &mut crate::jobs::Signals, old: &BTreeMap<(TownId, TradeId), f64>,
        new: &BTreeMap<(TownId, TradeId), f64>) {
    for (&(reader, trade), &n) in old {
        *signals.trades.entry(reader).or_default().entry(trade).or_insert(0.0) -= n;
    }
    for (&(reader, trade), &n) in new {
        *signals.trades.entry(reader).or_default().entry(trade).or_insert(0.0) += n;
    }
    for &(reader, trade) in old.keys().chain(new.keys()) {
        let gone = signals.trades.get(&reader).and_then(|m| m.get(&trade)).is_some_and(|n| *n <= 1e-9);
        if gone {
            if let Some(m) = signals.trades.get_mut(&reader) {
                m.remove(&trade);
            }
        }
        if signals.trades.get(&reader).is_some_and(|m| m.is_empty()) {
            signals.trades.remove(&reader);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Procurement: what a town's contracts want of it

/// What a town still has to deliver under its contracts in force, of each good, beyond what it holds
/// (`Procurement.wanted`; a player's order is one of them, the crown's shortfall is another slice's): its Merchant buys
/// it from its villagers, or sources it.
pub fn wanted(content: &Content, book: &Book, town: TownId, stock: &[f64]) -> BTreeMap<GoodId, f64> {
    let mut wanted = BTreeMap::new();
    for c in book.open.iter().filter_map(|&id| book.contracts.get(id)) {
        if c.in_force() && c.seller == town && c.training.is_none() && !wanted.contains_key(&c.good) {
            wanted.insert(c.good, crate::services::to_deliver(book, town, c.good));
        }
    }
    net(content, &mut wanted, stock);
    wanted
}

/// What a town's contracts have it deliver within `pace_days` from `now`, of each good, whether it holds it or not:
/// what its Merchant buys for now, not the whole of a long contract (`Procurement.soon` less its stock).
pub fn due(content: &Content, book: &Book, town: TownId, now: u64) -> Vec<f64> {
    let by = now + (content.dials.pace_days * content.day_ticks as f64) as u64;
    let mut due = vec![0.0; content.goods.len()];
    for c in book.open.iter().filter_map(|&id| book.contracts.get(id)) {
        if c.in_force() && c.seller == town && c.training.is_none() && c.good < due.len() {
            due[c.good] += c.owed(by);
        }
    }
    due
}

/// What a town wants soon, beyond what it holds: [`due`] less its stock (`Procurement.soon`).
pub fn soon(content: &Content, book: &Book, town: TownId, now: u64, stock: &[f64]) -> BTreeMap<GoodId, f64> {
    let mut soon: BTreeMap<GoodId, f64> = due(content, book, town, now).into_iter().enumerate().filter(|(_, n)| *n > 0.0)
        .collect();
    net(content, &mut soon, stock);
    soon
}

fn net(content: &Content, wants: &mut BTreeMap<GoodId, f64>, stock: &[f64]) {
    let _ = content;
    for (&g, n) in wants.iter_mut() {
        *n = (*n - stock.get(g).copied().unwrap_or(0.0).floor()).max(0.0);
    }
    wants.retain(|_, n| *n > 0.0);
}

/// At a villager's restock the Merchant switches on its managed lockers' selling offers for what's wanted, and off again
/// those it switched on that aren't, and tells the lockers what's due soon, to cap their runs (`Procurement.procure`,
/// `procureAt`). Returns the offers switched on.
pub fn procure(content: &Content, book: &Book, town: TownId, now: u64, stock: &Stock, roster: &Roster,
        lockers: &mut Lockers) -> usize {
    lockers.due = due(content, book, town, now);
    if roster.merchant.is_none() {
        return 0;
    }
    let wanted = wanted(content, book, town, &stock.0);
    let mut switched = 0;
    for i in lockers.managed.clone() {
        let Some(hand) = roster.hands.get(i) else { continue };
        let Some(locker) = lockers.hands.get_mut(i) else { continue };
        for o in locker.offers.iter_mut().filter(|o| o.sells && o.level <= hand.level) {
            let want = wanted.contains_key(&o.good);
            if want && !o.on {
                o.on = true;
                o.procured = true;
                switched += 1;
            } else if !want && o.on && o.procured {
                o.on = false;
                o.procured = false;
            }
        }
    }
    switched
}

// ---------------------------------------------------------------------------------------------------------------------
// Sourcing: what the world looks like to a Merchant

/// A route as sourcing reads it: the route, the hexes it crosses, when its wagon last left and where the wagon is.
#[derive(Clone, Debug)]
pub struct RouteFact {
    pub route: Route,
    pub path: Vec<Hex>,
    pub last_departure: Option<u64>,
    pub wagon: Option<(Leg, u64)>,
}

/// Every route's facts, in `seq` order.
pub fn route_facts(world: &mut World) -> Vec<RouteFact> {
    let mut q = world.query::<(&Route, &RouteState, Option<&RoutePath>)>();
    let mut out: Vec<RouteFact> = q.iter(world).map(|(r, s, p)| RouteFact {
        route: *r,
        path: p.map_or_else(Vec::new, |p| p.0.clone()),
        last_departure: s.last_departure,
        wagon: s.wagon.as_ref().map(|w| (w.leg, w.arrives)),
    }).collect();
    out.sort_by_key(|r| r.route.seq);
    out
}

/// What a supplier can do for a desk's order (`Broker.Means`): what it can spare of the good, whether its people make or
/// sell it, the road it would go by and the terms (Ex Works if the desk's wagons go there, else Delivered At Place), and
/// whether the two trust each other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Means {
    pub spare: f64,
    pub makes: bool,
    /// The route's place in [`Env::routes`].
    pub route: usize,
    pub incoterm: Incoterm,
    pub trusted: bool,
}

/// One contract sourcing would draft: `quantity` of `good` an installment, `installments` times, from `start`, every
/// `every`, at `price` each, on `incoterm`, with the advance if `advance`.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub seller: TownId,
    pub buyer: TownId,
    pub good: GoodId,
    pub quantity: f64,
    pub price: f64,
    pub start: u64,
    pub every: u64,
    pub installments: u32,
    pub incoterm: Incoterm,
    pub advance: bool,
    /// Why, in words, for the books.
    pub kind: &'static str,
}

/// What sourcing did for a desk's want: the contracts it would draft, what's covered by them and what was already coming,
/// and whether any market could have supplied it at all (none: a signal).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sourced {
    pub drafts: Vec<Draft>,
    pub covered: f64,
    pub supplied: bool,
}

/// The world as a Merchant reads it to source: the atlas, the stocktakes, the roads, the rosters.
pub struct Env<'a> {
    pub world: &'a World,
    pub content: &'a Content,
    pub now: u64,
    pub book: &'a Book,
    pub atlas: &'a Atlas,
    pub stocktakes: &'a Stocktakes,
    pub territory: &'a Territory,
    pub places: Places,
    pub routes: Vec<RouteFact>,
    pub towns: &'a [Entity],
}

impl<'a> Env<'a> {
    pub fn gather(world: &'a World, routes: Vec<RouteFact>) -> Env<'a> {
        let content = world.resource::<Content>();
        let towns: &[Entity] = &world.resource::<Towns>().0;
        let n = towns.len();
        let mut places = Places { hexes: vec![None; n], reach: vec![content.dials.word_of_mouth; n], ..Default::default() };
        for (t, &e) in towns.iter().enumerate() {
            if let Some(town) = world.get::<Town>(e) {
                places.hexes[t] = town.hex;
            }
            places.reach[t] = crate::atlas::reach(content, world.get::<Works>(e));
        }
        for r in &routes {
            places.roads.entry((r.route.from_id, r.route.to_id)).and_modify(|b| *b = (*b).min(r.route.blocks))
                .or_insert(r.route.blocks);
        }
        Env { world, content, now: world.resource::<Clock>().now, book: world.resource::<Book>(),
            atlas: world.resource::<Atlas>(), stocktakes: world.resource::<Stocktakes>(),
            territory: world.resource::<Territory>(), places, routes, towns }
    }

    fn day(&self) -> u64 {
        self.content.day_ticks.max(1)
    }

    fn town<T: Component>(&self, town: TownId) -> Option<&'a T> {
        self.world.get::<T>(*self.towns.get(town)?)
    }

    /// The route from one town to another, the shortest of them.
    pub fn route_between(&self, from: TownId, to: TownId) -> Option<usize> {
        (0..self.routes.len()).filter(|&i| self.routes[i].route.from_id == from && self.routes[i].route.to_id == to)
            .min_by_key(|&i| (self.routes[i].route.blocks, self.routes[i].route.seq))
    }

    /// How many of a good a market held at its last stocktake, whole (`Warehouse.onHand`); the desk's own, as it holds it.
    fn held_at(&self, market: TownId, desk: TownId, good: GoodId) -> f64 {
        if market == desk {
            return self.town::<Stock>(desk).map_or(0.0, |s| s.0.get(good).copied().unwrap_or(0.0).floor());
        }
        self.stocktakes.on_hand(market, good).floor()
    }

    /// What the market's warehouse aims to hold of a good, beyond its sales (`Warehouse.par(.., wanted false)`): its
    /// lockers' capacity for `shelf_days`, at least what its contracts to make need.
    pub fn par(&self, market: TownId, good: GoodId) -> f64 {
        self.town::<Trades>(market).map_or(0.0, |t| {
            crate::bargain::par(self.content, t.capacity.get(good).copied().unwrap_or(0.0), false)
                .max(t.inputs.get(good).copied().unwrap_or(0.0))
        })
    }

    /// Do the market's people make or sell the good (`Procurement.sellerLevel`, `Crafts.recipesFor`): a villager of a
    /// trade that offers it for emeralds at his level, or of a trade that makes it from inputs the market or the desk
    /// holds.
    pub fn makes(&self, market: TownId, desk: TownId, good: GoodId) -> bool {
        let Some(roster) = self.town::<Roster>(market) else { return false };
        let lockers = self.town::<Lockers>(market).filter(|l| l.hands.len() == roster.hands.len());
        let sells = roster.hands.iter().enumerate().filter(|(_, h)| !h.is_baby()).any(|(i, h)| {
            let offers = match lockers {
                Some(l) => &l.hands[i].offers,
                None => match self.content.trades.get(h.trade) {
                    Some(t) => &t.offers,
                    None => return false,
                },
            };
            offers.iter().any(|o| o.sells && o.good == good && o.level <= h.level)
        });
        sells || crate::services::recipes(self.content).iter().any(|r| r.output == good
            && roster.hands.iter().any(|h| !h.is_baby() && h.trade == r.trade)
            && r.inputs.iter().all(|&(g, _)| self.held_at(market, desk, g) > 0.0 || self.held_at(desk, desk, g) > 0.0))
    }

    /// What a supplier can do for the desk, if there's a way (`Broker.means`): a route between them, either way.
    pub fn means(&self, desk: TownId, supplier: TownId, good: GoodId) -> Option<Means> {
        let collect = self.route_between(desk, supplier);
        let bring = self.route_between(supplier, desk);
        let route = collect.or(bring)?;
        let trusted = self.book.trust(supplier, desk, self.now, self.day()) >= contracts::MIN_TRUST;
        let held = self.held_at(supplier, desk, good);
        // A town keeps half its kit for its own hands (an armoury's goods are there to be sent on: none here yet).
        let kit = crate::services::KITS.iter().any(|k| good_named(self.content, k.good) == Some(good));
        let mut spare = (if kit { (held / 2.0).floor() } else { held }) - self.par(supplier, good);
        let points = crate::road::points(self.content, good);
        if points > 0.0 {
            // And the food its wagons need, as far as its other food doesn't cover it.
            let other: f64 = (0..self.content.goods.len()).filter(|&g| g != good)
                .map(|g| self.stocktakes.on_hand(supplier, g).floor() * crate::road::points(self.content, g)).sum();
            spare -= ((2.0 * crate::settlers::FOOD_PAR_POINTS - other).max(0.0) / points).floor();
        }
        Some(Means { spare: spare.max(0.0), makes: self.makes(supplier, desk, good), route,
            incoterm: if collect.is_some() { Incoterm::Exw } else { Incoterm::Dap }, trusted })
    }

    /// The share of the wagon's money the posts on a road take (`Caravans.tollShare`): each owner of land it crosses with
    /// a post on the road, not the route's ends.
    pub fn toll_share(&self, route: &RouteFact) -> f64 {
        let ends = [route.route.from_id, route.route.to_id];
        let mut posted: Vec<TownId> = Vec::new();
        for &hex in &route.path {
            if let Some(owner) = self.territory.owner(hex).filter(|o| !ends.contains(o)) {
                if self.territory.holding(owner).is_some_and(|h| h.posts.contains(&hex)) && !posted.contains(&owner) {
                    posted.push(owner);
                }
            }
        }
        let rate = |o: TownId| self.territory.holding(o).and_then(|h| h.toll_rate).unwrap_or(self.content.dials.default_toll_rate);
        1.0 - posted.iter().map(|&o| 1.0 - rate(o)).product::<f64>()
    }

    /// The price each, in emeralds, a supplier asks on these terms: the good's worth and its margin (times `premium`),
    /// and under Delivered At Place the carriage and the tolls on the way (`Broker.landed` 450).
    pub fn landed(&self, good: GoodId, route: usize, incoterm: Incoterm, premium: f64) -> f64 {
        let d = &self.content.dials;
        let ex_works = self.content.goods[good].value * (1.0 + d.supplier_margin) * premium;
        let r = &self.routes[route];
        let per_wagon = d.wagon_slots as f64 * self.content.goods[good].stack as f64;
        let carriage = if crate::road::road_food(self.content) {
            crate::road::food_needed(self.content, r.route.blocks) * 2.0 * d.purse_per_food_point / per_wagon
        } else {
            r.route.road_cost / per_wagon
        };
        incoterm.price(ex_works, carriage, self.toll_share(r) * ex_works)
    }

    /// What the desk has under contract from the markets it knows and not yet received (`Broker.coming`).
    pub fn coming(&self, desk: TownId, good: GoodId) -> f64 {
        let known = self.places.known(self.content, self.atlas, desk);
        self.book.open.iter().filter_map(|&id| self.book.contracts.get(id))
            .filter(|c| matches!(c.state, State::Drafted | State::Active) && c.training.is_none() && c.buyer == desk
                && c.good == good && known.contains(&c.seller))
            .map(|c| c.quantity * c.installments as f64 - c.delivered).sum()
    }

    /// A lot to make: food by the stack (a Butcher's restock is dozens), else `maker_lot` (`Broker.makerLot`).
    pub fn maker_lot(&self, good: GoodId) -> f64 {
        let lot = self.content.dials.maker_lot as f64;
        if crate::road::points(self.content, good) > 0.0 { lot.max(self.content.goods[good].stack as f64) } else { lot }
    }

    /// When the route's wagon can next leave (`CaravanLedger.nextDeparture`): when it's home, and a day after the last.
    fn next_departure(&self, route: usize) -> u64 {
        let r = &self.routes[route];
        let home = match r.wagon {
            Some((Leg::Out, arrives)) => arrives + r.route.travel_ticks,
            Some((Leg::Home, arrives)) => arrives,
            None => self.now,
        };
        self.now.max(home).max(r.last_departure.map_or(0, |t| t + self.day()))
    }

    /// Contracts suppliers for goods the desk is short of, until what's contracted covers the need (`Broker.source`):
    /// markets it knows with a road between them, nearest first, that held the goods at their last stocktake, or (failing
    /// those) whose people make or sell them. Ex Works if its wagons go there, else Delivered At Place. One open contract
    /// per good per supplier. `need` counts whole items; `premium` times the price (the exchange's).
    pub fn source(&self, desk: TownId, good: GoodId, need: f64, premium: f64) -> Sourced {
        let need = need.floor();
        let mut out = Sourced { covered: self.coming(desk, good), ..Default::default() };
        if out.covered >= need {
            out.supplied = true;
            return out;
        }
        let d = &self.content.dials;
        let day = self.day();
        let markets = self.places.known(self.content, self.atlas, desk);
        for holders in [true, false] {
            for &supplier in &markets {
                if out.covered >= need {
                    continue;
                }
                if self.book.covers(supplier, desk, good) || out.drafts.iter().any(|dr| dr.seller == supplier) {
                    out.supplied = true; // (one order open at a time with a supplier: it is one)
                    continue;
                }
                let Some(m) = self.means(desk, supplier, good) else { continue };
                if if holders { m.spare <= 0.0 } else { !m.makes } {
                    continue;
                }
                out.supplied = true;
                let mut quantity = (need - out.covered).min(if holders { m.spare } else { self.maker_lot(good) });
                if !m.trusted {
                    quantity = quantity.min(d.trial_lot as f64); // a trial lot, to rebuild the record
                }
                // Work to make comes in small installments a day apart, the first after a lead: steady deliveries build
                // trust, and the seller isn't asked for everything at once.
                let installments = if holders || !m.trusted { 1 } else { (d.maker_installments as f64).min(quantity).max(1.0) as u32 };
                let each = (quantity / installments as f64).ceil();
                let price = self.landed(good, m.route, m.incoterm, premium);
                // Delivered at the buyer's, the first installment is due once the seller's next wagon can be there.
                let mut start = if holders { self.now } else { self.now + (d.maker_lead_days * day as f64) as u64 };
                if m.incoterm == Incoterm::Dap {
                    start = start.max(self.next_departure(m.route)) + self.routes[m.route].route.travel_ticks;
                }
                out.covered += each * installments as f64;
                out.drafts.push(Draft { seller: supplier, buyer: desk, good, quantity: each, price, start, every: day,
                    installments, incoterm: m.incoterm, advance: !holders && m.trusted,
                    kind: if holders { "from its stock" } else if m.trusted { "to make, a third up front" } else { "on trial" } });
            }
        }
        out
    }
}

/// Puts the contracts sourcing drafted into the book, at the desk (its wagon, or the supplier's, carries the copy), and
/// the town's reasons (`Exchange.note`) in its tally.
pub fn apply(world: &mut World, desk: TownId, drafts: &[Draft]) {
    let now = world.resource::<Clock>().now;
    let (day, share) = {
        let c = world.resource::<Content>();
        (c.day_ticks.max(1), c.dials.advance_share)
    };
    for dr in drafts {
        // One open contract per good per supplier: the exchange's and the desk's own wants were read from one book.
        if world.resource::<Book>().covers(dr.seller, dr.buyer, dr.good) {
            continue;
        }
        let id = world.resource_mut::<Book>().draft_with_advance(dr.seller, dr.buyer, dr.good, dr.quantity, dr.price,
            dr.start, dr.every, dr.installments, dr.incoterm, false, false, if dr.advance { share } else { 0.0 });
        world.resource_mut::<Book>().contracts[id].drafted = now;
        world.resource_mut::<Sourcing>().copies.insert(id);
        let term = if dr.incoterm == Incoterm::Exw { "Ex Works" } else { "Delivered At Place" };
        world.resource_mut::<crate::atlas::Whys>().why(desk, Some(dr.seller), Some(dr.good),
            Why::Note(format!("sourced {}, {term}", dr.kind)), dr.quantity * dr.installments as f64, now / day);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Host calls

impl Society {
    /// A town sources `need` of a good from the markets it knows (`Broker.source`): contracts drafted at its desk, in force
    /// once a wagon between it and the supplier has carried the copy. True if what's contracted now covers the need.
    pub fn source(&mut self, town: TownId, good: GoodId, need: f64) -> bool {
        self.source_at(town, good, need, 1.0)
    }

    /// As [`Society::source`], paying `premium` times the price (the exchange pays more the shorter it is).
    pub fn source_at(&mut self, town: TownId, good: GoodId, need: f64, premium: f64) -> bool {
        let routes = route_facts(&mut self.world);
        let sourced = Env::gather(&self.world, routes).source(town, good, need, premium);
        apply(&mut self.world, town, &sourced.drafts);
        sourced.covered >= need.floor()
    }

    /// What a town's contracts still have it deliver, beyond what it holds (`Procurement.wanted`).
    pub fn wanted(&self, town: TownId) -> Vec<(GoodId, f64)> {
        let stock = self.world.get::<Stock>(self.towns[town]).expect("a town");
        wanted(self.content(), self.world.resource::<Book>(), town, &stock.0).into_iter().collect()
    }

    /// What it wants soon, within `pace_days`, beyond what it holds (`Procurement.soon`).
    pub fn soon(&self, town: TownId) -> Vec<(GoodId, f64)> {
        let stock = self.world.get::<Stock>(self.towns[town]).expect("a town");
        soon(self.content(), self.world.resource::<Book>(), town, self.now(), &stock.0).into_iter().collect()
    }

    /// Its Merchant switches on its lockers' selling offers for what its contracts want, as at a villager's restock
    /// (`Procurement.procureAt`). A town with no roster does nothing.
    pub(crate) fn procure(&mut self, town: TownId) -> usize {
        let e = self.towns[town];
        let Some(mut lockers) = self.lockers(town) else { return 0 };
        let roster = self.world.get::<Roster>(e).expect("a roster").clone();
        let stock = self.world.get::<Stock>(e).expect("a town").clone();
        let switched = procure(self.content(), self.world.resource::<Book>(), town, self.now(), &stock, &roster, &mut lockers);
        self.world.entity_mut(e).insert(lockers);
        switched
    }

    /// Opens the metal exchange at a town's desk (`MetalExchange.open`, less the shops and the road to the market across
    /// the sea: see [`Society::open_exchange_overseas`]). The crown names the capital in its time.
    pub fn open_exchange(&mut self, town: TownId) {
        let mut ex = self.world.resource_mut::<MetalExchange>();
        ex.desk = Some(town);
        ex.last_day = None;
    }

    /// Opens the exchange at the town's desk, and the market across the sea (`MetalExchange.open`): an off-map market
    /// [`OVERSEAS_DISTANCE`] blocks west of the capital, holding `money`, selling food, with a bare road to the capital and
    /// each knowing the other. None if the town isn't placed.
    pub fn open_exchange_overseas(&mut self, town: TownId, money: f64) -> Option<TownId> {
        let hex = self.town_hex(town)?;
        self.open_exchange(town);
        let far = self.add_off_map("the market across the sea", hex.center_x() - OVERSEAS_DISTANCE, hex.center_z(), money);
        self.set_off_map_sells(far, true);
        self.world.resource_mut::<MetalExchange>().overseas = Some(far);
        let blocks = self.places().distance(town, far)?;
        let days = blocks.max(1) as f64 / crate::map::BLOCKS_PER_DAY;
        let every = self.content().dials.wagon_wait_days;
        let cost = self.per_hex() * (blocks as f64 / crate::hex::ACROSS).ceil().max(1.0);
        self.add_route(town, far, days, cost, every);
        let now = self.now();
        self.world.resource_mut::<Atlas>().learn_both(town, far, now);
        Some(far)
    }

    /// The exchange as it stands.
    pub fn exchange(&self) -> MetalExchange {
        self.world.resource::<MetalExchange>().clone()
    }

    /// How much over its worth the exchange pays for a good now (`MetalExchange.premium`): 1 for a good it doesn't keep.
    pub fn exchange_premium(&self, good: GoodId) -> f64 {
        let ex = self.world.resource::<MetalExchange>();
        metals(self.content()).iter().find(|m| m.good == good).map_or(1.0, |m| ex.premium(m))
    }

    /// The stock the exchange aims to hold of a good now, with the phase (0 for a good it doesn't keep).
    pub fn exchange_target(&self, good: GoodId) -> f64 {
        let ex = self.world.resource::<MetalExchange>();
        metals(self.content()).iter().find(|m| m.good == good).map_or(0.0, |m| ex.target(m))
    }

    /// Says what the exchange held of a metal at its last count (`setHeldForTest`).
    pub fn set_exchange_held(&mut self, good: GoodId, held: f64) {
        self.world.resource_mut::<MetalExchange>().held.insert(good, held);
    }

    /// The sovereign's phase, which sets how much the exchange aims to hold (the crown slice's, until it lands).
    pub fn set_phase(&mut self, phase: Phase) {
        self.world.resource_mut::<MetalExchange>().phase = phase;
    }

    /// Does the town keep a Merchant who brokers (a master)?
    pub fn brokers(&self, town: TownId) -> bool {
        self.world.get::<Roster>(self.towns[town]).and_then(|r| r.merchant_level()).is_some_and(|l| l >= MASTER)
            && !self.world.entity(self.towns[town]).contains::<OffMap>()
    }
}

impl Society {
    /// The trades the markets a town knows have posted as wanted, with emeralds of demand: marbles in its idle hands'
    /// draw (`Signals`).
    pub fn signals(&self, town: TownId) -> Vec<(TradeId, f64)> {
        self.world.resource::<crate::jobs::Signals>().of(town)
    }

    /// What a town needs of each good to make what it has contracted to sell (`Warehouse.inputs`), empty if nothing.
    pub fn inputs(&self, town: TownId) -> Vec<f64> {
        self.world.get::<Trades>(self.towns[town]).expect("a town").inputs.clone()
    }
}
