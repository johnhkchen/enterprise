//! The war's part of the random worlds (`army.rs`): in a quarter of the worlds an army is raised at a front off the
//! map. Their foods get a saturation (some are meat), up to two goods that aren't food or comforts are named for kit
//! (a weapon, armour, a shield or arrows), towns send hands to the front at once or to wait at the capital (some under a
//! levy contract), and the front is sent food and kit, some enchanted. Now and then more food and kit arrive,
//! recruits go on to the front, towns send more, and peace sends everyone home (the next war starts again).
//!
//! The checks: every soldier, recruit and walker with a hand is that hand, away, in its town's roster; no town lost
//! more soldiers than it raised; the expected losses carried are a fraction of a soldier; worn kit is short of worn
//! out, exposure, heartiness and pay are in their bounds; griefs are of whole soldiers. (Conservation and solvency
//! are `check.rs`'s, every day: food eaten and kit drawn leave the world's goods.)

use super::super::*;

const KIT: [&str; 8] = ["iron_sword", "bow", "iron_chestplate", "diamond_helmet", "shield", "arrow", "stone_axe",
    "leather_boots"];

fn foods(s: &Society) -> Vec<usize> {
    (0..s.content().goods.len()).filter(|&g| s.content().goods[g].food > 0.0).collect()
}

fn kit(s: &Society) -> Vec<usize> {
    (0..s.content().goods.len()).filter(|&g| KIT.contains(&s.content().goods[g].name.as_str())).collect()
}

/// Kit and food for the front.
fn supply(s: &mut Society, r: &mut Rng, front: usize) {
    for g in foods(s) {
        if r.below(2) == 0 {
            s.deliver(front, g, r.range(0.0, 200.0).floor());
        }
    }
    for g in kit(s) {
        if r.below(2) == 0 {
            let n = 1 + r.below(6) as u32;
            let name = s.content().goods[g].name.clone();
            if r.below(3) == 0 {
                s.add_kit(front, &name, n, r.below(5) as f64, r.below(3) as u8);
            } else {
                s.deliver(front, g, n as f64 * if name == "arrow" { 16.0 } else { 1.0 });
            }
        }
    }
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    let towns = s.town_count();
    if r.below(4) != 0 || towns == 0 {
        return;
    }
    {
        let mut content = s.content_mut();
        for g in content.goods.iter_mut().filter(|g| g.food > 0.0) {
            g.saturation = g.food * r.range(0.5, 2.6);
            g.meat = r.below(3) == 0;
        }
        // Up to two goods that are neither food nor comforts named for kit (the other slices have named every good
        // for theirs: what they did by name with these they do no more; no conversions or offers are rebuilt).
        let free: Vec<usize> = (0..content.goods.len())
            .filter(|&g| content.goods[g].food <= 0.0 && content.goods[g].comfort.is_none())
            .collect();
        for &g in free.iter().rev().take(r.below(3) as usize) {
            content.goods[g].name = KIT[r.below(KIT.len() as u64) as usize].to_string();
        }
    }
    let front = s.add_off_map("front", r.range(-3000.0, 3000.0), r.range(-3000.0, 3000.0), r.range(0.0, 100.0));
    s.set_front(front);
    supply(s, r, front);
    for _ in 0..1 + r.below(3) {
        let t = r.below(towns as u64) as usize;
        match r.below(3) {
            0 => {
                s.muster(t, 1 + r.below(3) as usize);
            }
            1 => {
                // Under a levy: a contract with the capital (where there's one), in force, its pay owed through it;
                // with another town, its pay in the army's ledger.
                let buyer = s.world.resource::<enterprise_core::realms::Crown>().capital
                    .filter(|_| r.below(4) != 0)
                    .unwrap_or_else(|| r.below(towns as u64) as usize);
                if buyer != t {
                    let levy = s.draft(t, buyer, 0, 1.0, 0.0, 400 * DAY, DAY, 1, enterprise_core::Incoterm::Exw, true);
                    s.muster_levied(t, 1 + r.below(3) as usize, Some(levy));
                }
            }
            _ => {
                s.enlist(t, 1 + r.below(5) as usize);
            }
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let Some(front) = s.army().front else { return };
    let towns = s.town_count();
    match r.below(12) {
        0 | 1 => supply(s, r, front),
        2 => {
            s.embark(1 + r.below(3) as usize);
        }
        3 => {
            s.enlist(r.below(towns as u64) as usize, 1 + r.below(2) as usize);
        }
        4 => {
            s.muster(r.below(towns as u64) as usize, 1);
        }
        5 if r.below(4) == 0 => {
            s.demobilise();
        }
        _ => {}
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let army = s.army();
    let away = |home: usize, hand: &Option<enterprise_core::Hand>| -> Result<(), String> {
        let Some(hand) = hand else { return Ok(()) };
        let n = s.roster(home).hands.iter().filter(|h| h.away && h.id == hand.id).count();
        if n == 1 { Ok(()) } else { Err(format!("town {home}: {n} hands away with id {}", hand.id)) }
    };
    for x in &army.soldiers {
        away(x.home, &x.hand)?;
        let worn = x.weapon.iter().chain(x.armour.iter().flatten()).chain(x.shield.iter());
        if let Some(w) = worn.into_iter().find(|w| !(0.0..1.0).contains(&w.worn)) {
            return Err(format!("a soldier of town {}: kit worn {}", x.home, w.worn));
        }
        if !(x.exposure.is_finite() && x.exposure >= 0.0) || x.pay < 0.0 {
            return Err(format!("a soldier of town {}: exposure {}, pay {}", x.home, x.exposure, x.pay));
        }
    }
    for x in &army.barracks {
        away(x.home, &x.hand)?;
    }
    for x in &army.walking {
        away(x.home, &x.hand)?;
    }
    for (t, rec) in &army.records {
        let serving = army.serving_from(*t) as u32;
        if rec.fallen + rec.deserted + serving > rec.raised || !rec.strength.is_finite() || rec.strength < 0.0 {
            return Err(format!("town {t}: {rec:?} with {serving} serving"));
        }
    }
    for carry in [army.falling, army.deserting] {
        if !(0.0..1.0).contains(&carry) {
            return Err(format!("a carry of {carry} soldiers"));
        }
    }
    if !(0.0..=1.0 + 1e-9).contains(&army.heartiness()) || army.eaten < 0.0 {
        return Err(format!("meals {} hearty, {} eaten", army.heartiness(), army.eaten));
    }
    if let Some((t, p)) = army.pay_owed.iter().find(|(_, p)| p.is_negative()) {
        return Err(format!("town {t}: pay owed {p}"));
    }
    for t in 0..s.town_count() {
        if let Some(g) = s.town(t).people.grief.iter().find(|(_, n)| *n < 1.0 || n.fract() != 0.0) {
            return Err(format!("town {t}: grief {g:?}"));
        }
    }
    Ok(())
}
