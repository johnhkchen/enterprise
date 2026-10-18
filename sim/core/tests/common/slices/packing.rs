//! Packing in the random worlds (`packing`, `docs/pallets.md` section 2): in about a third of the worlds some goods are
//! packed another way than in crates (barrels, bundles, not at all), and one needs cool storage; now and then a host has a
//! town pack goods of two or three kinds for another (`Society::pack_pallets`): goods of one key share pallets, an open
//! pallet is topped up by a second packing, what can't be crated rides loose or stays in stock; then a player carries the
//! pallets to their consignee's desk, which opens them (several goods on a pallet go into stock good by good). The
//! rules that ship owed goods, collect them Ex Works and top up a farm's earmark run in every world already; goods that
//! aren't crated reach them here.
//!
//! Invariants: a package holds no more than its kind holds of its good; a loose package is one a good on a pallet; the
//! goods on a pallet agree on their handling (`pallets.rs` checks that, and the caps). Goods and money are conserved by
//! the harness every day.

use super::super::*;
use enterprise_core::cargo::{How, Lading, Payment};
use enterprise_core::pallet::{Pallets, BARREL, BUNDLE, LOOSE};

pub fn extend(s: &mut Society, r: &mut Rng) {
    if r.below(3) != 0 {
        return;
    }
    let n = s.content().goods.len() as u64;
    let kind = [BARREL, BUNDLE, LOOSE][r.below(3) as usize];
    let (packed, cool) = (r.below(n) as usize, r.below(n) as usize);
    let mut content = s.content_mut();
    content.goods[packed].package = kind;
    content.goods[cool].handling.cool = true;
}

pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    let towns = s.town_count();
    if towns < 2 || r.below(10) != 0 {
        return;
    }
    let (at, mut to) = (r.below(towns as u64) as usize, r.below(towns as u64) as usize);
    if at == to {
        to = (to + 1) % towns;
    }
    let goods = s.content().goods.len() as u64;
    let lines: Vec<(usize, f64)> = (0..1 + r.below(3)).map(|_| (r.below(goods) as usize, r.range(1.0, 900.0).floor())).collect();
    let lading = Lading { payment: Payment::Owed, ..Lading::new(at, to, s.now()) };
    let how = [How::OWED, How::CRATED, How::TOP_UP][r.below(3) as usize];
    let mut ids = s.pack_pallets(at, lading.clone(), &lines, how);
    if how.top_up && r.below(2) == 0 {
        ids.extend(s.pack_pallets(at, lading, &lines, how));
    }
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        s.pallet_brought(to, id);
    }
}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let content = s.content();
    for p in s.world.resource::<Pallets>().pallets.values() {
        let mut loose: Vec<usize> = Vec::new();
        for (i, pk) in p.packages.iter().enumerate() {
            if pk.kind == LOOSE {
                if loose.contains(&pk.good) {
                    return Err(format!("pallet {} has two loose packages of good {}", p.id, pk.good));
                }
                loose.push(pk.good);
                continue;
            }
            let holds = content.packages[pk.kind].slots.max(1) as f64 * content.goods[pk.good].stack.max(1) as f64;
            if pk.count > holds + 1e-9 {
                return Err(format!("pallet {}'s package {i} holds {} of good {}, a {} holds {holds}", p.id, pk.count, pk.good,
                    content.packages[pk.kind].name));
            }
        }
    }
    Ok(())
}
