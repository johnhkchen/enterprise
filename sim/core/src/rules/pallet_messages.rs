//! Pallets arriving in parts (`pallet_messages.rs`): at a look, a pallet whose packages began coming to its consignee
//! before this look is received with those that have come, the seal broken and the rest lost.

use bevy_ecs::prelude::*;

use crate::cargo::Shipments;
use crate::contracts::Book;
use crate::pallet::Pallets;
use crate::pallet_messages::take_in;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(close_arrivals.in_set(Step::Settle).after(super::cargo::open).before(super::contracts::settle));
}

use super::Step;

/// Each pallet that began arriving before now, in pallet order: received short.
fn close_arrivals(clock: Res<Clock>, mut book: ResMut<Book>, mut ledger: ResMut<Ledger>, mut pallets: ResMut<Pallets>,
        mut towns: Query<(&Town, &mut Stock, &mut Shipments)>) {
    if pallets.arriving.is_empty() {
        return;
    }
    let due: Vec<_> = pallets.arriving.iter().filter(|(_, a)| a.since < clock.now)
        .map(|(&id, a)| (id, a.town, a.parts.clone())).collect();
    for (id, town, parts) in due {
        let Some((_, mut stock, mut store)) = towns.iter_mut().find(|(t, ..)| t.id == town) else { continue };
        take_in(&mut pallets, &mut book, &mut ledger, clock.now, town, &mut stock.0, &mut store, id, Some(&parts));
    }
}
