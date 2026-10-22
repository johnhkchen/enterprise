//! The host's pallets' part of the random worlds (`pallet_host.rs`): now and then a player has a town's Merchant pack
//! some of a good it holds for another town (`pack_for`), and a host puts what the core offers in a desk's chests, whole,
//! within the slots it has free (`stow_view`, `stowed`), while the rules load, carry, open and take apart the pallets
//! as they do. Checked every day, whatever the rules did: a pallet shown stands in the warehouse of the town whose desk
//! shows it, a desk is offered only what its town holds and shows nowhere, and never more than the slots it has.

use super::super::*;
use enterprise_core::pallet::At;
use enterprise_core::points::{Kind, Spot};
use enterprise_core::wire::{Answer, Place, Point, PointKind};

fn desk(t: usize) -> Point {
    Point { kind: PointKind::Desk, dim: "minecraft:overworld".into(), x: 1000 * t as i32, y: 64, z: 0 }
}

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let towns = s.town_count();
    if towns < 2 {
        return;
    }
    if r.below(4) == 0 {
        let t = r.below(towns as u64) as usize;
        let to = (t + 1 + r.below(towns as u64 - 1) as usize) % towns;
        let stock = s.town(t).stock;
        let g = r.below(stock.len() as u64) as usize;
        if stock[g] >= 1.0 {
            s.pack_for(t, to, &[(g, (stock[g] * r.range(0.1, 1.0)).floor())]);
        }
    }
    if r.below(2) == 0 {
        let t = r.below(towns as u64) as usize;
        let at = desk(t);
        let free = r.below(10) as u32;
        if let Some(Answer::Stow { pallets, .. }) = s.stow_view(&at, free) {
            for p in pallets {
                s.stowed(&at, p.id);
            }
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let spot = |p: &Point| Spot { dim: p.dim.clone(), x: p.x, y: p.y, z: p.z };
    for p in s.pallets().pallets.values() {
        let v = s.pallet_view(p);
        if let Some(at) = &v.shown {
            let town = s.point(&spot(at)).filter(|d| d.kind == Kind::Desk).map(|d| d.town);
            if town.is_none_or(|t| p.at != At::Town(t)) {
                return Err(format!("pallet {} shown at {at:?} stands at {:?}", p.id, p.at));
            }
        }
        if v.items as usize != p.packages.len() {
            return Err(format!("pallet {} shown as {} items for {} packages", p.id, v.items, p.packages.len()));
        }
    }
    for t in 0..s.town_count() {
        let at = desk(t);
        if s.point(&spot(&at)).is_none() {
            continue;
        }
        let Some(Answer::Stow { pallets, .. }) = s.stow_view(&at, 5) else { return Err(format!("town {t}: no stow answer")) };
        if pallets.iter().map(|p| p.items).sum::<u32>() > 5 {
            return Err(format!("town {t}: offered more than five slots take"));
        }
        if let Some(p) = pallets.iter().find(|p| p.shown.is_some() || p.at != (Place::Town { town: t as u32 })) {
            return Err(format!("town {t}: offered pallet {} shown at {:?}, standing at {:?}", p.id, p.shown, p.at));
        }
    }
    Ok(())
}
