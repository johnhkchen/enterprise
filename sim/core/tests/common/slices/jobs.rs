//! The jobs' part of the random worlds: Merchants (a desk) and idle hands in some towns, signals from markets and halls
//! wanted, savings in the households; each day a child grown or a hand arriving with no trade, and a Merchant's review of
//! its lockers (which counts the days a trade does nothing). Checked: the bag is finite and never negative, every hand
//! has a trade in the content, and a hand's locker is its trade's.

use super::super::*;
use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::roster::SQUAD;

pub fn extend(s: &mut Society, r: &mut Rng) {
    s.seed(r.next());
    for t in 0..s.town_count() {
        if s.roster(t).merchant.is_none() && r.below(2) == 0 {
            s.add_hand(t, "merchant", 1 + r.below(5) as u8);
        }
        for _ in 0..r.below(3) {
            s.add_hand(t, "none", 1);
        }
        if r.below(3) == 0 {
            s.set_signal(t, SQUAD[r.below(SQUAD.len() as u64) as usize], r.range(0.0, 20.0));
        }
        if r.below(4) == 0 {
            s.set_build_work(t, r.range(0.0, 20.0));
        }
        if r.below(3) == 0 {
            s.post(Account::Outside, Account::Households(t), r.range(0.0, 200.0), Reason::PaidIn);
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    for t in 0..s.town_count() {
        if r.below(12) == 0 {
            s.add_hand(t, "none", 1);
        }
        if r.below(8) == 0 {
            s.review_lockers(t);
        }
        if r.below(20) == 0 {
            s.set_signal(t, SQUAD[r.below(SQUAD.len() as u64) as usize], r.range(0.0, 20.0));
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        for (trade, m) in s.job_bag(t) {
            if !m.is_finite() || m < 0.0 {
                return Err(format!("town {t}: {m} marbles for {trade}"));
            }
        }
        let (roster, lockers) = (s.roster(t), s.lockers(t));
        if let Some(lockers) = lockers {
            for (i, h) in roster.hands.iter().enumerate() {
                if lockers.hands[i].trade != h.trade {
                    return Err(format!("town {t}: hand {i}'s locker is another trade's"));
                }
            }
        }
    }
    Ok(())
}
