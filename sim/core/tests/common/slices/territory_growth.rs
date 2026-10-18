//! Territory's part of the random worlds: towns on the map claim and grow, some post guards, set tolls, and
//! the books of who holds what must agree.

use super::super::*;
use enterprise_core::chokepoints::ARMOURED_TOLL_RATE;
use enterprise_core::territory::MAX_TOLL_RATE;

/// Some towns claim their home at once (the rest at their first look); some have confined villagers and works.
pub fn extend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        if r.below(2) == 0 {
            s.claim(t);
        }
        if r.below(4) == 0 {
            s.set_confined(t, r.below(6) as u32);
        }
        if r.below(4) == 0 {
            s.set_culture_extras(t, r.range(0.0, 6.0));
        }
    }
}

/// A host's doings: some towns post guards on their first hexes, set a toll, ask a fee; now and then a town is
/// granted a hex beside its land.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    for t in 0..s.town_count() {
        match r.below(12) {
            0 => s.set_guards(t, r.below(4) as usize),
            1 => s.set_toll(t, r.range(0.0, 0.3)),
            2 => s.set_passage_fee(t, r.range(0.0, 3.0).floor()),
            3 => {
                if let Some(&last) = s.holding(t).hexes.last() {
                    let n = last.neighbors()[r.below(6) as usize];
                    s.claim_hex(t, n);
                }
            }
            _ => {}
        }
    }
}

/// Every hex has one owner, and is in its owner's list; posts are held hexes; toll rates are within bounds; no more
/// hexes than the rings allow from home (but grants).
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let terr = s.territory();
    for (hex, owner) in &terr.owners {
        if !terr.holdings.get(owner).is_some_and(|h| h.hexes.contains(hex)) {
            return Err(format!("{hex} is {owner}'s, but not in its holding"));
        }
    }
    for (town, h) in &terr.holdings {
        for hex in &h.hexes {
            if terr.owner(*hex) != Some(*town) {
                return Err(format!("town {town} lists {hex}, held by {:?}", terr.owner(*hex)));
            }
        }
        if h.hexes.iter().enumerate().any(|(i, x)| h.hexes[..i].contains(x)) {
            return Err(format!("town {town} lists a hex twice"));
        }
        if let Some(p) = h.posts.iter().find(|p| !h.hexes.contains(p)) {
            return Err(format!("town {town} has a post at {p}, which it doesn't hold"));
        }
        // Armoured guards may ask past the cap, up to `ARMOURED_TOLL_RATE` and more with enchanted plates
        // (`chokepoints.rs`: the cap rises with the armoured share, to 1.5).
        let most = MAX_TOLL_RATE + (ARMOURED_TOLL_RATE - MAX_TOLL_RATE) * h.armoured.max(0.0) + 1e-9;
        if h.toll_rate.is_some_and(|x| !(0.0..=most).contains(&x)) {
            return Err(format!("town {town}: toll rate {:?}", h.toll_rate));
        }
        if h.culture < -1e-9 || !h.culture.is_finite() {
            return Err(format!("town {town}: culture {}", h.culture));
        }
    }
    Ok(())
}
