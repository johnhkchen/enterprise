//! Crates and their papers in the random worlds (`cargo.rs`): the base world's contracts already ship as pallets, farms
//! crate what they owe, and wagons carry lots for sale crated. Now and then a player carries a pallet: one made out to a
//! contract's buyer (paid for or not), into its warehouse; or one made out to another town, left to be fetched; or a
//! few empty crates. A town draws up papers of its own now and then. The invariants: no pallet holds less than nothing,
//! none waits at its own consignee (it's received at once), and the paperwork's sums never run backwards.

use super::super::*;
use enterprise_core::cargo::{one_pallet, Lading, Payment, LADING};
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
            let payment = if r.below(3) == 0 { Payment::Paid } else { Payment::Owed };
            let lading = Lading { contract: Some(c.id), payment, copy: true, ..Lading::new(c.seller, c.buyer, s.now()) };
            s.deliver_pallet(c.buyer, lading, one_pallet(s.content(), c.good, count));
        }
        1 => {
            // A paid-for pallet made out to one town, left at another for its wagons to fetch.
            let (at, to) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
            if at != to {
                let good = r.below(n_goods as u64) as usize;
                // (At least one: a crate with nothing in it is handed in below, and opened at once; one left waiting would be
                // a pallet of nothing, which `pallets.rs`'s check refuses.)
                let count = r.range(0.0, 700.0).floor().max(1.0);
                let lading = Lading { payment: Payment::Paid, ..Lading::new(at, to, s.now()) };
                s.deliver_pallet(at, lading, one_pallet(s.content(), good, count));
            }
        }
        2 => {
            // Empty crates handed in.
            let t = r.below(n_towns as u64) as usize;
            for _ in 0..r.below(20) {
                s.deliver_pallet(t, Lading::new(t, t, s.now()), one_pallet(s.content(), 0, 0.0));
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
        let held = s.town_pallets(t);
        if let Some(bad) = held.iter().find(|h| !(h.count() >= 0.0 && h.count().is_finite())) {
            return Err(format!("town {t} holds a pallet of {}", bad.count()));
        }
        if let Some(own) = held.iter().find(|h| h.consigned_to(t)) {
            return Err(format!("town {t} holds a pallet made out to itself unopened: {own:?}"));
        }
        let p = s.paperwork(t);
        if !(p.owed >= 0.0 && p.scribe_fees >= 0.0 && p.paper_owed >= 0.0 && p.ink_owed >= 0.0) {
            return Err(format!("town {t}: paperwork ran backwards: {p:?}"));
        }
    }
    for route in 0..s.route_count() {
        let (aboard, lots, _) = s.wagon_pallets(route);
        if let Some(bad) = aboard.iter().map(|h| h.count()).chain(lots.iter().map(|l| l.count()))
                .find(|n| !(*n >= 0.0 && n.is_finite())) {
            return Err(format!("route {route} carries a crate of {bad}"));
        }
    }
    Ok(())
}
