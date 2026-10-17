//! What a host's game brings: its goods, how long its day is, and the dials that tune the society.

use serde::{Deserialize, Serialize};
use bevy_ecs::prelude::Resource;

/// A good, by its place in [`Content::goods`].
pub type GoodId = usize;

/// A good the society trades: its name in the host's game, what one is worth, how many make a stack (a wagon's slot
/// holds a stack), the kind of comfort it is if any (by its place in [`Content::comforts`]), and the food points one
/// is worth if it's food.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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

/// A dial's value as the mod keeps every dial: a number. Whole-number dials round it (the mod's `Dial.getInt`), and a
/// switch is on for anything but 0.
pub trait DialValue: Sized {
    fn from_f64(value: f64) -> Result<Self, String>;
    fn to_f64(&self) -> f64;
}

impl DialValue for f64 {
    fn from_f64(value: f64) -> Result<Self, String> {
        if value.is_finite() { Ok(value) } else { Err(format!("{value} isn't a number")) }
    }
    fn to_f64(&self) -> f64 {
        *self
    }
}

impl DialValue for u32 {
    fn from_f64(value: f64) -> Result<Self, String> {
        let n = value.round();
        if n.is_finite() && (0.0..=u32::MAX as f64).contains(&n) { Ok(n as u32) } else { Err(format!("{value} isn't a count")) }
    }
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl DialValue for u64 {
    fn from_f64(value: f64) -> Result<Self, String> {
        let n = value.round();
        if n.is_finite() && (0.0..=u64::MAX as f64).contains(&n) { Ok(n as u64) } else { Err(format!("{value} isn't a count")) }
    }
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl DialValue for bool {
    fn from_f64(value: f64) -> Result<Self, String> {
        if value.is_finite() { Ok(value != 0.0) } else { Err(format!("{value} isn't a switch")) }
    }
    fn to_f64(&self) -> f64 {
        if *self { 1.0 } else { 0.0 }
    }
}

/// Dials by name, as the mod's `Dials.java` sets them for a world (`set`, `/enterprise dial <name> <value>`, or
/// `reset`). They're saved with the world, in its [`Content`].
macro_rules! dials_by_name {
    ($($name:ident),* $(,)?) => {
        impl Dials {
            /// Every dial's name, in the order they're declared.
            pub fn names() -> Vec<&'static str> {
                vec![$(stringify!($name)),*]
            }

            /// A dial's value, as a number.
            pub fn get(&self, name: &str) -> Option<f64> {
                match name {
                    $(stringify!($name) => Some(DialValue::to_f64(&self.$name)),)*
                    _ => None,
                }
            }

            /// Sets a dial for this world.
            pub fn set(&mut self, name: &str, value: f64) -> Result<(), String> {
                match name {
                    $(stringify!($name) => self.$name = DialValue::from_f64(value)?,)*
                    _ => return Err(format!("no dial called {name}")),
                }
                Ok(())
            }

            /// Puts a dial back to the mod's default.
            pub fn reset(&mut self, name: &str) -> Result<(), String> {
                let initial = Dials::default();
                match name {
                    $(stringify!($name) => self.$name = initial.$name,)*
                    _ => return Err(format!("no dial called {name}")),
                }
                Ok(())
            }
        }
    };
}

// Every field of `Dials`, one a line: a new dial adds its name here too (a test checks none is missing).
dials_by_name!(
    shelf_days,
    budget_share,
    max_budget_days,
    wagon_full,
    wagon_slots,
    look_ticks,
    standing_orders,
    reserve_days,
    budget_days,
    ration,
    comfort_days,
    comfort_keep_days,
    mood_half_life_days,
    villagers_per_want,
    crowding_mood,
    unpaid_mood,
    labour_share,
    home_mood,
    home_days,
    home_cost,
    birth_mood,
    birth_days,
    birth_cost,
    birth_food,
);

/// The host's goods, the kinds of comfort its people want, its day's length in ticks, and the dials.
#[derive(Clone, Debug, Resource, Serialize, Deserialize)]
pub struct Content {
    pub goods: Vec<Good>,
    pub comforts: Vec<String>,
    pub day_ticks: u64,
    pub dials: Dials,
    /// The trades a town's hands can have (`roster.rs`): the mod's sixteen by default.
    pub trades: Vec<crate::roster::Trade>,
}

impl Content {
    /// The mod's five kinds of comfort.
    pub const COMFORTS: [&'static str; 5] = ["treats", "reading", "clothing", "furnishings", "leisure"];

    pub fn new(goods: Vec<Good>, day_ticks: u64) -> Self {
        Content { goods, comforts: Self::COMFORTS.iter().map(|k| k.to_string()).collect(), day_ticks, dials: Dials::default(),
            trades: crate::roster::trades() }
    }

    /// The kind of comfort called `name`.
    pub fn comfort(&self, name: &str) -> Option<usize> {
        self.comforts.iter().position(|k| k == name)
    }

    /// The good called `name`, if the host has one.
    pub fn good(&self, name: &str) -> Option<GoodId> {
        self.goods.iter().position(|g| g.name == name)
    }

    /// The trade called `name`, with or without the host's namespace (`librarian`, `minecraft:librarian`).
    pub fn trade(&self, name: &str) -> Option<crate::roster::TradeId> {
        let name = name.rsplit(':').next().unwrap_or(name);
        self.trades.iter().position(|t| t.name == name)
    }
}
