//! The army's step (`army.rs`; `army/Army.java` `campaign`, as `Sovereign.onServerTick` calls it each day of the war;
//! `CaravanLedger.walkersHome`), first in **Pay**:
//!
//! 1. Soldiers and recruits walking home who have got there are at home again in their towns' rosters.
//! 2. With a front set, the campaign's days since the last, a day at a time, on the front's warehouse
//!    ([`Army::day`]): the army eats, its hungry desert, the rest draw and wear their kit, serve and run their risk.
//!    The fallen come off their towns' rosters for good and their towns grieve (`Territory.mourn`); deserters walk
//!    home from the front, their kit lost. The first look with a front arms the campaign's clock, as the mod's first
//!    call does.

use bevy_ecs::prelude::*;

use super::Step;
use crate::army::*;
use crate::content::Content;
use crate::contracts::Book;
use crate::kit::Kits;
use crate::lockers::Lockers;
use crate::roster::Roster;
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    // (first in the step, before the towns' guards are fed and paid: ordered against the step's chain)
    schedule.add_systems(campaign.in_set(Step::Pay).before(super::territory::upkeep));
}

/// The army's day: walkers home, then the campaign's days since the last on the front's warehouse.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn campaign(clock: Res<Clock>, content: Res<Content>, all: Res<Towns>, mut army: ResMut<Army>,
        mut ledger: ResMut<Ledger>, mut book: ResMut<Book>, places: Query<&Town>,
        mut fronts: Query<(&mut Stock, &mut Kits)>,
        mut homes: Query<(&mut People, Option<&mut Roster>, Option<&mut Lockers>)>) {
    let now = clock.now;
    let army = &mut *army;
    // Home again: at home in its town's roster (`CaravanLedger.walkersHome`).
    if army.walking.iter().any(|w| w.arrives <= now) {
        let (home, away): (Vec<Walker>, Vec<Walker>) =
            std::mem::take(&mut army.walking).into_iter().partition(|w| w.arrives <= now);
        army.walking = away;
        for w in home {
            let (Some(hand), Some(&e)) = (&w.hand, all.0.get(w.home)) else { continue };
            if let Ok((_, Some(mut roster), _)) = homes.get_mut(e) {
                crate::squad::release(&mut roster, std::slice::from_ref(hand));
            }
        }
    }
    let Some(front) = army.front.and_then(|f| all.0.get(f).copied()) else { return };
    let day = content.day_ticks.max(1);
    let Some(last) = army.last_day else {
        army.last_day = Some(now);
        return;
    };
    if now < last + day {
        return;
    }
    let days = (now - last) / day;
    army.last_day = Some(last + days * day);
    let hex = |e: Entity| places.get(e).ok().and_then(|t| t.hex);
    let front_hex = hex(front);
    for _ in 0..days {
        let Ok((mut stock, mut kits)) = fronts.get_mut(front) else { return };
        let losses = army.day(&content, &mut ledger, &mut book, &mut stock, &mut kits);
        // Fallen: gone from its town for good, which grieves.
        for soldier in losses.fallen {
            let Some(&e) = all.0.get(soldier.home) else { continue };
            let Ok((mut people, roster, lockers)) = homes.get_mut(e) else { continue };
            if let (Some(hand), Some(mut roster)) = (&soldier.hand, roster) {
                bury(&mut roster, lockers.map(|l| l.into_inner()), hand);
                people.villagers = roster.residents() as f64;
            }
            mourn(&content, &mut people, now);
        }
        // Deserted: it walks home from the front, as long as the road takes.
        for soldier in losses.deserted {
            let home_hex = all.0.get(soldier.home).and_then(|&e| hex(e));
            let ticks = travel_ticks(blocks_between(home_hex, front_hex));
            army.walking.push(Walker { home: soldier.home, hand: soldier.hand, arrives: now + ticks as u64 });
        }
    }
}
