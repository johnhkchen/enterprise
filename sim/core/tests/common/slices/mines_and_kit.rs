//! Mines and kit's part of the random worlds (`kit.rs`): some worlds name their first goods for the mine's ore and
//! the kit (raw iron, coal, pickaxes, hoes, bows, swords) and the land's gatherings (logs, sticks, feathers), every hex
//! has a land, towns take on Toolsmiths, Fletchers, Farmers, Nitwits and Masons (some Masters), dig mines or have them,
//! and are given kit, some enchanted. Now and then more kit, mending and tools for the hands arrive.
//!
//! The checks: pieces and shares are finite and in their bounds, no ore or game is owed below nothing, hands' tools
//! don't go below nothing, a deep mine is a mine. (Conservation and solvency are `check.rs`'s, every day.)

use super::super::*;
use enterprise_core::map::Land;

const NAMES: [&str; 8] = ["raw_iron", "coal", "iron_pickaxe", "iron_hoe", "bow", "iron_sword", "oak_log", "feather"];
const KIT: [&str; 4] = ["iron_pickaxe", "iron_hoe", "bow", "iron_sword"];

pub fn extend(s: &mut Society, r: &mut Rng) {
    let n = s.content().goods.len();
    if r.below(3) != 0 {
        {
            let mut content = s.content_mut();
            // The goods herds renames (4 and 5) are left to it.
            for (i, name) in NAMES.iter().enumerate().take(n.min(4)) {
                content.goods[i].name = name.to_string();
            }
        }
        s.rebuild_conversions();
        s.rebuild_animals();
    }
    let lands = [Land::Plains, Land::Woods, Land::Taiga, Land::Desert, Land::Shore, Land::Hills, Land::Water];
    for q in 0..12 {
        for rr in 0..12 {
            s.set_land(Hex::new(q, rr), lands[r.below(7) as usize]);
        }
    }
    for t in 0..s.town_count() {
        for trade in ["toolsmith", "fletcher", "farmer", "mason", "nitwit", "nitwit"] {
            if r.below(3) == 0 {
                s.add_hand(t, trade, 1 + r.below(5) as u8);
            }
        }
        if r.below(3) == 0 {
            s.pay(t, r.range(5.0, 80.0));
        }
        if r.below(4) == 0 {
            s.set_mine(t, r.below(3) == 0);
        }
        for name in KIT {
            if r.below(3) == 0 {
                s.add_kit(t, name, 1 + r.below(6) as u32, r.below(4) as f64, r.below(4) as u8);
            }
        }
        if r.below(5) == 0 {
            s.give_tools(t, 1 + r.below(4) as u32);
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if s.town_count() == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    match r.below(8) {
        0 => {
            s.add_kit(t, KIT[r.below(4) as usize], 1 + r.below(4) as u32, r.below(3) as f64, r.below(3) as u8);
        }
        1 => s.mend(t, s.now() + r.below(8) * 24_000),
        2 => s.keep_armoury(t, s.now() + r.below(8) * 24_000),
        3 => s.give_tools(t, 1 + r.below(3) as u32),
        4 => s.pay(t, r.range(0.0, 30.0)),
        5 => {
            // Ore and logs delivered: the hands and smiths have something to be wearing out and room to fill.
            if let Some(g) = s.content().good(["raw_iron", "oak_log", "feather"][r.below(3) as usize]) {
                s.deliver(t, g, r.below(200) as f64);
            }
        }
        _ => {}
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let kit = s.kit(t);
        for p in &kit.pieces {
            if p.count == 0 || !p.enchant_level.is_finite() || p.enchant_level < 0.0 {
                return Err(format!("town {t}: a piece of kit {p:?}"));
            }
        }
        if kit.hand_tools < 0.0 || !kit.hand_tools.is_finite() {
            return Err(format!("town {t}: {} tools for its hands", kit.hand_tools));
        }
        if let Some((g, o)) = kit.pending.iter().chain(kit.wear.iter()).find(|(_, o)| !o.is_finite() || **o < 0.0) {
            return Err(format!("town {t}: {o} of good {g} owed"));
        }
        if let Some(i) = kit.shares.iter().position(|&x| !(0.0..=1.5 + 1e-9).contains(&x)) {
            return Err(format!("town {t}: share {} of kit {i}", kit.shares[i]));
        }
        if !(0.0..=1.0 + 1e-9).contains(&s.hunting(t)) {
            return Err(format!("town {t}: hunting {}", s.hunting(t)));
        }
        if s.armed(t) > 1.0 + 1e-9 || s.armed(t) < 0.0 {
            return Err(format!("town {t}: armed {}", s.armed(t)));
        }
    }
    Ok(())
}
