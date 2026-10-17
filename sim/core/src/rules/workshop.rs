//! Work: each town's trades turn raw goods into the goods they deal in (`workshop.rs`).

use bevy_ecs::prelude::*;

use super::Step;
use crate::content::Content;
use crate::roster::Roster;
use crate::state::*;
use crate::workshop::{convert, Workshop};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(work.in_set(Step::Work));
}

/// Every town with a roster runs the conversions its trades can do, as far as their outputs are wanted.
pub fn work(content: Res<Content>, mut ledger: ResMut<Ledger>, routes: Query<&Route>,
        mut towns: Query<(&Town, &mut Stock, &People, &Roster, Option<&Workshop>)>) {
    if content.conversions.is_empty() {
        return;
    }
    // In town order, not the order of archetypes (a town with hooks lives apart from one without, and a load puts
    // them together), so the ledger's sums come out the same either way.
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (town, mut stock, people, roster, hooks) in order {
        let routes_out = routes.iter().filter(|r| r.from_id == town.id).count();
        convert(&content, roster, people, &mut stock, &mut ledger, routes_out, hooks);
    }
}
