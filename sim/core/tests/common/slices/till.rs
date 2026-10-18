//! The till's part of the random worlds (`till.rs`): every town's desk, its chests and Banker counted now and then by a
//! host whose chests hold what the core took them to, give or take what a player did since (emeralds put in or taken
//! out, coin paid in, the Banker's balance drawn on), the treasury shown in them at once ([`Society::show_cash`]), the
//! move written as far as it goes (a hopper or a full chest now and then), what couldn't be done sent back
//! ([`Society::cash_unmoved`]). Some towns' hosts never write (Java mode). Checked: no pile of coin stands for
//! less than nothing, or for anything with no spur in it.

use super::super::*;
use enterprise_core::cash_book::{Account, Reason};
use enterprise_core::points::{Kind, Spot};
use enterprise_core::Fixed;

fn spot(t: usize) -> Spot {
    Spot { dim: "minecraft:overworld".into(), x: 1000 * t as i32, y: 64, z: 11 }
}

pub fn extend(s: &mut Society, _r: &mut Rng) {
    for t in 0..s.town_count() {
        s.add_point(Kind::Desk, spot(t), t).unwrap();
    }
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if r.below(3) == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let at = spot(t);
    let Some(c) = s.cash(&at) else { return };
    // What the host holds: what the core took it to, and now and then a player's doing.
    let (mut emeralds, mut coins, mut bank) = (c.till.emeralds as i64, c.till.coins.spurs as i64, c.till.bank.map(|b| b.spurs as i64));
    match r.below(6) {
        0 => emeralds = (emeralds + r.range(-30.0, 60.0) as i64).max(0),
        1 => coins = (coins + r.range(-300.0, 900.0) as i64).max(0),
        2 => bank = Some((bank.unwrap_or(0) + r.range(-500.0, 1500.0) as i64).max(0)),
        3 if r.below(4) == 0 => bank = None,
        _ => {}
    }
    let writes = t % 3 != 0;
    s.count_cash(&at, emeralds as u64, coins as u64, bank.map(|b| b as u64), r.below(5) != 0, writes).unwrap();
    // The town spends or earns some, now and then, before its desk is looked at.
    if r.below(3) == 0 {
        let spend = Fixed::from(r.range(0.0, 20.0)).min(s.town(t).money.at_least_zero());
        s.post(Account::Treasury(t), Account::Outside, spend, Reason::Other("works".into()));
    }
    for m in s.show_cash(&[at.clone()]) {
        // A hopper or a full chest now and then: some of it can't be done.
        if r.below(4) == 0 {
            let part = |n: i64, r: &mut Rng| (n as f64 * r.unit()).floor() as i64;
            let (e, c, b) = (part(m.emeralds, r), part(m.coins, r), part(m.bank, r));
            assert!(s.cash_unmoved(&m.at, e, c, b));
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for (at, till) in &s.tills().0 {
        for (what, pile) in [("coin", Some(till.coins)), ("bank", till.bank)] {
            let Some(p) = pile else { continue };
            if p.worth.is_negative() || (p.spurs == 0 && !p.worth.is_zero()) {
                return Err(format!("the {what} at {at:?} holds {} spurs standing for {}", p.spurs, p.worth));
            }
        }
    }
    Ok(())
}
