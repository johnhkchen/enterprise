//! A host's lockers (`docs/binding-plan.md` 2, concern 6; task `locker-adapter`): where its hands' lockers stand, and
//! the Merchant's screen. `lockers.rs` runs a town's lockers as numbers, a hand's trades against the town's warehouse;
//! a host whose lockers are things in its world (the mod's: a chest touching a villager's workstation,
//! `docs/decisions.md` "Lockers") says where each stands, and the core follows it:
//!
//! - **Only a hand whose locker stands somewhere has one** ([`crate::lockers::Lockers::placed`], [`Lockers::has`]): the
//!   Merchant manages, values and trades only those. A town founded through the wire (`AddTown`) is its host's in this
//!   ([`Society::host_places_lockers`]); a town the core keeps as numbers (the study's) has every hand's locker, as
//!   before. `Input::Locker` ties a hand's locker to a `Locker` point of its town's, or unties it; a point broken unties
//!   it ([`Society::untie_lockers_at`]); a hand gone from its roster takes its tie with it (`Lockers::sync`).
//! - **A locker's chests are a shelf, not a store.** They're the town's, counted at each look like a desk's, and the
//!   core writes them (`chests.rs`): what the town uses is taken from them first, and what they still hold the Merchant
//!   carries to its desk and depots (`LockerTrading.collect`). The core's trades run against the warehouse
//!   (`lockers.rs`), so nothing is filled into a locker (`LockerTrading.supply` has no counterpart).
//! - **The screen switches the core's offers** (`Input::Switch`, `ToggleTradePayload`) and shows them
//!   (`Query::Lockers`, [`Society::locker_views`]): the hand's offers at its level, with the Merchant's price.
//!
//! The restocks a town missed while its chunks weren't loaded are the core's own (`lockers::catch_up`, every look,
//! loaded or not), so the mod's `Rounds` (forced chunk tickets) and `LockerCatchUp` (replays on load) have no
//! counterpart here: nothing needs loading for a town's lockers to trade.

use std::collections::BTreeMap;

use crate::lockers::{merchant_reputation, Lockers, EMERALD_STACK};
use crate::messages::TownId;
use crate::points::{Kind, Spot};
use crate::roster::{HandId, Roster};
use crate::society::Society;
use crate::wire::{Input, LockerView, OfferView, Point};

impl Society {
    /// The town's lockers are where its host places them from now on (if they weren't already): only a hand whose
    /// locker stands somewhere ([`Society::tie_locker`]) has one the Merchant can manage.
    pub fn host_places_lockers(&mut self, town: TownId) {
        let e = self.towns[town];
        let mut lockers = self.world.get::<Lockers>(e).cloned().unwrap_or_default();
        if lockers.placed.is_none() {
            lockers.placed = Some(BTreeMap::new());
            self.put_town_lockers(town, lockers);
        }
    }

    /// The hand's locker stands at `at` (a `Locker` point of its town's) or nowhere (`None`); the town's lockers are its
    /// host's from now on. A hand whose locker stood at `at` before has none now. False, and nothing changed, for a
    /// hand on no roster or a point that isn't a locker of the hand's town's.
    pub fn tie_locker(&mut self, hand: HandId, at: Option<Spot>) -> bool {
        let Some((town, _)) = self.find_hand(hand) else { return false };
        if let Some(spot) = &at {
            if !self.point(spot).is_some_and(|p| p.kind == Kind::Locker && p.town == town) {
                return false;
            }
        }
        let e = self.towns[town];
        let mut lockers = self.world.get::<Lockers>(e).cloned().unwrap_or_default();
        let placed = lockers.placed.get_or_insert_with(BTreeMap::new);
        match at {
            Some(spot) => {
                placed.retain(|&h, s| h == hand || *s != spot);
                placed.insert(hand, spot);
            }
            None => {
                placed.remove(&hand);
            }
        }
        self.put_town_lockers(town, lockers);
        true
    }

    /// The `Locker` point at `at` of `town`'s is gone: a hand whose locker stood there has none.
    pub fn untie_lockers_at(&mut self, town: TownId, at: &Spot) {
        let Some(&e) = self.towns.get(town) else { return };
        let Some(mut lockers) = self.world.get::<Lockers>(e).cloned() else { return };
        if let Some(placed) = lockers.placed.as_mut() {
            let before = placed.len();
            placed.retain(|_, s| s != at);
            if placed.len() != before {
                self.put_town_lockers(town, lockers);
            }
        }
    }

    /// One of a hand's offers (its place among the hand's offers, which is of `good`) switched on or off, as the
    /// Merchant's screen does. False, and nothing changed, for a hand on no roster, or an offer that isn't there or
    /// isn't of that good (the screen's view was stale).
    pub fn switch_offer(&mut self, hand: HandId, offer: usize, good: usize, on: bool) -> bool {
        let Some((town, i)) = self.find_hand(hand) else { return false };
        let Some(mut lockers) = self.lockers(town) else { return false };
        let Some(o) = lockers.hands.get_mut(i).and_then(|l| l.offers.get_mut(offer)).filter(|o| o.good == good) else {
            return false;
        };
        o.on = on;
        self.put_town_lockers(town, lockers);
        true
    }

    /// A town's lockers as the Merchant's screen shows them (`Query::Lockers`): how many its Merchant may manage,
    /// whether its host places them, and every hand with a trade a locker serves (grown, not the Merchant, not idle) in
    /// the roster's order, with its offers at its level.
    pub fn locker_views(&self, town: TownId) -> (u32, bool, Vec<LockerView>) {
        let e = self.towns[town];
        let placed = self.world.get::<Lockers>(e).is_some_and(|l| l.placed.is_some());
        let (Some(lockers), Some(_)) = (self.lockers(town), self.world.get::<Roster>(e)) else {
            return (0, placed, Vec::new());
        };
        let roster = self.roster(town);
        let content = self.content();
        let rep = merchant_reputation(roster.merchant_level().unwrap_or(1) as i32);
        let views = (0..roster.hands.len().min(lockers.hands.len()))
            .filter(|&i| crate::lockers::has_locker(content, &roster, i))
            .map(|i| {
                let h = &roster.hands[i];
                let l = &lockers.hands[i];
                let at = lockers.placed.as_ref().and_then(|p| p.get(&h.id)).map(|spot| Point::of(spot, Kind::Locker));
                let offers = l.offers.iter().enumerate().filter(|(_, o)| o.level <= h.level).map(|(k, o)| {
                    let stack = if o.sells { EMERALD_STACK } else { content.goods.get(o.good).map_or(64, |g| g.stack) };
                    OfferView {
                        offer: crate::wire::no(k),
                        good: crate::wire::no(o.good),
                        sells: o.sells,
                        count: o.count,
                        emeralds: o.emeralds,
                        price: o.price(rep, stack),
                        uses: o.uses,
                        used: o.used,
                        xp: o.xp,
                        level: o.level,
                        demand: o.demand,
                        inflated: o.inflated(),
                        on: o.on,
                        training: o.training,
                        procured: o.procured,
                    }
                }).collect();
                LockerView {
                    hand: h.id,
                    managed: lockers.managed.contains(&i),
                    client: h.id != 0 && lockers.clients.contains(&h.id),
                    at,
                    worth: l.worth,
                    offers,
                }
            })
            .collect();
        (roster.locker_limit(), placed, views)
    }

    /// A town's lockers put back, following its roster.
    fn put_town_lockers(&mut self, town: TownId, mut lockers: Lockers) {
        let e = self.towns[town];
        if let Some(roster) = self.world.get::<Roster>(e).cloned() {
            lockers.sync(self.content(), &roster);
        }
        self.world.entity_mut(e).insert(lockers);
    }

    /// Applies a locker's happening (`Input::Switch`, `Input::Locker`); false for any other. A hand or an offer gone by
    /// the time it applies is a stale view, ignored.
    pub(crate) fn apply_lockers(&mut self, input: &Input) -> bool {
        match input {
            Input::Switch { hand, offer, good, on } => {
                let _stale = self.switch_offer(*hand, *offer as usize, *good as usize, *on);
            }
            Input::Locker { hand, at } => {
                let _stale = self.tie_locker(*hand, at.as_ref().map(|p| p.spot()));
            }
            _ => return false,
        }
        true
    }
}
