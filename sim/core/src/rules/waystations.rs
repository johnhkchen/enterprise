//! Waystations' steps (`waystations.rs`):
//!
//! - **Keep:** each look, a stop its town finished is on the books (level 1); its garden grows since it was last
//!   counted; a level its keepers finished stands; and one that banked its next level's cost, its town's treasury
//!   holding it, starts putting it up.
//! - **Serve:** a wagon home again paid each stop it passed within a hex of, there and back, that isn't its own
//!   town's or its far end's: the rest, and its crew's meals where the stop grows food. Paid to the stop's town,
//!   banked to the stop.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::state::*;
use crate::waystations::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        keep.in_set(Step::Review).after(super::works::review).after(super::atlas::consider)
            .before(super::jobs::relocate).before(super::jobs::review),
        serve.in_set(Step::Services).after(super::territory::tolls).after(super::atlas::copy_books)
            .after(super::chokepoints::takings).after(super::services::abroad).after(super::training::copies),
    ));
}

pub fn keep(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut ways: ResMut<Waystations>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut q: Query<(&Town, &crate::works::Works, &mut Money)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    for &e in &towns.0 {
        let Ok((town, works, _)) = q.get(e) else { continue };
        for hex in works.waystations() {
            if ways.at(hex).is_none() {
                ways.stations.push(Station { hex, owner: town.id, level: 1, kept_at: now, ..Default::default() });
            }
        }
    }
    for s in ways.stations.iter_mut() {
        let days = now.saturating_sub(s.kept_at) as f64 / day as f64;
        s.kept_at = now;
        s.food = (s.food + food_a_day(s.level) * days).min(food_kept(s.level));
        if s.rising.is_some_and(|t| now >= t) {
            s.rising = None;
            s.level += 1;
        }
        let next = s.level + 1;
        let cost = Fixed::ceil(rise_cost(next));
        if s.rising.is_some() || s.level >= TOP || s.banked < cost {
            continue;
        }
        let Some(&owner) = towns.0.get(s.owner) else { continue };
        let Ok((_, _, mut money)) = q.get_mut(owner) else { continue };
        if money.0 < cost {
            continue;
        }
        post(&mut ledger, &mut book, Side::treasury(s.owner, &mut money.0), Side::outside(), cost,
            Reason::Other(format!("a waystation's level {next}")), now, day);
        s.banked -= cost;
        s.rising = Some(now + (rise_days(next) * day as f64) as u64);
    }
}

pub fn serve(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut ways: ResMut<Waystations>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, mut money: Query<&mut Money>,
        mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>) {
    if ways.stations.is_empty() {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let order = in_order(routes.iter().map(|(e, r, _, _)| (e, r)));
    for entity in order {
        let (_, route, mut state, path) = routes.get_mut(entity).unwrap();
        let Some(path) = path else { continue };
        if state.arrived != Some(Leg::Home) || route.carrier == crate::map::Carrier::Ship {
            continue;
        }
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let mouths = crate::road::mouths(Some(wagon));
        for s in ways.stations.iter_mut() {
            if s.owner == route.from_id || s.owner == route.to_id || !path.0.iter().any(|h| h.distance(s.hex) <= 1) {
                continue;
            }
            let meals = if food_a_day(s.level) > 0.0 { (2.0 * mouths).min(s.food.floor()) } else { 0.0 };
            let due = 2.0 * fee(s.level) + meals * content.dials.purse_per_food_point * MEAL_MARKUP;
            let paid = Fixed::floor(due).min(wagon.purse.at_least_zero());
            let Some(&e) = towns.0.get(s.owner) else { continue };
            let Ok(mut m) = money.get_mut(e) else { continue };
            if paid > Fixed::ZERO {
                post(&mut ledger, &mut book, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(s.owner, &mut m.0),
                    paid, Reason::Other("waystation".into()), now, day);
            }
            s.banked += paid;
            s.taken += paid;
            s.food -= meals;
            s.meals += meals;
            s.passes += 1;
        }
    }
}
