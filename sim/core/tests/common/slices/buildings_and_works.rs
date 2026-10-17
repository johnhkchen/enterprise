//! Public works in the random worlds (`works.rs`): some towns hold a few hexes round their own, have Masons and
//! partner trades, and money, so the content ones start bridges, walls and halls of themselves; now and then a host
//! starts one, a visiting Mason lays a day's work, or a project is finished at once. The invariants: blocks laid are
//! never more than the project's, a finished one has all its blocks, a finished bridge is on the map, the wall's share
//! is 0 to 1, no wage is owed backwards, and a hall is started once.

use super::super::*;
use enterprise_core::works::{Kind, What, KINDS};

pub fn extend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        if r.below(2) == 0 {
            continue;
        }
        if let Some(home) = s.town_hex(t) {
            for hex in home.neighbors().into_iter().take(r.below(6) as usize) {
                s.claim_hex(t, hex);
            }
        }
        for (trade, level) in [("mason", 1 + r.below(4) as u8), ("librarian", 3), ("cleric", 3), ("farmer", 2), ("merchant", 3)] {
            if r.below(3) == 0 {
                s.add_hand(t, trade, level);
            }
        }
        s.pay(t, r.range(20.0, 600.0));
        s.set_dock(t, r.below(2) == 0);
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let n = s.town_count();
    if n == 0 || r.below(4) != 0 {
        return;
    }
    let t = r.below(n as u64) as usize;
    match r.below(6) {
        0 => {
            let kind = KINDS[r.below(KINDS.len() as u64) as usize];
            s.start_project(t, What::Hall(kind));
        }
        1 => s.start_project(t, What::Wall),
        2 => {
            let hexes = s.holding(t).hexes;
            if hexes.len() >= 3 {
                let hex = hexes[r.below(hexes.len() as u64) as usize];
                s.start_project(t, What::Bridge { hex, from: hexes[0], to: hexes[hexes.len() - 1] });
            }
        }
        3 => {
            let skill = 1 + r.below(5) as u8;
            if s.masonry_wanted(t, skill, r.range(0.0, 8.0)) {
                let fee = s.masonry_abroad(t, skill);
                assert!(fee == skill as f64, "town {t}: a Mason of {skill} is paid {fee}");
            }
        }
        4 => {
            let works = s.works(t);
            if let Some(i) = works.underway() {
                s.finish_project(t, works.projects[i].what);
            }
        }
        _ => {
            s.help_project(t, r.range(0.0, 200.0));
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let works = s.works(t);
        let mut halls: Vec<Kind> = Vec::new();
        for p in &works.projects {
            let id = format!("town {t}: {:?}", p.what);
            if !(p.laid >= 0.0 && p.laid <= p.blocks_needed as f64 + 1e-9) {
                return Err(format!("{id} laid {} of {}", p.laid, p.blocks_needed));
            }
            if p.done && p.laid != p.blocks_needed as f64 {
                return Err(format!("{id} done with {} of {} blocks", p.laid, p.blocks_needed));
            }
            if !(p.wage_owed >= 0.0 && p.wage_owed < 100.0 && p.wages_paid >= 0.0) {
                return Err(format!("{id} owes {} and paid {}", p.wage_owed, p.wages_paid));
            }
            match p.what {
                What::Bridge { hex, .. } if p.done && !s.map().ground(hex).bridged => {
                    return Err(format!("{id}: done and not on the map"));
                }
                What::Hall(kind) => {
                    if halls.contains(&kind) {
                        return Err(format!("{id} started twice"));
                    }
                    halls.push(kind);
                }
                _ => {}
            }
        }
        let wall = works.walled();
        if !(0.0..=1.0).contains(&wall) {
            return Err(format!("town {t}: a wall {wall} standing"));
        }
        if !(s.holding(t).extra_culture >= 0.0) {
            return Err(format!("town {t}: culture {}", s.holding(t).extra_culture));
        }
    }
    Ok(())
}
