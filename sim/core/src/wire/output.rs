//! Outcomes out: what happened in the society, for the host to show.

use serde::{Deserialize, Serialize};

use super::{GoodNo, PackageLine, Place, RouteNo, TownNo};
use crate::fixed::Fixed;

/// What happened, for the host to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Output {
    WagonLeft { route: RouteNo, stacks: u32, worth: f64 },
    WagonSold { route: RouteNo, sold: f64, takings: Fixed },
    WagonHome { route: RouteNo, takings: Fixed },
    /// A master Merchant filled a player's order: `lots` lots, `count` goods, for `spurs`.
    OrderFilled { town: TownNo, order: u32, good: GoodNo, lots: u32, count: f64, spurs: f64 },
    /// A desk newly posted a want no market it knows holds, at `spurs_each` an item.
    Signal { town: TownNo, good: GoodNo, count: f64, spurs_each: f64 },
    /// A pallet was packed at `at`, of these packages in order: the host shows its crates there.
    PalletPacked { id: u64, at: Place, packages: Vec<PackageLine> },
    /// A pallet went to `to` (loaded, carried, delivered to a hold): its crates move with it.
    PalletMoved { id: u64, to: Place },
    /// A pallet was opened at `at` (by its consignee, or taken apart by its holder): its crates are empty, and it is gone.
    PalletOpened { id: u64, at: Place },
}
