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
}

/// An answer, in the place of its question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    /// What a town holds of each good, by the good's place in the content pack, as the core last counted it (at the
    /// last look, or the last happening that touched the town: asking touches nothing).
    Stock { town: TownNo, stock: Vec<f64> },
    /// Every trade point, in the order of their places (dimension, then x, y, z), with its town.
    Points { points: Vec<TownPoint> },
}

/// A trade point and the town it's for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TownPoint {
    pub point: Point,
    pub town: TownNo,
}
