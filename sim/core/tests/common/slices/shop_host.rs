//! A host's vendors' part of the random worlds (`shop_host.rs`): now and then a host places a vendor by a desk and counts
//! it (buying, selling, touching the desk, empty, a board's), or breaks one, and the look tells it what to show. Checked:
//! every shop a host told the core of stands at a shop point of its town's, no two shops at one point, and a vendor's
//! shelf, purse and room are numbers at or above nothing.

use std::collections::BTreeSet;

use super::super::*;
use enterprise_core::orders::Orders;
use enterprise_core::points::{Kind, Spot};
use enterprise_core::wire::{Input, Point, PointKind, VendorMode, VendorRead};

fn spot(t: usize, k: u64) -> Spot {
    Spot { dim: "minecraft:overworld".into(), x: 1000 * t as i32 + 20, y: 64, z: 100 + k as i32 }
}

fn point(at: &Spot) -> Point {
    Point { kind: PointKind::Shop, dim: at.dim.clone(), x: at.x, y: at.y, z: at.z }
}

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    if s.town_count() == 0 {
        return;
    }
    let t = r.below(s.town_count() as u64) as usize;
    let at = spot(t, r.below(5));
    match r.below(12) {
        // A vendor broken.
        0 => {
            if s.point(&at).is_some() {
                s.remove_point(&at).unwrap();
            }
        }
        // The board's vendor, put up.
        1 => {
            if s.point(&at).is_none() {
                s.add_point(Kind::Shop, at.clone(), t).unwrap();
            }
            if s.point(&at).is_some_and(|p| p.kind == Kind::Shop && p.town == t) {
                s.apply(&Input::Board { point: point(&at) });
            }
        }
        // A vendor counted.
        _ => {
            if s.point(&at).is_none() {
                s.add_point(Kind::Shop, at.clone(), t).unwrap();
            }
            if !s.point(&at).is_some_and(|p| p.kind == Kind::Shop && p.town == t) {
                return;
            }
            let goods = s.content().goods.len() as u64;
            let g = r.below(goods) as u32;
            let lot = [1, 4, 16, 32][r.below(4) as usize];
            let rate = s.mint_rate();
            let worth = s.content().goods[g as usize].value * lot as f64 * rate;
            let price = ((worth * r.range(0.5, 2.0)).ceil() as u32).max(1);
            let read = VendorRead {
                mode: [VendorMode::Idle, VendorMode::Buy, VendorMode::Sell][r.below(3) as usize],
                good: if r.below(8) == 0 { None } else { Some(g) },
                deed: None,
                lot,
                price,
                stock: r.below(80) as u32,
                room: r.below(8) as u32,
                purse: price as u64 * r.below(12),
                coins: if r.below(3) == 0 { r.below(40) } else { 0 },
                touches: r.below(3) == 0,
            };
            s.apply(&Input::Vendor { point: point(&at), read });
            s.show_shops(&[t], &[at]);
        }
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let orders = s.world.resource::<Orders>();
    let mut seen: BTreeSet<Spot> = BTreeSet::new();
    let shops = orders.orders.iter().map(|o| (o.at.clone(), o.town, "order"))
        .chain(orders.stalls.iter().map(|o| (o.at.clone(), o.town, "stall")))
        .chain(orders.shops.iter().map(|o| (o.at.clone(), o.town, "shop")));
    for (at, town, what) in shops {
        let Some(at) = at else { continue };
        match s.point(&at) {
            Some(p) if p.kind == Kind::Shop && p.town == town => {}
            other => return Err(format!("a {what} of town {town} at {at:?}, where {other:?} stands")),
        }
        if !seen.insert(at.clone()) {
            return Err(format!("two shops at {at:?}"));
        }
    }
    for o in orders.orders.iter().filter(|o| o.at.is_some()) {
        if o.room.is_some_and(|r| !(r >= 0.0)) {
            return Err(format!("order {} has room {:?}", o.id, o.room));
        }
    }
    for sh in orders.shops.iter().filter(|s| s.at.is_some()) {
        if let Some((_, lot, price, shelf)) = sh.shown {
            if !(lot >= 0.0 && price >= 0.0 && shelf >= 0.0) {
                return Err(format!("shop {sh:?} shown at {:?}", sh.shown));
            }
        }
    }
    Ok(())
}
