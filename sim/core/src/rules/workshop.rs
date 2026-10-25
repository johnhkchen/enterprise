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

/// Every town with a roster runs the conversions its trades can do, as far as their outputs are wanted: at home, and at
/// the far end of each route out, what its lockers take and it lacks (as the wagon's plan sees it: `rules/plan.rs`), so
/// a town with cane and a Cartographer makes paper for the Librarian down the road.
#[allow(clippy::type_complexity)]
pub fn work(clock: Res<Clock>, content: Res<Content>, book: Res<Book>, mut ledger: ResMut<Ledger>, routes: Query<&Route>,
        pallets: Res<crate::pallet::Pallets>,
        mut towns: Query<(&Town, &mut Stock, &People, &Roster, Option<&Workshop>, &Trades, Option<&crate::lockers::Lockers>)>) {
    if content.conversions.is_empty() {
        return;
    }
    let n = content.goods.len();
    // What the far ends of each town's routes out want and lack, by good (in the routes' order: a sum's last bit
    // mustn't hang on archetypes).
    let mut far_wants: std::collections::BTreeMap<usize, Vec<f64>> = std::collections::BTreeMap::new();
    let mut ordered: Vec<&Route> = routes.iter().collect();
    ordered.sort_by_key(|r| r.seq);
    for route in ordered {
        let Ok((_, stock, _, roster, _, trades, lockers)) = towns.get(route.to) else { continue };
        let taken = lockers.map(|l| l.taken(&content, roster, n));
        let wants = far_wants.entry(route.from_id).or_insert_with(|| vec![0.0; n]);
        for (g, want) in wants.iter_mut().enumerate() {
            if content.goods[g].comfort.is_some() || content.goods[g].food > 0.0 {
                continue; // (comforts and food: `wanted` counts them for each route out)
            }
            if let Some(par) = crate::lockers::wanted_par_of(&content, trades, taken.as_deref(), g) {
                *want += (par - stock.0[g]).max(0.0);
            }
        }
    }
    // In town order, not the order of archetypes (a town with hooks lives apart from one without, and a load puts
    // them together), so the ledger's sums come out the same either way.
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (town, mut stock, people, roster, hooks, _, _) in order {
        let routes_out = routes.iter().filter(|r| r.from_id == town.id).count();
        // Its bakers leave alone the crop it owes under harvest contracts (`ForwardSales.setAside`), or what the hook
        // says is set aside if that is more.
        let mut merged: Option<Workshop> = None;
        for c in &content.conversions {
            let g = c.inputs[0].0;
            let owed = crate::cargo::set_aside(&book, &pallets, town.id, g, clock.now, content.day_ticks);
            if owed > 0.0 {
                let h = merged.get_or_insert_with(|| hooks.cloned().unwrap_or_default());
                h.aside.resize(content.goods.len(), 0.0);
                h.aside[g] = h.aside[g].max(owed);
            }
        }
        convert(&content, roster, people, &mut stock, &mut ledger, routes_out, merged.as_ref().or(hooks),
            far_wants.get(&town.id).map(Vec::as_slice));
    }
}
