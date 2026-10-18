//! Hands' ids' part of the random worlds (`roster.rs`, `hands.rs`): now and then a villager comes to live at a town,
//! named by the host, and one the host named dies. Checked, every day whatever moved hands about (births, jobs, caravans,
//! settlers, war): every hand on a roster or with a party on the road has an id, none two alike; a town's lockers are
//! its hands' in its roster's order, and its Merchant's clients are among them; a hand the host named is found by its
//! name, and one it lost is on no roster.

use std::collections::BTreeSet;

use super::super::*;
use enterprise_core::lockers::Lockers;

const TRADES: [&str; 5] = ["farmer", "librarian", "mason", "none", "fletcher"];

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    if r.below(4) == 0 {
        let t = r.below(s.town_count() as u64) as usize;
        let trade = TRADES[r.below(TRADES.len() as u64) as usize];
        let level = 1 + r.below(5) as u8;
        s.join(t, trade, level, &format!("villager-{day}")).unwrap();
    }
    if r.below(6) == 0 {
        // A villager the host named dies, if it's at home (one away is off the host's world).
        let named: Vec<u32> = s.world.resource::<enterprise_core::roster::Hands>().tags.keys().copied()
            .filter(|&id| s.can_lose_hand(id).is_ok()).collect();
        if !named.is_empty() {
            let id = named[r.below(named.len() as u64) as usize];
            s.lose_hand(id).unwrap();
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    let parties = &s.world.resource::<enterprise_core::settlers::Settlers>().parties;
    let rosters: Vec<_> = (0..s.town_count()).map(|t| s.roster(t)).collect();
    for (t, roster) in rosters.iter().enumerate() {
        for h in &roster.hands {
            if h.id == 0 || !seen.insert(h.id) {
                return Err(format!("town {t}: a hand with id {} (none, or another's)", h.id));
            }
        }
        if let Some(lockers) = s.world.get::<Lockers>(s.world.resource::<enterprise_core::state::Towns>().0[t]) {
            // A hand joined since the last sync has no locker yet; a locker with no hand is one a leaver left behind.
            if lockers.hands.len() > roster.hands.len() {
                return Err(format!("town {t}: {} lockers for {} hands", lockers.hands.len(), roster.hands.len()));
            }
            if let Some((i, (l, h))) = lockers.hands.iter().zip(&roster.hands).enumerate().find(|(_, (l, h))| l.hand != h.id) {
                return Err(format!("town {t}: locker {i} is hand {}'s, the roster's hand there is {}", l.hand, h.id));
            }
            if let Some(c) = lockers.clients.iter().find(|&&c| roster.place(c).is_none()) {
                return Err(format!("town {t}: a client, hand {c}, not on its roster"));
            }
        }
    }
    for p in parties {
        for h in &p.hands {
            if h.id == 0 || !seen.insert(h.id) {
                return Err(format!("a party's hand with id {} (none, or another's)", h.id));
            }
        }
    }
    let hands = s.world.resource::<enterprise_core::roster::Hands>();
    if let Some(top) = seen.iter().max() {
        if *top > hands.last {
            return Err(format!("hand {top}, past the last id given ({})", hands.last));
        }
    }
    for (id, tag) in &hands.tags {
        if s.find_hand(*id).is_some() && s.hand_tagged(tag) != Some(*id) {
            return Err(format!("hand {id} is called {tag}, which finds {:?}", s.hand_tagged(tag)));
        }
    }
    Ok(())
}
