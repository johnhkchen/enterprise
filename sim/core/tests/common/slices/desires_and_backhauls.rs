//! Desires and backhauls' part of the random worlds (`desires.rs`, `rules/desires.rs`): towns hold comforts and gluts
//! of goods, some towns have several routes (a Merchant supplying other towns has outlets to buy for), and now and then
//! a host delivers a glut or comforts, or money, so wagons shop for home, buy surplus and gluts for the trip home, and
//! far ends buy comforts to sell on; towns list what they're after in their shops.

use super::super::*;
use enterprise_core::cash_book::Account;

pub fn extend(s: &mut Society, r: &mut Rng) {
    let n = s.town_count() as u64;
    let goods = s.content().goods.len() as u64;
    // Another route or two from the towns that have one, so a town's Merchant supplies several.
    for _ in 0..r.below(n + 1) {
        let (a, b) = (r.below(n) as usize, r.below(n) as usize);
        if a != b {
            s.add_route(a, b, r.range(0.2, 2.0), r.range(0.0, 1.0), r.range(1.0, 6.0));
        }
    }
    // Comforts and a glut or two, where wagons will find them.
    for t in 0..s.town_count() {
        if r.below(3) == 0 {
            let g = r.below(goods) as usize;
            let glut = if r.below(2) == 0 { r.range(100.0, 1500.0) } else { r.range(0.0, 200.0) };
            s.deliver(t, g, glut.floor());
        }
        if r.below(4) == 0 {
            s.pay(t, r.range(0.0, 60.0));
        }
    }
}

/// A host's doings: now and then a glut of a good or some money reaches a town.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let n = s.town_count() as u64;
    let goods = s.content().goods.len() as u64;
    if r.below(5) == 0 {
        let t = r.below(n) as usize;
        let g = r.below(goods) as usize;
        s.deliver(t, g, r.range(0.0, 800.0).floor());
    }
    if r.below(8) == 0 {
        s.pay(r.below(n) as usize, r.range(0.0, 40.0));
    }
}

/// Listings are real goods at real prices, a lot at least one, a handful at most; what a wagon saw listed is a price;
/// an outlet pays something and is short of something; wagons' purses never go under nothing and they never bought
/// less than they backhauled.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let goods = s.content().goods.len();
    for t in 0..s.town_count() {
        let shop = s.listings(t);
        if shop.len() > enterprise_core::desires::SHOP_SIZE {
            return Err(format!("town {t}'s shop lists {} goods", shop.len()));
        }
        for l in &shop {
            if l.good >= goods || !(l.lot >= 1.0 && l.lot.fract() == 0.0) || !(l.each > 0.0 && l.each.is_finite()) {
                return Err(format!("town {t}'s listing {l:?}"));
            }
        }
        for g in 0..goods {
            if s.seen_price(t, g).is_some_and(|p| !(p > 0.0 && p.is_finite())) {
                return Err(format!("town {t}'s seen price of {g}"));
            }
            if s.content().goods[g].comfort.is_some() {
                for home in 0..s.town_count() {
                    if let Some(o) = s.outlet(home, None, g) {
                        if !(o.each > 0.0 && o.each.is_finite() && o.units >= 1.0) {
                            return Err(format!("town {home}'s outlet for {g}: {o:?}"));
                        }
                    }
                }
            }
        }
    }
    for route in 0..s.route_count() {
        let (bought, paid, backhauled) = s.bought(route);
        if bought < 0.0 || paid < 0.0 || backhauled < 0.0 || backhauled > bought + 1e-9 || !paid.is_finite() {
            return Err(format!("route {route} bought {bought} for {paid}, {backhauled} as a backhaul"));
        }
        if s.balance(Account::Wagon(route)).is_some_and(|p| p < -1e-9) {
            return Err(format!("route {route}'s wagon holds {:?}", s.balance(Account::Wagon(route))));
        }
        if let Some(cargo) = s.wagon_cargo(route) {
            if cargo.iter().any(|n| *n < -1e-9 || !n.is_finite()) {
                return Err(format!("route {route}'s wagon carries {cargo:?}"));
            }
        }
    }
    Ok(())
}
