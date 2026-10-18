//! Trade points' part of the random worlds (`points.rs`): every town's desk a point, some with a locker too; now and
//! then a host takes stock at one, its chests holding more or less than the town does (a player put goods in or took
//! them out). Checked: every point is a town's, and what it last counted is a count.

use super::super::*;
use enterprise_core::points::{Kind, Spot};

fn spot(t: usize, locker: bool) -> Spot {
    Spot { dim: "minecraft:overworld".into(), x: 1000 * t as i32, y: 64, z: if locker { 3 } else { 0 } }
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        s.add_point(Kind::Desk, spot(t, false), t).unwrap();
        if r.below(3) == 0 {
            s.add_point(Kind::Locker, spot(t, true), t).unwrap();
        }
    }
}

/// A stocktake at a town's desk or locker: each good its chests hold around what the town holds, some none.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(2) == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let at = spot(t, r.below(3) == 0);
    if s.point(&at).is_none() {
        return;
    }
    let stock = s.town(t).stock;
    let mut counts = Vec::new();
    for (g, n) in stock.iter().enumerate() {
        if r.below(4) != 0 {
            counts.push((g, (n * r.range(0.0, 1.5)).floor()));
        }
    }
    s.take_stock(&at, &counts, 27).unwrap();
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for (at, p) in &s.points().0 {
        if p.town >= s.town_count() {
            return Err(format!("the point at {at:?} is for no town ({})", p.town));
        }
        if let Some(n) = p.counted.iter().find(|n| !n.is_finite() || **n < 0.0) {
            return Err(format!("the point at {at:?} last counted {n}"));
        }
    }
    Ok(())
}
