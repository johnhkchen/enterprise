//! Realms' steps (`realms.rs`): `Work`, a realm's people use what it bought and make what it makes, a day at a time;
//! `Sell`, after a foreign wagon has sold at a town, the crown's customs on its takings (`Realms.customs`).
//!
//! A realm's wagons need no squad and a realm's bargains no roster: that is in the planning and selling steps
//! (`rules/plan.rs`, `rules/arrive.rs`), which read a [`Realm`](crate::realms::Realm) where they read a roster.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::realms::{make, Crown, Realm};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        work.in_set(Step::Work).before(super::farms::work),
        // (as `arriveAtDestination` takes them, straight after the sale: before any of the wagon's purse is spent)
        customs.in_set(Step::Sell).after(super::arrive::sell).before(super::harvest::redeem).before(super::cargo::relot),
    ));
}

/// A realm's days, once a day (`Realms.onServerTick`): as many as have passed, at most 5 at a time, as the mod does.
pub fn work(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>,
        mut realms: Query<(&Town, &mut Stock, &mut Realm)>) {
    let day = content.day_ticks;
    let mut order: Vec<_> = realms.iter_mut().collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (_, mut stock, mut realm) in order {
        if clock.now < realm.last_day + day {
            continue;
        }
        let days = (clock.now - realm.last_day) / day;
        realm.last_day += days * day;
        make(&content, &mut ledger, &mut stock, &mut realm, days.min(5) as f64);
    }
}

/// A foreign trader sold at a town of ours: the crown takes `customs` of its takings, from its purse into the
/// capital's treasury, if a capital is named.
#[allow(clippy::too_many_arguments)]
pub fn customs(clock: Res<Clock>, content: Res<Content>, crown: Res<Crown>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut realms: Query<&mut Realm>, all: Res<Towns>, mut towns: Query<&mut Money>) {
    let Some(capital) = crown.capital.filter(|&c| c < all.0.len()) else { return };
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let Ok(mut realm) = realms.get_mut(route.from) else { continue };
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let duty = (wagon.takings_here * content.dials.customs).min(wagon.purse).max(0.0);
        let Ok(mut money) = towns.get_mut(all.0[capital]) else { continue };
        if duty <= 0.0 {
            continue;
        }
        post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(capital, &mut money.0),
            duty, Reason::Other("customs".into()), clock.now, content.day_ticks);
        wagon.takings_here -= duty;
        realm.customs_paid += duty;
    }
}
