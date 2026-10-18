//! Happenings in: what the host's game did since the last look, for the core to book.

use serde::{Deserialize, Serialize};

use super::{GoodNo, Point, TownNo};
use crate::fixed::Fixed;

/// A happening the host hands the core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Input {
    /// Time has passed, to this tick.
    Advance { to: u64 },
    /// Goods handed to a town from outside the society (a player's delivery).
    Deliver { town: TownNo, good: GoodNo, count: f64 },
    /// Money paid a town from outside the society (a player buying at its shops).
    Pay { town: TownNo, money: Fixed },
    /// A player's buying shop at a town's desk (`orders.rs`): `lot` of `good` at `price` spurs a lot, paid from `purse`.
    Order { town: TownNo, good: GoodNo, lot: f64, price: f64, purse: f64 },
    /// A player's selling shop at a town's desk: `stock` of `good` in lots of `lot` at `price` spurs a lot.
    Stall { town: TownNo, good: GoodNo, lot: f64, price: f64, stock: f64 },
    /// A shop touching a town's desk, the town's own for its Merchant to stock.
    OwnShop { town: TownNo },
    /// A player buys `count` of `good` from a town's own shop, at the price it lists.
    Buy { town: TownNo, good: GoodNo, count: f64 },
    /// A new town at its desk (a Merchant's desk placed): it stands where its desk does, with nothing held and no money
    /// (money comes in by `Pay`). It's numbered after the towns there are, and its desk is a point of its own.
    AddTown { name: String, desk: Point },
    /// A trade point of a town's (a locker, a shop, a banker, a depot, another desk) placed.
    AddPoint { point: Point, town: TownNo },
    /// A trade point broken. Its town stays, and keeps what it held.
    RemovePoint { point: Point },
    /// What a point's chests hold now, every good there (a good not listed holds none): the core books the difference
    /// from its last stocktake there as goods delivered to its town or taken from it (`points.rs`).
    Stocktake { point: Point, counts: Vec<Count> },
    /// A player carried a whole pallet (every package of it) to a town's desk (`PalletBrought`, `pallet_messages.rs`).
    /// Made out to the town, it is received there and then (its goods into stock, a contract's goods delivered under
    /// it, the crates kept); made out elsewhere, it waits in the town's warehouse to be fetched. A pallet that isn't
    /// in a town's warehouse (already moved, or on a wagon) is ignored: the host's view was stale.
    PalletBrought { town: TownNo, id: u64 },
    /// A player carried one package of a pallet made out to the town to its desk: `package` is the package's place on
    /// the pallet (`PalletPacked` lists them in order). The pallet is received when every package has come, or at the
    /// town's next look with those that have, its seal broken and the rest lost. Ignored for a pallet made out to
    /// another town.
    PackageBrought { town: TownNo, pallet: u64, package: u32 },
    /// A package of a pallet was lost (raided, broken, thrown in lava): it leaves the world (its goods booked as used,
    /// reason lost), the pallet's seal is broken and it goes on short; the packages after it move up one place. A pallet
    /// left with nothing is gone, and a bill of exchange that paid for it is unbacked.
    PackageLost { pallet: u64, package: u32 },
}

/// So many of a good.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Count {
    pub good: GoodNo,
    pub count: f64,
}
