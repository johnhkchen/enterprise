//! Bonds' steps (`bonds.rs`):
//!
//! - **Issue** (Review): an offer unsold for `bond_offer_days` lapses, and a bond unpaid twice its term defaults. A
//!   town with none open offers one for the work it was short of the money for: its stop's next level once wagons
//!   pass it, else what its works review wanted (`Works::short`), if a bond covers it. The capital keeps
//!   `royal_bonds_most` royal bonds out for its highways.
//! - **Papers** (at the far end, after the advances): the wagon's purse pays what its town owes the town here on bonds
//!   the town here holds, and the town here pays into it what it owes; then each buys the other's offered bond, the
//!   wagon's town out of its purse, the town here out of its treasury.
//! - **Plan:** the purse takes what home owes the far end on its bonds, and the price of the far end's offered bond if
//!   home would buy it; a trip to collect a quarter of a bond's face, or to buy one, is pressing.
//! - **Highways** (Review): once a day the crown lays the next surface on the most worn trail hex of a land road, out of
//!   what its royal bonds raised, from its treasury.

use bevy_ecs::prelude::*;

use super::{in_order, Step};
use crate::bonds::{Bonds, Purpose, State};
use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::state::*;
use crate::waystations::{rise_cost, Waystations, TOP};

pub fn register(schedule: &mut Schedule) {
    schedule.add_systems((
        issue.in_set(Step::Review).after(super::works::review).after(super::waystations::mend)
            .before(super::jobs::relocate).before(super::jobs::review),
        highways.in_set(Step::Review).after(issue)
            .before(super::jobs::relocate).before(super::jobs::review),
        papers.in_set(Step::Papers).after(super::advances::advances_papers).after(super::stranded::buy_at_town)
            .before(super::bills::present),
        plan.in_set(Step::PlanPapers).after(super::advances::plan_advances).before(super::bills::plan_papers),
    ));
}

fn reason() -> Reason {
    Reason::Other("bond".into())
}

fn repaid() -> Reason {
    Reason::Other("bond repaid".into())
}

/// What a town has idle to lend: its money beyond its reserve, what opening a road asks and `bond_idle`.
fn idle(content: &Content, money: Fixed, trades: &Trades, farm: Option<&crate::farms::Farm>) -> Fixed {
    money - super::farms::build_reserve(content, trades, farm) - Fixed::ceil(content.dials.commission_spare)
        - Fixed::ceil(content.dials.bond_idle)
}

pub fn issue(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, crown: Res<crate::realms::Crown>,
        ways: Res<Waystations>, mut bonds: ResMut<Bonds>, q: Query<(&Town, &crate::works::Works)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let d = &content.dials;
    for b in bonds.bonds.iter_mut() {
        match b.state {
            State::Offered if now >= b.offered + (d.bond_offer_days * day as f64) as u64 => b.state = State::Lapsed,
            State::Sold if b.repaid >= b.face => b.state = State::Repaid,
            State::Sold if b.sold.is_some_and(|s| now >= s + (2.0 * d.bond_term_days * day as f64) as u64) => {
                b.state = State::Defaulted
            }
            _ => {}
        }
    }
    let capital = crown.capital.filter(|&c| c < towns.0.len());
    for &e in &towns.0 {
        let Ok((town, works)) = q.get(e) else { continue };
        let t = town.id;
        if capital == Some(t) {
            let out = bonds.bonds.iter().filter(|b| b.issuer == t && b.open() && b.purpose == Purpose::Highways).count();
            if out < d.royal_bonds_most as usize {
                bonds.offer(t, Purpose::Highways, Fixed::ceil(d.royal_bond), now, d);
            }
        }
        if bonds.open_by(t).is_some() {
            continue;
        }
        // Its stop's next level, once wagons have shown its road is used; else what its works wanted.
        let stop = ways.stations.iter()
            .filter(|s| s.owner == t && s.level < TOP && s.rising.is_none() && s.passes >= d.bond_stop_passes as u64)
            .map(|s| (Purpose::StopLevel(s.hex), rise_cost(s.level + 1) - s.banked.to_f64()))
            .find(|&(_, need)| need > 0.0);
        let Some((purpose, need)) = stop.or(works.short) else { continue };
        if need <= 0.0 || need > d.bond_most {
            continue;
        }
        bonds.offer(t, purpose, Fixed::ceil(need.max(1.0)), now, d);
    }
}

/// A bond sold: its money where its purpose spends it (a stop's bank, the highways').
fn sold(bonds: &mut Bonds, ways: &mut Waystations, i: usize, holder: usize, now: u64) {
    let b = &mut bonds.bonds[i];
    b.state = State::Sold;
    b.sold = Some(now);
    b.holder = Some(holder);
    let (purpose, principal) = (b.purpose, b.principal);
    match purpose {
        Purpose::StopLevel(hex) => {
            if let Some(s) = ways.stations.iter_mut().find(|s| s.hex == hex) {
                s.banked += principal;
            }
        }
        Purpose::Highways => bonds.highways += principal,
        _ => {}
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn papers(clock: Res<Clock>, content: Res<Content>, mut bonds: ResMut<Bonds>, mut ways: ResMut<Waystations>,
        mut ledger: ResMut<Ledger>, mut cash: ResMut<CashBook>, mut routes: Query<(Entity, &Route, &mut RouteState)>,
        mut towns: Query<(&mut Money, &Trades, Option<&crate::farms::Farm>)>) {
    if bonds.bonds.is_empty() {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    for entity in in_order(routes.iter().map(|(e, r, _)| (e, r))) {
        let (_, route, mut state) = routes.get_mut(entity).unwrap();
        if state.arrived != Some(Leg::Out) {
            continue;
        }
        let route = *route;
        let (home, here) = (route.from_id, route.to_id);
        let Ok((mut money, trades, farm)) = towns.get_mut(route.to) else { continue };
        let Some(wagon) = state.wagon.as_mut() else { continue };
        let keep = crate::road::purse_keep(&content, &route, crate::road::mouths(Some(wagon)));
        for i in 0..bonds.bonds.len() {
            let b = &bonds.bonds[i];
            let due = b.due(now, day, d);
            if due <= Fixed::ZERO {
                continue;
            }
            let pay = if b.issuer == home && b.holder == Some(here) {
                // Home owes the town here: out of the purse, keeping its road food home.
                let pay = due.min((wagon.purse - keep).at_least_zero());
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(here, &mut money.0),
                    pay, repaid(), now, day);
                pay
            } else if b.issuer == here && b.holder == Some(home) {
                // The town here owes home: out of its treasury as far as it goes (a debt, as a bill is), into the purse.
                let pay = due.min(money.0.at_least_zero());
                post(&mut ledger, &mut cash, Side::treasury(here, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    pay, repaid(), now, day);
                pay
            } else {
                continue;
            };
            bonds.bonds[i].repaid += pay;
        }
        for i in 0..bonds.bonds.len() {
            let b = &bonds.bonds[i];
            if b.state != State::Offered {
                continue;
            }
            let price = b.principal;
            if b.issuer == here && bonds.held_by(home) < d.bonds_held_most as usize && wagon.purse - keep >= price {
                // Home buys the town here's, out of the purse its plan filled for it.
                post(&mut ledger, &mut cash, Side::wagon(route.seq, &mut wagon.purse), Side::treasury(here, &mut money.0),
                    price, reason(), now, day);
                sold(&mut bonds, &mut ways, i, home, now);
            } else if b.issuer == home && bonds.held_by(here) < d.bonds_held_most as usize
                    && idle(&content, money.0, trades, farm) >= price {
                // The town here buys home's: its money rides home in the purse.
                post(&mut ledger, &mut cash, Side::treasury(here, &mut money.0), Side::wagon(route.seq, &mut wagon.purse),
                    price, reason(), now, day);
                sold(&mut bonds, &mut ways, i, here, now);
            }
        }
    }
}

/// The purse takes what home owes the far end on its bonds and, where home would buy the far end's offered bond, its
/// price, as far as home's money goes beyond the road's.
#[allow(clippy::type_complexity)]
pub fn plan(clock: Res<Clock>, content: Res<Content>, bonds: Res<Bonds>, mut routes: Query<(&Route, &mut RouteState)>,
        towns: Query<(&Money, &Stock, &Trades, Option<&crate::farms::Farm>, Option<&crate::roster::Roster>)>) {
    if bonds.bonds.is_empty() {
        return;
    }
    let (now, day) = (clock.now, content.day_ticks);
    let d = &content.dials;
    for (route, mut state) in &mut routes {
        let Some(plan) = state.plan.as_mut() else { continue };
        let Ok((money, stock, trades, farm, roster)) = towns.get(route.from) else { continue };
        let keep_wheat = crate::road::keep_wheat(&content, trades, roster);
        let mouths = plan.crew.len().max(1) as f64;
        let means = money.0 - crate::road::road_money(&content, route, &stock.0, keep_wheat, mouths);
        let (home, far) = (route.from_id, route.to_id);
        for b in bonds.bonds.iter().filter(|b| b.issuer == home && b.holder == Some(far)) {
            let due = b.due(now, day, d);
            plan.purse += due.min((means - plan.purse).at_least_zero());
            plan.worth += due.to_f64();
        }
        // What the far end owes home: worth the trip, pressing once a quarter of the bond's due.
        for b in bonds.bonds.iter().filter(|b| b.issuer == far && b.holder == Some(home)) {
            let due = b.due(now, day, d);
            plan.worth += due.to_f64();
            plan.pressing |= due.to_f64() >= 0.25 * b.face.to_f64();
        }
        let can_hold = bonds.held_by(home) < d.bonds_held_most as usize;
        if let Some(b) = bonds.bonds.iter().find(|b| b.issuer == far && b.state == State::Offered).filter(|_| can_hold) {
            if idle(&content, money.0 - plan.purse, trades, farm) >= b.principal {
                plan.purse += b.principal;
                plan.pressing = true;
            }
        }
    }
}

/// The crown's king's highways: once a day, the next surface on the most worn trail hex of a land road (graded, then
/// paved), out of what its royal bonds raised, from its treasury.
#[allow(clippy::too_many_arguments)]
pub fn highways(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, crown: Res<crate::realms::Crown>,
        mut bonds: ResMut<Bonds>, mut map: ResMut<crate::map::Map>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>,
        mut outbox: ResMut<Outbox>, roads: Query<(&Route, &RoutePath)>, mut money: Query<&mut Money>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let today = now / day;
    let Some(capital) = crown.capital.filter(|&c| c < towns.0.len()) else { return };
    if bonds.highways <= Fixed::ZERO || bonds.highways_day >= today {
        return;
    }
    bonds.highways_day = today;
    let mut land: Vec<(&Route, &RoutePath)> = roads.iter().filter(|(r, _)| r.carrier != crate::map::Carrier::Ship).collect();
    land.sort_by_key(|(r, _)| r.seq);
    let Some((hex, route, path)) = super::waystations::most_worn(&map, &land, None, crate::map::PAVED, now, day) else { return };
    let cost = super::waystations::surface_cost(&map, hex);
    let Ok(mut m) = money.get_mut(towns.0[capital]) else { return };
    if bonds.highways < cost || m.0 < cost {
        return;
    }
    post(&mut ledger, &mut book, Side::treasury(capital, &mut m.0), Side::outside(), cost,
        Reason::Other("king's highways".into()), now, day);
    bonds.highways -= cost;
    bonds.highways_laid += 1;
    let next = map.surface(hex) + 1;
    super::waystations::lay(&mut map, &mut outbox, route, path, hex, next);
}
