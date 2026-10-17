//! The atlas and books' part of the random worlds (`atlas.rs`): a few towns have heard of others already, some
//! markets were counted (a host's stocktake, fresh or as its own wagon left), and now and then a host brings a town
//! news or counts a market. Wagons fill the atlases as they run, keep both towns' books and tally their reasons; towns
//! short of comforts commission routes of their own.

use super::super::*;
use enterprise_core::atlas::Kind;

pub fn extend(s: &mut Society, r: &mut Rng) {
    let n = s.town_count() as u64;
    for _ in 0..r.below(n + 1) {
        let (a, b) = (r.below(n) as usize, r.below(n) as usize);
        s.learn(a, b);
    }
    for t in 0..s.town_count() {
        if r.below(3) == 0 {
            let held: Vec<(usize, f64)> = s.town(t).stock.iter().copied().enumerate().collect();
            s.record_stocktake(t, &held, if r.below(2) == 0 { Some(0) } else { None });
        }
    }
}

/// A host's doings: now and then a town hears of a market, or a market is counted as it stands.
pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    let n = s.town_count() as u64;
    if r.below(8) == 0 {
        let (a, b) = (r.below(n) as usize, r.below(n) as usize);
        s.learn(a, b);
    }
    if r.below(8) == 0 {
        let t = r.below(n) as usize;
        let held: Vec<(usize, f64)> = s.town(t).stock.iter().copied().enumerate().collect();
        s.record_stocktake(t, &held, Some(day * DAY));
    }
}

/// Both ends of a road a wagon has run know each other, and no town knows itself; books hold only dealings, as far
/// back as a library keeps them at most; prices in them are what was paid; reasons are counted at least once.
pub fn check(s: &Society, day: u64) -> Result<(), String> {
    let goods = s.content().goods.len();
    for t in 0..s.town_count() {
        let known = s.known(t);
        if known.contains(&t) {
            return Err(format!("town {t} knows itself: {known:?}"));
        }
        let mut sorted = known.clone();
        sorted.sort();
        sorted.dedup();
        if sorted.len() != known.len() {
            return Err(format!("town {t} knows a market twice: {known:?}"));
        }
        for e in s.books(t) {
            if e.kind == Kind::Why || e.day > day || e.day + enterprise_core::atlas::LIBRARY_DAYS < day
                || e.count < 0.0 || e.money < 0.0 || !e.count.is_finite() || !e.money.is_finite() {
                return Err(format!("town {t}'s books hold {e:?} on day {day}"));
            }
            if let (Some(m), Some(g)) = (e.with, e.good) {
                if s.last_price(t, m, g).is_some_and(|p| !(p > 0.0 && p.is_finite())) {
                    return Err(format!("town {t}'s last price for {g} at {m} isn't a price"));
                }
            }
        }
        for w in s.whys(t) {
            if w.times == 0 || w.items < 0.0 || w.last > day {
                return Err(format!("town {t}'s reason {w:?} on day {day}"));
            }
        }
        if let Some(take) = s.stocktake(t) {
            if take.stock.len() != goods || take.when.is_some_and(|w| w > day * DAY + DAY) {
                return Err(format!("town {t}'s stocktake {take:?}"));
            }
        }
    }
    for route in 0..s.route_count() {
        let (from, to) = s.route_ends(route);
        if s.route(route).trips > 0 && (!s.known(from).contains(&to) || !s.known(to).contains(&from)) {
            return Err(format!("route {route} has run, yet {from} and {to} don't know each other"));
        }
    }
    Ok(())
}
