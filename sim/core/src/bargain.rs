//! Two Merchants settling a price, and the warehouse's par: what a wagon's goods sell for at the far end.
//!
//! The seller won't go below half a good's local value; the buyer pays up to the full value with an empty shelf,
//! scaling down to half at par (no deal at or above par). The price lands in that gap by level: the seller takes its
//! level / (both levels) of it (`docs/decisions.md`, "Merchants bargain by level"). The travelling Merchant's level is
//! its home's Merchant's. Ported from:
//!
//! - `money/Bargain.java`: `SELLER_FLOOR`, `sellerFloor`, `buyerCeiling`, `shortage`, `between` (29), `price`.
//! - `merchant/Warehouse.java` `par` (138), `capacity` (113), `recordCapacity` (78), `MIN_PAR`, `PAR_DAYS`: a good's
//!   par is `shelf_days` of what the town's lockers can take of it a day, at least a stack when a switched-on trade
//!   takes it. Capacity, not past use: a town short of stock would otherwise learn a small par and stay short. Each
//!   locker's session replaces that locker's figures ([`LockerCapacity`], `Society::record_capacity`).
//! - `caravan/Caravans.java` `sell` (2431): each good priced at the middle of the batch (the buyer's shortage shrinks
//!   as its shelf fills), twice over as what it can pay cuts the batch, the seller at its home Merchant's level, the
//!   buyer at its own (`rules/arrive.rs::sell`); `buyForHome` (1889: the ceiling the full value) and `buySurplus`
//!   (2778: the ceiling [`RESALE_CEILING`] of it) are [`between`] with other ceilings, for the slices that buy.
//!
//! Behind the dial `bargain` (on by default): off, every good sells at its full value, as before.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::roster::Roster;
use crate::society::Society;
use crate::state::Trades;

/// No Merchant sells below this share of a good's local value (`Bargain.SELLER_FLOOR`).
pub const SELLER_FLOOR: f64 = 0.5;
/// A good a switched-on trade takes has at least this par: a stack (`Warehouse.MIN_PAR`).
pub const MIN_PAR: f64 = 64.0;
/// Days of what its lockers actually sell a town's trading capital is (`Warehouse.PAR_DAYS`).
pub const PAR_DAYS: f64 = 2.0;
/// A travelling Merchant buying surplus to sell on pays at most this share of its local value
/// (`Caravans.RESALE_CEILING`).
pub const RESALE_CEILING: f64 = 0.75;
/// A buyer is as eager as its mood lets it be with this much money to spare (a founding purse).
pub const EAGER_PURSE: f64 = 64.0;

/// How far over its usual ceiling a buyer goes: 1 for a content town or one with nothing to spare, up to the dial
/// `eager_most` for one as unhappy as a mood of -1 or worse with [`EAGER_PURSE`] to spare. Scarce trade lets a poor,
/// unhappy town's emeralds pile up until a wagon's road pays (John: "You'd expect the towns to start paying more for
/// goods if they have low happiness and extra income").
pub fn eagerness(content: &Content, mood: f64, spare: f64) -> f64 {
    let unhappy = (-mood).clamp(0.0, 1.0);
    let afford = (spare / EAGER_PURSE).clamp(0.0, 1.0);
    1.0 + (content.dials.eager_most - 1.0).max(0.0) * unhappy * afford
}

/// The least a seller takes for a good worth `value`.
pub fn seller_floor(value: f64) -> f64 {
    value * SELLER_FLOOR
}

/// The most a buyer pays for a good worth `value`, given how short of par it is (0 at par, 1 with nothing on hand).
pub fn buyer_ceiling(value: f64, shortage: f64) -> f64 {
    value * (SELLER_FLOOR + (1.0 - SELLER_FLOOR) * shortage.clamp(0.0, 1.0))
}

/// Shortage below par: 1 with nothing on hand, 0 at or above par (and 0 with no par).
pub fn shortage(on_hand: f64, par: f64) -> f64 {
    if par <= 0.0 { 0.0 } else { (1.0 - on_hand / par).max(0.0) }
}

/// The price between a seller's floor and a buyer's ceiling, split by level (each at least 1); none if there's no gap.
pub fn between(floor: f64, ceiling: f64, seller_level: u8, buyer_level: u8) -> Option<f64> {
    if ceiling <= floor {
        return None;
    }
    let (s, b) = (seller_level.max(1) as f64, buyer_level.max(1) as f64);
    Some(floor + (ceiling - floor) * (s / (s + b)))
}

/// The settled price of a good worth `value` to a buyer this short of par; none if its ceiling doesn't clear the
/// seller's floor.
pub fn price(value: f64, shortage: f64, seller_level: u8, buyer_level: u8) -> Option<f64> {
    between(seller_floor(value), buyer_ceiling(value, shortage), seller_level, buyer_level)
}

/// How many of a good the warehouse aims to hold: `shelf_days` of what its lockers can take a day, rounded up, and at
/// least [`MIN_PAR`] when it's `wanted` (a switched-on trade takes it).
pub fn par(content: &Content, capacity_per_day: f64, wanted: bool) -> f64 {
    let days = (capacity_per_day * content.dials.shelf_days).ceil();
    if wanted { days.max(MIN_PAR) } else { days }
}

/// A town's Merchant's level for bargaining: 1 with no roster or no Merchant.
pub fn merchant_level(roster: Option<&Roster>) -> u8 {
    roster.and_then(|r| r.merchant_level()).unwrap_or(1)
}

/// What a good sells for at the far end, a batch of `n` onto `on_hand` toward `par`: the price at the middle of the
/// batch, the buyer's ceiling raised by its `eager`ness ([`eagerness`]: 1, none), or its full value (times `eager`)
/// with the dial off. None: no deal.
#[allow(clippy::too_many_arguments)]
pub fn unit_price(content: &Content, good: GoodId, on_hand: f64, n: f64, par: f64, seller: u8, buyer: u8, eager: f64)
        -> Option<f64> {
    let value = content.goods[good].value;
    if !content.dials.bargain {
        return Some(value * eager);
    }
    let ceiling = buyer_ceiling(value, shortage(on_hand + (n / 2.0).floor(), par)) * eager;
    between(seller_floor(value), ceiling, seller, buyer)
}

/// What each of a town's lockers can take of each good a day, as its last session recorded it, by locker
/// (`Warehouse.capacity`): their sums are the town's [`Trades::capacity`].
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct LockerCapacity(pub BTreeMap<u64, Vec<f64>>);

impl Society {
    /// A locker's session: what its switched-on trades can take of each good a day, replacing what it recorded last
    /// (`Warehouse.recordCapacity`); goods it doesn't name, nothing. The town's capacity is every locker's, summed.
    pub fn record_capacity(&mut self, town: crate::TownId, locker: u64, per_day: &[(GoodId, f64)]) {
        let n = self.content().goods.len();
        let e = self.towns[town];
        let mut figures = vec![0.0; n];
        for &(g, count) in per_day {
            figures[g] += count.max(0.0);
        }
        // Every town has the component from the start (`add_town`), so recording one never moves a town to another
        // archetype: the towns' order in the look's queries stays as a loaded world's.
        let mut lockers = self.world.get_mut::<LockerCapacity>(e).expect("a town");
        lockers.0.insert(locker, figures);
        let total: Vec<f64> = (0..n).map(|g| lockers.0.values().map(|f| f[g]).sum()).collect();
        self.world.get_mut::<Trades>(e).expect("a town").capacity = total;
    }

    /// What a town's lockers can take of a good a day (`Warehouse.capacity`): its lockers' sum since one recorded
    /// it, else what its people use (as `set_trade` set it).
    pub fn capacity(&self, town: crate::TownId, good: GoodId) -> f64 {
        let trades = self.world.get::<Trades>(self.towns[town]).expect("a town");
        trades.capacity.get(good).copied().unwrap_or(0.0)
    }

    /// The town's par for a good (`Warehouse.par`): `shelf_days` of its capacity, at least a stack when `wanted`.
    pub fn par(&self, town: crate::TownId, good: GoodId, wanted: bool) -> f64 {
        par(self.content(), self.capacity(town, good), wanted)
    }
}
