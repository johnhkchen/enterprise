//! A pallet as the host sees it (`docs/pallets.md` section 4): where it is, and what it holds.

use serde::{Deserialize, Serialize};

use super::{GoodNo, Point, RouteNo, TownNo};

/// Where a pallet is: the core's `pallet::At`, as numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    /// In a town's own premises (its warehouse: where the host shows its crates).
    Town { town: TownNo },
    /// Held in a hub's custody for others.
    Custody { town: TownNo },
    /// On a route's wagon: a record, nothing to show but its manifest.
    Wagon { route: RouteNo },
    /// In a consolidation of pallets for one run.
    Container { container: u64 },
}

/// One package on a pallet: a crate, a barrel, a bundle or loose stacks (`kind` is its place in the content's packages,
/// 0 loose), so many of one good.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackageLine {
    pub kind: u32,
    pub good: GoodNo,
    pub count: f64,
}

/// A pallet as a host shows and lists it (`pallet_host.rs`): its id, where it is and since when (the tick it came there),
/// its bill of lading (who ships it and to whom, by number and by name, the contract it goes under, whether it was paid
/// for in coin as it was packed, when the bill was drawn), whether it is sealed and still as it was sealed (`intact`),
/// its `digest` (of its bill and packages: a host's crates of it are of this pallet as it is while the digest and
/// `since` are theirs), its packages in order, the `items` a host shows it as (one a package: a crate, a barrel, a
/// bundle, or loose stacks strapped), its weight, the point whose chests a host put its packages in (`shown`; `None`
/// while they're nowhere the core knows of, or the pallet has moved or changed since), and the places of the packages
/// brought to its consignee so far (`arriving`: a pallet coming in parts).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PalletView {
    pub id: u64,
    pub at: Place,
    pub since: u64,
    pub shipper: TownNo,
    pub consignee: TownNo,
    pub from: String,
    pub to: String,
    pub contract: Option<u32>,
    pub paid: bool,
    pub issued: u64,
    pub sealed: bool,
    pub intact: bool,
    pub digest: u64,
    pub packages: Vec<PackageLine>,
    pub items: u32,
    pub weight: f64,
    pub shown: Option<Point>,
    pub arriving: Vec<u32>,
}

/// One line of a wagon's manifest: a pallet aboard, where it comes off (`None`: nowhere said, home at the trip's end),
/// and the consolidation it rides in (`None`: loose aboard).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestLine {
    pub pallet: PalletView,
    pub drop: Option<TownNo>,
    pub container: Option<u64>,
}
