//! The core's whole interface: happenings in, results out. The same messages whether the core runs inside a game,
//! beside it or in a test, so a run's messages can be logged and replayed. Towns and routes go by their number, in
//! the order they were added: stable across saves and hosts, unlike the ECS's own entity ids.

use serde::{Deserialize, Serialize};
use crate::content::GoodId;

pub type TownId = usize;
pub type RouteId = usize;

/// A happening the host hands the core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Input {
    /// Time has passed, to this tick.
    Advance { to: u64 },
    /// Goods handed to a town from outside the society (a player's delivery).
    Deliver { town: TownId, good: GoodId, count: f64 },
    /// Money paid a town from outside the society (a player buying at its shops).
    Pay { town: TownId, money: f64 },
}

/// What happened, for the host to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Output {
    WagonLeft { route: RouteId, stacks: u32, worth: f64 },
    WagonSold { route: RouteId, sold: f64, takings: f64 },
    WagonHome { route: RouteId, takings: f64 },
}
