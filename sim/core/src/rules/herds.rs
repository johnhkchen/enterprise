//! Work: each town's herds are tended and seeded (`herds.rs`), after its farm accrues (the mod's `touch`: `accrue`,
//! then `Husbandry.tend`), and before the workshop turns wool and leather into goods; the feed demand is known before
//! the farms' review plans.

use bevy_ecs::prelude::*;

use super::Step;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(crate::herds::tend_towns.in_set(Step::Work).after(super::farms::work).before(super::workshop::work));
}
