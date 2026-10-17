//! Crates and their papers in the random worlds (`cargo.rs`): the base world's contracts already ship as shipments,
//! farms crate what they owe, and wagons carry lots for sale crated. Now and then a player carries a crate: one made out
//! to a contract's buyer (paid for or not), into its warehouse; or one made out to another town, left to be fetched;
//! or a few empty crates. A town draws up papers of its own now and then. The invariants: no shipment is negative,
//! none waits at its own consignee (it's received at once), and the paperwork's sums never run backwards.

use super::super::*;
use enterprise_core::cargo::{Shipment, LADING};
use enterprise_core::State;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let (n_towns, n_goods) = (s.town_count(), s.content().goods.len());
    if n_towns < 2 {
        return;
    }
    match r.below(10) {
        0 => {
            // A player's crate for a contract in force, into the buyer's warehouse.
            let open: Vec<_> = s.contracts().into_iter().filter(|c| c.state == State::Active).collect();
            if open.is_empty() {
                return;
            }
            let c = &open[r.below(open.len() as u64) as usize];
            let count = (r.range(0.0, 1.5) * c.quantity).floor();
            s.deliver_shipment(c.buyer, Shipment { good: c.good, count, shipper: c.seller, consignee: Some(c.buyer),
                contract: Some(c.id), settled: r.below(3) == 0, copy: true });
        }
        1 => {
            // A paid-for crate made out to one town, left at another for its wagons to fetch.
            let (at, to) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
            if at != to {
                s.deliver_shipment(at, Shipment { good: r.below(n_goods as u64) as usize, count: r.range(0.0, 700.0).floor(),
                    shipper: at, consignee: Some(to), contract: None, settled: true, copy: false });
            }
        }
        2 => {
            // Empty crates handed in.
            let t = r.below(n_towns as u64) as usize;
            for _ in 0..r.below(20) {
                s.deliver_shipment(t, Shipment { good: 0, count: 0.0, shipper: t, consignee: Some(t), ..Default::default() });
            }
        }
        3 => {
            let t = r.below(n_towns as u64) as usize;
            s.draw_papers(t, r.below(12) as u32, LADING);
        }
        _ => {}
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let store = s.shipments(t);
        if let Some(bad) = store.held.iter().find(|h| !(h.count >= 0.0 && h.count.is_finite())) {
            return Err(format!("town {t} holds a crate of {}", bad.count));
        }
        if let Some(own) = store.held.iter().find(|h| h.consigned_to(t)) {
            return Err(format!("town {t} holds a crate made out to itself unopened: {own:?}"));
        }
        let p = s.paperwork(t);
        if !(p.owed >= 0.0 && p.scribe_fees >= 0.0 && p.paper_owed >= 0.0 && p.ink_owed >= 0.0) {
            return Err(format!("town {t}: paperwork ran backwards: {p:?}"));
        }
    }
    for route in 0..s.route_count() {
        let (aboard, _) = s.wagon_shipments(route);
        if let Some(bad) = aboard.iter().find(|h| !(h.count >= 0.0 && h.count.is_finite())) {
            return Err(format!("route {route} carries a crate of {}", bad.count));
        }
    }
    Ok(())
}
