//! The settlers' part of the random worlds: some towns grown past growing with hands waiting for work and bread, some
//! villages nobody trades with about the patch, and the invariants of the parties on the road.

use super::super::*;

pub fn extend(s: &mut Society, r: &mut Rng) {
    let n = s.town_count();
    // A few towns grown past growing: idle hands, food (twice the caravans' par and more), money.
    let food = (0..s.content().goods.len()).find(|&g| s.content().goods[g].food > 0.0);
    if let Some(food) = food {
        let points = s.content().goods[food].food;
        for t in 0..n {
            if r.below(2) == 0 {
                let hands = (9usize.saturating_sub(s.roster(t).residents())).max(1) + r.below(4) as usize;
                for _ in 0..hands {
                    s.add_hand(t, "none", 1);
                }
                s.deliver(t, food, (900.0 / points).ceil());
                s.pay(t, r.range(0.0, 150.0));
            }
        }
    }
    // Villages nobody trades with, on the patch.
    for _ in 0..r.below(3) {
        let hex = Hex::new(r.below(12) as i32, r.below(12) as i32);
        s.add_village(hex.center_x() + r.range(-20.0, 20.0), hex.center_z() + r.range(-20.0, 20.0));
    }
}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

/// A town has at most one party on the road, each carries what the rule gives it, and a party arrives no earlier than
/// it left.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let parties = s.parties();
    for (i, p) in parties.iter().enumerate() {
        if parties[i + 1..].iter().any(|q| q.from == p.from) {
            return Err(format!("town {} has two parties on the road", p.from));
        }
        if !(10.0..=42.0).contains(&p.capital) || p.hands == 0 || p.hands > 2 || p.bread < 0.0 || p.bread > 64.0 {
            return Err(format!("a party of {} hands carries {} and {} bread", p.hands, p.capital, p.bread));
        }
        if s.last_party(p.from).is_none_or(|t| t > p.arrives) {
            return Err(format!("town {}'s party arrives before it left", p.from));
        }
    }
    Ok(())
}
