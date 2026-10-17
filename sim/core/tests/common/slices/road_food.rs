//! Road food's part of the random worlds (`road.rs`): a third of the worlds get bread (a good that wasn't food or a
//! comfort becomes 5 points a piece), some towns baking it, some eating it, a few holding a store; now and then a
//! town's bread comes in from outside. Wagons eat it both ways, buy it at the far end, pack it, and wait for it.

use super::super::*;

/// The good made bread in this world, if any: the first that's neither food nor a comfort, from a random start.
fn bread(s: &Society, r: &mut Rng) -> Option<usize> {
    let goods = &s.content().goods;
    let start = r.below(goods.len() as u64) as usize;
    (0..goods.len()).map(|i| (start + i) % goods.len()).find(|&g| goods[g].food <= 0.0 && goods[g].comfort.is_none())
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(3) != 0 {
        return;
    }
    let Some(g) = bread(s, r) else { return };
    s.content_mut().goods[g].food = 5.0;
    for t in 0..s.town_count() {
        match r.below(4) {
            0 => s.set_trade(t, g, r.range(5.0, 80.0), 0.0),
            1 => s.set_trade(t, g, 0.0, r.range(0.0, 20.0)),
            _ => {}
        }
        if r.below(2) == 0 {
            s.deliver(t, g, r.range(0.0, 400.0).floor());
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(8) != 0 || s.town_count() == 0 {
        return;
    }
    let food: Vec<usize> = (0..s.content().goods.len()).filter(|&g| enterprise_core::road::points(s.content(), g) > 0.0).collect();
    if food.is_empty() {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let g = food[r.below(food.len() as u64) as usize];
    s.deliver(t, g, r.range(0.0, 64.0).floor());
}

/// No wagon waits at a far end longer than `max_wait_ticks` (and a look); provisions within its crew's food; without
/// road food in the content, nothing eaten, paid for or waited on.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let content = s.content();
    let food = enterprise_core::road::road_food(content);
    for route in 0..s.route_count() {
        let v = s.road_food(route);
        if let Some(since) = v.waiting_since {
            if s.now() - since > content.dials.max_wait_ticks + content.dials.look_ticks {
                return Err(format!("route {route} has waited since {since}: {v:?}"));
            }
        }
        // (a leg's food a mouth, for a crew of at most a squad: `squad.rs`)
        let most = content.dials.food_capacity as f64 * enterprise_core::squad::MAX as f64;
        if v.provisions < 0.0 || v.provisions > most || v.food_eaten < 0.0 || v.food_paid < 0.0 {
            return Err(format!("route {route}: {v:?}"));
        }
        if !food && (v.waiting_since.is_some() || v.short_rations > 0 || v.food_eaten > 0.0 || v.food_paid > 0.0) {
            return Err(format!("route {route} ate with no food in the content: {v:?}"));
        }
    }
    Ok(())
}
