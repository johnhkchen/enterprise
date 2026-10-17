//! Freight hubs in the random worlds (`freight.rs`): hubs emerge where the base world's roads pass a town with the
//! food, the money and a Merchant; in a third of the worlds a town is given a storehouse at once, and then a town whose
//! route ends there may owe a market the hub runs to under a contract, so its goods go on through the hub, with a bill
//! on the buyer. Now and then a host hands a hub's forwarder a crate for a market it serves. The invariants: only hubs
//! hold freight; no crate in a forwarder's hands is made out to the hub itself, or less than nothing; a booking is the
//! crate's own stacks on a run from the hub to its consignee, carted in; blocks and the shops' tickets stay within half a wagon, a block's runs
//! within `block_runs`; the books never run backwards; a wagon's crates on freight are aboard it, and it stops only at
//! hubs.

use super::super::*;
use enterprise_core::cargo::Shipment;

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(3) != 0 || s.town_count() < 3 {
        return;
    }
    let hub = r.below(s.town_count() as u64) as usize;
    s.make_hub(hub);
    // A town whose route ends at the hub owes a market the hub runs to.
    let shippers: Vec<usize> = (0..s.route_count()).map(|route| s.route_ends(route)).filter(|&(_, to)| to == hub)
        .map(|(from, _)| from).collect();
    if shippers.is_empty() || r.below(2) == 0 {
        return;
    }
    let seller = shippers[r.below(shippers.len() as u64) as usize];
    let buyers: Vec<usize> = s.legs_from(hub).iter().map(|l| l.to()).filter(|&b| b != seller && b != hub).collect();
    if buyers.is_empty() {
        return;
    }
    let buyer = buyers[r.below(buyers.len() as u64) as usize];
    let good = r.below(s.content().goods.len() as u64) as usize;
    let price = s.content().goods[good].value;
    s.draft(seller, buyer, good, (1 + r.below(4)) as f64 * 16.0, price, r.below(5) * DAY, (2 + r.below(5)) * DAY,
        1 + r.below(8) as u32, Incoterm::Dap, true);
}

/// A host's doings: now and then a crate for a market a hub serves, into its forwarder's hands.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(6) != 0 {
        return;
    }
    let hubs: Vec<usize> = (0..s.town_count()).filter(|&t| s.is_hub(t)).collect();
    if hubs.is_empty() {
        return;
    }
    let hub = hubs[r.below(hubs.len() as u64) as usize];
    let to: Vec<usize> = s.legs_from(hub).iter().map(|l| l.to()).filter(|&t| t != hub).collect();
    if to.is_empty() {
        return;
    }
    let consignee = to[r.below(to.len() as u64) as usize];
    let good = r.below(s.content().goods.len() as u64) as usize;
    let shipper = r.below(s.town_count() as u64) as usize;
    s.forward_shipment(hub, Shipment { good, count: r.range(1.0, 400.0).floor(), shipper, consignee: Some(consignee),
        contract: None, settled: true, copy: false }, r.below(2) == 0);
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let c = s.content();
    let share = (c.dials.wagon_slots as f64 * c.dials.freight_share) as u32;
    for t in 0..s.town_count() {
        let h = s.hub(t);
        if !s.is_hub(t) && (!h.held.is_empty() || !h.blocks.is_empty()) {
            return Err(format!("town {t} isn't a hub, yet holds freight: {h:?}"));
        }
        for held in &h.held {
            let x = &held.shipment;
            if !(x.count >= 0.0 && x.count.is_finite()) || x.consignee == Some(t) || x.consignee.is_none() {
                return Err(format!("hub {t}'s forwarder holds {x:?}"));
            }
            if let Some(b) = held.booked {
                if b.stacks != x.stacks(c) || b.leg.from() != t || !(b.fare >= 0.0 && b.fare.is_finite()) {
                    return Err(format!("hub {t}'s crate {x:?} is booked {b:?}"));
                }
                if b.leg.to() != x.consignee.unwrap() || !held.carted {
                    return Err(format!("hub {t}'s crate {x:?} is booked on a run elsewhere, or uncarted: {b:?}"));
                }
            }
        }
        for (key, b) in &h.blocks {
            if b.stacks > share || b.runs > c.dials.block_runs || !(b.price >= 0.0 && b.price.is_finite()) || b.wanted < 0.0 {
                return Err(format!("hub {t}'s block on {key}: {b:?}"));
            }
        }
        if let Some((key, n)) = h.tickets.iter().find(|(_, n)| **n > share) {
            return Err(format!("hub {t}'s shop on {key} has {n} tickets"));
        }
        let b = &h.book;
        if [b.fees, b.face, b.freight, b.drayage, b.landing].iter().any(|v| !(*v >= 0.0 && v.is_finite())) {
            return Err(format!("hub {t}'s book ran backwards: {b:?}"));
        }
    }
    for route in 0..s.route_count() {
        let Some(f) = s.wagon_freight(route) else { continue };
        let (aboard, _) = s.wagon_shipments(route);
        if let Some(x) = f.booked.iter().find(|x| !aboard.contains(x)) {
            return Err(format!("route {route}'s wagon has {x:?} on freight, not aboard"));
        }
        if let Some(stop) = f.stop.filter(|stop| !s.is_hub(stop.hub)) {
            return Err(format!("route {route}'s wagon stops at {}, not a hub", stop.hub));
        }
    }
    Ok(())
}
