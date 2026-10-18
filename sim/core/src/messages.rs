//! The core's whole interface: happenings in, results out. The same messages whether the core runs inside a game,
//! beside it or in a test, so a run's messages can be logged and replayed. Towns and routes go by their number, in
//! the order they were added: stable across saves and hosts, unlike the ECS's own entity ids.

use serde::{Deserialize, Serialize};
use crate::content::GoodId;
use crate::fixed::Fixed;

pub type TownId = usize;
pub type RouteId = usize;

/// A happening the host hands the core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Input {
    /// Time has passed, to this tick.
    Advance { to: u64 },
    /// Goods handed to a town from outside the society (a player's delivery).
    Deliver { town: TownId, good: GoodId, count: f64 },
    /// Money paid a town from outside the society (a player buying at its shops).
    Pay { town: TownId, money: Fixed },
    /// A player's buying shop at a town's desk (`orders.rs`): `lot` of `good` at `price` spurs a lot, paid from `purse`.
    Order { town: TownId, good: GoodId, lot: f64, price: f64, purse: f64 },
    /// A player's selling shop at a town's desk: `stock` of `good` in lots of `lot` at `price` spurs a lot.
    Stall { town: TownId, good: GoodId, lot: f64, price: f64, stock: f64 },
    /// A shop touching a town's desk, the town's own for its Merchant to stock.
    OwnShop { town: TownId },
    /// A player buys `count` of `good` from a town's own shop, at the price it lists.
    Buy { town: TownId, good: GoodId, count: f64 },
}

/// What happened, for the host to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Output {
    WagonLeft { route: RouteId, stacks: u32, worth: f64 },
    WagonSold { route: RouteId, sold: f64, takings: Fixed },
    WagonHome { route: RouteId, takings: Fixed },
    /// A master Merchant filled a player's order: `lots` lots, `count` goods, for `spurs`.
    OrderFilled { town: TownId, order: usize, good: GoodId, lots: u32, count: f64, spurs: f64 },
    /// A desk newly posted a want no market it knows holds, at `spurs_each` an item.
    Signal { town: TownId, good: GoodId, count: f64, spurs_each: f64 },
}
