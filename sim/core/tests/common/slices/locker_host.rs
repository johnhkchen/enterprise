//! A host's lockers' part of the random worlds (`locker_host.rs`): half the towns' lockers are their host's, each
//! hand's standing at a point the host placed; now and then a hand's locker is placed (a newcomer's), moved to another's
//! workstation, untied or taken away (its chest broken), and the screen switches one of a hand's offers. Checked: in a town whose lockers its host places, every hand the Merchant
//! manages has a locker standing at a locker point of its town's, and no two hands' at one.

use std::collections::BTreeSet;

use super::super::*;
use enterprise_core::lockers::Lockers;
use enterprise_core::points::{Kind, Spot};

fn spot(t: usize, k: u32) -> Spot {
    Spot { dim: "minecraft:overworld".into(), x: 1000 * t as i32 + 10, y: 64, z: k as i32 }
}

/// The hand's locker placed at its own workstation (a point added there if none stands).
fn place(s: &mut Society, t: usize, hand: u32, at: Spot) {
    if s.point(&at).is_none() {
        s.add_point(Kind::Locker, at.clone(), t).unwrap();
    }
    s.tie_locker(hand, Some(at));
}

/// Half the towns' lockers are their host's, each hand's at a workstation of its own, as a host with chests by every
/// workstation would have them.
pub fn extend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        if r.below(2) == 0 {
            s.host_places_lockers(t);
            for h in s.roster(t).hands.iter().map(|h| h.id).collect::<Vec<_>>() {
                place(s, t, h, spot(t, h));
            }
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let t = r.below(s.town_count() as u64) as usize;
    let roster = s.roster(t);
    if roster.hands.is_empty() {
        return;
    }
    let h = roster.hands[r.below(roster.hands.len() as u64) as usize].id;
    match r.below(10) {
        // A hand's locker placed at its workstation (a hand new to the town gets one).
        0..=5 => place(s, t, h, spot(t, h)),
        // Its chest moved to another hand's workstation, which loses its locker.
        6 => {
            let other = roster.hands[r.below(roster.hands.len() as u64) as usize].id;
            place(s, t, h, spot(t, other));
        }
        7 => {
            s.tie_locker(h, None);
        }
        // A locker's chest broken: its point gone, its hand's locker with it.
        8 => {
            let at = spot(t, h);
            if s.point(&at).is_some() {
                s.remove_point(&at).unwrap();
            }
        }
        // The screen switches one of a hand's offers.
        _ => {
            if let (Some(lockers), Some(i)) = (s.lockers(t), roster.place(h)) {
                let offers = &lockers.hands[i].offers;
                if !offers.is_empty() {
                    let k = r.below(offers.len() as u64) as usize;
                    let good = offers[k].good;
                    s.switch_offer(h, k, good, r.below(2) == 0);
                }
            }
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let e = s.world.resource::<enterprise_core::state::Towns>().0[t];
        let Some(lockers) = s.world.get::<Lockers>(e) else { continue };
        let Some(placed) = &lockers.placed else { continue };
        let roster = s.roster(t);
        let mut at = BTreeSet::new();
        for (hand, spot) in placed {
            if !s.point(spot).is_some_and(|p| p.kind == Kind::Locker && p.town == t) {
                return Err(format!("town {t}: hand {hand}'s locker at {spot:?}, no locker of the town's"));
            }
            if !at.insert(spot) {
                return Err(format!("town {t}: two hands' lockers at {spot:?}"));
            }
        }
        for &i in &lockers.managed {
            let id = roster.hands.get(i).map(|h| h.id);
            if !id.is_some_and(|id| placed.contains_key(&id)) {
                return Err(format!("town {t}: the Merchant manages hand {id:?}'s locker, which stands nowhere"));
            }
        }
    }
    Ok(())
}
