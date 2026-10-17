//! The herds' part of the random worlds: some worlds name goods for sheep's mutton and wool (so the flocks and their
//! products exist whatever the base goods were), towns get herding trades, pastures and wheat to feed them, and the
//! other kinds the base names allow keep their herds too.

use super::super::*;

pub fn extend(s: &mut Society, r: &mut Rng) {
    let n = s.content().goods.len();
    if n > 5 && r.below(2) == 0 {
        {
            let mut content = s.content_mut();
            content.goods[4].name = "mutton".to_string();
            content.goods[5].name = "white_wool".to_string();
        }
        s.rebuild_conversions();
    }
    s.rebuild_animals();
    for t in 0..s.town_count() {
        for trade in ["shepherd", "butcher", "leatherworker"] {
            if r.below(3) == 0 {
                s.add_hand(t, trade, 1 + r.below(5) as u8);
            }
        }
        s.set_pastures(t, r.below(4) as u32);
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let wheat = s.content().good("wheat");
    for t in 0..s.town_count() {
        if let (Some(w), true) = (wheat, r.below(3) == 0) {
            s.deliver(t, w, r.range(0.0, 20.0).floor());
        }
    }
}

/// No herd is negative or beyond what its pastures hold plus the seed, and the feed owed is under a unit.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let room = s.content().dials.pasture_animals as f64;
    for t in 0..s.town_count() {
        let h = s.herds(t);
        if h.count.iter().any(|&c| c < 0.0 || !c.is_finite()) {
            return Err(format!("town {t}: a herd below nought"));
        }
        if h.owed.iter().any(|&o| !(0.0..1.0 + 1e-9).contains(&o)) {
            return Err(format!("town {t}: feed owed outside 0 to 1"));
        }
        let total: f64 = h.count.iter().sum();
        if total > room * h.pastures.max(1) as f64 + 4.0 + 1e-9 {
            return Err(format!("town {t}: {total} animals in {} pastures", h.pastures));
        }
    }
    Ok(())
}
