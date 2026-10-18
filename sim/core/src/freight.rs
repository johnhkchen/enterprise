//! Freight: carriers selling space on their runs, forwarders buying it, and the hubs where goods change wagons
//! (`docs/decisions.md` "Freight: carriers, forwarders, drayage", "Mountains, and hubs that emerge", "Routes reorganise
//! by price; backhauls; llamas", "Tolls a caravan, and the network reorganising", "The ridge study made honest").
//!
//! A market with a storehouse is a **freight hub** ([`Hub`]). Every route touching it is a carrier's run through it, a
//! [`Leg`] from the hub: its own runs out, the runs of markets it's the far end of going home, and both ways of routes
//! that call there on their way ([`Network::legs_from`]). Each carrier sells space by the stack at its road's food over
//! what its last wagon could carry, and a margin ([`stack_price`]), a quarter of that on the way home (a backhaul:
//! [`leg_price`]), at most half a wagon a run, restocked each day.
//!
//! - **The forwarder** is the hub's Merchant. A wagon at the hub hands it crates bound for markets the hub serves, for
//!   a fee: the freight at the cheapest leg's price and the forwarder's margin, and the carting in and out
//!   ([`Hub::hand_over`]). The **draymen** cart them: the hub's idle hands and one more, 16 stacks a day each, sold by
//!   the cart of four stacks at half an emerald a drayman-day and the margin; with none free a crate stays aboard.
//!   The forwarder books each crate on the soonest run toward its consignee ([`Hub::forward`]): on a block of space it
//!   bought already, else on tickets from that run's shop with the hub's money; demand it couldn't book on a carrier's
//!   run it remembers, and buys blocks of space ahead each day, `block_runs` runs at `block_price` of the shop's, as many
//!   stacks as it wanted a run ([`Hub::buy_blocks`]), paying for each run's block as it loads, full or not
//!   ([`Hub::run_block`]). Booked crates load first ([`Hub::load_booked`]). It buys the shippers' bills on markets it runs
//!   to, at its discount desk (`bank.rs` [`crate::bank::Discounts`]).
//! - **Hubs emerge.** A market on the map becomes a hub on its own when other markets' routes pass within a hex of it,
//!   at least `hub_routes` pass or end there (or two pass), its Merchant is in, it holds the food to feed callers for
//!   their road home, and it has `storehouse_emeralds` beyond its reserve for the storehouse.
//! - **Stops on the way.** A route whose path crosses a hub's land (not one of its ends) calls there, out and home, its
//!   arrival set so far along the road and the rest kept for after ([`Network::stop_on`]): it pays the hub's landing fee
//!   (a quarter of passing its posts, at least a quarter emerald: [`stop_fee`]), buys food there for the road home if the
//!   far end may not feed it, hands the forwarder crates bound for markets the hub serves, sells it its bills, loads the
//!   crates booked on its run, and carries the hub's contract copies on to where it's bound.
//! - **Routes reorganise.** Each day a hub opens a run of its own to a far end of an established route calling there
//!   (five trips), one a day; and the owner of a long run calling at a hub that runs on to its far end compares its
//!   road's food (and tolls, and the landing) with a short run to the hub and freight from there for its usual load
//!   (carted, with the forwarder's margin), and winds the long run down when that costs at most three quarters
//!   (`SWITCH_SHARE`), opening the short run if it has none.
//!
//! Ported from `market/Freight.java`: `MARGIN`, `SHARE`, `FORWARD_MARGIN`, `DRAY_STACKS`, `DRAY_WAGE`, `CART`,
//! `HUB_ROUTES`, `STOREHOUSE_EMERALDS`, `STOP_FEE_SHARE`, `BASE_STOP_FEE`, `stopFee` (97), `land` (104), `Leg` (123),
//! `stopOn` (156), `SpaceBlock` (186), `isHub` (210), `legs`/`legsFrom` (215), `stackPrice` (273), `BACKHAUL_SHARE`,
//! `legPrice` (285), `forwardingPrice` (290), `booked` (315), `book` (331), `cartPrice` (353), `cartsPerDay` (358),
//! `onServerTick` (394), `ESTABLISHED_TRIPS`, `SWITCH_SHARE`, `CREW`, `runCost` (437), `reorganise` (448), `through`
//! (505), `considerHub` (519), `blockRoom` (565), `hasBlock` (575), `blockFares` (581), `buyBlocks` (591), `runBlock`
//! (619), `restock` (645), `restockDray` (674), `handOver` (741), `forward` (870), `nextRun` (948), `loadBooked`
//! (962), `discountBills` (982); `caravan/Caravans.java`: `atHub` (789), `pickForwarded` (965), `FORWARD_LEAD`, `setStop`
//! (1017), `callAtStop` (1038), `dispatch`'s booked crates first (390) and space sold on its day (409), `lastCrew` and
//! `lastCapacity` (565); `cargo/Crates.java` `receive`'s `billed` branch (290); `Dials.java` `BLOCK_RUNS`, `BLOCK_PRICE`,
//! `LINER_DAYS`; `caravan/CaravanLedger.java` `nextDeparture` (120).
//!
//! **Where the core differs.** The mod's freight shops are Numismatics vendors that hold the tickets' takings for the
//! carrier's caravan to collect when it calls; the core keeps no till: a booking on another carrier's run records its
//! fare, and the hub pays it into the carrier's wagon as the run loads the crate (the moment the mod's caravan collects
//! its shop), as far as the hub's treasury goes. A shop's tickets are its run's spare space (the mod's vendor holds a
//! ticket a slot, so its slots may cap them). Drayage is paid into the hub's treasury at once (the mod's drayage shop is
//! emptied into it the next day). A shipper's wagon to a hub carries the forwarding for the goods it sends on, from what
//! its town can spare (the mod's pays it from the purse it brought; the core's purse is the road home's food), and sends
//! on only what it can crate and pay to forward. A forwarder's crate under a contract was sent with a bill drawn on its
//! buyer (the mod's `billed` lading, the only way the core sends one), so its consignee takes it as paid. The discount
//! desk sits every day (the mod's `Discounts.open` days aside). Only roads over the map pass through a hub's land (a bare
//! route has no path), and the distances the reorganising reckons are between the hexes' centres. A crate booked on a
//! run that's gone (wound down) is booked again. Stranded paper, and the hub's own wagon's collections at its
//! towns (`collectForwarding`: the fare and the freight on credit), are `stranded.rs`'s. Not yet: the forwarder's
//! consignment (`Forwarder.java`), contract copies left at a hub for markets it
//! serves (`copiesVia`), rents at the hub (`RentDealer`), and the porter run's freight
//! check (`porterRun`: a run with space sold or crates booked goes by wagon).

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::{Component, Resource};
use serde::{Deserialize, Serialize};

use crate::content::Content;
use crate::fixed::Fixed;
use crate::hex::Hex;
use crate::messages::{RouteId, TownId};
use crate::pallet::{At, Lading, Package, Pallet, PalletId, Pallets};
use crate::state::{Route, RoutePath, RouteState};

/// Stacks a cart of drayage carries (`Freight.CART`).
pub const CART: u32 = 4;
/// A landing fee is at least this many emeralds (`Freight.BASE_STOP_FEE`, a share of the mint's rate in spurs).
pub const BASE_STOP_FEE: f64 = 0.25;
/// A hub opens a run of its own to a town once a route calling there has made this many trips; the owner of a long run
/// weighs the hub's only once it's made as many, and the hub's run on has made `ONWARD_TRIPS` (`ESTABLISHED_TRIPS`).
pub const ESTABLISHED_TRIPS: u32 = 5;
pub const ONWARD_TRIPS: u32 = 3;
/// A route's owner winds its long run down when going by the hub costs at most this share of it (`SWITCH_SHARE`).
pub const SWITCH_SHARE: f64 = 0.75;
/// Mouths a run's food is reckoned for when routes are weighed (`Freight.CREW`).
pub const CREW: f64 = 2.0;
/// A shipper sends goods on through a hub this many days ahead: the road beyond takes time (`Caravans.FORWARD_LEAD`).
pub const FORWARD_LEAD_DAYS: f64 = 3.0;
/// Items a stack, as the reorganising counts a route's usual load (`reorganise`'s `perTrip / 64`).
pub const STACK_ITEMS: f64 = 64.0;

/// A carrier's run from a hub (`Freight.Leg`): its route (home, far end), which way (out from home, or back to it), and
/// the hub it runs from: one of the route's ends, or a hub it calls at on its way. The carrier is the route's home.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Leg {
    pub route: RouteId,
    pub home: TownId,
    pub far: TownId,
    pub outbound: bool,
    pub at: TownId,
}

impl Leg {
    /// A route's run from one of its ends: out from its home, or back from its far end.
    pub fn of(route: &Route, outbound: bool) -> Leg {
        Leg { route: route.seq, home: route.from_id, far: route.to_id, outbound,
            at: if outbound { route.from_id } else { route.to_id } }
    }

    /// A route's run from a hub it calls at on its way.
    pub fn calling(route: &Route, outbound: bool, hub: TownId) -> Leg {
        Leg { at: hub, ..Leg::of(route, outbound) }
    }

    /// Where the run starts: the hub.
    pub fn from(&self) -> TownId {
        self.at
    }

    /// Where it's bound.
    pub fn to(&self) -> TownId {
        if self.outbound { self.far } else { self.home }
    }

    /// Whose run it is: the route's home.
    pub fn carrier(&self) -> TownId {
        self.home
    }

    /// Does the run pass through the hub (calling on its way), rather than start or end there?
    pub fn passing(&self) -> bool {
        self.at != self.home && self.at != self.far
    }

    /// Its key among the forwarder's blocks and the shops (`Leg.key`).
    pub fn key(&self) -> u64 {
        self.route as u64 * 4 + if self.outbound { 0 } else { 1 } + if self.passing() { 2 } else { 0 }
    }
}

/// A route as the freight rules see it: its ends, its road, its timetable and trips, the items its wagons sold or
/// delivered at the far end and bought there, all trips told (`traded`: the mod's `itemsDelivered + itemsBought`), its
/// last wagon's crew and space, its wagon on the road (which way, and when it gets there), and the hub it calls at on its way, if any (with how far
/// along its path).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub route: RouteId,
    pub from: TownId,
    pub to: TownId,
    pub blocks: u32,
    pub travel_ticks: u64,
    pub road_cost: Fixed,
    pub every_ticks: u64,
    pub last_departure: Option<u64>,
    pub path: Vec<Hex>,
    pub trips: u32,
    pub traded: f64,
    pub crew: f64,
    pub capacity: u32,
    pub wagon: Option<(crate::state::Leg, u64)>,
    pub stop: Option<(TownId, usize)>,
}

impl Run {
    /// A route as it stands (`crew` and `capacity` from its last wagon: one mouth and a wagon's slots before any).
    pub fn of(content: &Content, route: &Route, state: &RouteState, path: Option<&RoutePath>) -> Run {
        Run {
            route: route.seq,
            from: route.from_id,
            to: route.to_id,
            blocks: route.blocks,
            travel_ticks: route.travel_ticks,
            road_cost: route.road_cost,
            every_ticks: route.every_ticks,
            last_departure: state.last_departure,
            path: path.map(|p| p.0.clone()).unwrap_or_default(),
            trips: state.trips,
            traded: state.sold + state.delivered + state.bought,
            crew: state.freight.crew.unwrap_or(1.0),
            capacity: state.freight.capacity.unwrap_or(content.dials.wagon_slots),
            wagon: state.wagon.as_ref().map(|w| (w.leg, w.arrives)),
            stop: None,
        }
    }
}

/// The routes and hubs the freight rules work on (every route but those wound down), with where each town stands.
#[derive(Clone, Debug, Default)]
pub struct Network {
    pub runs: BTreeMap<RouteId, Run>,
    pub hubs: BTreeSet<TownId>,
    pub hexes: BTreeMap<TownId, Hex>,
}

impl Network {
    /// The network of `runs` (wound-down routes left out by the caller), with the towns that are hubs and where towns
    /// stand; each run's stop found.
    pub fn new(runs: Vec<Run>, hubs: BTreeSet<TownId>, hexes: BTreeMap<TownId, Hex>) -> Network {
        let mut net = Network { runs: runs.into_iter().map(|r| (r.route, r)).collect(), hubs, hexes };
        let stops: Vec<(RouteId, Option<(TownId, usize)>)> = net.runs.values().map(|r| (r.route, net.find_stop(r))).collect();
        for (route, stop) in stops {
            net.runs.get_mut(&route).unwrap().stop = stop;
        }
        net
    }

    /// The network as the world's routes and towns stand: `routes` (route, its state, its path) and `towns` (its number,
    /// its hex, whether it's a hub).
    pub fn of<'a>(content: &Content, routes: impl IntoIterator<Item = (&'a Route, &'a RouteState, Option<&'a RoutePath>)>,
            towns: impl IntoIterator<Item = (TownId, Option<Hex>, bool)>) -> Network {
        let runs: Vec<Run> = routes.into_iter().filter(|(_, s, _)| s.freight.wound_down.is_none())
            .map(|(r, s, p)| Run::of(content, r, s, p)).collect();
        let (mut hubs, mut hexes) = (BTreeSet::new(), BTreeMap::new());
        for (t, hex, hub) in towns {
            if let Some(h) = hex {
                hexes.insert(t, h);
            }
            if hub {
                hubs.insert(t);
            }
        }
        Network::new(runs, hubs, hexes)
    }

    pub fn is_hub(&self, town: TownId) -> bool {
        self.hubs.contains(&town)
    }

    pub fn run(&self, route: RouteId) -> Option<&Run> {
        self.runs.get(&route)
    }

    /// The hub a run calls at on its way (`Freight.stopOn`): a hub, not one of its ends, whose hex is within one of a hex
    /// its path crosses (not its first or last), the one nearest its home; and its place on the path.
    fn find_stop(&self, run: &Run) -> Option<(TownId, usize)> {
        if run.path.len() < 3 {
            return None;
        }
        let mut best: Option<(TownId, usize)> = None;
        for &hub in &self.hubs {
            if hub == run.from || hub == run.to {
                continue;
            }
            let Some(&hex) = self.hexes.get(&hub) else { continue };
            if let Some(i) = (1..run.path.len() - 1).find(|&i| run.path[i].distance(hex) <= 1) {
                if best.is_none_or(|(_, b)| i < b) {
                    best = Some((hub, i));
                }
            }
        }
        best
    }

    /// The hub a route calls at on its way, and how far along its path (`stopOn`).
    pub fn stop_on(&self, route: RouteId) -> Option<(TownId, usize)> {
        self.runs.get(&route).and_then(|r| r.stop)
    }

    /// Every carrier's run from the hub (`legsFrom`): routes from or to it, and both ways of routes calling there.
    pub fn legs_from(&self, hub: TownId) -> Vec<Leg> {
        let mut legs = Vec::new();
        for r in self.runs.values() {
            let route = Leg { route: r.route, home: r.from, far: r.to, outbound: true, at: r.from };
            if r.from == hub {
                legs.push(route);
            } else if r.to == hub {
                legs.push(Leg { outbound: false, at: r.to, ..route });
            } else if r.stop.is_some_and(|(h, _)| h == hub) {
                legs.push(Leg { at: hub, ..route });
                legs.push(Leg { outbound: false, at: hub, ..route });
            }
        }
        legs
    }

    /// Carriers' runs from the hub to `to` (`legs`).
    pub fn legs(&self, hub: TownId, to: TownId) -> Vec<Leg> {
        self.legs_from(hub).into_iter().filter(|l| l.to() == to).collect()
    }

    /// Routes of other markets whose path passes within a hex of the market's (`Freight.through`).
    pub fn through(&self, market: TownId) -> Vec<RouteId> {
        let Some(&hex) = self.hexes.get(&market) else { return Vec::new() };
        self.runs.values().filter(|r| r.from != market && r.to != market)
            .filter(|r| r.path.iter().any(|h| h.distance(hex) <= 1)).map(|r| r.route).collect()
    }

    /// Routes from or to the market.
    pub fn calls(&self, market: TownId) -> usize {
        self.runs.values().filter(|r| r.from == market || r.to == market).count()
    }

    /// A route between two towns, from `from` to `to`.
    pub fn route_between(&self, from: TownId, to: TownId) -> Option<&Run> {
        self.runs.values().find(|r| r.from == from && r.to == to)
    }

    /// How far apart two placed towns are, in blocks, between their hexes' centres.
    pub fn distance(&self, a: TownId, b: TownId) -> Option<u32> {
        let (x, y) = (self.hexes.get(&a)?, self.hexes.get(&b)?);
        let (dx, dz) = (x.center_x() - y.center_x(), x.center_z() - y.center_z());
        Some((dx * dx + dz * dz).sqrt() as u32)
    }

    /// When a route's next wagon leaves its home (`CaravanLedger.nextDeparture`): when its wagon on the road is home,
    /// or now, and not before its timetable's day.
    pub fn next_departure(&self, run: &Run, now: u64) -> u64 {
        let home = match run.wagon {
            Some((crate::state::Leg::Out, arrives)) => arrives.max(now) + run.travel_ticks,
            Some((crate::state::Leg::Home, arrives)) => arrives,
            None => now,
        };
        now.max(home).max(run.last_departure.map_or(0, |t| t + run.every_ticks))
    }

    /// When a leg's next run leaves the hub (`Freight.nextRun`): a hub's own wagon's next departure; a caller's about
    /// halfway out (or back) from its next departure; a far end's wagon home as it gets there.
    pub fn next_run(&self, leg: &Leg, now: u64) -> u64 {
        let Some(run) = self.runs.get(&leg.route) else { return u64::MAX };
        let next = self.next_departure(run, now);
        if leg.passing() {
            return next + run.travel_ticks / 2;
        }
        if leg.outbound {
            return next;
        }
        match run.wagon {
            Some((crate::state::Leg::Out, arrives)) => arrives,
            _ => next + run.travel_ticks,
        }
    }
}

/// What a stack of space on a run costs, in emeralds (`Freight.stackPrice`): its last wagon's crew's food both ways
/// (the team counts as a mouth) over what that wagon could carry, and the carriers' margin. Where content has no road
/// food, the road's cost a round trip in money over the wagon's space.
pub fn stack_price(content: &Content, run: &Run) -> f64 {
    let d = &content.dials;
    let road = if crate::road::road_food(content) {
        crate::road::food_needed(content, run.blocks) * 2.0 * run.crew * d.purse_per_food_point
    } else {
        run.road_cost.to_f64()
    };
    road / run.capacity.max(1) as f64 * (1.0 + d.freight_margin)
}

/// What a stack of space on a leg costs (`Freight.legPrice`): the run's price, `backhaul_share` of it going home.
pub fn leg_price(content: &Content, net: &Network, leg: &Leg) -> f64 {
    let Some(run) = net.run(leg.route) else { return 0.0 };
    stack_price(content, run) * if leg.outbound { 1.0 } else { content.dials.backhaul_share }
}

/// What a forwarder asks a stack on a leg (`Freight.forwardingPrice`): the freight and its margin.
pub fn forwarding_price(content: &Content, net: &Network, leg: &Leg) -> f64 {
    leg_price(content, net, leg) * (1.0 + content.dials.forward_margin)
}

/// What a cart of drayage costs, in emeralds (`Freight.cartPrice`): the draymen's wage for its stacks, and the margin.
pub fn cart_price(content: &Content) -> f64 {
    let d = &content.dials;
    d.dray_wage / d.dray_stacks.max(1) as f64 * CART as f64 * (1.0 + d.freight_margin)
}

/// Carts the hub's draymen move a day (`Freight.cartsPerDay`): its idle hands and one more, `dray_stacks` each.
pub fn carts_per_day(content: &Content, idle: usize) -> u32 {
    ((idle as u32 + 1) * content.dials.dray_stacks) / CART
}

/// Carts a crate of `stacks` takes, in from the quay and out to its run.
pub fn carts_for(stacks: u32) -> u32 {
    (2 * stacks).div_ceil(CART)
}

/// What a run's road food costs a trip, both ways, for a usual squad, in emeralds (`Freight.runCost`).
pub fn run_cost(content: &Content, blocks: u32) -> f64 {
    crate::road::food_needed(content, blocks) * 2.0 * CREW * content.dials.purse_per_food_point
}

/// A shop's price for something worth `emeralds`, in spurs at the mint's `rate`: whole spurs, at least one.
pub fn spurs_price(emeralds: f64, rate: f64) -> f64 {
    (emeralds * rate).ceil().max(1.0)
}

/// What the forwarder at `hub` asks for a crate of `stacks` bound for `to` (`handOver`'s fee), in emeralds, and the
/// drayage in it: the cheapest leg's forwarding a stack, and the carting in and out with the margin, in whole spurs at
/// `rate`, in thousandths rounded up (a cost). None where no carrier runs that way.
pub fn forwarding_fee(content: &Content, net: &Network, hub: TownId, to: TownId, stacks: u32, rate: f64)
        -> Option<(Fixed, Fixed)> {
    let cheapest = net.legs(hub, to).iter().map(|l| forwarding_price(content, net, l)).fold(f64::INFINITY, f64::min);
    if !cheapest.is_finite() {
        return None;
    }
    let drayage = carts_for(stacks) as f64 * spurs_price(cart_price(content), rate);
    let fee = (stacks as f64 * cheapest * rate + drayage * (1.0 + content.dials.forward_margin)).ceil();
    Some((Fixed::ceil(fee / rate), Fixed::ceil(drayage / rate)))
}

/// A caravan's landing fee at a hub, in emeralds (`Freight.stopFee`, `land`): a quarter of what passing its posts costs
/// (`passage`, in spurs), at least a quarter emerald, in whole spurs (in thousandths, rounded up).
pub fn stop_fee(content: &Content, passage: f64, rate: f64) -> Fixed {
    Fixed::ceil(stop_price(content, passage, rate).ceil() / rate)
}

/// The landing fee as the mod prices it, in spurs, unrounded (`Freight.stopFee`; [`stop_fee`] is what a caravan pays).
pub fn stop_price(content: &Content, passage: f64, rate: f64) -> f64 {
    (BASE_STOP_FEE * rate).max(passage * content.dials.stop_fee_share)
}

/// The forwarder's block of space on a carrier's run (`Freight.SpaceBlock`): `stacks` on each of its next `runs` runs
/// out of the hub at `price` a stack, paid as each run loads, full or not; what it wanted that way since the last run,
/// and a run's want smoothed (below 0: none yet).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub stacks: u32,
    pub runs: u32,
    pub price: f64,
    pub wanted: f64,
    pub average: f64,
}

impl Default for Block {
    fn default() -> Self {
        Block { stacks: 0, runs: 0, price: 0.0, wanted: 0.0, average: -1.0 }
    }
}

/// A crate's space bought on a leg's run (`Freight.book`'s ticket): its stacks, on the forwarder's block or on tickets,
/// and the fare the hub owes the carrier as its run loads (none on a block, paid for by the run, or on the hub's own).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Booking {
    pub leg: Leg,
    pub stacks: u32,
    pub fare: Fixed,
    pub block: bool,
}

/// A pallet in the forwarder's custody (`At::Custody` in the registry, held for others: never opened there): carted in
/// from the quay (drayage bought) or not yet, and booked or not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Custody {
    pub carted: bool,
    pub booked: Option<Booking>,
}

/// What a hub has done, in all (`Freight.book`): stacks forwarded and the fees for them, bills bought and their face,
/// stacks its runs carried on freight, freight it bought (tickets and blocks), drayage its draymen were paid, carts, and
/// landing fees (emeralds).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HubBook {
    pub forwarded: u32,
    pub fees: Fixed,
    pub bills: u32,
    pub face: Fixed,
    pub carried: u32,
    pub freight: Fixed,
    pub drayage: Fixed,
    pub carts: u32,
    pub landing: Fixed,
}

/// A town's storehouse and its forwarder (`Depots.STOREHOUSE`, `Freight`): every town has one, inert until it's a hub.
/// The pallets in its custody (by id: they're `At::Custody` in the registry, in the order they came), its blocks of space
/// and the demand it couldn't book, the tickets each leg's shop has left today and the carts its draymen have, and its
/// book.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Hub {
    pub storehouse: bool,
    pub custody: BTreeMap<PalletId, Custody>,
    pub blocks: BTreeMap<u64, Block>,
    pub wanted_on: BTreeMap<u64, f64>,
    pub tickets: BTreeMap<u64, u32>,
    pub carts: u32,
    pub book: HubBook,
}

/// What a hand-over came to: stacks taken, and the fee (with its drayage) in emeralds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HandOver {
    pub stacks: u32,
    pub fee: Fixed,
    pub drayage: Fixed,
}

/// Stacks of space a wagon may sell a run (`(int) (CARGO_SLOTS * SHARE)`).
pub fn share_stacks(content: &Content) -> u32 {
    (content.dials.wagon_slots as f64 * content.dials.freight_share) as u32
}

/// The crates a hub's forwarder holds in custody, packed.
pub fn custody_crates(pallets: &Pallets, hub: TownId) -> u32 {
    pallets.at_place(At::Custody(hub)).map(|p| p.crates()).sum()
}

impl Hub {
    /// Stacks booked on a leg among the crates waiting (`Freight.booked` over the warehouse).
    pub fn booked_on(&self, leg: &Leg) -> u32 {
        self.custody.values().filter_map(|h| h.booked).filter(|b| b.leg.key() == leg.key()).map(|b| b.stacks).sum()
    }

    /// Stacks of the forwarder's block on a leg still free for its next run (`blockRoom`).
    pub fn block_room(&self, leg: &Leg) -> u32 {
        match self.blocks.get(&leg.key()) {
            Some(b) if b.runs > 0 => b.stacks.saturating_sub(self.booked_on(leg)),
            _ => 0,
        }
    }

    /// Has the forwarder bought space on this leg's runs (`hasBlock`)?
    pub fn has_block(&self, leg: &Leg) -> bool {
        self.blocks.get(&leg.key()).is_some_and(|b| b.runs > 0)
    }

    /// The fares a carrier's run earns from the forwarder's block on a leg, in emeralds (`blockFares`).
    pub fn block_fares(&self, leg: &Leg) -> Fixed {
        self.blocks.get(&leg.key()).filter(|b| b.runs > 0).map_or(Fixed::ZERO, |b| Fixed::ceil(b.stacks as f64 * b.price))
    }

    /// The day's restock (`restock`, `restockDray`): each leg's shop stocked with the space its next run has to spare
    /// (its share of a wagon less what's booked and waiting, or the block if more), and the draymen's carts.
    pub fn restock(&mut self, content: &Content, net: &Network, hub: TownId, idle: usize) {
        let share = share_stacks(content);
        let mut tickets = BTreeMap::new();
        for leg in net.legs_from(hub) {
            let waiting = self.booked_on(&leg);
            let block = self.blocks.get(&leg.key()).map_or(0, |b| b.stacks);
            tickets.insert(leg.key(), share.saturating_sub(waiting.max(block)));
        }
        self.tickets = tickets;
        self.carts = carts_per_day(content, idle);
    }

    /// The forwarder takes crates bound for markets it serves off a wagon (`handOver`): for each of `crates` (its
    /// consignee, its stacks and its crates) made out elsewhere than the hub or one of `skip`, where a carrier runs that
    /// way and the warehouse has room (beside the `held` crates in custody already), the draymen free to cart it in and
    /// out, and `purse` (emeralds) to pay the fee (the cheapest leg's forwarding a stack, and the carting with the margin,
    /// in whole spurs at `rate`), the crate is the forwarder's. Returns what it came to, and which it took, in order: the
    /// caller takes them into custody, carted in.
    #[allow(clippy::too_many_arguments)]
    pub fn hand_over(&mut self, content: &Content, net: &Network, hub: TownId, held: u32, crates: &[(TownId, u32, u32)],
            purse: Fixed, rate: f64, skip: &[TownId]) -> (HandOver, Vec<usize>) {
        let mut out = HandOver::default();
        let (mut taken, mut held) = (Vec::new(), held);
        for (i, &(to, stacks, packed)) in crates.iter().enumerate() {
            if to == hub || skip.contains(&to) {
                continue;
            }
            let room = held + packed <= content.dials.crate_room;
            let carts = carts_for(stacks);
            let Some((fee, drayage)) = forwarding_fee(content, net, hub, to, stacks, rate).filter(|_| room) else {
                continue; // no carrier runs that way, or no room in the warehouse
            };
            if self.carts < carts || purse - out.fee < fee {
                continue; // no draymen free today (it stays aboard), or no money for the fee
            }
            self.carts -= carts;
            out.stacks += stacks;
            out.fee += fee;
            out.drayage += drayage;
            self.book.carts += carts;
            self.book.forwarded += stacks;
            self.book.fees += fee;
            held += packed;
            taken.push(i);
        }
        self.book.drayage += out.drayage;
        (out, taken)
    }

    /// The forwarder books the crates in its custody (`forward`), in the order they came: a crate not yet carted in is
    /// carted first, if the draymen are free (the hub's own drayage); then each is booked on the soonest carrier's run
    /// toward its consignee that has the space: its own block first; failing that, the demand is remembered on the
    /// soonest other carrier's run, and it buys just the tickets the crate needs at the soonest run's shop with what it
    /// can `spare` (emeralds, beyond its reserve; counted down), the fare owed the carrier as it loads (none on its own
    /// run). A crate booked on a run that's gone is booked again. Returns stacks booked, and the fares it took on.
    #[allow(clippy::too_many_arguments)]
    pub fn forward(&mut self, content: &Content, net: &Network, hub: TownId, pallets: &Pallets, now: u64, rate: f64,
            spare: &mut Fixed) -> (u32, Fixed) {
        let legs: BTreeSet<u64> = net.legs_from(hub).iter().map(|l| l.key()).collect();
        let mut held = std::mem::take(&mut self.custody);
        for h in held.values_mut() {
            if h.booked.is_some_and(|b| !legs.contains(&b.leg.key())) {
                h.booked = None;
            }
        }
        // Stacks booked on each leg, as the crates stand (a crate booked now counts against the block for the next).
        let mut on_legs: BTreeMap<u64, u32> = BTreeMap::new();
        for b in held.values().filter_map(|h| h.booked) {
            *on_legs.entry(b.leg.key()).or_insert(0) += b.stacks;
        }
        let (mut booked, mut fares) = (0, Fixed::ZERO);
        for p in pallets.at_place(At::Custody(hub)) {
            let Some(h) = held.get_mut(&p.id) else { continue };
            if h.booked.is_some() {
                continue;
            }
            let stacks = p.slots(content);
            if !h.carted {
                // Collected by the hub's own wagon: carted in and out now, the hub's own draymen paid by itself.
                let carts = carts_for(stacks);
                if self.carts < carts {
                    continue; // no draymen free today: it waits on the quay
                }
                self.carts -= carts;
                self.book.carts += carts;
                h.carted = true;
            }
            let to = p.lading.consignee;
            let mut on = net.legs(hub, to);
            on.sort_by_key(|l| net.next_run(l, now));
            // On the forwarder's own block first: space it's bought already.
            if let Some(leg) = on.iter().find(|l| self.room_on(&on_legs, l) >= stacks) {
                self.blocks.get_mut(&leg.key()).unwrap().wanted += stacks as f64;
                h.booked = Some(Booking { leg: *leg, stacks, fare: Fixed::ZERO, block: true });
                *on_legs.entry(leg.key()).or_insert(0) += stacks;
                booked += stacks;
                continue;
            }
            if let Some(leg) = on.iter().find(|l| l.carrier() != hub) {
                match self.blocks.get_mut(&leg.key()) {
                    Some(b) => b.wanted += stacks as f64, // overflow: it'll want a bigger block
                    None => *self.wanted_on.entry(leg.key()).or_insert(0.0) += stacks as f64,
                }
            }
            for leg in &on {
                let lots = self.tickets.get(&leg.key()).copied().unwrap_or(0);
                if lots < stacks {
                    continue;
                }
                let cost = Fixed::ceil(stacks as f64 * spurs_price(leg_price(content, net, leg), rate) / rate);
                if *spare < cost {
                    continue;
                }
                *spare -= cost;
                *self.tickets.get_mut(&leg.key()).unwrap() -= stacks;
                let fare = if leg.carrier() == hub { Fixed::ZERO } else { cost }; // its own run's shop: the hub pays itself
                fares += fare;
                self.book.freight += cost;
                h.booked = Some(Booking { leg: *leg, stacks, fare, block: false });
                *on_legs.entry(leg.key()).or_insert(0) += stacks;
                booked += stacks;
                break;
            }
        }
        self.custody = held;
        (booked, fares)
    }

    /// As [`Hub::block_room`], with the stacks booked on each leg as `booked` counts them.
    fn room_on(&self, booked: &BTreeMap<u64, u32>, leg: &Leg) -> u32 {
        match self.blocks.get(&leg.key()) {
            Some(b) if b.runs > 0 => b.stacks.saturating_sub(booked.get(&leg.key()).copied().unwrap_or(0)),
            _ => 0,
        }
    }

    /// Once a day the forwarder buys space ahead on the carriers' runs out of the hub it's been sending crates on (not
    /// its own runs), `block_runs` runs at a time at `block_price` of the leg's price, as big a block as it's wanted a run
    /// (smoothed), at most a wagon's share (`buyBlocks`).
    pub fn buy_blocks(&mut self, content: &Content, net: &Network, hub: TownId) {
        let d = &content.dials;
        for leg in net.legs_from(hub) {
            if leg.carrier() == hub {
                continue;
            }
            let key = leg.key();
            let b = self.blocks.get(&key);
            let wanted = match b {
                Some(b) => b.average.max(0.0),
                None => self.wanted_on.get(&key).copied().unwrap_or(0.0),
            };
            if b.is_some_and(|b| b.runs > 0) || wanted < 1.0 {
                continue;
            }
            let average = b.map_or(wanted, |b| b.average);
            let block = Block {
                stacks: (d.wagon_slots as f64 * d.freight_share).min(wanted.ceil()) as u32,
                runs: d.block_runs,
                price: leg_price(content, net, &leg) * d.block_price,
                wanted: 0.0,
                average,
            };
            self.blocks.insert(key, block);
            self.wanted_on.remove(&key);
        }
    }

    /// A carrier's run loads at the hub on a leg the forwarder has a block on (`runBlock`): the block's cost (paid by the
    /// caller, full or not, into the carrier's wagon), and what it wanted this run learned for the next block: smoothed
    /// by half, a block it didn't fill shrinking and one that overflowed growing for the runs still to come. Returns the
    /// cost in emeralds (0 without a block), rounded up to the thousandth.
    pub fn run_block(&mut self, content: &Content, leg: &Leg) -> Fixed {
        let d = &content.dials;
        let Some(b) = self.blocks.get_mut(&leg.key()).filter(|b| b.runs > 0) else { return Fixed::ZERO };
        let cost = Fixed::ceil(b.stacks as f64 * b.price);
        b.average = if b.average < 0.0 { b.wanted } else { 0.5 * b.average + 0.5 * b.wanted };
        b.wanted = 0.0;
        b.runs -= 1;
        if b.runs > 0 {
            b.stacks = (d.wagon_slots as f64 * d.freight_share).min(b.average.ceil()).max(1.0) as u32;
        }
        cost
    }

    /// Booked crates waiting for a leg's run, in the order they came, out of custody to load first, as many as `slots`
    /// hold (`loadBooked`): each with its booking, for the caller to load.
    pub fn load_booked(&mut self, content: &Content, pallets: &Pallets, hub: TownId, leg: &Leg, slots: u32)
            -> Vec<(PalletId, Booking)> {
        let mut room = slots;
        let mut load = Vec::new();
        for p in pallets.at_place(At::Custody(hub)) {
            let stacks = p.slots(content);
            let Some(b) = self.custody.get(&p.id).and_then(|h| h.booked) else { continue };
            if b.leg.key() == leg.key() && stacks <= room {
                room -= stacks;
                self.custody.remove(&p.id);
                self.book.carried += stacks;
                load.push((p.id, b));
            }
        }
        load
    }
}

/// The hub a wagon calls at on its leg, called there yet or not, and the ticks of road left after it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub hub: TownId,
    pub called: bool,
    pub remaining: u64,
}

/// A wagon's freight (`Wagon.freight`): the hub it calls at on this leg (and the leg its stop was reckoned for). Its
/// pallets on freight tickets, and those the hub's own wagon carries for its forwarder, are lines on its manifest
/// (`Wagon.manifest`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Aboard {
    pub stop: Option<Stop>,
    pub reckoned: Option<crate::state::Leg>,
}

/// A route's freight (`RouteState.freight`): its calls at hubs (`Route.stops`), its last wagon's mouths with the team
/// and its stacks of space (`lastCrew`, `lastCapacity`), and when its owner wound it down for a run by a hub.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteFreight {
    pub stops: u32,
    pub crew: Option<f64>,
    pub capacity: Option<u32>,
    pub wound_down: Option<u64>,
}

/// The freight rules' clock and the runs the hubs and owners opened at the day's reviews, opened once the look is done.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Freight {
    pub day: Option<u64>,
    pub pending: Vec<(TownId, TownId)>,
    /// Each hub's book of stranded paper bought and freight carried on credit (`stranded.rs`).
    pub stranded: BTreeMap<TownId, crate::stranded::StrandedBook>,
}

/// A pallet on freight received by its consignee, opened (`Crates.receive`): its copy filed, its goods into `stock`, its
/// crates kept empty to ship with; not paid for in coin when crated, under a contract in force with the consignee its
/// buyer, it's delivered, paid for by the bill drawn on the buyer when it was sent on (`billed`): at most the larger of
/// what's owed by the next installment and what's on the road.
pub fn receive(book: &mut crate::contracts::Book, now: u64, town: TownId, stock: &mut [f64],
        store: &mut crate::cargo::Shipments, pallet: &Pallet) {
    let Some(id) = pallet.lading.contract.filter(|_| !pallet.lading.prepaid()) else {
        crate::cargo::receive(book, now, town, stock, store, pallet);
        return;
    };
    let (good, count) = (pallet.good(), pallet.count());
    if pallet.lading.copy {
        book.deliver_copy(id, town);
    }
    stock[good] += count;
    store.empties += pallet.crates();
    let c = &book.contracts[id];
    if c.buyer != town || !c.in_force() || c.good != good {
        return;
    }
    let counted = count.min(c.owed(now + c.every).max(c.shipped)).max(0.0);
    let worth = Fixed::ceil(counted * c.price);
    book.settle(id, counted, counted, worth, Fixed::ZERO);
}

use crate::society::Society;

impl Society {
    /// The freight network as it stands: every route not wound down, the hubs, where towns stand.
    pub fn network(&self) -> Network {
        let content = self.content();
        let routes = self.routes.iter().filter_map(|&e| {
            let e = self.world.entity(e);
            Some((e.get::<Route>()?, e.get::<RouteState>()?, e.get::<RoutePath>()))
        });
        let towns = self.towns.iter().enumerate().map(|(t, &e)| {
            let e = self.world.entity(e);
            (t, e.get::<crate::state::Town>().and_then(|t| t.hex), e.get::<Hub>().is_some_and(|h| h.storehouse))
        });
        Network::of(content, routes, towns)
    }

    /// The town sets up a storehouse and is a freight hub from now on (a host's grant: the mod's `Depots.add`).
    pub fn make_hub(&mut self, town: TownId) {
        self.world.get_mut::<Hub>(self.towns[town]).expect("a town").storehouse = true;
    }

    /// Is the town a freight hub (`Freight.isHub`)?
    pub fn is_hub(&self, town: TownId) -> bool {
        self.world.get::<Hub>(self.towns[town]).is_some_and(|h| h.storehouse)
    }

    /// The town's storehouse and forwarder as they stand.
    pub fn hub(&self, town: TownId) -> Hub {
        self.world.get::<Hub>(self.towns[town]).cloned().unwrap_or_default()
    }

    /// A route's run out from its home (`outbound`) or back from its far end.
    pub fn leg(&self, route: RouteId, outbound: bool) -> Leg {
        Leg::of(self.world.get::<Route>(self.routes[route]).expect("a route"), outbound)
    }

    /// A route's run from a hub it calls at on its way.
    pub fn calling_leg(&self, route: RouteId, outbound: bool, hub: TownId) -> Leg {
        Leg::calling(self.world.get::<Route>(self.routes[route]).expect("a route"), outbound, hub)
    }

    /// Every carrier's run from the hub (`legsFrom`).
    pub fn legs_from(&self, hub: TownId) -> Vec<Leg> {
        self.network().legs_from(hub)
    }

    /// The hub a route calls at on its way, and how far along its path (`stopOn`).
    pub fn stop_on(&self, route: RouteId) -> Option<(TownId, usize)> {
        self.network().stop_on(route)
    }

    /// What a stack of space on a route's run costs, in emeralds (`stackPrice`).
    pub fn stack_price(&self, route: RouteId) -> f64 {
        let e = self.world.entity(self.routes[route]);
        stack_price(self.content(), &Run::of(self.content(), e.get::<Route>().unwrap(), e.get::<RouteState>().unwrap(),
            e.get::<RoutePath>()))
    }

    /// What a stack of space on a leg costs (`legPrice`), and what a forwarder asks for one (`forwardingPrice`).
    pub fn leg_price(&self, leg: &Leg) -> f64 {
        leg_price(self.content(), &self.network(), leg)
    }

    pub fn forwarding_price(&self, leg: &Leg) -> f64 {
        forwarding_price(self.content(), &self.network(), leg)
    }

    fn hub_mut(&mut self, town: TownId) -> Hub {
        std::mem::take(&mut *self.world.get_mut::<Hub>(self.towns[town]).expect("a town"))
    }

    fn put_hub(&mut self, town: TownId, hub: Hub) {
        *self.world.get_mut::<Hub>(self.towns[town]).expect("a town") = hub;
    }

    /// The hub's day's restock now (as its first look of a day does): each leg's tickets, and the draymen's carts.
    pub fn restock_hub(&mut self, hub: TownId) {
        let net = self.network();
        let idle = self.world.get::<crate::roster::Roster>(self.towns[hub]).map_or(0, |r| r.idle(self.content()).len());
        let content = self.content().clone();
        let mut h = self.hub_mut(hub);
        h.restock(&content, &net, hub, idle);
        self.put_hub(hub, h);
    }

    /// A host's crates offered to the hub's forwarder (a caravan's, in the mod's test: each its bill and packages): those
    /// it takes come into the society, into its custody, carted in, the fee paid in from `purse` (emeralds the host holds,
    /// outside the society) into the hub's treasury; the rest stay in `crates`. Returns stacks handed over (`handOver`).
    pub fn hand_over(&mut self, hub: TownId, crates: &mut Vec<(Lading, Vec<Package>)>, purse: &mut Fixed) -> u32 {
        let net = self.network();
        let content = self.content().clone();
        let (rate, now) = (self.mint_rate(), self.now());
        let offered: Vec<(TownId, u32, u32)> = crates.iter().map(|(l, pk)| {
            let p = Pallet { lading: l.clone(), packages: pk.clone(), ..pallet_of_host() };
            (l.consignee, p.slots(&content), p.crates())
        }).collect();
        let held = custody_crates(self.pallets(), hub);
        let mut h = self.hub_mut(hub);
        let (taken, which) = h.hand_over(&content, &net, hub, held, &offered, *purse, rate, &[]);
        let mut kept = Vec::new();
        for (i, (lading, packages)) in std::mem::take(crates).into_iter().enumerate() {
            if !which.contains(&i) {
                kept.push((lading, packages));
                continue;
            }
            // The crate comes into the society from the host's hands.
            let p = Pallet { lading, packages, ..pallet_of_host() };
            self.world.resource_mut::<crate::state::Ledger>().delivered[p.good()] += p.count();
            let shipper = p.lading.shipper;
            let mut pallets = self.world.resource_mut::<Pallets>();
            let id = pallets.pack(p.lading, p.packages, At::Custody(hub), now);
            pallets.seal(id, shipper, now); // sealed as it left its shipper's: the host carried it
            h.custody.insert(id, Custody { carted: true, booked: None });
        }
        *crates = kept;
        self.put_hub(hub, h);
        if taken.stacks > 0 {
            *purse -= taken.fee;
            use crate::cash_book::Account;
            self.post(Account::Outside, Account::Treasury(hub), taken.fee - taken.drayage, forwarding_reason());
            self.post(Account::Outside, Account::Treasury(hub), taken.drayage, drayage_reason());
        }
        taken.stacks
    }

    /// A host's crate into the hub's custody, booked on `leg` for `stacks` (`Freight.book`, then `insert`): its id.
    pub fn book(&mut self, hub: TownId, lading: Lading, packages: Vec<Package>, leg: Leg, stacks: u32) -> PalletId {
        self.pack_into_custody(hub, lading, packages, Custody { carted: true, booked: Some(Booking { leg, stacks, fare: Fixed::ZERO,
            block: false }) })
    }

    /// A host's crate into the hub's forwarder's custody, to be booked on (`Freight.forwarding`, then `insert`): carted in
    /// already, or waiting on the quay for the draymen. Its id.
    pub fn forward_pallet(&mut self, hub: TownId, lading: Lading, packages: Vec<Package>, carted: bool) -> PalletId {
        self.pack_into_custody(hub, lading, packages, Custody { carted, booked: None })
    }

    fn pack_into_custody(&mut self, hub: TownId, lading: Lading, packages: Vec<Package>, custody: Custody) -> PalletId {
        let now = self.now();
        let p = Pallet { lading, packages, ..pallet_of_host() };
        self.world.resource_mut::<crate::state::Ledger>().delivered[p.good()] += p.count();
        let shipper = p.lading.shipper;
        let mut pallets = self.world.resource_mut::<Pallets>();
        let id = pallets.pack(p.lading, p.packages, At::Custody(hub), now);
        pallets.seal(id, shipper, now); // sealed as it left its shipper's: the host carried it
        self.world.get_mut::<Hub>(self.towns[hub]).expect("a town").custody.insert(id, custody);
        id
    }

    /// The pallets in a hub's custody, in the order they came, with what the forwarder notes of each.
    pub fn custody(&self, hub: TownId) -> Vec<(Pallet, Custody)> {
        let h = self.world.get::<Hub>(self.towns[hub]);
        self.pallets().at_place(At::Custody(hub))
            .map(|p| (p.clone(), h.and_then(|h| h.custody.get(&p.id).copied()).unwrap_or_default())).collect()
    }

    /// The forwarder books the crates in its hands now (`forward`), with what the hub can spare beyond its reserve.
    /// Returns stacks booked.
    pub fn forward(&mut self, hub: TownId) -> u32 {
        let net = self.network();
        let content = self.content().clone();
        let (now, rate) = (self.now(), self.mint_rate());
        let e = self.world.entity(self.towns[hub]);
        let mut spare = e.get::<crate::state::Money>().unwrap().0
            - crate::rules::farms::reserve(&content, e.get::<crate::state::Trades>().unwrap(), e.get::<crate::farms::Farm>());
        let mut h = self.hub_mut(hub);
        let (booked, _) = h.forward(&content, &net, hub, self.world.resource::<Pallets>(), now, rate, &mut spare);
        self.put_hub(hub, h);
        booked
    }

    /// The forwarder buys its blocks of space now (`buyBlocks`).
    pub fn buy_blocks(&mut self, hub: TownId) {
        let net = self.network();
        let content = self.content().clone();
        let mut h = self.hub_mut(hub);
        h.buy_blocks(&content, &net, hub);
        self.put_hub(hub, h);
    }

    /// Has the hub's forwarder bought space on this leg's runs (`hasBlock`)?
    pub fn has_block(&self, hub: TownId, leg: &Leg) -> bool {
        self.world.get::<Hub>(self.towns[hub]).is_some_and(|h| h.has_block(leg))
    }

    /// A carrier's run loads at the hub on a leg the forwarder has a block on (`runBlock`): the hub pays for the block,
    /// as far as its treasury goes, into the carrier's wagon if it's on the road, else to the host (outside the society,
    /// as the mod's test takes it). Returns the emeralds paid.
    pub fn run_block(&mut self, hub: TownId, leg: &Leg) -> Fixed {
        let content = self.content().clone();
        let mut h = self.hub_mut(hub);
        let cost = h.run_block(&content, leg);
        self.put_hub(hub, h);
        let paid = cost.min(self.town(hub).money.at_least_zero());
        if paid > 0.0 {
            let on_road = self.balance(crate::cash_book::Account::Wagon(leg.route)).is_some();
            let to = if on_road { crate::cash_book::Account::Wagon(leg.route) } else { crate::cash_book::Account::Outside };
            self.post(crate::cash_book::Account::Treasury(hub), to, paid, fares_reason());
            self.world.get_mut::<Hub>(self.towns[hub]).unwrap().book.freight += paid;
        }
        paid
    }

    /// The runs the hubs and owners opened at this look's reviews (`reorganise`), opened as [`Society::open_route`]
    /// opens one (a road over the map if there's a way), if no such route is there by now.
    pub(crate) fn open_freight_runs(&mut self) {
        let pending = std::mem::take(&mut self.world.resource_mut::<Freight>().pending);
        for (from, to) in pending {
            let there = self.routes.iter().filter_map(|&e| self.world.get::<Route>(e).map(|r| (r, self.world.get::<RouteState>(e))))
                .any(|(r, s)| r.from_id == from && r.to_id == to && s.is_some_and(|s| s.freight.wound_down.is_none()));
            if !there {
                self.open_route(from, to);
            }
        }
    }
}

pub(crate) fn forwarding_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("forwarding".into())
}

pub(crate) fn drayage_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("drayage".into())
}

pub(crate) fn fares_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("fares".into())
}

pub(crate) fn stop_fee_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("stop fee".into())
}

pub(crate) fn storehouse_reason() -> crate::cash_book::Reason {
    crate::cash_book::Reason::Other("storehouse".into())
}

impl Society {
    /// A route's freight: its calls at hubs, its last wagon's crew and space, whether it was wound down.
    pub fn route_freight(&self, route: RouteId) -> RouteFreight {
        self.world.get::<RouteState>(self.routes[route]).map(|s| s.freight.clone()).unwrap_or_default()
    }

    /// The freight of a route's wagon on the road: its stop (its pallets on freight tickets are lines on its manifest:
    /// [`Society::wagon_manifest`]).
    pub fn wagon_freight(&self, route: RouteId) -> Option<Aboard> {
        self.world.get::<RouteState>(self.routes[route]).and_then(|s| s.wagon.as_ref()).map(|w| w.freight.clone())
    }
}

/// A pallet as a host hands it in, before the registry gives it an id and a place.
fn pallet_of_host() -> Pallet {
    Pallet { id: 0, kind: crate::pallet::LOAD, lading: Lading::default(), packages: Vec::new(), at: At::default(), since: 0,
        seal: None, claimed: None }
}
