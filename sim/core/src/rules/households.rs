//! Households' rules (`households.rs`): each day a town reckons its households, its hands' homes and their purses.

use bevy_ecs::prelude::*;

use crate::content::Content;
use crate::households::{allot, earners, homes, shares, Household, Households};
use crate::roster::Roster;
use crate::rules::Step;
use crate::state::*;

/// Is the hand of this id grown?
fn grown(roster: &Roster, id: crate::roster::HandId) -> bool {
    roster.hands.iter().any(|x| x.id == id && x.baby_until.is_none())
}

pub fn register(schedule: &mut Schedule) {
    // (after spoilage and before the jobs' reviews, and so the rest of the reviews: pinned as it was first run)
    schedule.add_systems(reckon.in_set(Step::Review).after(super::storage::spoil).after(super::farms::review)
        .before(super::jobs::relocate)
        .before(super::jobs::review));
}

/// Each town with a roster, once a day: its households' shares of their purse as they stand, then its hands gone
/// struck out of theirs (a household left empty gone, what it held passing to the rest), then its hands new to the
/// roster housed: a home of their own while the town has one empty, else (and always a child) the household of fewest.
pub fn reckon(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, mut households: ResMut<Households>,
        q: Query<(&Town, &People, &Roster), (Without<crate::realms::Realm>, Without<crate::realms::OffMap>)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    let households = &mut *households;
    for &e in &towns.0 {
        let Ok((town, people, roster)) = q.get(e) else { continue };
        let homes_of = households.towns.entry(town.id).or_default();
        if matches!(homes_of.reckoned, Some(k) if now < k + day) {
            continue;
        }
        homes_of.reckoned = Some(now);
        let list = &mut homes_of.list;
        // Their shares as they stand, before any leaves: what the purse gained, its earners earned.
        let weights: Vec<usize> = list.iter().map(|h| earners(roster, h)).collect();
        let purses = shares(list, &homes_of.charges, &weights, people.purse);
        homes_of.charges.clear();
        for (h, p) in list.iter_mut().zip(purses) {
            h.purse = p;
        }
        for h in list.iter_mut() {
            h.members.retain(|id| roster.hands.iter().any(|x| x.id == *id));
        }
        // (What an empty household held is the rest's at the share-out below: the purse is the same, they hold less.
        // What else it held is its kin's, `rules/property.rs`.)
        for h in list.iter().filter(|h| h.members.is_empty()) {
            homes_of.gone.push((h.id, h.kin));
        }
        list.retain(|h| !h.members.is_empty());
        let housed: std::collections::BTreeSet<_> = list.iter().flat_map(|h| h.members.iter().copied()).collect();
        let room = homes(people.beds);
        for hand in roster.hands.iter().filter(|x| x.id != 0 && !housed.contains(&x.id)) {
            let fewest = (0..list.len()).min_by_key(|&i| list[i].members.len());
            match fewest {
                Some(i) if hand.baby_until.is_some() || list.len() >= room => list[i].members.push(hand.id),
                _ => {
                    list.push(Household { id: households.next, members: vec![hand.id], since: now, ..Default::default() });
                    households.next += 1;
                }
            }
        }
        // A grown hand of a household over a home's beds moves out to a home empty in the town (a child grown, or one
        // that joined when there was none), the crowdedest first; what it earns is its new household's from then on.
        let beds = crate::households::HOME_BEDS as usize;
        while list.len() < room {
            let Some((i, at)) = (0..list.len()).filter(|&i| list[i].members.len() > beds)
                .filter_map(|i| list[i].members.iter().skip(1).rposition(|id| grown(roster, *id)).map(|at| (i, at + 1)))
                .max_by_key(|&(i, _)| (list[i].members.len(), std::cmp::Reverse(i))) else { break };
            let id = list[i].members.remove(at);
            let kin = Some(list[i].id);
            list.push(Household { id: households.next, members: vec![id], purse: Default::default(), since: now, kin });
            households.next += 1;
        }
        // And the purse shared again: what the gone held to their kin, a town's first households' from the start.
        let weights: Vec<usize> = list.iter().map(|h| earners(roster, h)).collect();
        let purses = allot(list, &weights, people.purse);
        for (h, p) in list.iter_mut().zip(purses) {
            h.purse = p;
        }
    }
}
