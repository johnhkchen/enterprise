//! Services' part of the random worlds (`services.rs`): the base rosters' Clerics, Farmers, Cartographers and smiths
//! (and a few more taken on here) work at home and go with the wagons where their services are wanted: blessings,
//! charting, map swaps and surveys of the roads, Shepherds driving the herds' animals, Farmers helping where fields lie
//! untended. Where the workshop named goods for its books (paper, leather, book), towns without a Librarian get paper
//! and leather, so a visiting Librarian binds books for them. (The workshop names every base good, so the metal chain's
//! goods aren't in the random worlds: its recipes are pinned in `tests/services.rs`.)

use super::super::*;

pub fn extend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        if r.below(4) == 0 {
            s.add_hand(t, ["cleric", "farmer", "cartographer", "armorer", "toolsmith"][r.below(5) as usize], 1 + r.below(5) as u8);
        }
    }
    binding_stock(s, r);
}

/// Paper and leather for some towns without a Librarian of their own.
fn binding_stock(s: &mut Society, r: &mut Rng) {
    let (Some(paper), Some(leather), Some(librarian)) = (s.content().good("paper"), s.content().good("leather"),
        s.content().trade("librarian")) else { return };
    for t in 0..s.town_count() {
        if r.below(3) == 0 && !s.roster(t).hands.iter().any(|h| h.trade == librarian) {
            s.deliver(t, paper, (3 * (1 + r.below(8))) as f64);
            s.deliver(t, leather, (1 + r.below(8)) as f64);
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(10) == 0 {
        binding_stock(s, r);
    }
    if r.below(5) == 0 && s.town_count() > 0 {
        // Fields no one tends in a town (what its farm's plan would leave idle), for a visiting Farmer.
        let t = r.below(s.town_count() as u64) as usize;
        let g = r.below(s.content().goods.len() as u64) as usize;
        let (idle, spare) = (r.range(0.0, 40.0), r.below(64) as u32);
        if s.farm(t).is_some() {
            s.set_idle(t, &[(g, idle)], spare);
        }
    }
}

/// A blessing lasts at most a visiting master's (2 days and 5) from now; a wagon drives at most 8 animals; the ledger
/// keeps its latest 40 and no service was paid less than nothing.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let d = &s.content().dials;
    let longest = s.now() + ((d.blessing_days + 5.0) * s.content().day_ticks as f64) as u64;
    for t in 0..s.town_count() {
        if let Some(until) = s.blessed_until(t).filter(|&u| u > longest) {
            return Err(format!("town {t} blessed until {until}, past {longest}"));
        }
        let done = s.done(t);
        if done.len() > enterprise_core::services::RECENT {
            return Err(format!("town {t}: {} services in the ledger's latest", done.len()));
        }
        if let Some(r) = done.iter().find(|r| r.paid.is_negative()) {
            return Err(format!("town {t}: {r:?}"));
        }
    }
    for route in 0..s.route_count() {
        let herd = s.wagon_herd(route).len();
        if herd > enterprise_core::services::HERD {
            return Err(format!("route {route} drives {herd} animals"));
        }
    }
    Ok(())
}
