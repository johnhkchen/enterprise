//! The lockers' part of the random worlds: the mod's trades given offers over the world's goods (its goods aren't
//! named as the mod's, so none come by default), a Merchant in half the towns, some offers switched on; each day a
//! host switching offers, a contract needing a trade, an old save with no restock clock. Checked: the Merchant
//! manages no more than its limit, uses never pass an offer's, and a good its lockers buy isn't also bought by its
//! people at its worth.

use super::super::*;
use enterprise_core::lockers::{has_locker, Offer};
use enterprise_core::roster::SQUAD;
use enterprise_core::state::{Towns, Trades};

pub fn extend(s: &mut Society, r: &mut Rng) {
    s.seed(r.next());
    let goods = s.content().goods.clone();
    for &name in &SQUAD[1..] {
        let t = s.content().trade(name).unwrap();
        for _ in 0..r.below(3) {
            let g = r.below(goods.len() as u64) as usize;
            let uses = [12, 16][r.below(2) as usize];
            let mult = [0.05, 0.2][r.below(2) as usize];
            let level = 1 + r.below(3) as u8;
            let offer = if r.below(3) == 0 {
                Offer::sells(g, 1 + r.below(4) as u32, 1 + r.below(16) as u32, uses, 1 + r.below(10) as u32, mult)
            } else {
                Offer::buys(g, 1 + r.below(goods[g].stack.min(32) as u64) as u32, 1 + r.below(2) as u32, uses,
                    [2, 10, 20][r.below(3) as usize], mult)
            };
            s.content_mut().trades[t].offers.push(offer.at(level));
        }
    }
    for t in 0..s.town_count() {
        if r.below(2) == 0 {
            s.add_hand(t, "merchant", 1 + r.below(5) as u8);
        }
        let lockers = s.lockers(t).unwrap();
        for (h, l) in lockers.hands.iter().enumerate() {
            if !l.offers.is_empty() && r.below(3) == 0 {
                s.switch(t, h, r.below(l.offers.len() as u64) as usize, true);
            }
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    for t in 0..s.town_count() {
        let Some(lockers) = s.lockers(t) else { continue };
        if r.below(10) == 0 && !lockers.hands.is_empty() {
            let h = r.below(lockers.hands.len() as u64) as usize;
            if !lockers.hands[h].offers.is_empty() {
                let o = r.below(lockers.hands[h].offers.len() as u64) as usize;
                s.switch(t, h, o, r.below(2) == 0);
            }
        }
        if r.below(20) == 0 {
            let trade = SQUAD[r.below(SQUAD.len() as u64) as usize];
            s.set_needed(t, if r.below(2) == 0 { &[] } else { std::slice::from_ref(&trade) });
        }
        if r.below(50) == 0 {
            s.set_restock_clock(t, None);
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let Some(lockers) = s.lockers(t) else { continue };
        let roster = s.roster(t);
        if lockers.managed.len() > roster.locker_limit() as usize {
            return Err(format!("town {t}: {} lockers managed, its Merchant's limit {}", lockers.managed.len(),
                roster.locker_limit()));
        }
        if let Some(&h) = lockers.managed.iter().find(|&&h| !has_locker(s.content(), &roster, h)) {
            return Err(format!("town {t}: hand {h} managed without a locker"));
        }
        if lockers.hands.iter().flat_map(|l| &l.offers).any(|o| o.used > o.uses) {
            return Err(format!("town {t}: an offer used more than its uses"));
        }
        if lockers.restocked_until.is_some_and(|c| c > s.now()) {
            return Err(format!("town {t}: its restock clock is ahead of now"));
        }
        let trades = s.world.get::<Trades>(s.world.resource::<Towns>().0[t]).unwrap();
        // What's set aside isn't bought at its worth too (a switch since the last look is set aside at the next).
        if let Some(g) = (0..trades.uses.len()).find(|&g| trades.uses[g] != 0.0 && lockers.kept[g].is_some_and(|k| k != 0.0)) {
            return Err(format!("town {t}: good {g} bought by its lockers and its people both"));
        }
    }
    Ok(())
}
