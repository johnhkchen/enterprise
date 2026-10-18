//! Outcomes out: what happened in the society, for the host to show.

use serde::{Deserialize, Serialize};

use super::{GoodId, RouteId, TownId};
use crate::fixed::Fixed;

/// What happened, for the host to show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Output {
    WagonLeft { route: RouteId, stacks: u32, worth: f64 },
    WagonSold { route: RouteId, sold: f64, takings: Fixed },
    WagonHome { route: RouteId, takings: Fixed },
    /// A master Merchant filled a player's order: `lots` lots, `count` goods, for `spurs`.
    OrderFilled { town: TownId, order: usize, good: GoodId, lots: u32, count: f64, spurs: f64 },
    /// A desk newly posted a want no market it knows holds, at `spurs_each` an item.
    Signal { town: TownId, good: GoodId, count: f64, spurs_each: f64 },
}
