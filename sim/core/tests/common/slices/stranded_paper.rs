//! Stranded paper in the random worlds (`stranded.rs`): in a third of the worlds a town is made a hub that runs to a market
//! (a run of its own, opened if the base world's roads gave it none); now and then a town draws a bill on that market for
//! a host (or holds a crate paid for by one), and a road is closed (a route wound down): paper stranded for the hub to buy,
//! at its wagon's call or the town's. The invariants: a hub's book never runs backwards, nor pays more coin than the face it
//! bought; no bill is drawn on its own holder; a bill filed in a warehouse is its owner's; no savings borrowed are negative;
//! a wagon's crates for its forwarder are aboard it.

use super::super::*;
use enterprise_core::bills::{BillState, Held};
use enterprise_core::cargo::{crated, Lading, Payment};

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(3) != 0 || s.town_count() < 3 {
        return;
    }
    let hub = r.below(s.town_count() as u64) as usize;
    s.make_hub(hub);
    let others: Vec<usize> = (0..s.town_count()).filter(|&t| t != hub && !s.is_off_map(t)).collect();
    if others.len() < 2 {
        return;
    }
    let capital = others[r.below(others.len() as u64) as usize];
    if s.legs_from(hub).iter().all(|l| l.to() != capital) {
        s.add_route(hub, capital, r.range(0.05, 0.4), 0.0, 3.0 + r.below(4) as f64);
    }
}

/// A host's doings: now and then a stale bill on a market a hub runs to, a crate paid for by one, a closed road.
pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    let hubs: Vec<usize> = (0..s.town_count()).filter(|&t| s.is_hub(t) && !s.is_off_map(t)).collect();
    if hubs.is_empty() {
        return;
    }
    let hub = hubs[r.below(hubs.len() as u64) as usize];
    if r.below(5) == 0 {
        let to: Vec<usize> = s.legs_from(hub).iter().map(|l| l.to()).filter(|&t| t != hub && !s.is_off_map(t)).collect();
        let towns: Vec<usize> = (0..s.town_count()).filter(|&t| t != hub && !s.is_off_map(t)).collect();
        if !to.is_empty() && towns.len() > 1 {
            let capital = to[r.below(to.len() as u64) as usize];
            let town = towns[r.below(towns.len() as u64) as usize];
            if town != capital {
                let bill = s.draw_bill(capital, town, r.range(2.0, 40.0));
                if r.below(3) == 0 {
                    // The goods it paid for, crated for the capital: the bill names them.
                    let good = r.below(s.content().goods.len() as u64) as usize;
                    let count = r.range(1.0, 200.0).floor();
                    let lading = Lading { payment: Payment::Paid, ..Lading::new(town, capital, s.now()) };
                    if let Some(id) = s.deliver_pallet(town, lading, crated(s.content(), good, count)) {
                        s.bill_pallets(bill, &[id]);
                    }
                }
            }
        }
    }
    // A road closes, now and then, once the world is under way.
    if day > 5 && r.below(30) == 0 && s.route_count() > 0 {
        let (from, to) = s.route_ends(r.below(s.route_count() as u64) as usize);
        s.close_route(from, to);
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let b = s.stranded_book(t);
        if [b.face, b.coin, b.promised, b.credit].iter().any(|v| v.is_negative()) {
            return Err(format!("hub {t}'s stranded book ran backwards: {b:?}"));
        }
        if b.coin > b.face {
            return Err(format!("hub {t} paid {} in coin for {} of paper", b.coin, b.face));
        }
        if b.bills > 0 && !s.is_hub(t) {
            return Err(format!("town {t} bought stranded paper, and is no hub"));
        }
        if s.deposits(t) < -1e-9 {
            return Err(format!("town {t} owes its households {}", s.deposits(t)));
        }
    }
    for route in 0..s.route_count() {
        let Some(m) = s.wagon_manifest(route) else { continue };
        let (aboard, ..) = s.wagon_pallets(route);
        if let Some(x) = m.lines.iter().filter(|l| l.for_forwarder).find(|l| !aboard.iter().any(|p| p.id == l.pallet)) {
            return Err(format!("route {route}'s wagon has {x:?} for its forwarder, not aboard"));
        }
    }
    for b in s.bills() {
        if b.drawee == b.payee {
            return Err(format!("a bill drawn on its own payee: {b:?}"));
        }
        if let (BillState::Outstanding, Held::Town(t)) = (b.state, b.held) {
            if b.payee != t {
                return Err(format!("bill {} is filed with town {t}, payable to {}", b.id, b.payee));
            }
        }
    }
    Ok(())
}
