//! Questions a host asks (`Command::Ask`): what its commands and screens show, read only from these.

use serde::{Deserialize, Serialize};

use super::{HandNo, Point, TownNo};

/// A question for the core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Query {
    /// What a town holds.
    Stock { town: TownNo },
    /// Every trade point and its town.
    Points,
    /// The goods of the society's content, by their place: what a good's number means. A host that loads a save asks
    /// it to map its items to numbers again, since the content is the save's (a pack given at `Create` is kept with
    /// the society), not whatever pack the host would build today.
    Goods,
    /// Every dial: its value now, the value it starts at (what `reset` puts it back to) and what it does.
    Dials,
    /// A town's hands, each by its id with the host's tag: what a host ties its villagers to (after a load too).
    Hands { town: TownNo },
}

/// An answer, in the place of its question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    /// What a town holds of each good, by the good's place in the content pack, as the core last counted it (at the
    /// last look, or the last happening that touched the town: asking touches nothing).
    Stock { town: TownNo, stock: Vec<f64> },
    /// Every trade point, in the order of their places (dimension, then x, y, z), with its town.
    Points { points: Vec<TownPoint> },
    /// Every good's name, in the order of their numbers (a `GoodNo` is a place here).
    Goods { goods: Vec<String> },
    /// Every dial, in the order the core lists them.
    Dials { dials: Vec<DialView> },
    /// A town's hands in its roster's order (children grown by now), and how many lockers its desk's Merchant may manage
    /// (its level's cap, `Roster::locker_limit`; none without one). Empty for a town that keeps no roster.
    Hands { town: TownNo, lockers: u32, hands: Vec<HandView> },
}

/// One of a town's hands: its id (stable while it lives, wherever it goes: a place in the list isn't), the host's tag
/// for it (empty if none), its trade by the core's name, level and XP, whether it's a child, away (with a caravan, at
/// war) or the desk's Merchant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HandView {
    pub hand: HandNo,
    pub tag: String,
    pub trade: String,
    pub level: u8,
    pub xp: u32,
    pub baby: bool,
    pub away: bool,
    pub merchant: bool,
}

/// A trade point and the town it's for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TownPoint {
    pub point: Point,
    pub town: TownNo,
}

/// A dial as `/enterprise dial` lists it: its name, its value now and the value it starts the world at, and what it does
/// (for a dial documented with its neighbours, the group's words).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialView {
    pub name: String,
    pub value: f64,
    pub initial: f64,
    pub does: String,
}
