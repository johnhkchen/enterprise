//! The land's part of the random worlds (`land.rs`): now and then a host surveys a placed town, its hex's ground and a
//! neighbour's (a lake, a cliff now and then), the crops it counted in a chunk or two of its hexes (and of a hex nobody
//! holds), then its `Survey`; the works it's shown to lay, it lays most of (a wall perhaps only in part), and says so.
//! Checked: right after a survey a town's farm plots are what's counted in its hexes with the fields it hasn't laid;
//! every day, every share shown is between nothing and the whole, and every field unlaid is of a field crop. (The
//! fields the rules plant are noted around a wire `Advance`, which the random worlds don't send: `tests/land.rs`.)

use super::super::*;
use enterprise_core::land::LandBook;
use enterprise_core::wire::{Count, HexAt, Input, LandKind, Output};

const LANDS: [LandKind; 7] = [LandKind::Plains, LandKind::Woods, LandKind::Taiga, LandKind::Desert, LandKind::Shore,
    LandKind::Hills, LandKind::Water];

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    survey(s, r, day);
}

fn survey(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(3) != 0 || s.town_count() == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let Some(home) = s.town_hex(t) else { return };
    let crops: Vec<u32> = enterprise_core::farms::field_crops(s.content()).into_iter().map(|g| g as u32).collect();
    let at = |h: enterprise_core::Hex| HexAt { q: h.q, r: h.r };
    let mut inputs = Vec::new();
    // Its ground: mostly dry and gentle, now and then a lake or a cliff beside it (caravans go round).
    for hex in [home, home.neighbors()[r.below(6) as usize]] {
        let (water, relief) = match r.below(8) {
            0 => (r.range(0.6, 1.0), r.range(0.0, 4.0)),
            1 => (0.0, r.range(32.0, 60.0)),
            _ => (r.range(0.0, 0.3), r.range(0.0, 12.0)),
        };
        inputs.push(Input::Ground { hex: at(hex), water, relief, land: LANDS[r.below(7) as usize], height: None, river: false });
    }
    // The crops in a chunk or two of its hexes, or of a hex beside them nobody may hold.
    let hexes = s.field_hexes(t);
    for _ in 0..1 + r.below(2) {
        let hex = if hexes.is_empty() || r.below(5) == 0 { home.neighbors()[r.below(6) as usize] }
            else { hexes[r.below(hexes.len() as u64) as usize] };
        let counts: Vec<Count> = if crops.is_empty() || r.below(6) == 0 { vec![] }
            else { (0..1 + r.below(2)).map(|_| Count { good: crops[r.below(crops.len() as u64) as usize], count: r.range(0.0, 90.0).floor() }).collect() };
        inputs.push(Input::Crops { hex: at(hex), chunk_x: r.below(4) as i32, chunk_z: r.below(4) as i32, counts });
    }
    inputs.push(Input::Survey { town: t as u32, bell_x: home.center_x() as i32, bell_z: home.center_z() as i32 });
    for input in &inputs {
        s.apply(input);
    }
    let fields: Vec<(usize, u32)> = s.fields_of(t).into_iter().filter(|&(_, n)| n > 0).collect();
    let farm: Vec<(usize, u32)> = s.farm(t).expect("a survey sets the farm up").farm.plots.into_iter().filter(|&(_, n)| n > 0).collect();
    assert_eq!(farm, fields, "town {t}: right after its survey its plots aren't what's counted in its hexes and unlaid");
    // What it's shown to lay: most of it laid, a wall perhaps in part.
    for out in s.show_land(&[t]) {
        let Output::Build { work, share, .. } = out else { continue };
        if r.below(4) != 0 {
            let laid = if r.below(3) == 0 { share * r.unit() } else { share };
            s.apply(&Input::Built { town: t as u32, work, share: laid });
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let book = s.world.resource::<LandBook>();
    for (t, shown) in &book.shown {
        if let Some((what, share)) = shown.iter().find(|(_, x)| !(0.0..=1.0).contains(x)) {
            return Err(format!("town {t}: {what:?} shown {share}"));
        }
    }
    for (t, unlaid) in &book.unlaid {
        if let Some(c) = unlaid.iter().find(|c| !enterprise_core::farms::field_crops(s.content()).contains(c)) {
            return Err(format!("town {t}: an unlaid field of {}, no crop", s.content().goods[*c].name));
        }
    }
    Ok(())
}
