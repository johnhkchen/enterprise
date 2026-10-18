//! How a player's shop reaches the society (`market/Broker.java`, `Exchange.java`, `Shops.java`; `docs/decisions.md`
//! "The master Merchant and players' shops"). The shops are the player's, so they live outside the society: the host
//! tells the core about them and the core's master Merchants (level 5, `rules/orders.rs`) deal with them each look.
//!
//! - **A buying shop is a standing order** ([`Order`], `Broker.fill`, `contractFor`): a lot of a good at a price in
//!   spurs, paid from the shop's purse. The Merchant drafts a contract with it (Ex Works: the shop is at the seller's
//!   market, so it's in force at once) of as many daily lots as the purse pays for, up to `shop_installments`, at the
//!   shop's price, if that's at least `agent_floor` of the goods' worth. Each lot is delivered as it falls due from
//!   what the warehouse can spare beyond its par, and paid from the purse. What it can't fill stays `open`: the region
//!   works on it (a wagon bound for the desk brings it, `rules/plan.rs`; the desk's own wagons buy it up as surplus at
//!   the far end, `rules/desires.rs`), and if no market it knows holds it and no stall here sells it, it's posted as a
//!   signal ([`Signals`]; the desk's one list).
//! - **A selling shop is supply** ([`Stall`], `Broker.buy`): the Merchant buys from it what its orders and its lockers
//!   want, at up to the goods' worth (`maker_ceiling`), or what a signal offers for a posted want.
//! - **The town's own shop** ([`OwnShop`], `Broker.sellSurplus`): the Merchant stocks it with the town's biggest
//!   surplus (no food, nothing ordered) in lots of `own_lot` at `own_markup` over worth, a stack on the shelf, and
//!   banks the takings. A player buys from it with [`Society::buy`].
//!
//! **Where the core differs.**
//! - Contracts are between towns, so a shop's contract names the desk's town on both sides (the shop is at its market,
//!   and the purse is the player's, outside the society: what it pays comes in from the outside).
//! - Trust is per shop, as `contractFor` keys it by the desk and the shop's position: each [`Order`] keeps its own record
//!   of installments kept and missed, taken in from how its contract was judged, and the `MIN_TRUST` check reads that,
//!   never the desk's pooled record in the book.
//! - A far end's order adds its good to the wanted set; the room it gives is `Warehouse.par(to, item, true)` (a stack at
//!   least) less what's held, and a good it wants anyway keeps its usual shelf (`rules/plan.rs`, `rules/arrive.rs`).
//! - `source` for what the desk can't fill is sourcing's (`rules/sourcing.rs`): the shop's contract is a seller contract
//!   of the desk's, which sourcing reads for its wants, and the largest of the desk's open orders for a good stands
//!   beside them (`remaining - spare` as the mod asks shop by shop, where sourcing nets the whole stock). Sourcing skips a good the desk's own people make or sell
//!   (their Merchant buys of them), even for an order; the mod doesn't.
//! - The desk's signals are one list, and the only stream to the jobs (see `rules/orders.rs`). The war's unmet wants
//!   aren't here (the crown's); sourcing's unmet contract wants stand in their place.
//! - Which goods the desk wants (`wantedAt`) is what its lockers take and the inputs to what it has contracted to sell,
//!   not the mod's workshop conversions or a realm's wants. Spare stock also leaves what the workshop sets aside.
//! - `Shops.roomFor`, enchanted-goods commissions and the metal and ore chains of `Jobs.tradeFor` are not ported: the
//!   first two have no counterpart in the core. Spurs are the host's coin: money in the core is emeralds, at the mint's rate.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::messages::{Input, TownId};
use crate::society::Society;

/// The Merchant's level at which it brokers (`Broker.MASTER`).
pub const MASTER: u8 = 5;

/// A player's buying shop: it wants `lot` of `good` at a time at `price` spurs a lot, paying from `purse` (spurs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: usize,
    pub town: TownId,
    pub good: GoodId,
    pub lot: f64,
    pub price: f64,
    pub purse: f64,
    /// What the desk couldn't fill of the contract's remaining lots at its last look: the open order the region works on.
    pub open: f64,
    /// The contract with the desk in force (or last drafted).
    pub contract: Option<usize>,
    /// Goods delivered to the shop so far, and the spurs paid for them.
    pub delivered: f64,
    pub paid: f64,
    /// This shop's own record of installments kept and missed (`Broker.contractFor` keys trust by the desk and the
    /// shop's position): the trust a contract with it needs is read here, never from the desk's pooled record.
    #[serde(default)]
    pub record: crate::contracts::Record,
    /// How much of the contract's judging the record has taken in: installments judged, and misses counted.
    #[serde(default)]
    pub seen: (u32, u32),
}

/// A player's selling shop: `stock` of `good` for sale in lots of `lot` at `price` spurs a lot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stall {
    pub id: usize,
    pub town: TownId,
    pub good: GoodId,
    pub lot: f64,
    pub price: f64,
    pub stock: f64,
}

impl Stall {
    /// Lots on the shelf (`Shops.lots`).
    pub fn lots(&self) -> f64 {
        if self.lot > 0.0 { (self.stock / self.lot).floor() } else { 0.0 }
    }
}

/// A shop touching the desk: the town's own (`Shops.own`). Its Merchant lists a surplus good on it, and a player's
/// purchases drop `takings` (spurs) in its coin box until the Merchant banks them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OwnShop {
    pub id: usize,
    pub town: TownId,
    pub good: Option<GoodId>,
    pub lot: f64,
    pub price: f64,
    /// Goods on its shelf: out of the warehouse, not yet bought.
    pub shelf: f64,
    pub takings: f64,
}

/// What a desk wants that no market it knows holds, and the premium it will pay (`Exchange.Signal`): work for players
/// to bring from another region or make.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Signal {
    pub good: GoodId,
    pub count: f64,
    /// Spurs it will pay for each.
    pub spurs_each: f64,
    pub why: String,
}

/// A desk's trade with the shops, tallied (`Exchange.Tally`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tally {
    pub lots_sold: f64,
    pub spurs_from_sales: f64,
    pub lots_bought: f64,
    pub spurs_on_purchases: f64,
    pub spurs_from_own_shops: f64,
}

/// Every shop the host has told the core of, what each desk wants because of them, and its tally.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Orders {
    pub next: usize,
    pub orders: Vec<Order>,
    pub stalls: Vec<Stall>,
    pub shops: Vec<OwnShop>,
    /// The open orders at each desk by good, as of its Merchant's last look (`Exchange.wanted`): what wagons bound for
    /// the desk bring, and what its own wagons buy up at the far end.
    pub wanted: BTreeMap<TownId, BTreeMap<GoodId, f64>>,
    pub tally: BTreeMap<TownId, Tally>,
}

impl Orders {
    pub fn is_empty(&self) -> bool {
        self.orders.is_empty() && self.stalls.is_empty() && self.shops.is_empty()
    }

    /// What a desk's open orders still want of a good.
    pub fn wanted_of(&self, town: TownId, good: GoodId) -> f64 {
        self.wanted.get(&town).and_then(|w| w.get(&good)).copied().unwrap_or(0.0)
    }
}

/// What each desk has posted (`Exchange.signals`), the biggest first, and whether any desk's list changed since the
/// jobs' marbles were last made from them.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Signals {
    pub desks: BTreeMap<TownId, Vec<Signal>>,
    #[serde(default)]
    pub changed: bool,
}

/// The spurs a lot of a good is worth at the mint's `rate` (spurs an emerald): the filter an order's price is held to.
pub fn worth_of(content: &Content, good: GoodId, lot: f64, rate: f64) -> f64 {
    content.goods[good].value * lot * rate
}

// The host's calls for the shops.
impl Society {
    /// A player's buying shop at a town's desk: `lot` of `good` at `price` spurs, with `purse` spurs to pay with.
    /// Returns the order's number.
    pub fn order(&mut self, town: TownId, good: GoodId, lot: f64, price: f64, purse: f64) -> usize {
        assert!(town < self.towns.len() && good < self.content().goods.len(), "an order for a town and good that exist");
        let mut orders = self.world.resource_mut::<Orders>();
        let id = orders.next;
        orders.next += 1;
        orders.orders.push(Order { id, town, good, lot: lot.max(1.0).floor(), price: price.max(0.0).floor(),
            purse: purse.max(0.0).floor(), ..Default::default() });
        id
    }

    /// Spurs more in a buying shop's purse.
    pub fn fund_order(&mut self, id: usize, spurs: f64) {
        if let Some(o) = self.world.resource_mut::<Orders>().orders.iter_mut().find(|o| o.id == id) {
            o.purse += spurs.max(0.0).floor();
        }
    }

    /// A player takes their buying shop down: its contract closes and its open order goes.
    pub fn cancel_order(&mut self, id: usize) {
        let mut orders = self.world.resource_mut::<Orders>();
        let Some(i) = orders.orders.iter().position(|o| o.id == id) else { return };
        let order = orders.orders.remove(i);
        // Its good's signal comes down with it: the next look posts it again if the lockers still want it.
        let others = orders.orders.iter().any(|o| o.town == order.town && o.good == order.good);
        if !others {
            let mut posted = self.world.resource_mut::<Signals>();
            if let Some(list) = posted.desks.get_mut(&order.town) {
                list.retain(|s| s.good != order.good);
                if list.is_empty() {
                    posted.desks.remove(&order.town);
                }
                posted.changed = true;
            }
        }
        let mut orders = self.world.resource_mut::<Orders>();
        if let Some(wanted) = orders.wanted.get_mut(&order.town) {
            if let Some(w) = wanted.get_mut(&order.good) {
                *w = (*w - order.open).max(0.0);
            }
            wanted.retain(|_, w| *w > 0.0);
        }
        if let Some(c) = order.contract {
            self.world.resource_mut::<crate::contracts::Book>().close(c);
        }
    }

    /// A player's selling shop at a town's desk: `stock` of `good` in lots of `lot` at `price` spurs a lot.
    pub fn stall(&mut self, town: TownId, good: GoodId, lot: f64, price: f64, stock: f64) -> usize {
        assert!(town < self.towns.len() && good < self.content().goods.len(), "a stall for a town and good that exist");
        let mut orders = self.world.resource_mut::<Orders>();
        let id = orders.next;
        orders.next += 1;
        orders.stalls.push(Stall { id, town, good, lot: lot.max(1.0).floor(), price: price.max(0.0).floor(),
            stock: stock.max(0.0).floor() });
        id
    }

    /// More goods on a selling shop's shelf.
    pub fn restock_stall(&mut self, id: usize, count: f64) {
        if let Some(s) = self.world.resource_mut::<Orders>().stalls.iter_mut().find(|s| s.id == id) {
            s.stock += count.max(0.0).floor();
        }
    }

    /// A shop touching a town's desk: the town's own, for its Merchant to stock. Returns its number.
    pub fn own_shop(&mut self, town: TownId) -> usize {
        assert!(town < self.towns.len(), "a town that exists");
        let mut orders = self.world.resource_mut::<Orders>();
        let id = orders.next;
        orders.next += 1;
        orders.shops.push(OwnShop { id, town, ..Default::default() });
        id
    }

    /// A player buys `count` of `good` from a town's own shop that lists it, in its whole lots (a vendor sells a lot at
    /// a time: what's short of one is left) at the price it lists: the goods leave its shelf and the spurs drop in its
    /// coin box for the Merchant to bank. Returns the spurs paid.
    pub fn buy(&mut self, town: TownId, good: GoodId, count: f64) -> Result<f64, String> {
        let orders = self.world.resource::<Orders>();
        let Some(i) = orders.shops.iter().position(|s| s.town == town && s.good == Some(good) && s.lot >= 1.0
            && (count / s.lot).floor() >= 1.0 && s.shelf >= (count / s.lot).floor() * s.lot) else {
            return Err(format!("no shop of town {town} has a whole lot of {count} of good {good} on its shelf"));
        };
        let s = &mut self.world.resource_mut::<Orders>().shops[i];
        let lots = (count / s.lot).floor();
        let (count, spurs) = (lots * s.lot, lots * s.price);
        s.shelf -= count;
        s.takings += spurs;
        self.world.resource_mut::<crate::state::Ledger>().used[good] += count;
        Ok(spurs)
    }

    /// The buying shops at a town's desk, as the Merchant last saw them.
    pub fn orders(&self, town: TownId) -> Vec<Order> {
        self.world.resource::<Orders>().orders.iter().filter(|o| o.town == town).cloned().collect()
    }

    /// The selling shops at a town's desk.
    pub fn stalls(&self, town: TownId) -> Vec<Stall> {
        self.world.resource::<Orders>().stalls.iter().filter(|s| s.town == town).cloned().collect()
    }

    /// The town's own shops.
    pub fn own_shops(&self, town: TownId) -> Vec<OwnShop> {
        self.world.resource::<Orders>().shops.iter().filter(|s| s.town == town).cloned().collect()
    }

    /// What a desk's open orders still want, by good (`Exchange.wanted`).
    pub fn order_wants(&self, town: TownId) -> BTreeMap<GoodId, f64> {
        self.world.resource::<Orders>().wanted.get(&town).cloned().unwrap_or_default()
    }

    /// What a desk has posted as wanted that no market it knows holds (`Exchange.signals`), biggest first.
    pub fn posted_signals(&self, town: TownId) -> Vec<Signal> {
        self.world.resource::<Signals>().desks.get(&town).cloned().unwrap_or_default()
    }

    /// A desk's trade with the shops so far.
    pub fn tally(&self, town: TownId) -> Tally {
        self.world.resource::<Orders>().tally.get(&town).copied().unwrap_or_default()
    }

    /// Applies a shop happening; false if the input isn't one.
    pub(crate) fn apply_shop(&mut self, input: &Input) -> bool {
        match *input {
            Input::Order { town, good, lot, price, purse } => {
                self.order(town as TownId, good as GoodId, lot, price, purse);
            }
            Input::Stall { town, good, lot, price, stock } => {
                self.stall(town as TownId, good as GoodId, lot, price, stock);
            }
            Input::OwnShop { town } => {
                self.own_shop(town as TownId);
            }
            Input::Buy { town, good, count } => {
                let _ = self.buy(town as TownId, good as GoodId, count);
            }
            _ => return false,
        }
        true
    }
}
