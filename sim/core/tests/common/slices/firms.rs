//! Firms' part of the random worlds (`firms.rs`): each firm's owners' shares add up to 1, no hand works for two firms,
//! and no firm's treasury is below nothing.

use super::super::*;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

pub fn check(s: &Society, day: u64) -> Result<(), String> {
    let firms = s.firms();
    let mut hands: Vec<_> = firms.iter().flat_map(|f| f.employees.iter().map(|e| e.0)).collect();
    let n = hands.len();
    hands.sort();
    hands.dedup();
    if hands.len() != n {
        return Err(format!("day {day}: a hand working for two firms: {firms:?}"));
    }
    for f in &firms {
        let shares: f64 = f.owners.iter().map(|o| o.1).sum();
        if (shares - 1.0).abs() > 1e-9 {
            return Err(format!("firm {}: its owners' shares add up to {shares}", f.id));
        }
        if f.treasury.is_negative() {
            return Err(format!("firm {}: its treasury {} below nothing", f.id, f.treasury));
        }
    }
    Ok(())
}
