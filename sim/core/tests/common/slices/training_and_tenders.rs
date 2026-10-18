//! Training and tenders' part of the random worlds: some towns hold training contracts for a smith (some with an
//! offtake, some with a third up front, drafted at the seller or at both), a few boards carry tenders, and each day a hand
//! joins, levels up or a new tender is posted for wagons to carry and Merchants to take. Checked: tenders have a sound bonus
//! and a count, a training contract's term is finite, a completed one has its bonus owed or paid, the pupil's work waiting
//! to be crated is cleared by each look's end.

use super::super::*;
use enterprise_core::contracts::State;

const SMITHS: [&str; 3] = ["toolsmith", "weaponsmith", "armorer"];

pub fn extend(s: &mut Society, r: &mut Rng) {
    s.seed(r.next());
    let n = s.town_count() as u64;
    if r.below(2) == 0 {
        // Half the worlds have room to keep what a pupil is paid to make.
        s.set_dial("warehouse_slots", 4000.0).unwrap();
    }
    for _ in 0..r.below(3) {
        let (a, b) = (r.below(n) as usize, r.below(n) as usize);
        if a != b {
            let trade = SMITHS[r.below(3) as usize];
            s.draft_training(a, b, trade, 2 + r.below(4) as u8, r.range(3.0, 60.0), [0.0, 0.5, 1.0][r.below(3) as usize],
                (5 + r.below(35)) * DAY, r.below(2) == 0, r.below(2) == 0);
        }
    }
    for _ in 0..r.below(3) {
        let board = r.below(n) as usize;
        s.tender_for(board, SMITHS[r.below(3) as usize], 2 + r.below(4) as u8, r.range(3.0, 60.0), [0.0, 0.5][r.below(2) as usize],
            1 + r.below(3) as u32, (4 + r.below(40)) * DAY);
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    let n = s.town_count() as u64;
    if r.below(15) == 0 {
        s.tender(r.below(n) as usize, SMITHS[r.below(3) as usize], 2 + r.below(4) as u8, r.range(3.0, 60.0),
            (day + 4 + r.below(30)) * DAY);
    }
    if r.below(10) == 0 {
        s.add_hand(r.below(n) as usize, SMITHS[r.below(3) as usize], 1 + r.below(5) as u8);
    }
    if r.below(25) == 0 {
        let (a, b) = (r.below(n) as usize, r.below(n) as usize);
        if a != b {
            s.draft_training(a, b, SMITHS[r.below(3) as usize], 2 + r.below(4) as u8, r.range(3.0, 60.0), [0.0, 1.0][r.below(2) as usize],
                (day + 4 + r.below(30)) * DAY, r.below(2) == 0, r.below(2) == 0);
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in s.tenders() {
        if !t.work.bonus.is_positive() {
            return Err(format!("tender {}: a bonus of {}", t.id, t.work.bonus));
        }
        if t.board >= s.town_count() || t.buyer >= s.town_count() || t.origin >= s.town_count() {
            return Err(format!("tender {}: a town that isn't there", t.id));
        }
        if t.open > 1000 {
            return Err(format!("tender {}: {} open", t.id, t.open));
        }
    }
    for c in s.contracts() {
        if c.training.is_none() {
            continue;
        }
        if c.quantity != 0.0 {
            return Err(format!("contract {}: a training term with {} goods", c.id, c.quantity));
        }
        if c.state == State::Completed && (c.delivered != 1.0 || c.owing < 0.0) {
            return Err(format!("contract {}: completed but delivered {}, owing {}", c.id, c.delivered, c.owing));
        }
    }
    for town in 0..s.town_count() {
        if let Some(lockers) = s.lockers(town) {
            if !lockers.lessons.made.is_empty() {
                return Err(format!("town {town}: the pupil's work is left uncrated at a look's end"));
            }
        }
    }
    Ok(())
}
