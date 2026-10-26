//! Review: a town's idle hands find work (`territory/Jobs.java` `employ`; see `jobs.rs`). Once a day, towns in their
//! order, after the Merchant's review of its lockers (which counts the days a trade has done nothing) and the
//! settlers' (trained adults go with caravans first): one unemployed adult, or a slack one, takes a trade drawn from
//! the town's bag of marbles when the town can pay for its stall. Not while the town is raising a party of settlers
//! (`Territory.grow`: its idle hands wait for that), nor on the day one left (the mod's `grow` hires before `consider`
//! sends, so the party's day hires nobody). An idle adult it leaves without work is noted ([`Hiring::left_idle`]): the
//! squad trains only such a hand, after this (`rules/squad.rs` `train`; the mod's `Territory.grow` trained the squad
//! before anything, and a town's first day turned the villager who'd have farmed into a second Merchant).

use bevy_ecs::prelude::*;

use super::Step;
use crate::bills::Bills;
use crate::cash_book::CashBook;
use crate::content::Content;
use crate::farms::{Deposits, Farm};
use crate::jobs::{employ, Hiring, Place, Signals};
use crate::lockers::{Books, Lockers};
use crate::settlers::{raising, Settlers};
use crate::state::*;

pub fn register(schedule: &mut Schedule) {
    // After the census and the farm's review, as the mod's `grow` hires after `afterCensus` counts the town.
    schedule.add_systems(review.in_set(Step::Review).after(super::lockers::review).after(super::settlers::review)
        .after(super::farms::review).after(super::territory::review));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn review(clock: Res<Clock>, content: Res<Content>, bills: Res<Bills>, signals: Res<Signals>, order: Res<Towns>,
        settlers: Res<Settlers>, mut hiring: ResMut<Hiring>, mut ledger: ResMut<Ledger>, mut book: ResMut<CashBook>, seed: Res<Rng>,
        mut dice: ResMut<Dice>, mut recipes: Local<Option<Vec<crate::services::Recipe>>>,
        routes: Query<&Route>,
        mut towns: Query<(&Town, &mut Money, &mut People, &Stock, &Trades, &Farm, &mut Deposits, &mut Budget,
            Option<&mut Roster>, &mut Lockers)>) {
    let (now, today) = (clock.now, clock.now / content.day_ticks.max(1));
    // What each town's villagers buy a day at its managed lockers: its neighbours make it for them.
    let demand: std::collections::BTreeMap<usize, std::collections::BTreeMap<crate::content::GoodId, f64>> = towns.iter()
        .filter_map(|(town, .., roster, lockers)| Some((town.id, crate::lockers::census_demand(roster?, lockers))))
        .collect();
    if content.is_changed() {
        *recipes = None;
    }
    let recipes = recipes.get_or_insert_with(|| crate::services::recipes(&content));
    for &e in order.0.iter() {
        let Ok((town, mut money, mut people, stock, trades, farm, mut deposits, mut budget, roster, mut lockers)) =
            towns.get_mut(e) else { continue };
        let Some(mut roster) = roster else { continue };
        if hiring.reviewed.get(&town.id) == Some(&today) {
            continue;
        }
        hiring.reviewed.insert(town.id, today);
        let sent_today = settlers.last_sent.get(&town.id).is_some_and(|&t| t / content.day_ticks.max(1) == today);
        if sent_today || raising(&content, &settlers, town.id, &*people, stock, now) {
            continue;
        }
        let roads = routes.iter().any(|r| r.from_id == town.id || r.to_id == town.id);
        let mut far: Vec<usize> = routes.iter()
            .filter_map(|r| if r.from_id == town.id { Some(r.to_id) } else if r.to_id == town.id { Some(r.from_id) } else { None })
            .collect();
        far.sort_unstable();
        far.dedup();
        let far = crate::jobs::far_marbles(&content, recipes, &roster, far.iter().filter_map(|t| demand.get(t)));
        let people = &mut *people;
        let mut place = Place { town: town.id, money: &mut money.0, purse: &mut people.purse,
            deposits: &mut deposits, budget: &mut budget, stock, trades, farm, met: &people.comfort_met,
            roster: &mut roster, lockers: &mut lockers, roads, far };
        let mut books = Books { ledger: &mut ledger, book: &mut book, rng: dice.of(town.id, &seed) };
        let idle = place.roster.hands.iter().any(|h| !h.is_baby() && !h.away && content.trades.get(h.trade).is_some_and(|t| t.none));
        if employ(&content, &mut books, &bills, &signals, now, &mut place).is_none() && idle {
            hiring.left_idle.insert(town.id, today);
        }
    }
}
