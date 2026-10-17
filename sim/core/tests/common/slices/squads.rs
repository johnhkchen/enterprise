//! Squads' part of the random worlds (`squad.rs`): half the worlds' rostered towns take on a few more hands of the
//! squad's trades and idle ones (Nitwits, hands waiting for work, to sign on, train or carry); some routes want a
//! trade's work there; in a quarter of the worlds a plain good is a saddle, and a few towns keep saddles. Wagons from
//! towns with a roster are crewed (or stay home with no squad); those without go with one mouth as before.

use super::super::*;

const TRADES: [&str; 8] = ["none", "nitwit", "leatherworker", "librarian", "cartographer", "shepherd", "cleric", "farmer"];

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(4) == 0 {
        let goods = &s.content().goods;
        let start = r.below(goods.len() as u64) as usize;
        let plain = (0..goods.len()).map(|i| (start + i) % goods.len())
            .find(|&g| goods[g].food <= 0.0 && goods[g].comfort.is_none() && !goods[g].name.starts_with("saddle"));
        if let Some(g) = plain {
            s.content_mut().goods[g].name = enterprise_core::squad::SADDLE.to_string();
            for t in 0..s.town_count() {
                if r.below(3) == 0 {
                    s.deliver(t, g, 1.0 + r.below(3) as f64);
                }
            }
        }
    }
    if r.below(2) == 0 {
        return;
    }
    for t in 0..s.town_count() {
        if s.roster(t).hands.is_empty() || r.below(2) == 0 {
            continue;
        }
        for _ in 0..r.below(5) {
            s.add_hand(t, TRADES[r.below(TRADES.len() as u64) as usize], 1 + r.below(5) as u8);
        }
    }
    for route in 0..s.route_count() {
        if r.below(3) == 0 {
            let n = r.below(3) as usize;
            let wanted: Vec<&str> = (0..n).map(|_| TRADES[2 + r.below(TRADES.len() as u64 - 2) as usize]).collect();
            s.want_trades(route, &wanted);
        }
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(10) != 0 || s.route_count() == 0 {
        return;
    }
    let route = r.below(s.route_count() as u64) as usize;
    let wanted = [TRADES[2 + r.below(TRADES.len() as u64 - 2) as usize]];
    s.want_trades(route, if r.below(2) == 0 { &wanted[..] } else { &[] });
}

/// A crew is never more than four, and a wagon's slots are its crew's; every hand away from a town is on one of its
/// wagons or walking home from one.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let d = &s.content().dials;
    let mut out = vec![0usize; s.town_count()];
    for route in 0..s.route_count() {
        let c = s.crew(route);
        if c.crew.len() > enterprise_core::squad::MAX {
            return Err(format!("route {route}'s crew is {}: {c:?}", c.crew.len()));
        }
        if c.arrives.is_some() && !c.crew.is_empty() && c.slots < if c.porter { d.pack_stacks } else { d.wagon_slots } {
            return Err(format!("route {route} carries {} slots: {c:?}", c.slots));
        }
        out[c.home] += c.crew.len() + c.walkers;
    }
    // Or at the war (`army.rs`): its soldiers, its recruits waiting at the capital, and those walking home from either.
    let army = s.army();
    let at_war = army.soldiers.iter().map(|x| (x.home, &x.hand)).chain(army.barracks.iter().map(|x| (x.home, &x.hand)))
        .chain(army.walking.iter().map(|x| (x.home, &x.hand)));
    for (home, _) in at_war.filter(|(_, hand)| hand.is_some()) {
        if let Some(n) = out.get_mut(home) {
            *n += 1;
        }
    }
    // Or riding with a wagon as a recruit (`crown.rs`): a levy's to the capital, the barracks' to the front.
    for x in s.recruits_on_the_road().iter().filter(|x| x.hand.is_some()) {
        if let Some(n) = out.get_mut(x.home) {
            *n += 1;
        }
    }
    for (t, n) in out.iter().enumerate() {
        let away = s.roster(t).hands.iter().filter(|h| h.away).count();
        if away != *n {
            return Err(format!("town {t} has {away} hands away and {n} on its wagons or walking home"));
        }
    }
    Ok(())
}
