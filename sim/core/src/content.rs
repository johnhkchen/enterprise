//! What a host's game brings: its goods, how long its day is, and the dials that tune the society.

use bevy_ecs::prelude::Resource;

/// A good, by its place in [`Content::goods`].
pub type GoodId = usize;

/// A good the society trades: its name in the host's game, what one is worth, how many make a stack (a wagon's slot
/// holds a stack), the kind of comfort it is if any (by its place in [`Content::comforts`]), and the food points one
/// is worth if it's food.
#[derive(Clone, Debug)]
pub struct Good {
    pub name: String,
    pub value: f64,
    pub stack: u32,
    pub comfort: Option<usize>,
    pub food: f64,
}

impl Good {
    pub fn new(name: &str, value: f64, stack: u32) -> Self {
        Good { name: name.to_string(), value, stack, comfort: None, food: 0.0 }
    }

    /// A comfort of the kind at `kind` in [`Content::comforts`].
    pub fn comfort(mut self, kind: usize) -> Self {
        self.comfort = Some(kind);
        self
    }

    /// Food, worth `points` a piece.
    pub fn food(mut self, points: f64) -> Self {
        self.food = points;
        self
    }
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
    /// A wagon that sold a good well drafts a standing order for it.
    pub standing_orders: bool,
    /// Days of what its people use, at its worth, a town keeps back from spending: its trading capital.
    pub reserve_days: f64,
    /// Bills on a town falling due this many days ahead aren't free money.
    pub budget_days: f64,
    /// What each villager uses of each kind of comfort a day, in emeralds' worth; days of it a town buys up to, and
    /// keeps when others would buy it.
    pub ration: f64,
    pub comfort_days: f64,
    pub comfort_keep_days: f64,
    /// Mood moves halfway to where it's heading in this many days; every so many villagers cost a point of it;
    /// each villager without a bed half a point; unpaid wages, up to two points.
    pub mood_half_life_days: f64,
    pub villagers_per_want: f64,
    pub crowding_mood: f64,
    pub unpaid_mood: f64,
    /// The households' wages: this share of the worth of what the town makes.
    pub labour_share: f64,
    /// A home (two beds) once beds are full and mood is at least `home_mood`, after `home_days`, for `home_cost`.
    pub home_mood: f64,
    pub home_days: f64,
    pub home_cost: f64,
    /// A child once a bed is free and mood is at least `birth_mood`, after `birth_days`, for `birth_cost` and
    /// `birth_food` food points.
    pub birth_mood: f64,
    pub birth_days: f64,
    pub birth_cost: f64,
    pub birth_food: f64,
}

impl Default for Dials {
    fn default() -> Self {
        Dials { shelf_days: 5.0, budget_share: 0.5, max_budget_days: 6.0, wagon_full: 0.75, wagon_slots: 27, look_ticks: 1200,
            standing_orders: true, reserve_days: 2.0, budget_days: 3.0, ration: 0.1, comfort_days: 6.0,
            comfort_keep_days: 4.0, mood_half_life_days: 3.0, villagers_per_want: 8.0, crowding_mood: 0.5,
            unpaid_mood: 2.0, labour_share: 0.5, home_mood: 0.5, home_days: 4.0, home_cost: 10.0, birth_mood: 1.0,
            birth_days: 4.0, birth_cost: 3.0, birth_food: 36.0 }
    }
}

/// The host's goods, the kinds of comfort its people want, its day's length in ticks, and the dials.
#[derive(Clone, Debug, Resource)]
pub struct Content {
    pub goods: Vec<Good>,
    pub comforts: Vec<String>,
    pub day_ticks: u64,
    pub dials: Dials,
}

impl Content {
    /// The mod's five kinds of comfort.
    pub const COMFORTS: [&'static str; 5] = ["treats", "reading", "clothing", "furnishings", "leisure"];

    pub fn new(goods: Vec<Good>, day_ticks: u64) -> Self {
        Content { goods, comforts: Self::COMFORTS.iter().map(|k| k.to_string()).collect(), day_ticks, dials: Dials::default() }
    }

    /// The kind of comfort called `name`.
    pub fn comfort(&self, name: &str) -> Option<usize> {
        self.comforts.iter().position(|k| k == name)
    }

    /// The good called `name`, if the host has one.
    pub fn good(&self, name: &str) -> Option<GoodId> {
        self.goods.iter().position(|g| g.name == name)
    }
}
