//! Work: each town's herds are tended and seeded (`herds.rs`), before the workshop turns wool and leather into goods
//! (and, when the farms land, before they plan, so feed demand is known).

use bevy_ecs::prelude::*;

use super::Step;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(crate::herds::tend_towns.in_set(Step::Work).before(super::workshop::work));
}
