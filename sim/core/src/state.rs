//! The society's state as Bevy components and resources: towns and routes are entities, the clock and the ledger
//! of what came into the world and left it are resources. All of it plain data, for a host to save with its own save.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::messages::Output;
/// A town's people, hand by hand (`roster.rs`): a component on towns that keep one.
pub use crate::roster::{Hand, Roster};
/// A rostered town's lockers: its hands' offers and which the Merchant manages (`lockers.rs`).
pub use crate::lockers::Lockers;

/// The society's time, in the host's ticks.
#[derive(Resource, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Clock {
    pub now: u64,
}

/// What came into the world and left it, for the checks: goods made, used and delivered; money people paid in and
/// money spent on the road.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Ledger {
    pub made: Vec<f64>,
    pub used: Vec<f64>,
    pub delivered: Vec<f64>,
    pub income: f64,
    pub road: f64,
    /// Money spent out of the world: homes built, children raised.
    pub spent: f64,
}

/// The world's dice (decision 8 of `docs/core-plan.md`): xorshift64*, seeded, saved with the state, and drawn only
/// where the mod rolls dice (the Merchant's XP from a locker trade, `lockers.rs`; the jobs draw). Seeded 1 unless the
/// host seeds it (`Society::seed`).
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct Rng(pub u64);

impl Default for Rng {
    fn default() -> Self {
        Rng(1)
    }
}

impl Rng {
    /// A generator from `seed` (0 isn't a state xorshift can leave, so it's taken as 1).
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A roll of 0 to `n - 1`, as Java's `nextInt(n)` (0 when `n` is 0).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
}

/// What happened this look, for the host to show.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Outbox(pub Vec<Output>);

/// A town, by name, and its number (messages and contracts name towns by it).
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Town {
    pub name: String,
    pub id: usize,
    /// The hex it stands in, once the host has placed it.
    pub hex: Option<crate::hex::Hex>,
}

/// Every town's entity, by its number.
#[derive(Resource, Clone, Debug, Default)]
pub struct Towns(pub Vec<Entity>);

/// A town's money.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Money(pub f64);

/// A town's people: how many, their beds and the most beds its land will hold, their mood, how well each kind of
/// comfort was met last time and what's still owed of it (below 0: credit from a big item), progress toward the next
/// home and child, the share of wages paid, and the households' purse (their wages, which pay for their comforts).
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct People {
    pub villagers: f64,
    pub beds: f64,
    pub land: f64,
    pub mood: f64,
    pub comfort_met: Vec<f64>,
    pub comfort_owed: Vec<f64>,
    pub home_progress: f64,
    pub birth_progress: f64,
    pub paid_share: f64,
    pub purse: f64,
    pub last: u64,
    pub homes_built: u32,
    pub born: u32,
}

/// What a town holds of each good.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stock(pub Vec<f64>);

/// What a town's people make and use of each good a day. They buy what they use from the warehouse, at its worth:
/// the town's income.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Trades {
    pub makes: Vec<f64>,
    pub uses: Vec<f64>,
    /// What the town's lockers can take of each good a day (`Warehouse.capacity`, `bargain.rs`): its par is
    /// `shelf_days` of it. `set_trade` sets it to what its people use; a host with lockers records it per locker.
    pub capacity: Vec<f64>,
}

/// When a town was last brought up to date.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Touched(pub u64);

/// A town's day's budget, as its money manager set it at the day's first look.
#[derive(Component, Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Budget {
    pub day: u64,
    pub budget: f64,
    pub spent: f64,
    pub set: bool,
}

/// Which way a wagon is going.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Leg {
    #[default]
    Out,
    Home,
}

/// A wagon on the road: its cargo of each good, the money it carries, which way, and when it gets there; what of
/// its cargo is owed under which contract, the Ex Works contracts it's collecting for, and the contracts drafted on
/// this trip (their copies ride home with it).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Wagon {
    pub cargo: Vec<f64>,
    pub purse: f64,
    pub leg: Leg,
    pub arrives: u64,
    pub contracted: Vec<(usize, f64)>,
    pub collecting: Vec<usize>,
    pub drafts: Vec<usize>,
    pub bills: Vec<usize>,
    /// What it sold of each good at the far end this visit, and its takings.
    pub sold_here: Vec<f64>,
    pub takings_here: f64,
    /// Road food (`road.rs`): the food points it carries for the road home, packed at home where the far end may not
    /// feed it; and since when it has waited at the far end for food it couldn't get (the arriving steps leave it be).
    pub provisions: f64,
    pub waiting_since: Option<u64>,
    /// Crates and their papers (`cargo.rs`): the shipments aboard (crated goods, not in `cargo`), the empty crates it
    /// carries home, and the lots for sale opened at the far end this visit (good, count).
    pub shipments: Vec<crate::cargo::Shipment>,
    pub empties: u32,
    pub lots: Vec<(usize, f64)>,
}

/// A town's run to another: how long the road takes one way, what a round trip costs in road food, its timetable.
/// `seq` orders routes, so every look takes them the same way.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Route {
    pub seq: usize,
    #[serde(skip, default = "unlinked")]
    pub from: Entity,
    #[serde(skip, default = "unlinked")]
    pub to: Entity,
    pub from_id: usize,
    pub to_id: usize,
    pub travel_ticks: u64,
    pub road_cost: f64,
    pub every_ticks: u64,
    /// How long the road is in blocks, for its food (`road.rs`): the path's, or a bare route's days at 1,200 a day.
    pub blocks: u32,
}

/// The hexes a route's wagons cross, start to goal: on a road planned over the map (`Society::add_road`), not on a
/// bare route. Its own component, not a field of [`Route`], so `Route` stays `Copy` for the steps that copy it.
#[derive(Component, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct RoutePath(pub Vec<crate::hex::Hex>);

/// An idle route's plan for a wagon this look, built up by the planning steps (each module adds what it owes, wants or
/// carries) and acted on by the deciding step: the cargo, of which owed under which contract; the Ex Works contracts
/// it would collect for and the money for them; its worth, its stacks, and whether something can't wait.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Plan {
    pub cargo: Vec<f64>,
    pub contracted: Vec<(usize, f64)>,
    pub collecting: Vec<usize>,
    pub purse: f64,
    pub worth: f64,
    pub stacks: u32,
    pub pressing: bool,
    /// Crated goods it would take from home's crates (`cargo.rs`): owed under contract, not in `cargo`.
    pub shipments: Vec<crate::cargo::Shipment>,
}

/// What a route has done, its wagon if one's on the road, the leg its wagon finished this look (the arriving steps
/// act on it), and its plan this look if it's idle.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct RouteState {
    pub last_departure: Option<u64>,
    pub last_sold: Option<u64>,
    pub wagon: Option<Wagon>,
    pub arrived: Option<Leg>,
    pub plan: Option<Plan>,
    pub trips: u32,
    pub sold: f64,
    pub carried: f64,
    /// Road food (`road.rs`): trips home on short rations, food points eaten both ways (`foodEaten`), money
    /// paid far ends for it (`foodSpurs`), and why the wagon last stayed home.
    pub short_rations: u32,
    pub food_eaten: f64,
    pub food_paid: f64,
    pub stayed: Option<crate::road::Stay>,
}

/// A route's ends, before a load links them to their towns' entities again (from `from_id` and `to_id`).
fn unlinked() -> Entity {
    Entity::PLACEHOLDER
}

/// The towns' entities aren't saved: a load spawns the towns again and rebuilds the list (`save.rs`).
impl Serialize for Towns {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_unit_struct("Towns")
    }
}

impl<'de> Deserialize<'de> for Towns {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <()>::deserialize(deserializer)?;
        Ok(Towns::default())
    }
}
