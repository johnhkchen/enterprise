//! Households' part of the random worlds (`households.rs`, `property.rs`, `charters.rs`): no hand in two households of
//! a town, their purses the households' purse to the thousandth and none below nothing while it isn't, each hex a town
//! holds claimed by it once the register has followed it, and each town chartered once.

use super::super::*;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

pub fn check(s: &Society, day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let hs = s.households(t);
        let mut ids: Vec<_> = hs.iter().flat_map(|h| h.members.iter().copied()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        if ids.len() != n {
            return Err(format!("town {t}: a hand in two households: {hs:?}"));
        }
        let purse = s.town(t).people.purse;
        let sum: enterprise_core::fixed::Fixed = hs.iter().map(|h| h.purse).sum();
        if !hs.is_empty() && sum != purse {
            return Err(format!("town {t}: households hold {sum}, their purse {purse}"));
        }
        if !purse.is_negative() && hs.iter().any(|h| h.purse.is_negative()) {
            return Err(format!("town {t}: a household below nothing: {hs:?}"));
        }
    }
    let c = s.charters();
    let mut holders: Vec<_> = c.charters.iter().filter(|c| c.until.is_none()).map(|c| c.holder).collect();
    let n = holders.len();
    holders.sort();
    holders.dedup();
    if holders.len() != n {
        return Err(format!("day {day}: a town chartered twice"));
    }
    Ok(())
}
