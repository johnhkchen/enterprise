//! Touch: a town's roster brought up to date, its children grown by the time elapsed (`territory/Growth.java`
//! `growUp`), and its `People.villagers` set to its residents, before its people live the time since.

use bevy_ecs::prelude::*;

use super::Step;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(
        follow.in_set(Step::Touch).after(super::touch::catch_up).before(crate::people::live),
    );
}

/// Every town with a roster: children grow up by the time elapsed, and its villagers are its residents.
pub fn follow(clock: Res<Clock>, mut towns: Query<(&mut Roster, &mut People)>) {
    for (mut roster, mut people) in &mut towns {
        roster.grow_up(clock.now);
        people.villagers = roster.residents() as f64;
    }
}
