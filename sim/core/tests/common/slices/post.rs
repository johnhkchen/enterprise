//! The post in the random worlds (`post.rs`): in half the worlds a contract is drafted at one town for another no wagon
//! runs to, and a bill drawn between two such towns, so copies and bills go by post; now and then a town writes to a
//! hub to come call. The invariants: letters go between two towns inside the realm, arrive no sooner than posted and
//! carry nothing less than nothing; bills in a letter are with the riders, not at a town; calls are a hub's and runs on
//! call a hub's; a collection due is a town's booking; lots never sell more than they hold or owe less than nothing.

use super::super::*;
use enterprise_core::bills::Held;
use enterprise_core::post::Kind;

/// Two towns no route joins, either way.
fn unjoined(s: &Society, r: &mut Rng) -> Option<(usize, usize)> {
    let n = s.town_count();
    let joined = |a: usize, b: usize| (0..s.route_count()).any(|x| {
        let (f, t) = s.route_ends(x);
        (f, t) == (a, b) || (f, t) == (b, a)
    });
    let pairs: Vec<(usize, usize)> = (0..n).flat_map(|a| (0..n).map(move |b| (a, b)))
        .filter(|&(a, b)| a != b && !joined(a, b)).collect();
    (!pairs.is_empty()).then(|| pairs[r.below(pairs.len() as u64) as usize])
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(2) != 0 {
        return;
    }
    if let Some((a, b)) = unjoined(s, r) {
        let good = r.below(s.content().goods.len() as u64) as usize;
        let price = s.content().goods[good].value;
        s.draft(a, b, good, (1 + r.below(4)) as f64 * 16.0, price, (1 + r.below(5)) * DAY, (1 + r.below(4)) * DAY,
            1 + r.below(6) as u32, Incoterm::Dap, false);
    }
    if let Some((drawee, payee)) = unjoined(s, r) {
        s.draw_bill(drawee, payee, r.range(1.0, 40.0));
    }
}

/// A host's doings: now and then a town writes to a hub to come call, its goods worth something or little.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(8) != 0 {
        return;
    }
    let hubs: Vec<usize> = (0..s.town_count()).filter(|&t| s.is_hub(t)).collect();
    if hubs.is_empty() {
        return;
    }
    let hub = hubs[r.below(hubs.len() as u64) as usize];
    let town = r.below(s.town_count() as u64) as usize;
    if town != hub {
        s.book_call(town, hub, r.range(0.0, 300.0));
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let mail = s.mail();
    let bills = s.bills();
    for l in &mail.letters {
        if l.from == l.to || l.from >= s.town_count() || l.to >= s.town_count() || l.arrives < l.posted
            || !(l.value >= 0.0 && l.value.is_finite()) {
            return Err(format!("a letter {l:?}"));
        }
        if s.is_off_map(l.to) || s.is_off_map(l.from) || s.realm(l.to).is_some() || s.realm(l.from).is_some() {
            return Err(format!("a letter beyond the border: {l:?}"));
        }
        if l.kind == Kind::Bill && l.bills.iter().any(|&id| matches!(bills[id].held, Held::Town(_))) {
            return Err(format!("a bill in the post is at a town: {l:?}"));
        }
        if l.kind == Kind::Copy && l.contract.is_none_or(|c| c >= s.contracts().len()) {
            return Err(format!("a copy of nothing: {l:?}"));
        }
    }
    for c in &mail.calls {
        if c.hub == c.town || !(c.value >= 0.0 && c.value.is_finite()) {
            return Err(format!("a call {c:?}"));
        }
    }
    for route in 0..s.route_count() {
        let (from, _) = s.route_ends(route);
        if s.on_call(route) && !s.is_hub(from) {
            return Err(format!("route {route} is on call from {from}, not a hub"));
        }
    }
    let f = s.forwarder();
    for &(hub, town) in &f.collecting {
        if !f.bookings.iter().any(|b| b.hub == hub && b.town == town) {
            return Err(format!("{hub} collects at {town} with no booking"));
        }
    }
    for b in &f.bookings {
        if !s.is_hub(b.hub) || b.hub == b.town {
            return Err(format!("a booking {b:?}"));
        }
    }
    for l in &f.lots {
        let ok = |v: f64| v >= 0.0 && v.is_finite();
        if l.hub == l.town || !ok(l.count) || !ok(l.sold) || l.sold > l.count + 1e-9 || !ok(l.proceeds) || !ok(l.fare) {
            return Err(format!("a lot {l:?}"));
        }
    }
    if ![f.remitted, f.commission, f.consigned].iter().all(|v| *v >= 0.0 && v.is_finite()) {
        return Err(format!("the forwarders' totals ran backwards: {f:?}"));
    }
    Ok(())
}
