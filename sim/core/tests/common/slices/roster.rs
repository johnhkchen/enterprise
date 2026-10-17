//! The roster's part of the random worlds: the base world sets each town's hands; here, that they hold.

use super::super::*;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

/// Every town's villagers are its roster's residents, and every hand's level is 1 to 5.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let (roster, people) = (s.roster(t), s.town(t).people);
        if roster.residents() as f64 != people.villagers {
            return Err(format!("town {t}: {} residents, {} villagers", roster.residents(), people.villagers));
        }
        if !roster.hands.iter().all(|h| (1..=5).contains(&h.level)) {
            return Err(format!("town {t}: a level outside 1 to 5"));
        }
    }
    Ok(())
}
