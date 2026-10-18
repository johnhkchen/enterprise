//! Routes and their wagons as the host sees them (`docs/binding-plan.md` 2, concern 8): numbers for the Merchant's
//! screen and the route commands (`wagon_host.rs` keeps what the core does with them).

use serde::{Deserialize, Serialize};

use super::{HandNo, Point, RouteNo, TownNo};

/// A route as the host shows it: its number and ends, its road's length in blocks and a leg's days on it, a wagon every
/// `every_days`, whether it's open (`false`: wound down, its wagon comes home and stays) and on call (the post's: it leaves
/// only on a day a call is booked), its trips and what it sold in all, its wagon on the road if one is, and a word on
/// where it stands (`status`: on the way out, at the far end, on the way home, home, wound down).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteLine {
    pub route: RouteNo,
    pub from: TownNo,
    pub to: TownNo,
    pub blocks: u32,
    pub days: f64,
    pub every_days: f64,
    pub open: bool,
    pub on_call: bool,
    pub trips: u32,
    pub sold: f64,
    pub wagon: Option<WagonLine>,
    pub status: String,
}

/// A route's wagon on the road: which way (`out`: to the far end; else home), the tick it gets there (or got there, while
/// it waits at the far end), the hands of its crew by id (away from home while it's on the road: the host's villagers
/// stowed), the stacks it carries, and whether it's a porter on foot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WagonLine {
    pub out: bool,
    pub arrives: u64,
    pub crew: Vec<HandNo>,
    pub stacks: u32,
    pub porter: bool,
}

/// A market a town knows (nearest first, as its atlas has them): the town, its desk (`None`: a town the host didn't
/// place), how far it is in blocks (`None`: nobody knows), whether it's a settlement (its land surveyed round a bell; a
/// market with none is only a desk), whether a wagon carries the food to get there (a leg's food within
/// `food_capacity`) and whether the town sends wagons there now (an open route).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarketLine {
    pub town: TownNo,
    pub desk: Option<Point>,
    pub blocks: Option<u32>,
    pub settlement: bool,
    pub reachable: bool,
    pub routed: bool,
}
