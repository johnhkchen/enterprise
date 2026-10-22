//! The crown's part of the random worlds through the wire (`crown_host.rs`): now and then a host names a capital,
//! mobilises (putting up the vendor the core asks for and counting it) or makes peace. Checked: the crown's shop stands at a shop point, and every market point is a town off the map.

use super::super::*;
use enterprise_core::crown_host::CrownShop;
use enterprise_core::points::{Kind, Spot};
use enterprise_core::wire::{Input, Output, Point, PointKind, VendorMode, VendorRead};

fn spot(kind_z: i32, k: u64) -> Spot {
    Spot { dim: "minecraft:overworld".into(), x: 9_000 + k as i32, y: 64, z: kind_z }
}

fn point(kind: PointKind, at: &Spot) -> Point {
    Point { kind, dim: at.dim.clone(), x: at.x, y: at.y, z: at.z }
}

/// (No market off the map opened by the host here: a town added shifts a world's whole course, and the sweep's mood floor
/// allows no slack for a town that happens to be shrinking (`random_worlds_sweep`): 3 seeds in 300 broke it with no rule
/// broken. `Input::OffMap` is `add_off_map` and `add_point`, which the `realms` and `war_army` slices run through the Rust
/// API; `tests/crown_host.rs` and the `crown` scenario open them through the wire.)
pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    if s.town_count() == 0 {
        return;
    }
    match r.below(30) {
        0 if s.capital().is_none() => {
            let t = r.below(s.town_count() as u64) as u32;
            s.apply(&Input::Capital { town: t, spurs_per_day: 4 * r.below(8) });
        }
        1 => {
            if let Some(capital) = s.capital() {
                let out = s.apply(&Input::Mobilise { town: capital as u32, spurs_per_day: 100 * r.below(6) });
                if out.iter().any(|o| matches!(o, Output::BuildCrownShop { .. })) {
                    let at = spot(-9_000, day);
                    if s.point(&at).is_none() && s.add_point(Kind::Shop, at.clone(), capital).is_ok() {
                        s.apply(&Input::CrownShop { point: point(PointKind::Shop, &at) });
                    }
                }
            }
        }
        // (No war declared here: a levy shrinks a town sharply, which the sweep's mood floor allows no slack for
        // (`random_worlds_sweep`); the `crown` and `war_army` slices declare through the Rust API in worlds that allow it,
        // and `tests/crown_host.rs` and the `crown` scenario declare through the wire.)
        3 => {
            s.apply(&Input::Peace);
        }
        _ => {
            // The crown's shop counted: what a player left in it, a card funded or not.
            let Some(at) = s.world.resource::<CrownShop>().at.clone() else { return };
            let Some(gold) = s.content().good("gold_ingot") else { return };
            let read = VendorRead {
                mode: [VendorMode::Idle, VendorMode::Buy, VendorMode::Buy][r.below(3) as usize],
                good: Some(gold as u32),
                deed: None,
                lot: 8,
                price: 1 + r.below(2_000) as u32,
                stock: r.below(30) as u32,
                room: 54,
                purse: r.below(4_000),
                coins: 0,
                touches: false,
            };
            s.apply(&Input::Vendor { point: point(PointKind::Shop, &at), read });
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    if let Some(at) = &s.world.resource::<CrownShop>().at {
        match s.point(at) {
            // (another slice may name another capital by the Rust API: the shop is taken down when it is next counted)
            Some(p) if p.kind == Kind::Shop => {}
            other => return Err(format!("the crown's shop at {at:?}, where {other:?} stands")),
        }
    }
    for (at, p) in &s.points().0 {
        if p.kind == Kind::Market && !s.is_off_map(p.town) {
            return Err(format!("a market point at {at:?} for town {}, which is not off the map", p.town));
        }
    }
    Ok(())
}
