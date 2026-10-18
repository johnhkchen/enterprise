//! The census's part of the random worlds (`census.rs`): now and then a host counts a town that keeps a roster, seeing
//! most of the villagers its hands at home are named by (some of them not, now and then a newcomer, a child, or the
//! villager of a hand that's gone), with its beds as they are. Checked every day: a town's villagers are its roster's
//! residents; no retired tag is a living hand's.

use super::super::*;
use enterprise_core::census::Sightings;
use enterprise_core::wire::Resident;

const TRADES: [&str; 5] = ["farmer", "librarian", "minecraft:mason", "none", "baker"];

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, day: u64) {
    if r.below(3) != 0 || s.town_count() == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let roster = s.roster(t);
    if roster.hands.is_empty() {
        return;
    }
    let mut seen = Vec::new();
    for h in roster.hands.iter().filter(|h| !h.away) {
        if let Some(tag) = s.hand_tag(h.id) {
            if r.below(5) != 0 {
                let trade = if r.below(8) == 0 { TRADES[r.below(TRADES.len() as u64) as usize].to_string() }
                    else { s.content().trades[h.trade].name.clone() };
                seen.push(Resident { tag: tag.to_string(), trade, level: h.level + (r.below(6) == 0) as u8, baby: h.is_baby() });
            }
        }
    }
    if r.below(4) == 0 {
        let trade = TRADES[r.below(TRADES.len() as u64) as usize];
        seen.push(Resident { tag: format!("census-{day}"), trade: trade.into(), level: 1 + r.below(5) as u8, baby: r.below(3) == 0 });
    }
    if r.below(6) == 0 {
        // A villager of a hand gone, or with settlers, still about.
        let gone: Vec<String> = s.world.resource::<enterprise_core::roster::Hands>().tags.iter()
            .filter(|(id, _)| s.find_hand(**id).is_none()).map(|(_, t)| t.clone())
            .chain(s.world.resource::<Sightings>().retired.keys().cloned()).collect();
        if !gone.is_empty() {
            seen.push(Resident { tag: gone[r.below(gone.len() as u64) as usize].clone(), trade: "none".into(), level: 1, baby: false });
        }
    }
    let beds = s.town(t).people.beds.max(0.0) as u32 + (r.below(4) == 0) as u32;
    s.census(t, beds, &seen);
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let sightings = s.world.resource::<Sightings>();
    for (tag, id) in &sightings.retired {
        if let Some(living) = s.hand_tagged(tag) {
            return Err(format!("the retired tag {tag} (hand {id}) is hand {living}'s"));
        }
    }
    for t in 0..s.town_count() {
        let roster = s.roster(t);
        if !roster.hands.is_empty() && s.town(t).people.villagers != roster.residents() as f64 {
            return Err(format!("town {t}: {} villagers, {} on its roster", s.town(t).people.villagers, roster.residents()));
        }
    }
    Ok(())
}
