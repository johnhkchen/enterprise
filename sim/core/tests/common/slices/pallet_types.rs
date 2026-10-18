//! The pallet registry in the random worlds (`pallet.rs`): since `pallets-for-crates` every crate under a bill is a
//! pallet in it. The invariants here keep the registry in step with the world: its index matches each pallet's place, a
//! wagon's pallets ride only while its wagon is on the road and are exactly its manifest's lines, a hub's custody
//! records are exactly the pallets in its custody, and a pallet on a carrier or in custody is sealed and unchanged since.
//! `pallet-sweep` brings the rest (`docs/pallets.md` section 5).

use super::super::*;
use enterprise_core::pallet::{At, Pallets};

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

/// A package is lost now and then from a pallet standing in a warehouse (`pallet-messages`: raided, broken, thrown in
/// lava), as a host would tell the core; the goods are booked as used, and the pallet goes on short.
/// And now and then a player carries one package of such a pallet to its consignee (`PackageBrought`): it arrives in
/// parts, and the rules that fetch, take into custody or take apart the rest must leave the registry whole.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let (lose, bring) = (r.below(4) == 0, r.below(4) == 0);
    let standing: Vec<(u64, usize, usize)> = s.world.resource::<Pallets>().pallets.values()
        .filter(|p| matches!(p.at, At::Town(_))).map(|p| (p.id, p.packages.len(), p.lading.consignee)).collect();
    if standing.is_empty() {
        return;
    }
    let (a, b) = (r.below(standing.len() as u64) as usize, r.below(standing.len() as u64) as usize);
    let (pa, pb) = (r.below(standing[a].1 as u64) as u32, r.below(standing[b].1 as u64) as u32);
    if bring {
        let (pallet, _, town) = standing[b];
        s.apply(&enterprise_core::wire::Input::PackageBrought { town, pallet, package: pb });
    }
    if lose && s.pallets().get(standing[a].0).is_some_and(|p| (pa as usize) < p.packages.len()) {
        s.apply(&enterprise_core::wire::Input::PackageLost { pallet: standing[a].0, package: pa });
    }
}

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
    if let Some(id) = p.arriving.keys().find(|id| !p.get(**id).is_some_and(|x| matches!(x.at, At::Town(_)))) {
        return Err(format!("pallet {id} is arriving in parts, but isn't standing in a town's warehouse"));
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
