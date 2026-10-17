//! Forward crop sales and rents in the random worlds: some goods crops (so markets offer farms forward sales and rents),
//! some harvest contracts and rents sold at the start, and later a rent sold, a deed handed to a bearer, its crop taken
//! or the rent bought back; and the invariants of the paper they keep.

use super::super::*;
use enterprise_core::harvest::RentState;
use enterprise_core::State;

pub fn extend(s: &mut Society, r: &mut Rng) {
    s.content_mut().dials.forward_sales = true;
    let (n_towns, n_goods) = (s.town_count(), s.content().goods.len());
    if n_towns < 2 {
        return;
    }
    for _ in 0..r.below(3) {
        let (a, b) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if a == b {
            continue;
        }
        let terms = [Incoterm::Exw, Incoterm::Cpt, Incoterm::Dap, Incoterm::Ddp][r.below(4) as usize];
        s.draft_harvest(a, b, r.below(n_goods as u64) as usize, (1 + r.below(8)) as f64 * 16.0, r.range(0.02, 2.0),
            r.below(10) * DAY, (1 + r.below(4)) * DAY, 1 + r.below(10) as u32, terms, r.below(4) != 0, r.below(5) == 0);
    }
    for _ in 0..r.below(3) {
        let (farm, holder) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if farm != holder {
            s.pay(holder, r.range(0.0, 400.0));
            s.sell_rent(farm, holder, r.below(n_goods as u64) as usize, (1 + r.below(8)) as f64 * 16.0, r.range(0.02, 1.0));
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    let (n_towns, n_goods) = (s.town_count(), s.content().goods.len());
    if n_towns < 2 {
        return;
    }
    match r.below(12) {
        0 => {
            let (farm, holder) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
            if farm != holder {
                s.pay(holder, r.range(0.0, 300.0));
                s.sell_rent(farm, holder, r.below(n_goods as u64) as usize, (1 + r.below(4)) as f64 * 16.0, r.range(0.02, 1.0));
            }
        }
        1 => {
            let rents = s.rents();
            if !rents.is_empty() {
                let rent = &rents[r.below(rents.len() as u64) as usize];
                s.set_bearer(rent.id, r.below(2) == 0);
            }
        }
        2 => {
            let rents = s.rents();
            if !rents.is_empty() {
                let id = r.below(rents.len() as u64) as usize;
                let taken = s.take_accrued(id, r.range(0.0, 100.0));
                assert!(taken >= 0.0, "day {day}: took {taken}");
            }
        }
        3 => {
            let rents = s.rents();
            if !rents.is_empty() {
                s.redeem_rent(r.below(rents.len() as u64) as usize);
            }
        }
        _ => {}
    }
}

/// A rent's arrears and accrued crop are never negative; one in force has a coupon that hasn't broken, one that's over
/// has none in force; a harvest contract never judges more installments than it has; owed crop set aside is never
/// negative.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let contracts = s.contracts();
    for r in s.rents() {
        if r.arrears < -1e-9 || r.accrued < -1e-9 || r.capital < 0.0 {
            return Err(format!("rent {}: arrears {}, accrued {}, capital {}", r.id, r.arrears, r.accrued, r.capital));
        }
        let coupon = r.coupon.map(|id| &contracts[id]);
        match (r.state, coupon) {
            (RentState::Active, Some(c)) if c.state == State::Broken => return Err(format!("rent {}: its coupon broke", r.id)),
            (RentState::Active, None) => return Err(format!("rent {}: in force with no coupon", r.id)),
            (RentState::Redeemed | RentState::Defaulted, Some(c)) if c.in_force() =>
                return Err(format!("rent {}: over, but its coupon {} is in force", r.id, c.id)),
            _ => {}
        }
    }
    for c in contracts.iter().filter(|c| c.harvest.is_some()) {
        if c.judged > c.installments {
            return Err(format!("contract {}: {} judged of {}", c.id, c.judged, c.installments));
        }
    }
    for t in 0..s.town_count() {
        for g in 0..s.content().goods.len() {
            if s.set_aside_for_harvest(t, g) < 0.0 {
                return Err(format!("town {t}: a negative amount set aside of good {g}"));
            }
        }
    }
    Ok(())
}
