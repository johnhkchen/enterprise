//! Questions a host asks (`Command::Ask`): what its commands and screens show, read only from these.

use serde::{Deserialize, Serialize};

use super::{Point, TownNo};

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
