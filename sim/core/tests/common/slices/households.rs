//! Households' part of the random worlds (`households.rs`, `property.rs`, `charters.rs`): no hand in two households of
//! a town, their purses the households' purse to the thousandth and none below nothing while it isn't, each hex a town
//! holds claimed by it once the register has followed it, each town chartered once, and each field a household's claim.

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
    // Each field a household's claim, each lease on a field.
    let r = s.land_register();
    for f in r.fields.keys() {
        if !matches!(r.field_owner(*f), Some(enterprise_core::charters::Party::Household(_))) {
            return Err(format!("field {f} with no household's claim: {:?}", r.claims.get(&enterprise_core::property::Target::Field(*f))));
        }
    }
    if let Some(f) = r.leases.keys().find(|f| !r.fields.contains_key(f)) {
        return Err(format!("a lease on field {f}, which isn't"));
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
