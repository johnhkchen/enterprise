//! Taxes' steps (`taxes.rs`):
//!
//! - **Assess** (Review): once a day, each town with a road to or from the capital owes the crown the rent of the hexes
//!   it holds for the days since the last assessment, and its inventory tax on what it holds beyond what it wants
//!   (`taxes::held_beyond`).
//! - **Collect** (at the far end, after the bonds): the capital's wagon takes what the town here owes out of its
//!   treasury; a town's wagon at the capital pays what its town owes out of its purse. What's collected goes to the
//!   king's highways.
//! - **Plan:** a town's wagon to the capital takes what its town owes in its purse.
//! - **Congestion** (home again, after the stops): a wagon pays each town whose crowded land it crossed.
//! - **Customs** (at the far end, once it has sold): a wagon of one people selling at a town of another pays it its duty.
//! - **Harbour** (at the far end, once it has sold): a ship landing at a port pays it its fee for each stack it landed,
//!   out of what it took there.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::bonds::Bonds;
use crate::cash_book::{post, CashBook, Side};
use crate::content::Content;
use crate::state::*;
use crate::taxes::{congestion_reason, customs_reason, harbour_reason, held_beyond, inventory_tax_reason, land_rent,
    land_tax_reason, Taxes};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        assess.in_set(Step::Review).after(super::bonds::highways).before(super::jobs::relocate).before(super::jobs::review),
        collect.in_set(Step::Papers).after(super::bonds::papers).before(super::bills::present),
        congestion.in_set(Step::Services).after(super::waystations::serve),
        customs.in_set(Step::Sell).after(super::realms::customs).before(super::harvest::redeem).before(super::cargo::relot),
        harbour.in_set(Step::Sell).after(customs).before(super::harvest::redeem).before(super::cargo::relot),
        plan.in_set(Step::PlanPapers).after(super::bonds::plan).before(super::bills::plan_papers),
    ));
}

fn capital(crown: &crate::realms::Crown, towns: &Towns) -> Option<usize> {
    crown.capital.filter(|&c| c < towns.0.len())
}

pub fn assess(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, crown: Res<crate::realms::Crown>,
        territory: Res<crate::territory::Territory>, map: Res<crate::map::Map>, mut taxes: ResMut<Taxes>,
        book: Res<crate::contracts::Book>, pallets: Res<crate::pallet::Pallets>, routes: Query<&Route>,
        places: Query<(&Town, &Stock)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let Some(capital) = capital(&crown, &towns) else { return };
    let last = *taxes.assessed.get_or_insert(now);
    if now < last + day {
        return;
    }
    taxes.assessed = Some(now);
    let days = (now - last) as f64 / day as f64;
    let mut realm: Vec<usize> = routes.iter()
        .filter_map(|r| if r.from_id == capital { Some(r.to_id) } else if r.to_id == capital { Some(r.from_id) } else { None })
        .filter(|&t| t != capital && t < towns.0.len()).collect();
    realm.sort();
    realm.dedup();
    for t in realm {
        let Ok((place, stock)) = places.get(towns.0[t]) else { continue };
        let aside = crate::cargo::set_aside_all(&book, &pallets, t, content.goods.len(), now, content.day_ticks);
        let held = held_beyond(&content, &stock.0, &aside);
        if held > 0.0 {
            *taxes.goods_owed.entry(t).or_default() += held * content.dials.inventory_tax * days;
        }
        let Some(holding) = territory.holding(t) else { continue };
        let rent: f64 = holding.hexes.iter().map(|&h| land_rent(&content, &map, h, Some(h) == place.hex)).sum();
        *taxes.land_owed.entry(t).or_default() += rent * days;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn collect(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, crown: Res<crate::realms::Crown>,
        mut taxes: ResMut<Taxes>, mut bonds: ResMut<Bonds>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut money: Query<&mut Money>) {
    let Some(capital) = capital(&crown, &towns) else { return };
    let (now, day) = (clock.now, content.day_ticks);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) || (route.from_id != capital && route.to_id != capital) {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let Ok(mut here) = money.get_mut(route.to) else { continue };
        let paid = if route.from_id == capital {
            // The capital's wagon collects from the town here.
            let pay = taxes.owed(route.to_id).min(here.0.at_least_zero());
            let (land, goods) = taxes.paid(route.to_id, pay);
            post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut here.0), Side::wagon(route.seq, &mut wagon.purse),
                land, land_tax_reason(), now, day);
            post(&mut ledger, &mut cash, Side::treasury(route.to_id, &mut here.0), Side::wagon(route.seq, &mut wagon.purse),
                goods, inventory_tax_reason(), now, day);
            pay
        } else {
            // The town's wagon pays at the capital, out of the purse, keeping its road food home.
            let keep = crate::road::purse_keep(&content, &route, crate::road::mouths(Some(wagon)));
            let pay = taxes.owed(route.from_id).min((wagon.purse - keep).at_least_zero());
            let (land, goods) = taxes.paid(route.from_id, pay);
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(capital, &mut here.0),
                land, land_tax_reason(), now, day);
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(capital, &mut here.0),
                goods, inventory_tax_reason(), now, day);
            pay
        };
        bonds.highways += paid;
    }
}

/// A town's wagon to the capital takes what its town owes of its land tax, as far as its money goes beyond the road's.
#[allow(clippy::type_complexity)]
pub fn plan(content: Res<Content>, towns: Res<Towns>, crown: Res<crate::realms::Crown>, taxes: Res<Taxes>,
        mut routes: Query<(&Route, &mut RouteState)>,
        homes: Query<(&Money, &Stock, &Trades, Option<&crate::roster::Roster>)>) {
    let Some(capital) = capital(&crown, &towns) else { return };
    for (route, mut state) in &mut routes {
        if route.to_id != capital {
            continue;
        }
        let Some(plan) = state.plan.as_mut() else { continue };
        let Ok((money, stock, trades, roster)) = homes.get(route.from) else { continue };
        let keep_wheat = crate::road::keep_wheat(&content, trades, roster);
        let mouths = plan.crew.len().max(1) as f64;
        let means = money.0 - crate::road::road_money(&content, route, &stock.0, keep_wheat, mouths);
        let owed = taxes.owed(route.from_id);
        plan.purse += owed.min((means - plan.purse).at_least_zero());
    }
}

/// A ship landing at the far end's port pays it `harbour_fee` for each stack it set out with, out of its purse, keeping
/// its road food home.
pub fn harbour(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>,
        mut routes: Query<(Entity, &Route, &mut RouteState)>, mut money: Query<&mut Money>) {
    let (now, day) = (clock.now, content.day_ticks);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) || route.carrier != crate::map::Carrier::Ship {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let Ok(mut port) = money.get_mut(route.to) else { continue };
        let keep = crate::road::purse_keep(&content, &route, crate::road::mouths(Some(wagon)));
        let due = crate::fixed::Fixed::floor(content.dials.harbour_fee * wagon.stacks_out as f64);
        let pay = due.min((wagon.purse - keep).at_least_zero());
        post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut port.0),
            pay, harbour_reason(), now, day);
    }
}

/// A wagon home again by land pays each town whose crowded land it crossed (not its own) `congestion_fee` for each stack
/// it carried, out and home, over each such hex as crowded as it is now.
#[allow(clippy::too_many_arguments)]
pub fn congestion(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, map: Res<crate::map::Map>,
        territory: Res<crate::territory::Territory>, pallets: Res<crate::pallet::Pallets>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState, Option<&RoutePath>)>,
        mut money: Query<&mut Money>) {
    let (now, day) = (clock.now, content.day_ticks);
    if content.dials.congestion_fee <= 0.0 {
        return;
    }
    for entity in in_order(routes.iter().map(|(e, r, _, _)| (e, r))) {
        let (_, route, mut state, path) = routes.get_mut(entity).unwrap();
        let Some(path) = path else { continue };
        if state.arrived != Some(Leg::Home) || route.carrier == crate::map::Carrier::Ship {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let carried = (wagon.stacks_out + crate::cargo::stacks(&content, &pallets, route.seq, wagon)) as f64;
        let mut owed: std::collections::BTreeMap<usize, f64> = Default::default();
        for &h in &path.0 {
            let crowded = map.crowding(h, now, day);
            let Some(owner) = territory.owner(h).filter(|&o| o != route.from_id) else { continue };
            if crowded > 0.0 {
                *owed.entry(owner).or_default() += content.dials.congestion_fee * carried * crowded;
            }
        }
        for (owner, due) in owed {
            let Some(&e) = towns.0.get(owner) else { continue };
            let Ok(mut m) = money.get_mut(e) else { continue };
            let pay = crate::fixed::Fixed::floor(due).min(wagon.purse.at_least_zero());
            post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(owner, &mut m.0), pay,
                congestion_reason(), now, day);
        }
    }
}

/// A wagon that sold at a town of another people pays it `customs_duty` of what it took there, rounded up (a duty owed),
/// as far as its purse goes.
pub fn customs(clock: Res<Clock>, content: Res<Content>, taxes: Res<Taxes>, mut ledger: ResMut<Ledger>,
        mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>, mut money: Query<&mut Money>) {
    if taxes.peoples.is_empty() {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let (Some(home), Some(here)) = (taxes.peoples.get(&route.from_id), taxes.peoples.get(&route.to_id)) else { continue };
        if home == here {
            continue;
        }
        let route = *route;
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let duty = wagon.takings_here.mul_ceil(content.dials.customs_duty).min(wagon.purse).at_least_zero();
        let Ok(mut m) = money.get_mut(route.to) else { continue };
        if !duty.is_positive() {
            continue;
        }
        post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(route.to_id, &mut m.0), duty,
            customs_reason(), now, day);
        wagon.takings_here -= duty;
    }
}
