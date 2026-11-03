//! Taxes (John, 2026-11-03: "Next let's talk taxation": withheld payroll and income tax, land value tax, VAT; "Towns
//! and crown split"). The core's households are one purse a town, so a tax here moves money between a town's purse and
//! its treasury (or the crown's), not between rich and poor: what it does is put idle money where works are paid from.
//!
//! - **Income tax, withheld:** of every wage a town's treasury pays its households, and of what its villagers earn by
//!   their work (`hand_wage`), `income_tax` goes back to the treasury at the source, the same posting: nothing to evade.

use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::Content;
use crate::fixed::Fixed;
use crate::state::Ledger;

pub fn income_tax_reason() -> Reason {
    Reason::Other("income tax".into())
}

/// Withholds the town's income tax on `gross` its households were just paid: from their purse to its treasury.
#[allow(clippy::too_many_arguments)]
pub fn withhold(content: &Content, ledger: &mut Ledger, book: &mut CashBook, town: usize, treasury: &mut Fixed,
        purse: &mut Fixed, gross: Fixed, now: u64, day: u64) {
    let tax = Fixed::floor(gross.to_f64() * content.dials.income_tax).min(purse.at_least_zero());
    if tax > Fixed::ZERO {
        post(ledger, book, Side::households(town, purse), Side::treasury(town, treasury), tax, income_tax_reason(), now, day);
    }
}
