//! Households (John, 2026-11-03, "How to Add Society Systems to the Sim": "Group villagers into households: shared
//! home, pooled wallet. Households make the economic choices"; households "By shared home"). The first of the society's
//! records the later rules run on: property, firms, charters and mandates (`property.rs`, `firms.rs`, `charters.rs`).
//!
//! A town's hands live in households, a household to a home of [`HOME_BEDS`] beds: each hand of its roster in exactly
//! one, by its id ([`crate::roster::HandId`]). Each day (`rules/households.rs`) a town reckons its households: a hand
//! gone leaves its household (a household left empty is gone, and what it held passes to the rest, its kin); a hand
//! new to the roster takes a home of its own while the town has homes empty, or else, and always a child, joins the
//! household of fewest; and a grown hand of a household over a home's beds moves out to a home empty in the town.
//!
//! **Their purses are the households' purse split** (`People::purse`, which the rules pay wages into and comforts and
//! homes out of): a household's [`Household::purse`] is its share of it, and the households' shares always add up to
//! it. What the purse gained since they were last reckoned goes to them by their grown hands (who earn it), what it
//! lost comes out of them by what they hold ([`allot`]). Nothing reads a household's purse yet: the households'
//! choices come with firms (`docs/decisions.md`). A town without a roster (its villagers only a number) has none.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::roster::{HandId, Roster};

/// A household, by the id the society gave it when it formed: never another's.
pub type HouseholdId = u32;

/// The beds of a home (a home built adds two: `people::live`).
pub const HOME_BEDS: f64 = 2.0;

/// A household: its hands, by id, and its share of the households' purse.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Household {
    pub id: HouseholdId,
    pub members: Vec<HandId>,
    pub purse: Fixed,
    /// When it formed.
    pub since: u64,
    /// The household its first hand moved out of: its kin, who inherit what it holds.
    #[serde(default)]
    pub kin: Option<HouseholdId>,
}

/// A town's households, and the day they were last reckoned.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Homes {
    pub list: Vec<Household>,
    pub reckoned: Option<u64>,
    /// Households gone since the register last followed them, and their kin (`rules/property.rs` hands on what they
    /// held).
    #[serde(default)]
    pub gone: Vec<(HouseholdId, Option<HouseholdId>)>,
    /// What households paid out of the households' purse for themselves since they were last reckoned (a field:
    /// `rules/farms.rs`): theirs alone at the next share-out ([`shares`]).
    #[serde(default)]
    pub charges: Vec<(HouseholdId, Fixed)>,
    /// Fields bought since, for whom: (buyer, crop, plots), for the register (`rules/property.rs`).
    #[serde(default)]
    pub bought: Vec<(HouseholdId, crate::content::GoodId, u32)>,
}

/// Every town's households, and the next household's id.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Households {
    pub towns: BTreeMap<TownId, Homes>,
    pub next: HouseholdId,
}

/// The homes a town of `beds` has: never none.
pub fn homes(beds: f64) -> usize {
    ((beds / HOME_BEDS).floor() as usize).max(1)
}

/// Moves `amount` (at most what the others hold) into household `to`'s share from the others', by what each holds.
/// Returns what moved.
pub fn gather(list: &mut [Household], to: usize, amount: Fixed) -> Fixed {
    let others: Fixed = list.iter().enumerate().filter(|&(i, _)| i != to).map(|(_, h)| h.purse.at_least_zero()).sum();
    let amount = amount.min(others);
    if !amount.is_positive() {
        return Fixed::ZERO;
    }
    let mut taken = 0;
    for (i, h) in list.iter_mut().enumerate() {
        if i != to && h.purse.is_positive() {
            let share = (amount.millis() as i128 * h.purse.millis() as i128 / others.millis() as i128) as i64;
            h.purse -= Fixed::from_millis(share);
            taken += share;
        }
    }
    // (Fewer thousandths left than those holding any, each of which still holds one.)
    let mut left = amount.millis() - taken;
    for (i, h) in list.iter_mut().enumerate() {
        if left > 0 && i != to && h.purse.is_positive() {
            h.purse -= Fixed::from_millis(1);
            left -= 1;
        }
    }
    list[to].purse += amount;
    amount
}

/// Moves `amount` (at most what it holds) from household `from`'s share to `to`'s. Returns what moved.
pub fn pay(list: &mut [Household], from: usize, to: usize, amount: Fixed) -> Fixed {
    let amount = amount.min(list[from].purse.at_least_zero());
    if from != to && amount.is_positive() {
        list[from].purse -= amount;
        list[to].purse += amount;
    }
    amount
}

/// The hands of a household that earn the town's wages: those grown, but those a firm employs.
pub fn earners(roster: &Roster, h: &Household, employed: &BTreeSet<HandId>) -> usize {
    h.members.iter().filter(|&&id| !employed.contains(&id) && roster.hands.iter().any(|x| x.id == id && x.baby_until.is_none()))
        .count()
}

/// Each household's purse once `total` (the households' purse) is split among them: what it gained since their shares
/// were set goes to them by `weights` (their grown hands; by heads where none has any), what it lost comes out of them
/// by what each holds. The shares add up to `total` to the thousandth, and none is below nothing while `total` isn't.
pub fn allot(list: &[Household], weights: &[usize], total: Fixed) -> Vec<Fixed> {
    let mut purses: Vec<Fixed> = list.iter().map(|h| h.purse).collect();
    if purses.is_empty() {
        return purses;
    }
    let held: Fixed = purses.iter().sum();
    let delta = (total - held).millis();
    if delta > 0 {
        let mut w: Vec<i64> = weights.iter().map(|&n| n as i64).collect();
        if w.iter().all(|&n| n == 0) {
            w = list.iter().map(|h| h.members.len().max(1) as i64).collect();
        }
        let sum: i64 = w.iter().sum();
        let mut given = 0;
        for (p, n) in purses.iter_mut().zip(&w) {
            let share = (delta as i128 * *n as i128 / sum as i128) as i64;
            *p += Fixed::from_millis(share);
            given += share;
        }
        // The thousandths left over (fewer than those that earn), one each, to those that earn in their order.
        let mut left = delta - given;
        for (p, n) in purses.iter_mut().zip(&w) {
            if left > 0 && *n > 0 {
                *p += Fixed::from_millis(1);
                left -= 1;
            }
        }
    } else if delta < 0 {
        let take = -delta;
        if held.millis() <= take {
            // All they held, and what more it owes on the first.
            for p in purses.iter_mut() {
                *p = Fixed::ZERO;
            }
            purses[0] = total;
            return purses;
        }
        let mut taken = 0;
        for p in purses.iter_mut() {
            let share = (take as i128 * p.millis() as i128 / held.millis() as i128) as i64;
            *p -= Fixed::from_millis(share);
            taken += share;
        }
        // (Fewer thousandths left than those holding any, each of which still holds one.)
        let mut left = take - taken;
        for p in purses.iter_mut() {
            if left > 0 && p.is_positive() {
                *p -= Fixed::from_millis(1);
                left -= 1;
            }
        }
    }
    purses
}

/// Each household's purse once `total` is split among them ([`allot`]), what each paid for itself since (`charges`)
/// out of its own share first, as far as it holds.
pub fn shares(list: &[Household], charges: &[(HouseholdId, Fixed)], weights: &[usize], total: Fixed) -> Vec<Fixed> {
    let mut list = list.to_vec();
    for (id, c) in charges {
        if let Some(h) = list.iter_mut().find(|h| h.id == *id) {
            h.purse -= (*c).min(h.purse.at_least_zero());
        }
    }
    allot(&list, weights, total)
}

impl crate::society::Society {
    /// A town's households, each with its share of the households' purse as of now.
    pub fn households(&self, town: TownId) -> Vec<Household> {
        let Some(homes) = self.world.resource::<Households>().towns.get(&town) else { return Vec::new() };
        let roster = self.roster(town);
        let employed = self.world.resource::<crate::firms::Firms>().employed(town);
        let weights: Vec<usize> = homes.list.iter().map(|h| earners(&roster, h, &employed)).collect();
        let purses = shares(&homes.list, &homes.charges, &weights, self.town(town).people.purse);
        homes.list.iter().zip(purses).map(|(h, purse)| Household { purse, ..h.clone() }).collect()
    }
}
