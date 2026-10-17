//! What a host's game brings: its goods, how long its day is, and the dials that tune the society.

use bevy_ecs::prelude::Resource;

/// A good, by its place in [`Content::goods`].
pub type GoodId = usize;

/// A good the society trades: its name in the host's game, what one is worth, and how many make a stack (a wagon's
/// slot holds a stack).
#[derive(Clone, Debug)]
pub struct Good {
    pub name: String,
    pub value: f64,
    pub stack: u32,
}

/// Settings a study tunes, as in the mod's `Dials`.
#[derive(Clone, Debug)]
pub struct Dials {
    /// Days of what its people use a town's warehouse aims to hold of each good (its shelf).
    pub shelf_days: f64,
    /// Share of its free money a town will spend in a day.
    pub budget_share: f64,
    /// A buyer allows a seller who's been away at most this many days' budget at once.
    pub max_budget_days: f64,
    /// Share of its slots a wagon fills before it goes early.
    pub wagon_full: f64,
    /// Slots in a wagon, a stack each.
    pub wagon_slots: u32,
    /// How often the core looks at its wagons and towns, in ticks.
    pub look_ticks: u64,
}

impl Default for Dials {
    fn default() -> Self {
        Dials { shelf_days: 5.0, budget_share: 0.5, max_budget_days: 6.0, wagon_full: 0.75, wagon_slots: 27, look_ticks: 1200 }
    }
}

/// The host's goods, its day's length in ticks, and the dials.
#[derive(Clone, Debug, Resource)]
pub struct Content {
    pub goods: Vec<Good>,
    pub day_ticks: u64,
    pub dials: Dials,
}

impl Content {
    pub fn new(goods: Vec<Good>, day_ticks: u64) -> Self {
        Content { goods, day_ticks, dials: Dials::default() }
    }

    /// The good called `name`, if the host has one.
    pub fn good(&self, name: &str) -> Option<GoodId> {
        self.goods.iter().position(|g| g.name == name)
    }
}
