//! The sweep's part of the random worlds (`docs/core-plan.md` 5.1): nothing added to a world, no doings; invariants
//! beyond conservation and solvency, checked at every day's end in every world every slice has made:
//!
//! - **Mood** stays finite and never sinks below the lowest its target can be ([`enterprise_core::sweep::mood_floor`]:
//!   its size and its villagers without beds, its wages unpaid, its grief at the most); none did in 300 seeds, though
//!   a town that had just shrunk (a levy, a host's `set_people`) could lag below it with no rule broken: the check
//!   allows no slack for that, so a slice that shrinks towns sharply may need it. A town whose villagers all have beds and that has no more than 32
//!   of them is never below -8 (the brief's floor: crowding and size take a town lower, as `Territory.moodTarget` does).
//! - **No stall:** no wagon is out more than 30 days (a trip that never ends); a route home that isn't wound down for
//!   a hub nor on call has said why it stayed (`stayed`), unless it has never been planned (just opened). (A route
//!   that stays "waiting for a fuller load" a month on is one whose load isn't worth its road food: it's on its day,
//!   `rules/plan.rs::decide`, so goods and a paying buyer send it within `every_days`.)
//! - **No contract waits forever to be judged:** judging ran today or yesterday, and one in force for goods past its
//!   last deadline (its last installment's day and its grace, or a harvest's window) and three graces more has every
//!   installment judged by the next judging (one whose last copy arrived since today's comes into force late). (The
//!   mod leaves one short of its total, broken by fewer than three misses, in force after that, as
//!   `ContractBook.review` does; the sweep counts them.) Delivered, paid, owing, shipped and refunds are never below nothing.
//!   (While `review` judges every installment due, as it does now, the copy always comes out fully judged, so this
//!   amounts to "judging ran today or yesterday": the unjudged branch guards a review that would stop short.)
//! - **Wages paid or credited:** a farm's whole emeralds owed are settled every look (paid, credited, or the Farmers'
//!   share taken in crops), so less than one is owed at a day's end; the shares paid are shares; the households' purse
//!   is never below nothing.

use super::super::*;
use enterprise_core::contracts::{Book, GRACE_DAYS};
use enterprise_core::Contract;
use enterprise_core::sweep::mood_floor;

/// How long a wagon may be out, and the most days past an in-force contract's last deadline before it's all judged.
pub const STALL_DAYS: u64 = 30;
pub const JUDGED_GRACES: u64 = 3;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    mood(s)?;
    stalls(s)?;
    contracts(s)?;
    wages(s)
}

pub fn mood(s: &Society) -> Result<(), String> {
    let d = &s.content().dials;
    for t in 0..s.town_count() {
        let p = s.town(t).people;
        let floor = mood_floor(d, &p);
        if !p.mood.is_finite() || p.mood < floor {
            return Err(format!("town {t}: mood {} below its floor {floor} ({} villagers, {} beds)", p.mood,
                p.villagers, p.beds));
        }
    }
    Ok(())
}

pub fn stalls(s: &Society) -> Result<(), String> {
    let (now, day) = (s.now(), s.content().day_ticks);
    for r in 0..s.route_count() {
        let (route, state) = s.route_state(r);
        let out = now.saturating_sub(state.last_departure.unwrap_or(0));
        if state.wagon.is_some() && out > STALL_DAYS * day {
            let w = state.wagon.as_ref().unwrap();
            return Err(format!("route {r}: its wagon out {} days ({:?} leg, arrives {}, waiting since {:?})",
                out / day, w.leg, w.arrives, w.waiting_since));
        }
        let quiet = state.freight.wound_down.is_some() || route.on_call;
        if state.wagon.is_none() && state.stayed.is_none() && !quiet
            && (state.trips > 0 || state.last_departure.is_some()) {
            return Err(format!("route {r}: home after {} trips with no word why it stays", state.trips));
        }
    }
    Ok(())
}

pub fn contracts(s: &Society) -> Result<(), String> {
    let (now, day) = (s.now(), s.content().day_ticks);
    let grace = (GRACE_DAYS * day as f64) as u64;
    let book = s.world.resource::<Book>();
    // Judging runs at each day's first look; a contract that came into force since (its last copy arrived) is judged
    // at the next: the book as that judging would leave it.
    if book.reviewed.is_some_and(|d| d + 1 < now / day) {
        return Err(format!("contracts last judged on day {:?}, now day {}", book.reviewed, now / day));
    }
    // (Judged again only when one in force is past its deadline unjudged: a copy of the book, judged as the next look
    // would.)
    let late = |c: &Contract| c.in_force() && c.training.is_none() && c.levy.is_none() && c.installments > 0
        && now > c.start + c.every * (c.installments - 1) as u64 + c.wait_ticks(grace, day) + JUDGED_GRACES * grace
        && c.judged < c.installments;
    let next = book.open.iter().any(|&id| late(&book.contracts[id])).then(|| {
        let mut next = book.clone();
        next.review(now, day, |_| false);
        next
    });
    for c in &book.contracts {
        let judged = next.as_ref().map_or(c.judged, |n| n.contracts[c.id].judged);
        // (Delivered may pass the total: a crate sent on by a forwarder and billed counts up to what's on the road,
        // whatever came another way, as `Crates.receive` does.)
        if c.delivered < -1e-9 || c.paid < -1e-9 || c.owing < -1e-9 || c.shipped < -1e-9 || c.refund < -1e-9 {
            return Err(format!("contract {}: {} delivered, paid {}, owing {}, shipped {}, refund {}", c.id,
                c.delivered, c.paid, c.owing, c.shipped, c.refund));
        }
        // (A training contract or a levy delivers a master or soldiers, not goods, and isn't judged on installments.)
        let goods = c.training.is_none() && c.levy.is_none();
        if !c.in_force() || !goods || c.installments == 0 {
            continue;
        }
        let last = c.start + c.every * (c.installments - 1) as u64 + c.wait_ticks(grace, day);
        if now > last + JUDGED_GRACES * grace && judged < c.installments {
            return Err(format!("contract {}: in force {} days past its last deadline, {judged} of {} installments \
                judged by the next look", c.id, (now - last) / day, c.installments));
        }
    }
    Ok(())
}

pub fn wages(s: &Society) -> Result<(), String> {
    for t in 0..s.town_count() {
        let p = s.town(t).people;
        if !(0.0..=1.0).contains(&p.paid_share) || p.purse < -1e-9 {
            return Err(format!("town {t}: paid share {}, purse {}", p.paid_share, p.purse));
        }
        if let Some(f) = s.farm(t) {
            if f.farm.wage_owed >= 1.0 || f.farm.wages_unpaid < -1e-9 || f.farm.wages_paid < -1e-9 {
                return Err(format!("town {t}: farm wages {} owed unsettled ({} paid, {} unpaid)", f.farm.wage_owed,
                    f.farm.wages_paid, f.farm.wages_unpaid));
            }
        }
    }
    Ok(())
}
