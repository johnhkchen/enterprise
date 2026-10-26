//! The farms' part of the random worlds: some goods become crops (one named wheat, so a granary banks it, and one
//! carrot, for the root cellar), some towns take on Farmers and plots of them, a village demand, a store; each day a
//! host's lockers sell crops now and then, and treasuries borrow their households' savings.

use super::super::*;
use enterprise_core::farms::{self, Store};

/// The crops of a world: the goods named wheat and carrot here, and any others marked.
fn crops(s: &Society) -> Vec<usize> {
    (0..s.content().goods.len()).filter(|&g| s.content().goods[g].crop).collect()
}

pub fn extend(s: &mut Society, r: &mut Rng) {
    let n = s.content().goods.len();
    if r.below(4) == 0 {
        return;
    }
    let wheat = r.below(n as u64) as usize;
    let carrot = (wheat + 1 + r.below(n as u64 - 1) as usize) % n;
    {
        let mut content = s.content_mut();
        content.goods[wheat].name = "wheat".into();
        content.goods[wheat].crop = true;
        if r.below(2) == 0 {
            content.goods[carrot].name = "carrot".into();
            content.goods[carrot].crop = true;
        }
    }
    let crops = crops(s);
    for t in 0..s.town_count() {
        if r.below(2) == 0 {
            continue;
        }
        for _ in 0..1 + r.below(2) {
            s.add_hand(t, "farmer", 1 + r.below(5) as u8);
        }
        for &c in &crops {
            if r.below(3) > 0 {
                s.set_plots(t, c, r.below(160) as u32);
            }
            if r.below(3) == 0 {
                s.set_farm_demand(t, c, r.range(0.0, 40.0));
            }
        }
        if r.below(4) == 0 {
            s.build_store(t, Store::ALL[r.below(2) as usize]);
        }
    }
}

/// A host's lockers selling crops now and then; treasuries borrowing what's on call of their households' savings.
pub fn daily(s: &mut Society, r: &mut Rng, _day: u64) {
    for t in 0..s.town_count() {
        if s.farm(t).is_none() {
            continue;
        }
        if r.below(3) == 0 {
            s.record_crop_sale(t, r.range(0.0, 5.0));
        }
        if r.below(8) == 0 {
            s.borrow_deposits(t, r.range(0.0, 30.0));
        }
    }
}

/// The share stays between its floor and ceiling, nothing owed or held is below nothing, a store holds no more than
/// it can, and a share paid is a share.
pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    for t in 0..s.town_count() {
        let Some(v) = s.farm(t) else { continue };
        let f = &v.farm;
        let floor = s.content().dials.min_labor_share.min(farms::MAX_LABOR_SHARE);
        if !(floor - 1e-12..=farms::MAX_LABOR_SHARE + 1e-12).contains(&f.labor_share) {
            return Err(format!("town {t}: labour share {}", f.labor_share));
        }
        if f.credit < -1e-9 || f.wage_owed < -1e-9 || v.deposits < -1e-9 {
            return Err(format!("town {t}: credit {}, wages owed {}, deposits {}", f.credit, f.wage_owed, v.deposits));
        }
        if f.pending.values().chain(f.banked.values()).any(|n| *n < -1e-9) {
            return Err(format!("town {t}: pending {:?}, banked {:?}", f.pending, f.banked));
        }
        if !(0.0..=1.0).contains(&f.paid_share) {
            return Err(format!("town {t}: paid share {}", f.paid_share));
        }
        for store in &f.stores {
            if farms::in_store(s.content(), f, *store) > store.holds() {
                return Err(format!("town {t}: {store:?} over full"));
            }
        }
    }
    Ok(())
}
