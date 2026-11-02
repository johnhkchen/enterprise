//! Waystations' keep (John, 2026-11-02: "Way stations should be able to collect a small toll for water, resting, and
//! sell their own food for caravans"; "Let's use a level up mechanism for these, so towns can improve the way station
//! in stages, so starting small is possible"). A town builds a stop on the road (`works::What::Waystation`, level 1);
//! from then on it keeps itself, its takings banked in its town's treasury (`Station::banked`), and grows a level when
//! they pay for it:
//!
//! - **1, a stop:** a trough and a roof. A wagon of another town passing within a hex of it pays [`fee`] each way, for
//!   water and a rest.
//! - **2, a waystation** (the people's kit): a kitchen garden besides, its food grown [`food_a_day`] and kept to
//!   [`food_kept`], sold to a passing wagon's crew a meal a mouth each way at bread's worth and [`MEAL_MARKUP`].
//! - **3, a caravan inn** (the people's inn or caravanserai): more food, a dearer rest.
//!
//! A level is paid out of what the stop banked ([`rise_cost`]), and its keepers put it up in [`rise_days`]: a town's own
//! works go on meanwhile. Its own town's wagons rest free.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;
use crate::hex::Hex;

/// The top level: a caravan inn.
pub const TOP: u8 = 3;

/// Emeralds a passing wagon pays a stop of this level, each way, for water and a rest.
pub fn fee(level: u8) -> f64 {
    [0.0, 0.1, 0.2, 0.3][level.min(TOP) as usize]
}

/// Food points a stop of this level grows a day, and keeps at most.
pub fn food_a_day(level: u8) -> f64 {
    [0.0, 0.0, 6.0, 12.0][level.min(TOP) as usize]
}

pub fn food_kept(level: u8) -> f64 {
    [0.0, 0.0, 48.0, 96.0][level.min(TOP) as usize]
}

/// Hexes from a stop its road works reach, by level, and the surface they lay at most: a stop grades its trails, a
/// waystation and an inn pave them (John: "invest the income in road improvements that over time allow for longer,
/// quicker travel. It should work hand in hand with the desire path and path slope system"): only a trail wagons wore
/// is worked, the most worn first, a hex a day.
pub fn road_reach(level: u8) -> i32 {
    [0, 2, 3, 4][level.min(TOP) as usize]
}

pub fn best_surface(level: u8) -> u8 {
    if level >= 2 { crate::map::PAVED } else { crate::map::GRADED }
}

/// Emeralds a hex of trail takes to grade, and to pave once graded.
pub const GRADE_COST: f64 = 2.0;
pub const PAVE_COST: f64 = 4.0;

/// A meal sells at its food's worth (`purse_per_food_point`) and this much again.
pub const MEAL_MARKUP: f64 = 1.25;

/// Emeralds a stop banks before it rises to `level`, and the days its keepers take to put it up.
pub fn rise_cost(level: u8) -> f64 {
    match level {
        2 => 12.0,
        _ => 30.0,
    }
}

pub fn rise_days(level: u8) -> f64 {
    match level {
        2 => 4.0,
        _ => 10.0,
    }
}

/// A town's stop on the road, as it keeps itself.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Station {
    pub hex: Hex,
    pub owner: usize,
    pub level: u8,
    /// Its takings, kept in its town's treasury and spent on its next level.
    pub banked: Fixed,
    /// Food points it holds for meals.
    pub food: f64,
    /// When its garden was last counted.
    pub kept_at: u64,
    /// When the level it's putting up stands, if it is.
    pub rising: Option<u64>,
    /// The level the host has shown.
    pub shown: u8,
    /// Wagons that stopped, what they paid, and the meals they ate.
    pub passes: u64,
    pub taken: Fixed,
    pub meals: f64,
    /// The day its road works last laid a hex, and the hexes they laid.
    #[serde(default)]
    pub mended: u64,
    #[serde(default)]
    pub laid: u32,
}

/// Every town's stops, in the order they were built.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Waystations {
    pub stations: Vec<Station>,
}

impl Waystations {
    /// The stop on a hex, if there is one.
    pub fn at(&self, hex: Hex) -> Option<&Station> {
        self.stations.iter().find(|s| s.hex == hex)
    }
}

impl crate::society::Society {
    /// Every town's stops on the road.
    pub fn waystations(&self) -> Vec<Station> {
        self.world.resource::<Waystations>().stations.clone()
    }

    /// Banks emeralds its town already holds to the stop on `hex` (a host's command, a test's): toward its next level.
    pub fn bank_at_waystation(&mut self, hex: Hex, emeralds: f64) {
        if let Some(s) = self.world.resource_mut::<Waystations>().stations.iter_mut().find(|s| s.hex == hex) {
            s.banked += Fixed::floor(emeralds);
        }
    }
}
