//! Chokepoints' part of the random worlds: towns review their tolls as wagons cross their land (that's the look's
//! own doing); some guards are armed; now and then a town asks what a bridge would bring it. The checks: a town never
//! asks a route more than going around costs (spatial equilibrium: the toll settles at the detour's price), its posts'
//! options grow a post at a time, and a bridge's worth is never more than the most it may ask.

use super::super::*;
use enterprise_core::chokepoints::ARMED_DETOUR_SHARE;

/// A trading pair or two across the map (a seller making a good, a buyer using it, a road between), so wagons cross
/// the other towns' land; some towns' guards are armed. (Armour would let a toll past 25%, which `territory_growth`'s
/// check refuses.)
pub fn extend(s: &mut Society, r: &mut Rng) {
    let goods = s.content().goods.len() as u64;
    for _ in 0..1 + r.below(2) {
        let n = s.town_count();
        let (seller, buyer) = (s.add_town(&format!("town{n}"), r.range(20.0, 80.0)), s.add_town(&format!("town{}", n + 1), r.range(100.0, 400.0)));
        let g = r.below(goods) as usize;
        for (t, makes, uses) in [(seller, r.range(50.0, 200.0), 0.0), (buyer, 0.0, r.range(10.0, 60.0))] {
            s.set_people(t, 4.0, 4.0, 8.0);
            s.set_roster(t, &[("farmer", 1), ("farmer", 2), ("mason", 1), ("nitwit", 1)]);
            s.set_trade(t, g, makes, uses);
            let hex = Hex::new(r.below(12) as i32, r.below(12) as i32);
            s.place_town(t, hex.center_x(), hex.center_z());
        }
        s.deliver(seller, g, r.range(0.0, 300.0).floor());
        s.add_road(seller, buyer, r.range(1.0, 3.0));
    }
    for t in 0..s.town_count() {
        if r.below(4) == 0 {
            s.set_guard_kit(t, r.unit(), 0.0);
        }
    }
}

/// Now and then a town asks what a bridge on one of its hexes would bring in: never less than nothing, never more
/// than a quarter of every route's expected coins.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(4) != 0 || s.town_count() == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let hexes = s.holding(t).hexes;
    if hexes.is_empty() {
        return;
    }
    let hex = hexes[r.below(hexes.len() as u64) as usize];
    let v = s.bridge_value(t, hex);
    let most: f64 = (0..s.route_count()).map(|x| 0.25 * s.expected_value(x)).sum();
    assert!(v.tolls_per_day >= 0.0 && v.tolls_per_day <= most + 1e-6, "town {t}: a bridge at {hex} worth {}", v.tolls_per_day);
}

/// A town's plan asks each route it counts on no more than its share of the detour, toll and fee together, and so
/// never more than going around would cost; every option is the one before with a post more; detours aren't negative.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for (town, h) in &s.territory().holdings {
        let plan = &h.toll_plan;
        for a in &plan.asked {
            let asked = plan.rate * a.value + plan.fee;
            if a.share > ARMED_DETOUR_SHARE + 1e-12 || asked > a.share * a.detour + 1e-6 || asked > a.detour + 1e-6 {
                return Err(format!("town {town} asks route {} {asked} spurs against a {} detour", a.route, a.detour));
            }
        }
        for c in &h.crossings {
            for (i, o) in c.options.iter().enumerate() {
                if o.posts.len() != i + 1 || (i > 0 && o.posts[..i] != c.options[i - 1].posts[..]) {
                    return Err(format!("town {town}, route {}: options {:?}", c.route, c.options));
                }
                if o.detour.is_nan() || o.detour < 0.0 {
                    return Err(format!("town {town}, route {}: detour {}", c.route, o.detour));
                }
            }
        }
    }
    Ok(())
}
