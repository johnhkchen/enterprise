//! What towns want, in money, and what a Merchant buys on the way home (`docs/decisions.md` "Desires", "Towns buy
//! comforts by caravan", "Hauling only what sells", "The town's shop").
//!
//! - **What a town will pay for a comfort** ([`will_pay`], `Desires.reckon`): up to [`Dials::desire_most`] times its
//!   worth when none of the kind has been had, as far as its households and its spare money can pay for what it's short
//!   of; a town holding its fill pays [`Dials::desire_glut`] of it (the mod's `MOST` 2, `GLUT` 0.5). The mod's impact is
//!   linear in the shortage; the square-root price impact of `docs/core-math.md` stays "maybe".
//! - **What a town is short of** ([`comfort_wants`], `Caravans.comfortWants`): a kind's par less what the market held at
//!   its last stocktake, less what its villagers will have used since (`villagers x ration x days`): the stocktake is
//!   taken just after a wagon sells, when the shelves are fullest. A market never counted is read as it stands.
//! - **What a Merchant buys to sell on** ([`outlet`], [`worth_buying`], [`haul_each`], `Desires.outlet`, `worthBuying`,
//!   `haulEach`): the best outlet for a comfort among the markets the Merchant's routes supply, what they'd pay less the
//!   road (a stack's share of a round trip's road food); a good is worth buying to sell on when its price is at most
//!   that less the margin ([`Dials::resale_margin`], 15%). What a market would pay is the price its shop listed when
//!   a wagon last called ([`Listings`]), else the last price the Merchant's books show there, else reckoned.
//! - **The town's shop** (`market/ShopBoard.java` `wants`, `add`): a town lists each kind of comfort it's short of and
//!   holds none of, and each good its lockers take that it holds none of, at what it would pay, while it has a road for
//!   it and money for a lot; the three most wanted, comforts first. The listing is news: a wagon calling reads it
//!   ([`Listings::seen`]). (The vendors and the shelf are Minecraft's: the core keeps the listing and its price.)
//! - **Shopping and backhauls** (`rules/desires.rs`): a town sending its own wagon gives it money to bring home the
//!   comforts it lacks, whole items bought up to the far end's spare at bargained prices; and the Merchant buys the far
//!   end's surplus for the trip home, only goods another town on the network wants (up to the resale ceiling of its
//!   worth, never below the far end's 320 food points) or a glut the Merchant's own home can use.
//!
//! Ported from `caravan/Desires.java` (`MOST`, `GLUT`, `MARGIN`, `willPay` 33/41, `reckon` 55, `shortOf`, `haulEach`
//! 86, `outlet` 99, `worthBuying` 125), `caravan/Caravans.java` (`comfortWants` 2207, `pickComforts` 2233, `dispatch`
//! 326–337 and 472–494, `buyForHome` 1868, `networkWants` 2623, `buyBackhaul` 2673, `buySurplus` 2742, `sell` 2431,
//! `boughtThere` 113), `caravan/Commissions.java` `shoppingList` (141) and `market/ShopBoard.java` `wants` (193), `add`
//! (222), `sighted`, `seen`.
//!
//! **Where the core differs.** Money is emerald-worth, not spurs: a shop's lot is about two emeralds' worth and the
//! price isn't rounded to spurs. A town with no stocktake is read as it stands (as `atlas.rs` does). The stand-in good a
//! town lists for a kind it has none of is the first good of the kind in content (the mod's: cookie, book, boots,
//! carpet, pipe tobacco). Not ported: the crown's food reserve in a war (`Sovereign.foodReserve`), players' shops'
//! orders in the network's wants (`Exchange.wanted`, `player-orders`), and crates bought (`Crates`: what's bought comes
//! aboard loose). A Librarian copies the far end's books at `Services`, after these purchases (the mod copies them
//! first), so a stocktake shows the shelf as the Merchant left it.

use std::collections::BTreeMap;

use bevy_ecs::prelude::{Entity, Resource};
use serde::{Deserialize, Serialize};

use crate::atlas::{Books, Stocktake, Stocktakes};
use crate::content::{Content, GoodId};
use crate::messages::TownId;
use crate::people::comfort_par;
use crate::society::Society;
use crate::state::*;

/// A glut is held beyond this many times the par (`Caravans.GLUT`).
pub const GLUT_PARS: f64 = 2.0;
/// An overstocked market sells its glut this much under its worth for each par it holds beyond, at most half
/// (`GLUT_DISCOUNT`, `MOST_GLUT_DISCOUNT`).
pub const GLUT_DISCOUNT: f64 = 0.25;
pub const MOST_GLUT_DISCOUNT: f64 = 0.5;
/// Listings in a town's shop (`ShopBoard.SIZE`), and what a lot of one is worth, about (`ShopBoard.add`: two emeralds).
pub const SHOP_SIZE: usize = 3;
pub const SHOP_LOT_WORTH: f64 = 2.0;
/// Comforts rank above the goods a town's lockers take, in its shop (`ShopBoard.COMFORT_FIRST`).
const COMFORT_FIRST: f64 = 1_000_000.0;
/// A shopping trip worth less than this in emeralds isn't worth the road (`Caravans.dispatch` 333).
pub const SHOPPING_LEAST: f64 = 0.5;

/// A good a town's shop lists: a lot of it at so much each, as news for the Merchants who call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    pub good: GoodId,
    pub lot: f64,
    pub each: f64,
}

/// Each town's shop listings, and the prices wagons last saw listed at a market (`ShopBoard.listings`, `seen`): a wagon
/// calling reads the shop, and what it saw is what Merchants elsewhere think the market would pay.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Listings {
    pub shops: BTreeMap<TownId, Vec<Listing>>,
    pub seen: BTreeMap<TownId, BTreeMap<GoodId, f64>>,
}

impl Listings {
    /// A wagon calls at the market: what its shop lists is news it carries (`ShopBoard.sighted`).
    pub fn sighted(&mut self, market: TownId) {
        let prices = self.shops.get(&market).map(|l| l.iter().map(|l| (l.good, l.each)).collect()).unwrap_or_default();
        self.seen.insert(market, prices);
    }
}

/// What a market is, as the Merchants who deal with it can see (a view over its components).
#[derive(Clone, Copy, Debug)]
pub struct Facts<'a> {
    /// Its villagers, and its par of each kind of comfort in emeralds' worth ([`comfort_par`]).
    pub villagers: f64,
    pub par: f64,
    /// How well each kind was met at its last count, and what its households hold.
    pub met: &'a [f64],
    pub purse: f64,
    /// Its money beyond its reserve (negative when under it).
    pub spare: f64,
    /// What it holds now.
    pub stock: &'a [f64],
}

/// What a market holds of a kind of comfort, in emeralds' worth.
pub fn held_in(content: &Content, stock: &[f64], kind: usize) -> f64 {
    content.goods.iter().enumerate().filter(|(_, g)| g.comfort == Some(kind))
        .map(|(i, g)| stock.get(i).copied().unwrap_or(0.0) * g.value).sum()
}

/// What the market will want of each kind of comfort when a wagon leaving now arrives (at `arrival`), in emeralds'
/// worth (0: none): its `par` less what it held (`stock`, counted at `taken`; none: as it stands, none used) less what
/// its villagers will have used since (`Caravans.comfortWants`).
pub fn comfort_wants(content: &Content, par: f64, villagers: f64, stock: &[f64], taken: Option<u64>, arrival: u64) -> Vec<f64> {
    let days = taken.map_or(0.0, |t| arrival.saturating_sub(t) as f64 / content.day_ticks.max(1) as f64);
    let used = villagers * content.dials.ration * days;
    (0..content.comforts.len()).map(|kind| {
        let held = (held_in(content, stock, kind) - used).max(0.0);
        (par - held).max(0.0)
    }).collect()
}

/// As [`comfort_wants`], from a market's facts and the last stocktake of it a Merchant knows.
pub fn wants_at(content: &Content, taken: Option<&Stocktake>, facts: &Facts, arrival: u64) -> Vec<f64> {
    match taken {
        Some(s) => comfort_wants(content, facts.par, facts.villagers, &s.stock, s.when, arrival),
        None => comfort_wants(content, facts.par, facts.villagers, facts.stock, None, arrival),
    }
}

/// Units of a comfort the market is short of: its shortfall in that kind, at the good's worth (`Desires.shortOf`).
pub fn short_of(content: &Content, wants: &[f64], good: GoodId) -> f64 {
    match content.goods[good].comfort {
        Some(kind) => (wants.get(kind).copied().unwrap_or(0.0) / content.goods[good].value.max(1e-9)).ceil(),
        None => 0.0,
    }
}

/// What a town pays for one of a comfort worth `value`, given how well its kind was `met` (0 to 1), what it's `short`
/// of in that kind (emeralds' worth) and the `money` its households and its spare treasury hold (`Desires.reckon`).
/// Up to twice the worth with none of the kind had; half when it holds its fill; less as its money can't cover what
/// it's short of.
pub fn will_pay(content: &Content, value: f64, met: f64, short: f64, money: f64) -> f64 {
    let d = &content.dials;
    if short <= 0.0 {
        return value * d.desire_glut;
    }
    let eager = 1.0 + (d.desire_most - 1.0) * (1.0 - met.clamp(0.0, 1.0));
    let afford = (money / short).min(1.0);
    value * d.desire_glut.max(eager * afford)
}

/// What a market would pay for one of `good`, reckoned (`Desires.reckon`): a good that's no comfort, or a market with
/// no people, its worth.
pub fn reckon(content: &Content, good: GoodId, facts: &Facts, taken: Option<&Stocktake>, now: u64) -> f64 {
    let value = content.goods[good].value;
    let Some(kind) = content.goods[good].comfort else { return value };
    if facts.villagers <= 0.0 {
        return value;
    }
    let wants = wants_at(content, taken, facts, now);
    will_pay(content, value, facts.met.get(kind).copied().unwrap_or(0.0), wants[kind], facts.purse + facts.spare.max(0.0))
}

/// The road's cost of carrying one of a good on a route of `blocks`, both ways: road food spread over the cargo space
/// (`Desires.haulEach`). `last_crew` and `last_capacity` are what its last wagon had (0: it hasn't left: one mouth and
/// a wagon's slots).
pub fn haul_each(content: &Content, blocks: u32, last_crew: u32, last_capacity: u32, good: GoodId) -> f64 {
    let food = 2.0 * crate::road::food_needed(content, blocks) * last_crew.max(1) as f64;
    let capacity = if last_capacity == 0 { content.dials.wagon_slots } else { last_capacity };
    let per_stack = food * content.dials.purse_per_food_point / capacity.max(1) as f64;
    per_stack / content.goods[good].stack.max(1) as f64
}

/// Where a Merchant could sell a good on, and how much: the best of its routes' far ends (`Desires.Outlet`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outlet {
    pub market: TownId,
    pub each: f64,
    pub units: f64,
}

/// Is a price worth paying to sell on there: the outlet's price, less the margin, covers it (`Desires.worthBuying`)?
pub fn worth_buying(content: &Content, price: f64, outlet: &Outlet) -> bool {
    price <= outlet.each * (1.0 - content.dials.resale_margin)
}

/// A route as the Merchants who run it see it: its ends, its length and what its last wagon had.
#[derive(Clone, Copy, Debug)]
pub struct RouteInfo {
    pub from: TownId,
    pub to: TownId,
    pub to_entity: Entity,
    pub blocks: u32,
    pub last_crew: u32,
    pub last_capacity: u32,
}

impl RouteInfo {
    pub fn of(route: &Route, state: &RouteState) -> Self {
        RouteInfo { from: route.from_id, to: route.to_id, to_entity: route.to, blocks: route.blocks,
            last_crew: state.last_crew, last_capacity: state.last_capacity }
    }
}

/// What a Merchant knows of other markets: its books, the shops' listings its wagons saw and the stocktakes it has.
pub struct Knowledge<'a> {
    pub content: &'a Content,
    pub now: u64,
    pub stocktakes: &'a Stocktakes,
    pub books: &'a Books,
    pub listings: &'a Listings,
}

impl Knowledge<'_> {
    /// What the market at `desk` would pay for one of `good`, as `home`'s Merchant knows it (`Desires.willPay`): the
    /// shop's price a wagon last saw, else the last price its own books show it paid or was paid there, else reckoned.
    pub fn will_pay(&self, home: TownId, desk: TownId, facts: &Facts, good: GoodId) -> f64 {
        if let Some(price) = self.listings.seen.get(&desk).and_then(|m| m.get(&good)) {
            return *price;
        }
        if let Some(price) = self.books.last_price(home, desk, good) {
            return price;
        }
        reckon(self.content, good, facts, self.stocktakes.0.get(&desk), self.now)
    }

    /// The best outlet for a comfort among the markets `home`'s routes supply (other than `except`): what it would
    /// pay, less the road; and how many it's short of in all, across them (`Desires.outlet`).
    pub fn outlet<'a>(&self, routes: &[RouteInfo], home: TownId, except: Option<TownId>, good: GoodId,
            facts: &dyn Fn(Entity) -> Option<Facts<'a>>) -> Option<Outlet> {
        self.content.goods[good].comfort?;
        let (mut best, mut best_each, mut units) = (None, 0.0, 0.0);
        for r in routes {
            if r.from != home || Some(r.to) == except {
                continue;
            }
            let Some(f) = facts(r.to_entity) else { continue };
            let short = short_of(self.content, &wants_at(self.content, self.stocktakes.0.get(&r.to), &f, self.now), good);
            if short <= 0.0 {
                continue;
            }
            units += short;
            let each = self.will_pay(home, r.to, &f, good) - haul_each(self.content, r.blocks, r.last_crew, r.last_capacity, good);
            if each > best_each {
                best_each = each;
                best = Some(r.to);
            }
        }
        best.map(|market| Outlet { market, each: best_each, units })
    }
}

/// What a town sending its own caravan wants it to bring home: each kind of comfort it holds less of than its par, in
/// emeralds' worth (0: none; `Commissions.shoppingList`). A town with no people shops for nobody.
pub fn shopping_list(content: &Content, par: f64, stock: &[f64]) -> Vec<f64> {
    (0..content.comforts.len()).map(|kind| (par - held_in(content, stock, kind)).max(0.0)).collect()
}

/// The comforts to take for what the far end wants (`wants`, emeralds' worth by kind), a little over, the cheapest
/// goods of each kind first, whole items out of `avail` (`Caravans.pickComforts`): good and count.
pub fn pick_comforts(content: &Content, wants: &[f64], avail: &[f64]) -> Vec<(GoodId, f64)> {
    let mut out = Vec::new();
    for (kind, &want) in wants.iter().enumerate() {
        if want <= 0.0 {
            continue;
        }
        let mut goods: Vec<GoodId> = (0..content.goods.len()).filter(|&g| content.goods[g].comfort == Some(kind)).collect();
        goods.sort_by(|&a, &b| content.goods[a].value.total_cmp(&content.goods[b].value));
        let mut worth = 0.0;
        for g in goods {
            let each = content.goods[g].value.max(1e-9);
            let count = ((want * content.dials.comfort_margin - worth) / each).ceil();
            if count <= 0.0 {
                break;
            }
            let n = count.min(avail[g].floor().max(0.0));
            if n > 0.0 {
                worth += n * each;
                out.push((g, n));
            }
        }
    }
    out
}

/// A market's want of a good its lockers or its people take: its a day's capacity, else its use (`Trades.capacity`).
pub fn want_per_day(trades: &Trades, good: GoodId) -> f64 {
    let capacity = trades.capacity.get(good).copied().unwrap_or(0.0);
    if capacity > 0.0 { capacity } else { trades.uses.get(good).copied().unwrap_or(0.0) }
}

/// The shop's listings, most wanted first (`ShopBoard.wants`): each kind of comfort the town is short of and holds
/// none of, at what it would pay, and each good its lockers take that it holds none of, at its worth; each only if
/// the town can pay for a lot. `reckon` is what it'd pay for a good.
pub fn shop_wants(content: &Content, facts: &Facts, trades: &Trades, money: f64, road: bool,
        reckon: impl Fn(GoodId) -> f64) -> Vec<Listing> {
    if !road {
        return Vec::new();
    }
    let shopping = shopping_list(content, facts.par, facts.stock);
    let mut ranked: Vec<(Listing, f64)> = Vec::new();
    let add = |ranked: &mut Vec<(Listing, f64)>, good: GoodId, each: f64, short: f64| {
        if each <= 0.0 {
            return;
        }
        let stack = content.goods[good].stack.max(1) as f64;
        let lot = (SHOP_LOT_WORTH / each).ceil().min(stack).max(1.0);
        if money < each * lot {
            return; // it couldn't pay for a lot: no honest listing
        }
        ranked.push((Listing { good, lot, each }, short));
    };
    for (kind, &want) in shopping.iter().enumerate() {
        if want <= 0.0 || held_in(content, facts.stock, kind) > 0.0 {
            continue; // it has some: the kind isn't missing
        }
        let Some(good) = (0..content.goods.len()).find(|&g| content.goods[g].comfort == Some(kind)) else { continue };
        add(&mut ranked, good, reckon(good), COMFORT_FIRST + want);
    }
    for good in 0..content.goods.len() {
        let wanted = trades.capacity.get(good).is_some_and(|c| *c > 0.0);
        if wanted && facts.stock.get(good).copied().unwrap_or(0.0) < 1.0 && content.goods[good].comfort.is_none() {
            let value = content.goods[good].value;
            add(&mut ranked, good, value, value * 64.0);
        }
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked.into_iter().map(|(l, _)| l).take(SHOP_SIZE).collect()
}

/// A town's components as the Merchants who deal with it can see them. (An owned copy, for a host.)
#[derive(Clone, Debug, Default)]
pub struct OwnedFacts {
    pub villagers: f64,
    pub par: f64,
    pub met: Vec<f64>,
    pub purse: f64,
    pub spare: f64,
    pub stock: Vec<f64>,
}

impl OwnedFacts {
    pub fn view(&self) -> Facts<'_> {
        Facts { villagers: self.villagers, par: self.par, met: &self.met, purse: self.purse, spare: self.spare, stock: &self.stock }
    }
}

impl Society {
    /// A town's facts, as the Merchants who deal with it see them.
    pub(crate) fn facts(&self, town: TownId) -> OwnedFacts {
        let e = self.world.entity(self.towns[town]);
        let people = e.get::<People>().expect("a town");
        let trades = e.get::<Trades>().expect("a town");
        let content = self.content();
        OwnedFacts {
            villagers: people.villagers,
            par: comfort_par(content, people),
            met: people.comfort_met.clone(),
            purse: people.purse,
            spare: e.get::<Money>().expect("a town").0 - crate::rules::farms::reserve(content, trades, e.get::<crate::farms::Farm>()),
            stock: e.get::<Stock>().expect("a town").0.clone(),
        }
    }

    /// What a market will want of each kind of comfort when a wagon arrives at `arrival`, in emeralds' worth (0: none):
    /// its par less what its last stocktake held, less what its villagers will have used since
    /// (`Caravans.comfortWants`); a market never counted, as it stands.
    pub fn comfort_wants(&self, town: TownId, arrival: u64) -> Vec<f64> {
        let facts = self.facts(town);
        wants_at(self.content(), self.world.resource::<Stocktakes>().0.get(&town), &facts.view(), arrival)
    }

    /// What the market at `desk` would pay for one of `good` now, as `home`'s Merchant knows it (`Desires.willPay`).
    pub fn will_pay(&self, home: TownId, desk: TownId, good: GoodId) -> f64 {
        let facts = self.facts(desk);
        self.knowledge(|k| k.will_pay(home, desk, &facts.view(), good))
    }

    /// The best outlet for a comfort among the markets `home`'s routes supply, other than `except` (`Desires.outlet`).
    pub fn outlet(&self, home: TownId, except: Option<TownId>, good: GoodId) -> Option<Outlet> {
        let routes = self.route_infos();
        let facts: BTreeMap<Entity, OwnedFacts> = self.towns.iter().enumerate().map(|(t, &e)| (e, self.facts(t))).collect();
        self.knowledge(|k| k.outlet(&routes, home, except, good, &|e| facts.get(&e).map(|f| f.view())))
    }

    /// What the market's shop lists now, most wanted first (`ShopBoard.listings`).
    pub fn listings(&self, town: TownId) -> Vec<Listing> {
        self.world.resource::<Listings>().shops.get(&town).cloned().unwrap_or_default()
    }

    /// The price a wagon last saw listed at a market for a good, in emeralds each (`ShopBoard.seen`).
    pub fn seen_price(&self, town: TownId, good: GoodId) -> Option<f64> {
        self.world.resource::<Listings>().seen.get(&town).and_then(|m| m.get(&good)).copied()
    }

    /// Items a route's wagons have bought at the far end, the emeralds paid, and of the items what was a backhaul
    /// (`Route.itemsBought`, `purchaseSpurs`, `backhaulItems`).
    pub fn bought(&self, route: crate::messages::RouteId) -> (f64, f64, f64) {
        let s = self.world.get::<RouteState>(self.routes[route]).expect("a route");
        (s.bought, s.purchases, s.backhauled)
    }

    /// What a route's wagon on the road carries loose, if it's out.
    pub fn wagon_cargo(&self, route: crate::messages::RouteId) -> Option<Vec<f64>> {
        self.world.get::<RouteState>(self.routes[route]).and_then(|s| s.wagon.as_ref()).map(|w| w.cargo.clone())
    }

    /// What a route's wagon on the road is to shop for at the far end, by kind of comfort, and what it carries.
    pub fn wagon_shopping(&self, route: crate::messages::RouteId) -> Option<Vec<f64>> {
        self.world.get::<RouteState>(self.routes[route]).and_then(|s| s.wagon.as_ref()).map(|w| w.shopping.clone())
    }

    /// The routes as the Merchants who run them see them.
    pub(crate) fn route_infos(&self) -> Vec<RouteInfo> {
        self.routes.iter().filter_map(|&e| {
            let r = self.world.get::<Route>(e)?;
            Some(RouteInfo::of(r, self.world.get::<RouteState>(e)?))
        }).collect()
    }

    fn knowledge<T>(&self, f: impl FnOnce(&Knowledge) -> T) -> T {
        f(&Knowledge {
            content: self.content(),
            now: self.now(),
            stocktakes: self.world.resource::<Stocktakes>(),
            books: self.world.resource::<Books>(),
            listings: self.world.resource::<Listings>(),
        })
    }
}
