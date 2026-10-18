//! Pallet claims in the random worlds (`pallet-claims`): in some worlds a road gets a second route to the same buyer, and
//! now and then a contracted DDP pallet waits at the seller, so two routes compete for one pallet. The invariant: a
//! pallet a plan holds is at the town its claiming route leaves from (a claim never follows a pallet onto a wagon or
//! into custody) and the claiming route exists.

use super::super::*;
use enterprise_core::cargo::{crated, Lading, Payment};
use enterprise_core::pallet::At;
use enterprise_core::Incoterm;

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(4) != 0 || s.route_count() == 0 {
        return;
    }
    let (from, to) = s.route_ends(r.below(s.route_count() as u64) as usize);
    if from != to && !s.is_off_map(from) && !s.is_off_map(to) {
        s.add_route(from, to, r.range(0.05, 0.4), 0.0, 3.0 + r.below(4) as f64);
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    if day < 2 || r.below(8) != 0 || s.route_count() == 0 {
        return;
    }
    let (from, to) = s.route_ends(r.below(s.route_count() as u64) as usize);
    if from == to || s.is_off_map(from) || s.is_off_map(to) {
        return;
    }
    let good = r.below(s.content().goods.len() as u64) as usize;
    let count = r.range(8.0, 120.0).floor();
    let id = s.draft(from, to, good, count, 0.05, s.now(), s.content().day_ticks, 1, Incoterm::Ddp, true);
    let lading = Lading { contract: Some(id), payment: Payment::Owed, copy: true, ..Lading::new(from, to, s.now()) };
    s.deliver_pallet(from, lading, crated(s.content(), good, count));
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let p = s.pallets();
    for (id, x) in &p.pallets {
        let Some(route) = x.claimed else { continue };
        if route >= s.route_count() {
            return Err(format!("pallet {id} is claimed by route {route}, which doesn't exist"));
        }
        let home = s.route_ends(route).0;
        if x.at != At::Town(home) {
            return Err(format!("pallet {id} is claimed by route {route} (from town {home}) but is at {:?}", x.at));
        }
    }
    Ok(())
}
