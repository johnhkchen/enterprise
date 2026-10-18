//! Happenings in: what the host's game did since the last look, for the core to book.

use serde::{Deserialize, Serialize};

use super::{GoodId, Point, TownId};
use crate::fixed::Fixed;

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
    /// A new town at its desk (a Merchant's desk placed): it stands where its desk does, with nothing held and no money
    /// (money comes in by `Pay`). It's numbered after the towns there are, and its desk is a point of its own.
    AddTown { name: String, desk: Point },
    /// A trade point of a town's (a locker, a shop, a banker, a depot, another desk) placed.
    AddPoint { point: Point, town: TownId },
    /// A trade point broken. Its town stays, and keeps what it held.
    RemovePoint { point: Point },
    /// What a point's chests hold now, every good there (a good not listed holds none): the core books the difference
    /// from its last stocktake there as goods delivered to its town or taken from it (`points.rs`).
    Stocktake { point: Point, counts: Vec<Count> },
}

/// So many of a good.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Count {
    pub good: GoodId,
    pub count: f64,
}
