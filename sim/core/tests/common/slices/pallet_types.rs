//! The pallet registry in the random worlds (`pallet.rs`): since `pallets-for-crates` every crate under a bill is a
//! pallet in it. The invariants here keep the registry in step with the world: its index matches each pallet's place, a
//! wagon's pallets ride only while its wagon is on the road and are exactly its manifest's lines, a hub's custody
//! records are exactly the pallets in its custody, and a pallet on a carrier or in custody is sealed and unchanged since.
//! `pallet-sweep` brings the rest (`docs/pallets.md` section 5).

use super::super::*;
use enterprise_core::pallet::{At, Pallets};

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let p = s.world.resource::<Pallets>();
    let indexed: usize = p.by_place.values().map(|v| v.len()).sum();
    if indexed != p.pallets.len() {
        return Err(format!("{indexed} pallets indexed, {} in the registry", p.pallets.len()));
    }
    for (place, ids) in &p.by_place {
        if let Some(id) = ids.iter().find(|id| p.get(**id).is_none_or(|x| x.at != *place)) {
            return Err(format!("pallet {id} indexed at {place:?}, but it's at {:?}", p.get(*id).map(|x| x.at)));
        }
        let carried = matches!(place, At::Wagon(_) | At::Custody(_));
        if let Some(x) = ids.iter().filter_map(|id| p.get(*id)).find(|x| carried && (x.seal.is_none() || !x.intact())) {
            return Err(format!("pallet {} at {place:?} isn't sealed as it was: {x:?}", x.id));
        }
    }
    for route in 0..s.route_count() {
        let aboard: Vec<u64> = p.at(At::Wagon(route)).to_vec();
        let Some(m) = s.wagon_manifest(route) else {
            if !aboard.is_empty() {
                return Err(format!("route {route} has no wagon on the road, yet pallets {aboard:?} are aboard it"));
            }
            continue;
        };
        let mut lines: Vec<u64> = m.lines.iter().map(|l| l.pallet).collect();
        let mut held = aboard.clone();
        lines.sort_unstable();
        held.sort_unstable();
        if lines != held {
            return Err(format!("route {route}'s manifest lists {lines:?}, but {held:?} are aboard"));
        }
    }
    for t in 0..s.town_count() {
        let mut records: Vec<u64> = s.hub(t).custody.keys().copied().collect();
        let mut held: Vec<u64> = p.at(At::Custody(t)).to_vec();
        records.sort_unstable();
        held.sort_unstable();
        if records != held {
            return Err(format!("hub {t} keeps custody records of {records:?}, but holds {held:?}"));
        }
    }
    Ok(())
}
