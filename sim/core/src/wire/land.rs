//! The land as the host sees it (`docs/binding-plan.md` 2, concern 7): hexes and their ground, and the works a town
//! builds on them, as numbers (`land.rs` keeps what the core does with them).

use serde::{Deserialize, Serialize};

use super::GoodNo;

/// A hex of the world grid (`hex.rs`: axial, pointy-top, 80 blocks across), by its two coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HexAt {
    pub q: i32,
    pub r: i32,
}

impl HexAt {
    pub(crate) fn hex(self) -> crate::hex::Hex {
        crate::hex::Hex::new(self.q, self.r)
    }

    pub(crate) fn of(hex: crate::hex::Hex) -> HexAt {
        HexAt { q: hex.q, r: hex.r }
    }
}

/// What a hex's land gives the hands working it (`map.rs`' `Land`: the mod's biome tags, one kind a hex).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandKind {
    /// Open ground: plains, meadows and the like.
    Plains,
    /// Forest, swamp, jungle and savanna.
    Woods,
    /// Taiga and snow.
    Taiga,
    /// Desert and badlands.
    Desert,
    /// Beach.
    Shore,
    /// Hills and mountains.
    Hills,
    /// Ocean and river.
    Water,
}

impl LandKind {
    pub(crate) fn land(self) -> crate::map::Land {
        use crate::map::Land;
        match self {
            LandKind::Plains => Land::Plains,
            LandKind::Woods => Land::Woods,
            LandKind::Taiga => Land::Taiga,
            LandKind::Desert => Land::Desert,
            LandKind::Shore => Land::Shore,
            LandKind::Hills => Land::Hills,
            LandKind::Water => Land::Water,
        }
    }

    pub(crate) fn of(land: crate::map::Land) -> LandKind {
        use crate::map::Land;
        match land {
            Land::Plains => LandKind::Plains,
            Land::Woods => LandKind::Woods,
            Land::Taiga => LandKind::Taiga,
            Land::Desert => LandKind::Desert,
            Land::Shore => LandKind::Shore,
            Land::Hills => LandKind::Hills,
            Land::Water => LandKind::Water,
        }
    }
}

/// Something a town builds that the host lays in its world: a bridge's deck on a hex, between two of its neighbours;
/// the town's wall round its bell; a hall (by its name: `square`, `granary`, `root_cellar`, `counting_house`, `temple`,
/// `library`, `smithy`, `watchtower`, `stone_docks`, `caravanserai`); a field of a crop; a family's home (two beds,
/// `people.rs`), where the host finds room by the bell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Work {
    Bridge { hex: HexAt, from: HexAt, to: HexAt },
    Wall,
    Hall { kind: String },
    Field { crop: GoodNo },
    Home,
}

/// One of a town's works as the core has it: how much of it is laid by its masons (0 to 1), whether it's finished, and
/// how much of it the host has shown in its world (`Input::Built`). A field the town planted is laid and finished at
/// once, and shown when the host has laid it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkView {
    pub work: Work,
    pub laid: f64,
    pub done: bool,
    pub shown: f64,
}

/// A hex's ground as the core has it: its share under water, its relief in blocks, what its land gives, and whether a
/// finished bridge stands on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundView {
    pub hex: HexAt,
    pub water: f64,
    pub relief: f64,
    pub land: LandKind,
    pub bridged: bool,
}
