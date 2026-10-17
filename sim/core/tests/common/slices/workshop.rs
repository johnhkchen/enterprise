//! The workshop's part of the random worlds: the first goods take the mod's names (wheat, cane, paper, bread, biscuits,
//! books, leather) so its conversions exist, keeping the base world's comfort and food flags (so a conversion's output
//! is a comfort, a food or neither as the seed falls, and every slice keeps the base world's comforts and food), and
//! some towns get farmers and librarians, a locker's par for paper, and forward sales that set wheat aside.

use super::super::*;

const NAMES: [&str; 7] = ["wheat", "sugar_cane", "paper", "bread", "cookie", "book", "leather"];

pub fn extend(s: &mut Society, r: &mut Rng) {
    let n = s.content().goods.len().min(NAMES.len());
    {
        let mut content = s.content_mut();
        for (i, name) in NAMES.iter().enumerate().take(n) {
            content.goods[i].name = name.to_string();
        }
    }
    s.rebuild_conversions();
    for t in 0..s.town_count() {
        for trade in ["farmer", "librarian", "cartographer"] {
            if r.below(2) == 0 {
                s.add_hand(t, trade, 1 + r.below(5) as u8);
            }
        }
        if n > 2 && r.below(2) == 0 {
            s.set_locker_par(t, 2, r.range(0.0, 200.0).floor());
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    for t in 0..s.town_count() {
        if r.below(4) == 0 {
            s.set_aside(t, 0, r.range(0.0, 300.0).floor());
        }
    }
}

/// Every conversion names goods and trades the content has, and a town's demand for inputs is never negative.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let c = s.content();
    for conv in &c.conversions {
        if conv.output >= c.goods.len() || conv.inputs.iter().any(|&(g, _)| g >= c.goods.len())
            || conv.trades.iter().any(|&t| t >= c.trades.len()) {
            return Err("a conversion names a good or trade the content lacks".into());
        }
    }
    for t in 0..s.town_count() {
        if s.input_demand(t).iter().any(|d| d.per_day < 0.0 || d.each < 0.0) {
            return Err(format!("town {t}: a negative demand for inputs"));
        }
    }
    Ok(())
}
