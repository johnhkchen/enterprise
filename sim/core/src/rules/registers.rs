//! The registers' rules (`property.rs`, `charters.rs`): once a day they follow what the society holds and who runs it.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::charters::{Charter, Charters, Choosing, Grant, Mandate, Party, Source, Verb};
use crate::content::Content;
use crate::households::Households;
use crate::property::{Acquired, Claim, Register, Target};
use crate::roster::Roster;
use crate::rules::Step;
use crate::state::*;
use crate::territory::Territory;

pub fn register(schedule: &mut Schedule) {
    // (after the households' reckoning and before the jobs' reviews, and so the rest of the reviews)
    schedule.add_systems(register_claims.in_set(Step::Review).after(super::households::reckon)
        .before(super::jobs::relocate).before(super::jobs::review));
}

/// Once a day: each town's hexes, halls and households' homes claimed, and claims on what's lost struck out; each town
/// chartered by the crown; each town's Merchant its agent for its trade.
#[allow(clippy::too_many_arguments)]
pub fn register_claims(clock: Res<Clock>, content: Res<Content>, towns: Res<Towns>, territory: Res<Territory>,
        households: Res<Households>, mut register: ResMut<Register>, mut charters: ResMut<Charters>,
        q: Query<(&Town, Option<&crate::works::Works>, Option<&Roster>),
            (Without<crate::realms::Realm>, Without<crate::realms::OffMap>)>) {
    let (now, day) = (clock.now, content.day_ticks.max(1));
    if matches!(register.reckoned, Some(k) if now < k + day) {
        return;
    }
    register.reckoned = Some(now);
    let mut held: BTreeMap<Target, (Party, Acquired)> = BTreeMap::new();
    for &e in &towns.0 {
        let Ok((town, works, roster)) = q.get(e) else { continue };
        let t = town.id;
        let party = Party::Town(t);
        let founding = !register.claims.values().any(|c| c.owner == party);
        for &h in territory.holding(t).map_or(&[][..], |h| &h.hexes[..]) {
            let how = match register.claims.get(&Target::Hex(h)) {
                Some(c) if c.owner != party => Acquired::Taken,
                _ if founding => Acquired::Founding,
                _ => Acquired::Grown,
            };
            held.insert(Target::Hex(h), (party, how));
        }
        for k in works.map(|w| w.built_of()).unwrap_or_default() {
            held.insert(Target::Hall(t, k), (party, Acquired::Built));
        }
        for h in households.towns.get(&t).map_or(&[][..], |h| &h.list[..]) {
            held.insert(Target::Home(h.id), (Party::Household(h.id), Acquired::Homesteaded));
        }
        if charters.held_by(party).is_none() {
            let id = charters.next_charter;
            charters.next_charter += 1;
            charters.charters.push(Charter { id, grantor: Party::Crown, holder: party,
                grants: vec![Grant::Market, Grant::SelfGovernment], choosing: Choosing::Custom,
                revocable_by: vec![Party::Crown], since: now, until: None });
        }
        let merchant = roster.and_then(|r| r.merchant.and_then(|i| r.hands.get(i))).map(|h| h.id).filter(|&id| id != 0);
        let charters = &mut *charters;
        for m in charters.mandates.iter_mut().filter(|m| m.principal == party && m.until.is_none()
                && m.scope.contains(&Verb::Trade) && Some(m.agent) != merchant) {
            m.until = Some(now);
        }
        if let Some(agent) = merchant.filter(|&a| !charters.of(party).any(|m| m.agent == a)) {
            let id = charters.next_mandate;
            charters.next_mandate += 1;
            charters.mandates.push(Mandate { id, principal: party, agent, scope: vec![Verb::Trade],
                source: Source::Appointment, since: now, term: None, until: None });
        }
    }
    let register = &mut *register;
    // (A field's claim is property's: `rules/property.rs`.)
    register.claims.retain(|target, c| matches!(target, Target::Field(_))
        || held.get(target).is_some_and(|(owner, _)| *owner == c.owner));
    for (target, (owner, how)) in held {
        if !register.claims.contains_key(&target) {
            register.claims.insert(target, Claim { id: register.next, owner, how, since: now });
            register.next += 1;
        }
    }
}
