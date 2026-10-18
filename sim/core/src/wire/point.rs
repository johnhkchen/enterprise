//! Trade points (`docs/architecture.md`, convention 3): the registered places in the host's game where the society
//! meets it. A desk, a locker, a shop, a banker, a depot: deliveries, pickups and stocktakes are messages about them,
//! and the core keeps them (`points.rs`), so a host keeps no society ids of its own and rebuilds its lookup from
//! `Query::Points` after a load.

use serde::{Deserialize, Serialize};

use crate::points::{Kind, Spot};

/// A trade point: what it is, and where, by the host's own coordinates (a dimension and a block). One point stands in a
/// place: a happening naming a point names the one standing there, and is refused if it names another kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub kind: PointKind,
    pub dim: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// What a trade point is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointKind {
    /// A Merchant's desk: its town's warehouse, its chests the town's stock.
    Desk,
    /// A villager's locker.
    Locker,
    /// A shop.
    Shop,
    /// A banker (coins in and out).
    Banker,
    /// A depot (a station's or a dock's store).
    Depot,
}

impl Point {
    /// Where it stands, as the core keeps it.
    pub(crate) fn spot(&self) -> Spot {
        Spot { dim: self.dim.clone(), x: self.x, y: self.y, z: self.z }
    }

    pub(crate) fn kind(&self) -> Kind {
        match self.kind {
            PointKind::Desk => Kind::Desk,
            PointKind::Locker => Kind::Locker,
            PointKind::Shop => Kind::Shop,
            PointKind::Banker => Kind::Banker,
            PointKind::Depot => Kind::Depot,
        }
    }

    /// A point the core keeps, for the host.
    pub(crate) fn of(spot: &Spot, kind: Kind) -> Point {
        let kind = match kind {
            Kind::Desk => PointKind::Desk,
            Kind::Locker => PointKind::Locker,
            Kind::Shop => PointKind::Shop,
            Kind::Banker => PointKind::Banker,
            Kind::Depot => PointKind::Depot,
        };
        Point { kind, dim: spot.dim.clone(), x: spot.x, y: spot.y, z: spot.z }
    }
}
