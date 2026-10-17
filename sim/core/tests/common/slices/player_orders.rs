//! Player orders' part of the random worlds (`orders.rs`, `rules/orders.rs`): some towns get a master Merchant and
//! players' shops, buying shops with prices around the goods' worth (some too low to take on), selling shops and the
//! town's own; now and then a host adds a shop, tops one up, or a player buys from the town's. Checked: no purse, shelf
//! or stall goes under nothing, a signal is a real want at a real price, an order never paid out more than its price
//! for what it was handed.

use super::super::*;
use enterprise_core::orders::Orders;

pub fn extend(s: &mut Society, r: &mut Rng) {
    for t in 0..s.town_count() {
        if r.below(3) != 0 {
            continue;
        }
        s.add_hand(t, "merchant", 5);
        for _ in 0..r.below(4) {
            add_shop(s, r, t);
        }
    }
}

/// A buying shop, a selling shop or a town's own at a desk, with prices around the good's worth.
fn add_shop(s: &mut Society, r: &mut Rng, t: usize) {
    let goods = s.content().goods.len() as u64;
    let g = r.below(goods) as usize;
    let rate = s.mint_rate();
    let lot = [1.0, 4.0, 16.0, 32.0][r.below(4) as usize];
    let worth = s.content().goods[g].value * lot * rate;
    let price = (worth * r.range(0.5, 2.0)).ceil().max(1.0);
    match r.below(4) {
        0 | 1 => {
            s.order(t, g, lot, price, (price * r.range(0.0, 14.0)).floor());
        }
        2 => {
            s.stall(t, g, lot, price, lot * r.below(6) as f64);
        }
        _ => {
            s.own_shop(t);
        }
    }
}

/// A host's doings: a shop opens, a purse or stall is topped up, a player buys at the town's shop or takes theirs down.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let n = s.town_count() as u64;
    let t = r.below(n) as usize;
    match r.below(10) {
        0 if s.roster(t).merchant_level().is_some_and(|l| l >= 5) => add_shop(s, r, t),
        1 => {
            if let Some(o) = s.orders(t).first() {
                s.fund_order(o.id, r.range(0.0, 200.0).floor());
            }
        }
        2 => {
            if let Some(st) = s.stalls(t).first() {
                s.restock_stall(st.id, r.below(40) as f64);
            }
        }
        3 => {
            if let Some(shop) = s.own_shops(t).iter().find(|o| o.good.is_some()) {
                let _ = s.buy(t, shop.good.unwrap(), (1 + r.below(48)) as f64);
            }
        }
        4 if r.below(4) == 0 => {
            if let Some(o) = s.orders(t).first() {
                s.cancel_order(o.id);
            }
        }
        _ => {}
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let goods = s.content().goods.len();
    let orders = s.world.resource::<Orders>();
    for o in &orders.orders {
        if !(o.purse >= 0.0 && o.open >= 0.0 && o.delivered >= 0.0 && o.paid >= 0.0 && o.purse.is_finite()) {
            return Err(format!("order {o:?}"));
        }
        if o.paid > o.price * (o.delivered / o.lot) + 1e-6 {
            return Err(format!("order {} paid {} for {} goods at {} a lot of {}", o.id, o.paid, o.delivered, o.price, o.lot));
        }
        if o.delivered % o.lot != 0.0 {
            return Err(format!("order {} was handed {} goods, not whole lots of {}", o.id, o.delivered, o.lot));
        }
    }
    for st in &orders.stalls {
        if !(st.stock >= 0.0 && st.stock.is_finite()) {
            return Err(format!("stall {st:?}"));
        }
    }
    for shop in &orders.shops {
        if !(shop.shelf >= 0.0 && shop.takings >= 0.0) || (shop.shelf > 0.0 && shop.good.is_none()) {
            return Err(format!("shop {shop:?}"));
        }
        if shop.good.is_some_and(|g| g >= goods) {
            return Err(format!("shop {shop:?} lists no good"));
        }
    }
    for (t, wanted) in &orders.wanted {
        let open: f64 = orders.orders.iter().filter(|o| o.town == *t).map(|o| o.open).sum();
        let total: f64 = wanted.values().sum();
        if wanted.values().any(|w| !(*w > 0.0)) || total > open + 1e-6 {
            return Err(format!("town {t} wants {wanted:?} of {open} open"));
        }
    }
    for t in 0..s.town_count() {
        for sig in s.posted_signals(t) {
            if sig.good >= goods || !(sig.count > 0.0 && sig.spurs_each >= 1.0 && sig.spurs_each.is_finite()) {
                return Err(format!("town {t}'s signal {sig:?}"));
            }
        }
        let tally = s.tally(t);
        if ![tally.lots_sold, tally.spurs_from_sales, tally.lots_bought, tally.spurs_on_purchases, tally.spurs_from_own_shops]
            .iter().all(|x| *x >= 0.0 && x.is_finite()) {
            return Err(format!("town {t}'s tally {tally:?}"));
        }
    }
    Ok(())
}
