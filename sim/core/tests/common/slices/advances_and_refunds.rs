//! Advances and refunds in the random worlds: some contracts drafted with an advance, and the invariants of the books
//! they keep.

use super::super::*;
use enterprise_core::State;

/// A few more contracts, a third of them with an advance, held by the buyer (its wagon carries the copy).
pub fn extend(s: &mut Society, r: &mut Rng) {
    let (n_towns, n_goods) = (s.town_count(), s.content().goods.len());
    if n_towns < 2 {
        return;
    }
    for _ in 0..r.below(4) {
        let (a, b) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if a == b {
            continue;
        }
        let terms = [Incoterm::Exw, Incoterm::Cpt, Incoterm::Dap, Incoterm::Ddp][r.below(4) as usize];
        let advance = r.below(3) == 0;
        s.draft_advanced(a, b, r.below(n_goods as u64) as usize, (1 + r.below(8)) as f64 * 16.0, r.range(0.02, 2.0),
            r.below(10) * DAY, (1 + r.below(4)) * DAY, 1 + r.below(10) as u32, terms, r.below(2) == 0, r.below(4) == 0, advance);
    }
}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

/// What's worked off is never more than was paid, a broken contract has worked off all it can (the rest is owed back),
/// nothing is owed back before a break, and an advance paid never passes its share.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for c in s.contracts() {
        let id = c.id;
        if c.recouped > c.advanced + 1e-6 || c.refund < 0.0 || c.advanced < 0.0 {
            return Err(format!("contract {id}: advanced {}, worked off {}, owed back {}", c.advanced, c.recouped, c.refund));
        }
        if c.state == State::Broken && (c.advanced - c.recouped).abs() > 1e-6 {
            return Err(format!("contract {id}: broken with {} not worked off or owed back", c.advanced - c.recouped));
        }
        // (A broken contract whose goods were on the road when it broke completes when they arrive, as the mod's
        // `settle` has it, with the refund still owed.)
        if !matches!(c.state, State::Broken | State::Completed) && c.refund > 0.0 {
            return Err(format!("contract {id}: {} owed back, not broken", c.refund));
        }
        if c.advance_share == 0.0 && (c.advanced != 0.0 || c.recouped != 0.0) {
            return Err(format!("contract {id}: an advance paid with none asked for"));
        }
        if c.advanced > c.advance_share * c.worth() + 1e-6 {
            return Err(format!("contract {id}: {} paid up front, more than its share of {}", c.advanced, c.worth()));
        }
    }
    Ok(())
}
