//! Chests' part of the random worlds (`chests.rs`): every town's second desk and a depot of its, counted now and then
//! by a host whose chests hold what the core took them to, give or take what a player did since; the town's stock
//! shown in them at once ([`Society::show_stock`]), each move written as far as the chests have room (a hopper took
//! some), what couldn't be done sent back ([`Society::unmoved`]). Now and then the desk is broken and put back, the
//! town's again. Checked: every count is a whole number at or above nothing, every shortfall too, and no point stands
//! where a broken desk's place is remembered.

use super::super::*;
use enterprise_core::points::{Kind, Spot};

fn spot(t: usize, depot: bool) -> Spot {
    Spot { dim: "minecraft:overworld".into(), x: 1000 * t as i32, y: 64, z: if depot { 9 } else { 7 } }
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        s.add_point(Kind::Desk, spot(t, false), t).unwrap();
        if r.below(2) == 0 {
            s.add_point(Kind::Depot, spot(t, true), t).unwrap();
        }
    }
}

/// One town's chests counted, its stock shown in them, the moves written as far as they go.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(3) == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let n = s.content().goods.len();
    let mut counted = Vec::new();
    for at in [spot(t, false), spot(t, true)] {
        let Some(c) = s.chests(&at) else { continue };
        // What its chests hold: what the core took them to, and now and then a player's doing.
        let mut held: Vec<f64> = (0..n).map(|g| c.held.get(g).copied().unwrap_or(0.0)).collect();
        if r.below(4) == 0 {
            let g = r.below(n as u64) as usize;
            held[g] = (held[g] + r.range(-40.0, 40.0)).floor().max(0.0);
        }
        let counts: Vec<(usize, f64)> = held.iter().enumerate().filter(|(_, h)| **h > 0.0).map(|(g, h)| (g, *h)).collect();
        s.take_stock(&at, &counts, r.below(8) as u32).unwrap();
        counted.push(at);
    }
    for m in s.show_stock(&counted) {
        // A hopper or a player got there first, now and then: some of the move can't be done.
        let short = if r.below(3) == 0 { (m.count.abs() as f64 * r.unit()).floor() as i64 * m.count.signum() } else { 0 };
        if short != 0 {
            assert!(s.unmoved(&m.at, m.kind, m.good, short));
        }
    }
    // Now and then the desk is broken, its chests' goods with it, and put back: the town's again.
    if r.below(20) == 0 {
        let at = spot(t, false);
        if s.point(&at).is_some() {
            let towns = s.town_count();
            s.remove_point(&at).unwrap();
            assert_eq!(s.found("again", at.clone()).unwrap(), t, "a desk put back is its town's");
            assert_eq!(s.town_count(), towns);
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for (at, p) in &s.points().0 {
        if s.former_desks().0.contains_key(at) {
            return Err(format!("a point stands at {at:?}, a broken desk's remembered place"));
        }
        let ours = at.z == 7 || at.z == 9;
        if let Some(n) = p.counted.iter().find(|n| **n < 0.0 || (ours && n.fract() != 0.0)) {
            return Err(format!("the point at {at:?} is taken to hold {n}"));
        }
        if let Some(n) = p.short.iter().find(|n| !n.is_finite() || **n < 0.0) {
            return Err(format!("the point at {at:?} came up {n} short"));
        }
    }
    for (at, &town) in &s.former_desks().0 {
        if town >= s.town_count() {
            return Err(format!("a broken desk at {at:?} was no town's ({town})"));
        }
    }
    Ok(())
}
