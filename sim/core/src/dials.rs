//! Settings a study tunes, as in the mod's `Dials` (`Dials.java`): one catalogue, set by name for a world (`set`,
//! `/enterprise dial <name> <value>`, or `reset`), saved with it in its [`crate::Content`].
//!
//! **A dial is one line** in the list at the end, `name: type = the mod's default,`, under its doc comment. A slice
//! adds its dials as a block of lines under a comment naming it, at the end of the list. Git merges this file by
//! union (`.gitattributes`): two slices adding dials never conflict here; a line changed or taken out would come back
//! beside its new self, so changes to lines already here are the integrator's.

use serde::{Deserialize, Serialize};

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

/// The list made into `Dials`: the struct (in the list's order, which is the save's), its defaults, and dials by name.
macro_rules! dials {
    ($($(#[$doc:meta])* $name:ident: $ty:ty = $default:expr,)*) => {
        /// Settings a study tunes, as in the mod's `Dials`.
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct Dials {
            $($(#[$doc])* pub $name: $ty,)*
        }

        impl Default for Dials {
            fn default() -> Self {
                Dials { $($name: $default,)* }
            }
        }

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

dials! {
    /// Days of what its people use a town's warehouse aims to hold of each good (its shelf).
    shelf_days: f64 = 5.0,
    /// Share of its free money a town will spend in a day.
    budget_share: f64 = 0.5,
    /// A buyer allows a seller who's been away at most this many days' budget at once.
    max_budget_days: f64 = 6.0,
    /// Share of its slots a wagon fills before it goes early.
    wagon_full: f64 = 0.75,
    /// Slots in a wagon, a stack each.
    wagon_slots: u32 = 27,
    /// How often the core looks at its wagons and towns, in ticks.
    look_ticks: u64 = 1200,
    /// A wagon that sold a good well drafts a standing order for it.
    standing_orders: bool = true,
    /// Days of what its people use, at its worth, a town keeps back from spending: its trading capital.
    reserve_days: f64 = 2.0,
    /// Bills on a town falling due this many days ahead aren't free money.
    budget_days: f64 = 3.0,
    /// What each villager uses of each kind of comfort a day, in emeralds' worth; days of it a town buys up to, and
    /// keeps when others would buy it.
    ration: f64 = 0.1,
    comfort_days: f64 = 6.0,
    comfort_keep_days: f64 = 4.0,
    /// Mood moves halfway to where it's heading in this many days; every so many villagers cost a point of it;
    /// each villager without a bed half a point; unpaid wages, up to two points.
    mood_half_life_days: f64 = 3.0,
    villagers_per_want: f64 = 8.0,
    crowding_mood: f64 = 0.5,
    unpaid_mood: f64 = 2.0,
    /// The households' wages: this share of the worth of what the town makes.
    labour_share: f64 = 0.5,
    /// A home (two beds) once beds are full and mood is at least `home_mood`, after `home_days`, for `home_cost`.
    home_mood: f64 = 0.5,
    home_days: f64 = 4.0,
    home_cost: f64 = 10.0,
    /// A child once a bed is free and mood is at least `birth_mood`, after `birth_days`, for `birth_cost` and
    /// `birth_food` food points.
    birth_mood: f64 = 1.0,
    birth_days: f64 = 4.0,
    birth_cost: f64 = 3.0,
    birth_food: f64 = 36.0,
    /// Territory (`territory-growth`): culture a hex costs per hex already held, and how many rings out a town may grow;
    /// a guard's daily food (points) and pay, days of pay a guard on a tolling post waits for; the toll a town asks
    /// until it sets its own.
    culture_per_hex: f64 = 5.0,
    max_rings: u32 = 2,
    guard_food: f64 = 6.0,
    guard_pay: f64 = 0.5,
    guard_credit_days: f64 = 3.0,
    default_toll_rate: f64 = 0.05,
    // Advances and refunds (`contract/Advance.java`).
    /// The share of a contract's worth the buyer pays up front, for a draft that asks for an advance (a third).
    advance_share: f64 = 1.0 / 3.0,
    // bargain-and-par
    /// Merchants bargain by level at the far end (`Bargain.java`); off, every good sells at its full value.
    bargain: bool = true,
}
