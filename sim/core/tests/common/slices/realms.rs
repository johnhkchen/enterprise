//! Realms' part of the random worlds (`realms.rs`): in a third of the worlds, a realm beyond the border (making some
//! goods, wanting others, with a few roads to our towns and back), sometimes a plain off-map market too (selling food
//! or not, with a contract drafted to it), and sometimes a capital named for the customs. Its wagons carry two mouths
//! and no squad; a realm is always off the map; customs are never negative.

use super::super::*;

pub fn extend(s: &mut Society, r: &mut Rng) {
    let towns = s.town_count();
    let goods = s.content().goods.len();
    if r.below(3) != 0 || towns == 0 {
        return;
    }
    let mut makes: Vec<(usize, f64, f64)> = Vec::new();
    for _ in 0..1 + r.below(3) {
        let good = r.below(goods as u64) as usize;
        if !makes.iter().any(|&(g, ..)| g == good) {
            let per_day = r.range(4.0, 64.0).floor();
            makes.push((good, per_day, per_day * (2.0 + r.below(6) as f64)));
        }
    }
    let wants: Vec<usize> = (0..goods).filter(|_| r.below(2) == 0).collect();
    let realm = s.add_realm("realm", r.range(-5000.0, 5000.0), r.range(-5000.0, 5000.0), r.range(0.0, 2000.0), &makes, &wants);
    for _ in 0..1 + r.below(3) {
        let t = r.below(towns as u64) as usize;
        s.add_route(realm, t, r.range(0.2, 3.0), r.range(0.0, 3.0), r.range(1.0, 6.0));
        if r.below(2) == 0 {
            s.add_route(t, realm, r.range(0.2, 3.0), r.range(0.0, 3.0), r.range(1.0, 6.0));
        }
    }
    if r.below(2) == 0 {
        s.set_capital(Some(r.below(towns as u64) as usize));
    }
    if r.below(3) == 0 {
        let front = s.add_off_map("front", r.range(-5000.0, 5000.0), r.range(-5000.0, 5000.0), r.range(0.0, 500.0));
        s.set_off_map_sells(front, r.below(2) == 0);
        let from = r.below(towns as u64) as usize;
        s.add_route(from, front, r.range(0.2, 2.0), r.range(0.0, 3.0), r.range(1.0, 6.0));
        s.draft(from, front, r.below(goods as u64) as usize, 32.0, r.range(0.02, 1.0), 0, DAY, 1,
            enterprise_core::Incoterm::Dap, true);
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(15) == 0 && s.town_count() > 0 {
        let capital = (r.below(2) == 0).then(|| r.below(s.town_count() as u64) as usize);
        s.set_capital(capital);
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        if let Some(realm) = s.realm(t) {
            if !(realm.customs_paid >= 0.0 && realm.goods_made >= 0.0) {
                return Err(format!("realm {t}: {realm:?}"));
            }
            if !s.is_off_map(t) {
                return Err(format!("realm {t} is on the map"));
            }
        }
    }
    Ok(())
}
