//! The crown's part of the random worlds (`crown.rs`): in a fifth of the worlds a town is named the capital (minting a
//! revenue), a good that's neither food nor a comfort may be named for kit and a food for meat, the capital mobilises
//! on day 3 (posting requisitions, tendering for masters), declares war on day 8 (the front opens, the towns on its
//! roads are levied, mercenaries hired), and makes peace 20 days later. Now and then kit and food reach the capital
//! for its stockpile and the front's orders.
//!
//! The checks: every recruit riding a wagon with a hand is that hand, away, in its town's roster; a levy never raises
//! more than it asks; requisitions are open between nothing and what's wanted, their bids under the crown's price and
//! one a seller; at war there's a front; the coin minted never falls. (Conservation and solvency are `check.rs`'s,
//! every day: the minting comes in from outside through the cash book.)

use super::super::*;
use enterprise_core::crown::Theatre;
use enterprise_core::sourcing::Phase;

/// Kit a good may be named for.
const KIT: [&str; 7] = ["iron_sword", "diamond_sword", "shield", "arrow", "bow", "iron_chestplate", "diamond_helmet"];

/// The crown's world (one with a revenue named) and its capital: the exchange's desk, which naming it opened there.
fn capital(s: &Society) -> Option<usize> {
    (s.sovereign().peace_revenue > 0).then(|| s.exchange().desk).flatten()
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    let towns = s.town_count();
    if r.below(5) != 0 || towns < 2 {
        return;
    }
    // A town with a roster, on the map.
    let Some(capital) = (0..towns).find(|&t| !s.is_off_map(t) && !s.roster(t).hands.is_empty()) else { return };
    {
        let mut content = s.content_mut();
        let free: Vec<usize> = (0..content.goods.len())
            .filter(|&g| content.goods[g].food <= 0.0 && content.goods[g].comfort.is_none())
            .collect();
        if let Some(&g) = free.first().filter(|_| r.below(2) == 0) {
            content.goods[g].name = KIT[r.below(KIT.len() as u64) as usize].to_string();
        }
        if let Some(g) = content.goods.iter_mut().find(|g| g.food > 0.0).filter(|_| r.below(2) == 0) {
            g.meat = true;
            g.saturation = g.food * 2.0;
        }
    }
    s.name_capital(capital, 1 + r.below(400));
}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    let Some(home) = capital(s) else { return };
    // Another slice's host may have named another capital, or none: the crown's is this one.
    s.set_capital(Some(home));
    match day {
        3 => {
            s.mobilise(home, 500 + r.below(2500));
        }
        8 => {
            let theatre = [Theatre::Faction, Theatre::Undead, Theatre::Nether][r.below(3) as usize];
            s.declare(theatre);
        }
        28 => {
            s.peace();
        }
        _ => {}
    }
    if r.below(4) == 0 {
        let goods = s.content().goods.len();
        let g = r.below(goods as u64) as usize;
        s.deliver(home, g, r.range(0.0, 64.0).floor());
    }
    if day > 8 && day < 28 && r.below(6) == 0 {
        s.hire_mercenaries();
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for x in s.recruits_on_the_road() {
        let Some(hand) = &x.hand else { continue };
        let n = s.roster(x.home).hands.iter().filter(|h| h.away && h.id == hand.id).count();
        if n != 1 {
            return Err(format!("town {}: {n} hands away with id {} for a recruit on the road", x.home, hand.id));
        }
    }
    for c in s.contracts() {
        if let Some(levy) = c.levy {
            if levy.raised > levy.soldiers || c.quantity != 0.0 {
                return Err(format!("levy {}: {levy:?}, quantity {}", c.id, c.quantity));
            }
        }
    }
    let sov = s.sovereign();
    for r in &sov.requisitions {
        if !(r.open >= -1e-9 && r.open <= r.wanted + 1e-9) {
            return Err(format!("requisition {}: {} open of {}", r.id, r.open, r.wanted));
        }
        for (i, b) in r.bids.iter().enumerate() {
            if b.each > r.ceiling(s.content(), b.good) + 1e-9 || b.count <= 0.0
                || r.bids[..i].iter().any(|o| o.seller == b.seller) {
                return Err(format!("requisition {}: bid {b:?}", r.id));
            }
        }
    }
    if sov.phase == Phase::War && sov.front.is_none() {
        return Err("at war with no front".into());
    }
    if !(sov.minted >= 0.0) {
        return Err(format!("minted {}", sov.minted));
    }
    Ok(())
}
