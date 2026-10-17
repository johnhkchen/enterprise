//! The cash book's part of the random worlds: towns spending at their own choosing, through the Accountant.

use super::super::*;
use enterprise_core::cash_book::{Account, Reason};

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

/// A day's spending: some towns ask approval for a work out of the world, some take what the budget allows for their
/// households, the rest spend nothing.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    for t in 0..s.town_count() {
        match r.below(4) {
            0 => {
                let cost = r.range(0.0, 30.0);
                if s.approve(t, "works", cost) {
                    let cost = cost.min(s.town(t).money.max(0.0));
                    s.post(Account::Treasury(t), Account::Outside, cost, Reason::Other("works".into()));
                }
            }
            1 => {
                let gift = s.allow(t, "a feast", r.range(0.0, 10.0)).min(s.town(t).money.max(0.0));
                s.post(Account::Treasury(t), Account::Households(t), gift, Reason::Other("a feast".into()));
            }
            _ => {}
        }
    }
}

pub fn check(_s: &Society, _day: u64) -> Result<(), String> {
    Ok(())
}
