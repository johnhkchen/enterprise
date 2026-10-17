//! Work: each town's trades turn raw goods into the goods they deal in (`workshop.rs`).

use bevy_ecs::prelude::*;

use super::Step;
use crate::contracts::Book;
use crate::content::Content;
use crate::roster::Roster;
use crate::state::*;
use crate::workshop::{convert, Workshop};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(work.in_set(Step::Work));
}

/// Every town with a roster runs the conversions its trades can do, as far as their outputs are wanted.
pub fn work(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut ledger: ResMut<Ledger>, routes: Query<&Route>,
        mut towns: Query<(&Town, &mut Stock, &People, &Roster, Option<&Workshop>, Option<&crate::cargo::Shipments>)>) {
    if content.conversions.is_empty() {
        return;
    }
    // In town order, not the order of archetypes (a town with hooks lives apart from one without, and a load puts
    // them together), so the ledger's sums come out the same either way.
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (town, mut stock, people, roster, hooks, store) in order {
        let routes_out = routes.iter().filter(|r| r.from_id == town.id).count();
        // Its bakers leave alone the crop it owes under harvest contracts (`ForwardSales.setAside`), or what the hook
        // says is set aside if that is more.
        let mut merged: Option<Workshop> = None;
        for c in &content.conversions {
            let g = c.inputs[0].0;
            let owed = crate::cargo::set_aside(&book, store, town.id, g, clock.now, content.day_ticks);
            if owed > 0.0 {
                let h = merged.get_or_insert_with(|| hooks.cloned().unwrap_or_default());
                h.aside.resize(content.goods.len(), 0.0);
                h.aside[g] = h.aside[g].max(owed);
            }
        }
        convert(&content, roster, people, &mut stock, &mut ledger, routes_out, merged.as_ref().or(hooks));
    }
}
