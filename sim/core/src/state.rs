//! The society's state as Bevy components and resources: towns and routes are entities, the clock and the ledger
//! of what came into the world and left it are resources. All of it plain data, for a host to save with its own save.

use bevy_ecs::prelude::*;

use crate::messages::Output;

/// The society's time, in the host's ticks.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Clock {
    pub now: u64,
}

/// What came into the world and left it, for the checks: goods made, used and delivered; money people paid in and
/// money spent on the road.
#[derive(Resource, Clone, Debug, Default)]
pub struct Ledger {
    pub made: Vec<f64>,
    pub used: Vec<f64>,
    pub delivered: Vec<f64>,
    pub income: f64,
    pub road: f64,
    /// Money spent out of the world: homes built, children raised.
    pub spent: f64,
}

/// What happened this look, for the host to show.
#[derive(Resource, Clone, Debug, Default)]
pub struct Outbox(pub Vec<Output>);

/// A town, by name, and its number (messages and contracts name towns by it).
#[derive(Component, Clone, Debug)]
pub struct Town {
    pub name: String,
    pub id: usize,
}

/// Every town's entity, by its number.
#[derive(Resource, Clone, Debug, Default)]
pub struct Towns(pub Vec<Entity>);

/// A town's money.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Money(pub f64);

/// A town's people: how many, their beds and the most beds its land will hold, their mood, how well each kind of
/// comfort was met last time and what's still owed of it (below 0: credit from a big item), progress toward the next
/// home and child, the share of wages paid, and the households' purse (their wages, which pay for their comforts).
#[derive(Component, Clone, Debug, Default)]
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
#[derive(Component, Clone, Debug, Default)]
pub struct Stock(pub Vec<f64>);

/// What a town's people make and use of each good a day. They buy what they use from the warehouse, at its worth:
/// the town's income.
#[derive(Component, Clone, Debug, Default)]
pub struct Trades {
    pub makes: Vec<f64>,
    pub uses: Vec<f64>,
}

/// When a town was last brought up to date.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Touched(pub u64);

/// A town's day's budget, as its money manager set it at the day's first look.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Budget {
    pub day: u64,
    pub budget: f64,
    pub spent: f64,
    pub set: bool,
}

/// Which way a wagon is going.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Leg {
    #[default]
    Out,
    Home,
}

/// A wagon on the road: its cargo of each good, the money it carries, which way, and when it gets there; what of
/// its cargo is owed under which contract, the Ex Works contracts it's collecting for, and the contracts drafted on
/// this trip (their copies ride home with it).
#[derive(Clone, Debug, Default)]
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
}

/// A town's run to another: how long the road takes one way, what a round trip costs in road food, its timetable.
/// `seq` orders routes, so every look takes them the same way.
#[derive(Component, Clone, Copy, Debug)]
pub struct Route {
    pub seq: usize,
    pub from: Entity,
    pub to: Entity,
    pub from_id: usize,
    pub to_id: usize,
    pub travel_ticks: u64,
    pub road_cost: f64,
    pub every_ticks: u64,
}

/// An idle route's plan for a wagon this look, built up by the planning steps (each module adds what it owes, wants or
/// carries) and acted on by the deciding step: the cargo, of which owed under which contract; the Ex Works contracts
/// it would collect for and the money for them; its worth, its stacks, and whether something can't wait.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub cargo: Vec<f64>,
    pub contracted: Vec<(usize, f64)>,
    pub collecting: Vec<usize>,
    pub purse: f64,
    pub worth: f64,
    pub stacks: u32,
    pub pressing: bool,
}

/// What a route has done, its wagon if one's on the road, the leg its wagon finished this look (the arriving steps
/// act on it), and its plan this look if it's idle.
#[derive(Component, Clone, Debug, Default)]
pub struct RouteState {
    pub last_departure: Option<u64>,
    pub last_sold: Option<u64>,
    pub wagon: Option<Wagon>,
    pub arrived: Option<Leg>,
    pub plan: Option<Plan>,
    pub trips: u32,
    pub sold: f64,
    pub carried: f64,
}
