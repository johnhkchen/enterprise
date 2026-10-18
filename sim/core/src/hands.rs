//! Hands as a host names them (`docs/binding-plan.md` 1.3): every hand on a roster has an id the core gave it
//! ([`HandId`], `roster.rs`), and the host's own name for it beside it (a villager's UUID: [`Hands::tags`]), so a host
//! keeps no society ids of its own and finds its villagers' hands again after a load (`Query::Hands`).
//!
//! The mod keeps the same in its census roster (`settlement/Settlements.java` `Resident`, by the villager's UUID); a
//! hand joins when a villager comes to live at a town ([`Society::join`]) and leaves when one dies or is taken from the
//! world ([`Society::lose_hand`]); the rules move hands about by their ids (a caravan's crew, a soldier, a party of
//! settlers) and the host's tag goes with them.

use crate::lockers::Lockers;
use crate::messages::TownId;
use crate::roster::{Hand, HandId, Hands, Roster, MAX_LEVEL};
use crate::society::Society;
use crate::wire::{HandView, Input};

impl Society {
    /// A villager came to live at a town: a hand of `trade` (by name, with or without the host's namespace) at `level`
    /// (1 to 5, with the XP it took), known to the host as `tag` (none if empty). A Merchant becomes the desk's if the
    /// town has none; a town that kept no roster starts one. Returns the hand's id.
    pub fn join(&mut self, town: TownId, trade: &str, level: u8, tag: &str) -> Result<HandId, String> {
        self.can_join(town, trade, level, tag)?;
        let i = self.add_hand(town, trade, level);
        let id = self.roster(town).hands[i].id;
        if !tag.is_empty() {
            self.world.resource_mut::<Hands>().tags.insert(id, tag.to_string());
        }
        Ok(id)
    }

    /// Why a hand couldn't join a town as [`Society::join`] would add it, if it couldn't.
    pub fn can_join(&self, town: TownId, trade: &str, level: u8, tag: &str) -> Result<(), String> {
        if town >= self.town_count() {
            return Err(format!("no town {town}"));
        }
        self.can_be_hand(trade, level, tag)
    }

    /// Why a hand of `trade` at `level` called `tag` couldn't join any town, if it couldn't: a trade the core hasn't, a
    /// level outside 1 to 5, a tag another hand has.
    pub fn can_be_hand(&self, trade: &str, level: u8, tag: &str) -> Result<(), String> {
        if self.content().trade(trade).is_none() {
            return Err(format!("no trade called {trade}"));
        }
        if !(1..=MAX_LEVEL).contains(&level) {
            return Err(format!("a hand of level {level} (1 to {MAX_LEVEL})"));
        }
        if !tag.is_empty() {
            if let Some(other) = self.hand_tagged(tag) {
                return Err(format!("hand {other} is called {tag} already"));
            }
        }
        Ok(())
    }

    /// The hand the host calls `tag`, if a hand still in the society is called that (on a roster, or with a party of
    /// settlers on the road: a hand the rules took from the world, a soldier fallen, keeps its tag but is no more).
    pub fn hand_tagged(&self, tag: &str) -> Option<HandId> {
        let hands = self.world.resource::<Hands>();
        let parties = &self.world.resource::<crate::settlers::Settlers>().parties;
        let on_the_road = |id: HandId| parties.iter().any(|p| p.hands.iter().any(|h| h.id == id));
        hands.tags.iter().filter(|(_, t)| t.as_str() == tag).map(|(id, _)| *id)
            .find(|&id| self.find_hand(id).is_some() || on_the_road(id))
    }

    /// Where the hand with this id is: its town, and its place in the town's roster (good until the roster changes).
    pub fn find_hand(&self, id: HandId) -> Option<(TownId, usize)> {
        (0..self.town_count()).find_map(|t| {
            let roster = self.world.get::<Roster>(self.towns[t])?;
            roster.place(id).map(|i| (t, i))
        })
    }

    /// The host's name for a hand, if it gave one.
    pub fn hand_tag(&self, id: HandId) -> Option<&str> {
        self.world.resource::<Hands>().tag(id)
    }

    /// Why the hand with this id couldn't be lost, if it couldn't: it's on no roster, or it's away (out with a caravan,
    /// at war: off the host's world, the core's to bring home or bury).
    pub fn can_lose_hand(&self, id: HandId) -> Result<(TownId, usize), String> {
        let (town, i) = self.find_hand(id).ok_or_else(|| format!("no hand {id} on a roster"))?;
        if self.world.get::<Roster>(self.towns[town]).is_some_and(|r| r.hands[i].away) {
            return Err(format!("hand {id} is away from town {town}"));
        }
        Ok((town, i))
    }

    /// A hand at home left the society for good (its villager died, or was taken from the world): off its town's
    /// roster with its locker (`settlers::take_hand`), its tag forgotten. Returns its town.
    pub fn lose_hand(&mut self, id: HandId) -> Result<TownId, String> {
        let (town, i) = self.can_lose_hand(id)?;
        let e = self.towns[town];
        if let Some(mut lockers) = self.world.get_mut::<Lockers>(e) {
            lockers.take_hand(i);
        }
        let mut roster = self.world.get::<Roster>(e).expect("the hand's roster").clone();
        crate::settlers::take_hand(&mut roster, i);
        self.world.get_mut::<crate::state::People>(e).expect("a town").villagers = roster.residents() as f64;
        self.world.entity_mut(e).insert(roster);
        self.world.resource_mut::<Hands>().tags.remove(&id);
        Ok(town)
    }

    /// A town's hands as a host sees them (`Query::Hands`), in its roster's order, children grown by now.
    pub fn hand_views(&self, town: TownId) -> Vec<HandView> {
        let roster = self.roster(town);
        let content = self.content();
        let hands = self.world.resource::<Hands>();
        roster.hands.iter().enumerate().map(|(i, h): (usize, &Hand)| HandView {
            hand: h.id,
            tag: hands.tag(h.id).unwrap_or_default().to_string(),
            trade: content.trades.get(h.trade).map(|t| t.name.clone()).unwrap_or_default(),
            level: h.level,
            xp: h.xp,
            baby: h.is_baby(),
            away: h.away,
            merchant: roster.merchant == Some(i),
        }).collect()
    }

    /// Applies a hand's happening (`Input::AddHand`, `HandLost`); false for any other. Panics on one the wire would
    /// have refused: it checks them first (`wire::handle`).
    pub(crate) fn apply_hands(&mut self, input: &Input) -> bool {
        match input {
            Input::AddHand { town, trade, level, tag } => {
                self.join(*town as TownId, trade, *level, tag).expect("a hand the wire checked");
            }
            Input::HandLost { hand } => {
                self.lose_hand(*hand).expect("a hand on a roster");
            }
            _ => return false,
        }
        true
    }
}
