//! Itineraries and drops in the random worlds (`itinerary-drops`, `docs/pallets.md` section 3): no doings of its own
//! (every world's wagons have their calls); the invariants, at each day's end, for every wagon on the road: its calls end
//! where its leg ends, the calls on its way are neither of its route's ends, and every line of its manifest has a drop
//! at a call still ahead (going out, the way back's too): a booked line at its run's next stop, a line for the hub's
//! forwarder at home, a line made out to where the leg ends there, and each consolidation where its stop is.

use super::super::*;
use enterprise_core::state::Leg;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for route in 0..s.route_count() {
        let (Some(it), Some(m), Some(f)) = (s.wagon_itinerary(route), s.wagon_manifest(route), s.wagon_freight(route)) else {
            continue;
        };
        let Some(leg) = f.reckoned else { continue };
        let (home, far) = s.route_ends(route);
        let out = leg == Leg::Out;
        let end = if out { far } else { home };
        if it.end() != Some(end) || it.next >= it.calls.len() {
            return Err(format!("route {route}'s wagon, {leg:?} for {end}, has the calls {it:?}"));
        }
        if let Some(c) = it.on_way().iter().find(|c| c.town == home || c.town == far) {
            return Err(format!("route {route}'s wagon calls on its way at {}, one of its ends", c.town));
        }
        let ahead: Vec<usize> = it.ahead(it.next, out, end, home).into_iter().map(|(t, _)| t).collect();
        for l in &m.lines {
            let Some(drop) = l.drop else {
                return Err(format!("route {route}'s wagon carries pallet {} with no drop", l.pallet));
            };
            if !ahead.contains(&drop) {
                return Err(format!("route {route}'s pallet {} drops at {drop}, no call ahead ({ahead:?})", l.pallet));
            }
            if let Some(b) = l.booking.filter(|b| b.leg.to() != drop) {
                return Err(format!("route {route}'s pallet {} drops at {drop}, but it's booked to {}", l.pallet, b.leg.to()));
            }
            if l.for_forwarder && drop != home {
                return Err(format!("route {route}'s pallet {} for its hub's forwarder drops at {drop}, not home", l.pallet));
            }
            let to = s.pallets().get(l.pallet).map(|p| p.lading.consignee);
            if to == Some(end) && drop != end {
                return Err(format!("route {route}'s pallet {} is made out to {end}, where its leg ends, but drops at {drop}",
                    l.pallet));
            }
        }
        if let Some(cl) = m.containers.iter().find(|cl| !cl.drop.is_some_and(|d| ahead.contains(&d))) {
            return Err(format!("route {route}'s consolidation {} comes off at {:?}, no call ahead", cl.container, cl.drop));
        }
    }
    Ok(())
}
