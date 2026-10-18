//! A pallet as the host sees it (`docs/pallets.md` section 4): where it is, and what it holds.

use serde::{Deserialize, Serialize};

use super::{GoodId, RouteId, TownId};

/// Where a pallet is: the core's `pallet::At`, as numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    /// In a town's own premises (its warehouse: where the host shows its crates).
    Town { town: TownId },
    /// Held in a hub's custody for others.
    Custody { town: TownId },
    /// On a route's wagon: a record, nothing to show but its manifest.
    Wagon { route: RouteId },
    /// In a consolidation of pallets for one run.
    Container { container: u64 },
}

/// One package on a pallet: a crate, a barrel, a bundle or loose stacks (`kind` is its place in the content's packages,
/// 0 loose), so many of one good.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackageLine {
    pub kind: u32,
    pub good: GoodId,
    pub count: f64,
}
