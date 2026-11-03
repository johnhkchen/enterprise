//! Storage's rules (`storage.rs`): spoilage, each day.

use bevy_ecs::prelude::*;

use crate::content::Content;
use crate::contracts::Book;
use crate::rules::Step;
use crate::state::*;
use crate::storage::{keeping, spoil_rate, Storage};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems(spoil.in_set(Step::Review).after(super::security::garrison).before(super::jobs::relocate)
        .before(super::jobs::review));
}

/// What each town holds beyond its keep of a good, and what it set aside for its forward sales, goes off its share a
/// day, out of the world. (Not a realm's or a market across the sea: their makings are the world's edge. Nor what a hub
/// holds on consignment.)
pub fn spoil(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, book: Res<Book>,
        pallets: Res<crate::pallet::Pallets>, forwarder: Res<crate::post::Forwarder>, mut storage: ResMut<Storage>,
        mut ledger: ResMut<Ledger>,
        mut q: Query<(&Town, &mut Stock, Option<&crate::works::Works>),
            (Without<crate::realms::Realm>, Without<crate::realms::OffMap>)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let d = &content.dials;
    let n = content.goods.len();
    for &e in &towns.0 {
        let Ok((town, mut stock, works)) = q.get_mut(e) else { continue };
        let stores = crate::storage::Stores::of(works);
        let stored = storage.towns.entry(town.id).or_default();
        let last = match stored.reckoned {
            Some(k) if now >= k + day => k,
            Some(_) => continue,
            None => {
                stored.reckoned = Some(now);
                continue;
            }
        };
        stored.reckoned = Some(now);
        let days = ((now - last) as f64 / day as f64).min(crate::territory::MAX_DAYS);
        let mut aside = crate::cargo::set_aside_all(&book, &pallets, town.id, n, now, content.day_ticks);
        // (Nor what a hub keeps on consignment: its renters pay for its keeping.)
        for (g, a) in aside.iter_mut().enumerate() {
            *a = a.max(0.0) + forwarder.on_consignment(town.id, g);
        }
        for g in 0..n.min(stock.0.len()) {
            let rate = spoil_rate(d, keeping(&content, g), stores);
            let beyond = stock.0[g] - d.spoil_keep - aside[g].max(0.0);
            if rate <= 0.0 || beyond <= 0.0 {
                continue;
            }
            let off = beyond * (1.0 - (1.0 - rate.min(1.0)).powf(days));
            stock.0[g] -= off;
            ledger.used[g] += off;
            stored.spoiled += off * content.goods[g].value;
        }
    }
}
