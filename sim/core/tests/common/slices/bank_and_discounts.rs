//! Banks in the random worlds (`bank.rs`): in half the worlds one town's market opens a bank on the first day,
//! and now and then a town draws a bill on itself to pay another (a host's bill), for the banks' wagons to buy. The
//! invariants: a loan is owed by a contract's seller to a bank, never more than its principal and interest, never paid
//! off below nothing; a bank never paid more for bills than their face, nor took back more than it lent and its
//! interest.

use super::super::*;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    let n_towns = s.town_count();
    if n_towns < 2 {
        return;
    }
    if day == 1 && r.below(2) == 0 {
        s.open_bank(r.below(n_towns as u64) as usize);
    }
    if r.below(8) == 0 {
        let (drawee, payee) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if drawee != payee {
            s.draw_bill(drawee, payee, r.range(1.0, 30.0));
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let d = &s.content().dials;
    let contracts = s.contracts();
    for l in s.loans() {
        if l.owed <= 0.0 || l.owed > l.principal * (1.0 + d.loan_interest) + 1e-9 || l.principal <= 0.0 {
            return Err(format!("loan {}: {} owed on {} lent", l.id, l.owed, l.principal));
        }
        let c = &contracts[l.contract];
        if c.seller != l.borrower || !s.is_bank(l.bank) && s.bank(l.bank).loans_made == 0 {
            return Err(format!("loan {}: to {} on contract {} sold by {}, from {}", l.id, l.borrower, c.id, c.seller, l.bank));
        }
    }
    for t in 0..s.town_count() {
        let b = s.bank(t);
        if b.bills_paid > b.bills_face + 1e-9 || b.repaid > b.lent * (1.0 + d.loan_interest) + 1e-6 || b.bills_paid < 0.0 {
            return Err(format!("bank {t}: {b:?}"));
        }
    }
    Ok(())
}
