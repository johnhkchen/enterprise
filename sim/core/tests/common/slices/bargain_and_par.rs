//! The bargain's part of the random worlds: a quarter of the worlds sell at the flat price (the dial off), and some
//! towns' lockers record what they can take a day, a session now and then replacing a locker's figures.

use super::super::*;

/// Lockers a town may have, numbered from 1.
const LOCKERS: u64 = 4;

/// One locker's session: some goods it can take a day, or none (its trades switched off).
fn session(s: &mut Society, r: &mut Rng, town: usize) {
    let goods = s.content().goods.len() as u64;
    let locker = 1 + r.below(LOCKERS);
    let per_day: Vec<(usize, f64)> = (0..r.below(3)).map(|_| (r.below(goods) as usize, r.range(0.0, 400.0).floor())).collect();
    s.record_capacity(town, locker, &per_day);
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(4) == 0 {
        s.set_dial("bargain", 0.0).unwrap();
    }
    for t in 0..s.town_count() {
        if r.below(3) == 0 {
            for _ in 0..1 + r.below(LOCKERS) {
                session(s, r, t);
            }
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    for t in 0..s.town_count() {
        if r.below(20) == 0 {
            session(s, r, t);
        }
    }
}

/// A town's capacity is never below 0, and a wanted good's par is at least a stack.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        for g in 0..s.content().goods.len() {
            if s.capacity(t, g) < 0.0 || s.par(t, g, true) < enterprise_core::bargain::MIN_PAR {
                return Err(format!("town {t}, good {g}: capacity {}, par {}", s.capacity(t, g), s.par(t, g, true)));
            }
        }
    }
    Ok(())
}
