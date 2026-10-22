//! What the host's other commands ask of the core (`docs/binding-plan.md` 2, concern 11; task `commands-and-studies`):
//! a town's territory report (`/enterprise territory`), the war (`/enterprise war`, `war tally`) and the routes' books the
//! studies read. `commands_host.rs` builds them from the state the core already keeps; nothing here is saved.

use serde::{Deserialize, Serialize};

use super::{Point, RouteNo, TownNo};
use crate::fixed::Fixed;

/// A good (by name) and a number: items a day a farm grows, a comfort's share met, the kit issued.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NameCount {
    pub name: String,
    pub count: f64,
}

/// What a town has sold forward of a crop: items a day, at a price in emeralds each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForwardLine {
    pub good: String,
    pub per_day: f64,
    pub price: f64,
}

/// A work of a town's, in words: what it is (`bridge on q,r`, `wall`, a hall's name), how much of it stands (0 to 1),
/// whether it is finished and the emeralds paid its masons.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectLine {
    pub what: String,
    pub progress: f64,
    pub done: bool,
    pub wages: Fixed,
}

/// A town's part in the war: what `/enterprise war tally` tells of it (`crown.rs` `TownTally`) and the trust between it and
/// the crown's capital.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WarTown {
    pub town: TownNo,
    pub name: String,
    pub raised: u32,
    pub fallen: u32,
    pub deserted: u32,
    pub days: u32,
    pub hungry: u32,
    pub strength: f64,
    pub kit: f64,
    pub earned: Fixed,
    pub trust: f64,
}

/// A route's books, the studies' per-day columns: its ends (by town and desk), its length, its trips, what it sold and
/// bought and paid, in spurs at the mint's rate now (`rate`) where the mod counted spurs, in items where it counted items.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteBook {
    pub route: RouteNo,
    pub from: TownNo,
    pub to: TownNo,
    pub from_desk: Option<Point>,
    pub to_desk: Option<Point>,
    pub blocks: u32,
    pub trips: u32,
    pub sold_items: f64,
    pub sales_spurs: f64,
    pub service_spurs: f64,
    pub bought_items: f64,
    pub purchase_spurs: f64,
    pub food_spurs: f64,
    pub tolls_spurs: f64,
    pub home_spurs: f64,
    pub food_eaten: f64,
    pub short_rations: u32,
    pub delivered: f64,
}
