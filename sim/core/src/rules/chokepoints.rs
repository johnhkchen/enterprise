//! Chokepoints' steps: a wagon home is seen by each town whose land its road crosses, with what going around posts
//! there would cost it, before it pays its tolls (`Services`); what it brings home after them is its route's takings;
//! every few days each town reviews its tolls and posts (`Review`). See `chokepoints.rs`.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::chokepoints::*;
use crate::content::Content;
use crate::map::Map;
use crate::mint::Mint;
use crate::state::*;
use crate::territory::Territory;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        review.in_set(Step::Review).after(crate::rules::territory::review),
        observe_crossings.in_set(Step::Services).before(crate::rules::territory::tolls),
        takings.in_set(Step::Services).after(crate::rules::territory::tolls),
    ));
}

/// Each town reviews its tolls when `toll_review_days` have passed (`Territory.reviewTolls`, at each touch of its
/// desk in the mod), at the mint's rate. The first look only starts its clock.
pub fn review(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mint: Res<Mint>,
        mut territory: ResMut<Territory>, q: Query<&Town>) {
    let d = &content.dials;
    if !d.toll_reviews {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let spurs = mint.rate(now, day);
    for &e in &towns.0 {
        let Ok(town) = q.get(e) else { continue };
        territory.holdings.entry(town.id).or_default().review_tolls(now, day, spurs, d);
    }
}

/// A wagon home on a road over the map is seen by each town whose land it crossed, carrying its purse less the tolls
/// before (`Caravans.payTolls` → `observeCrossing`): the route's value is what its wagons bring home a trip.
#[allow(clippy::too_many_arguments)]
pub fn observe_crossings(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, map: Res<Map>, mint: Res<Mint>,
        takings: Res<Takings>, mut territory: ResMut<Territory>, places: Query<&Town>,
        routes: Query<(Entity, &Route, &RouteState, Option<&RoutePath>)>) {
    let spurs = mint.rate(clock.now, content.day_ticks);
    let per_hex = per_hex(&content) * spurs;
    let home = |t: usize| towns.0.get(t).and_then(|&e| places.get(e).ok()).and_then(|town| town.hex);
    for entity in in_order(routes.iter().map(|(e, r, _, _)| (e, r))) {
        let (_, route, state, path) = routes.get(entity).unwrap();
        let Some(path) = path else { continue };
        if state.arrived != Some(Leg::Home) {
            continue;
        }
        let Some(wagon) = state.wagon.as_ref() else { continue };
        let (Some(start), Some(goal)) = (home(route.from_id), home(route.to_id)) else { continue };
        observe(&mut territory, &map, &content.dials, route.seq, [route.from_id, route.to_id], start, goal, &path.0,
            wagon.purse * spurs, per_hex, takings.expected_value(route.seq), &home);
    }
}

/// What a wagon brings home, after its tolls, in spurs: its route's takings (`Route.spursCollected`).
pub fn takings(clock: Res<Clock>, content: Res<Content>, mint: Res<Mint>, mut takings: ResMut<Takings>,
        routes: Query<(&Route, &RouteState)>) {
    let spurs = mint.rate(clock.now, content.day_ticks);
    let mut home: Vec<(usize, f64)> = routes.iter()
        .filter(|(_, s)| s.arrived == Some(Leg::Home))
        .filter_map(|(r, s)| s.wagon.as_ref().map(|w| (r.seq, w.purse * spurs)))
        .collect();
    home.sort_by_key(|&(seq, _)| seq);
    for (seq, spurs) in home {
        takings.add(seq, spurs);
    }
}
