//! Sourcing and procurement in the random worlds (`sourcing.rs`, `rules/sourcing.rs`): some towns get a master Merchant and
//! contracts to deliver goods they don't hold, markets that count goods in their stocktakes, roads to them, so towns source
//! what they lack; some worlds open the capital's metal exchange (with a market across the sea) and a phase; and now and
//! then a market counts a different stock, or the sovereign changes phase.

use super::super::*;
use enterprise_core::sourcing::{Phase, MASTER};

/// What a market counts: some of the goods, a few hundred of each.
fn counted(r: &mut Rng, n_goods: usize) -> Vec<(usize, f64)> {
    let mut held = Vec::new();
    for g in 0..n_goods {
        if r.below(2) == 0 {
            held.push((g, r.range(0.0, 400.0).floor()));
        }
    }
    held
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    let (n_towns, n_goods) = (s.town_count(), s.content().goods.len());
    if n_towns < 2 {
        return;
    }
    // Masters, who broker, in a few towns.
    for t in 0..n_towns {
        if r.below(3) == 0 {
            s.add_hand(t, "merchant", MASTER);
        }
    }
    // Markets count what they hold (and each town hears of some), and a road to some of them.
    for t in 0..n_towns {
        let held = counted(r, n_goods);
        s.record_stocktake(t, &held, Some(0));
        if r.below(2) == 0 {
            s.learn(r.below(n_towns as u64) as usize, t);
        }
    }
    for _ in 0..r.below(n_towns as u64 + 1) {
        let (a, b) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if a != b {
            s.add_route(a, b, r.range(0.2, 3.0), r.range(0.0, 1.0), r.range(1.0, 6.0));
        }
    }
    // Contracts to deliver, in force at once, of goods the seller may not hold: its Merchant wants them.
    for _ in 0..r.below(4) {
        let (a, b) = (r.below(n_towns as u64) as usize, r.below(n_towns as u64) as usize);
        if a != b {
            s.draft(a, b, r.below(n_goods as u64) as usize, (1 + r.below(6)) as f64 * 4.0, r.range(0.02, 2.0),
                r.below(6) * DAY, (1 + r.below(3)) * DAY, 1 + r.below(8) as u32, Incoterm::Exw, true);
        }
    }
    // The capital's metal exchange in some worlds, with its market across the sea if the town is placed.
    if r.below(3) == 0 {
        // Two of its goods are iron and coal, which the exchange keeps (as the herds' random worlds rename goods to
        // sheep's): two the other slices have no use of by name.
        let spare: Vec<usize> = (0..n_goods)
            .filter(|&g| matches!(s.content().goods[g].name.as_str(), "sugar_cane" | "cookie" | "book") || s.content().goods[g].name.starts_with("good"))
            .collect();
        for (&g, name) in spare.iter().zip(["iron_ingot", "coal"]) {
            s.content_mut().goods[g].name = name.to_string();
        }
        let capital = r.below(n_towns as u64) as usize;
        if s.open_exchange_overseas(capital, r.range(20.0, 300.0)).is_none() {
            s.open_exchange(capital);
        }
        s.set_phase([Phase::Peace, Phase::Mobilising, Phase::War][r.below(3) as usize]);
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let (n_towns, n_goods) = (s.town_count(), s.content().goods.len());
    if r.below(6) == 0 {
        let t = r.below(n_towns as u64) as usize;
        let held = counted(r, n_goods);
        s.record_stocktake(t, &held, None);
    }
    if r.below(20) == 0 {
        s.set_phase([Phase::Peace, Phase::Mobilising, Phase::War][r.below(3) as usize]);
    }
}

/// What it sourced is whole lots at a price: a quantity of at least one, finite, a start not before it was drafted for a
/// trial lot, the advance only its share; a town's inputs are never negative and only a seller has any; the exchange's
/// premium stays within its clamp; and nothing it switched on for a contract is anything but an offer that sells.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for c in s.contracts().iter().filter(|c| c.drafted > 0) {
        if !(c.quantity >= 1.0 && c.quantity.fract() == 0.0 && c.price > 0.0 && c.price.is_finite() && c.installments >= 1) {
            return Err(format!("contract {}: sourced {} x {} at {}", c.id, c.quantity, c.installments, c.price));
        }
        if c.seller == c.buyer {
            return Err(format!("contract {}: sourced from itself", c.id));
        }
        if c.advance_share != 0.0 && (c.advance_share - s.dial("advance_share").unwrap()).abs() > 1e-12 {
            return Err(format!("contract {}: an advance of {}", c.id, c.advance_share));
        }
    }
    for t in 0..s.town_count() {
        if s.inputs(t).iter().any(|n| *n < 0.0 || !n.is_finite()) {
            return Err(format!("town {t} needs {:?}", s.inputs(t)));
        }
        if let Some(lockers) = s.lockers(t) {
            for (h, l) in lockers.hands.iter().enumerate() {
                if let Some(o) = l.offers.iter().find(|o| o.procured && !o.sells) {
                    return Err(format!("town {t}'s hand {h} procured a buying offer {o:?}"));
                }
            }
        }
    }
    for g in 0..s.content().goods.len() {
        let p = s.exchange_premium(g);
        if !(0.75..=1.5).contains(&p) {
            return Err(format!("the exchange's premium for {g} is {p}"));
        }
    }
    Ok(())
}
