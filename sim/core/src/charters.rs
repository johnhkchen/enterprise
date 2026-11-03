//! Charters and mandates (John, 2026-11-03, "How to Add Society Systems to the Sim": "A charter is a set of rules
//! stored as data, with a grantor. A mandate is authority delegated from a principal to an agent. Together they replace
//! hardcoded government types"). Records only, for now: nothing reads them.
//!
//! Every town holds a charter of the crown (`crown.rs`, the regional authority off the map): to hold a market and to
//! govern itself, its leaders chosen by custom (the top of its notables acclaimed: none yet), revocable by the crown.
//! Its treasury is the town's commons (John: "Town commons"), and its Merchant runs its trade for it by mandate: an
//! appointment, the town the principal, scope [`Verb::Trade`]. A new Merchant ends the old mandate and takes a new one
//! (`rules/registers.rs`); ended mandates are kept, with when they ended, as the town's history of its offices.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::firms::FirmId;
use crate::households::HouseholdId;
use crate::messages::TownId;
use crate::roster::HandId;

pub type CharterId = u32;
pub type MandateId = u32;

/// Who can own, grant, hold or delegate: the crown, a town (its commons), a household, a firm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Party {
    Crown,
    Town(TownId),
    Household(HouseholdId),
    Firm(FirmId),
}

/// What a charter grants its holder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Grant {
    /// To hold a market: a shop and its lockers.
    Market,
    /// To govern itself: to choose its leaders as its charter says.
    SelfGovernment,
    /// To levy a toll of this share of what passes.
    Toll(f64),
    /// The only maker of a recipe within this many hexes.
    Monopoly { recipe: String, radius: i32 },
}

/// How a charter's holder chooses its leaders, and so how wealth and standing become votes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Choosing {
    /// By custom: the notable of highest standing, acclaimed.
    #[default]
    Custom,
    /// A vote a household.
    Households,
    /// Votes weighted by the land each holds.
    Land,
    /// The elders only.
    Elders,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Charter {
    pub id: CharterId,
    pub grantor: Party,
    pub holder: Party,
    pub grants: Vec<Grant>,
    pub choosing: Choosing,
    pub revocable_by: Vec<Party>,
    pub since: u64,
    pub until: Option<u64>,
}

/// What an agent may do for its principal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verb {
    Trade,
    Hire,
    Build,
    Tax,
    Arbitrate,
    Command,
}

/// How an agent came by its mandate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Appointment,
    Election,
    Inheritance,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mandate {
    pub id: MandateId,
    pub principal: Party,
    pub agent: HandId,
    pub scope: Vec<Verb>,
    pub source: Source,
    pub since: u64,
    /// Its term, in ticks (none: until recalled), and when it ended.
    pub term: Option<u64>,
    pub until: Option<u64>,
}

/// Every charter and mandate, and the day they were last registered.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Charters {
    pub charters: Vec<Charter>,
    pub mandates: Vec<Mandate>,
    pub next_charter: CharterId,
    pub next_mandate: MandateId,
    pub reckoned: Option<u64>,
}

impl Charters {
    /// The charter a party holds now, if any.
    pub fn held_by(&self, holder: Party) -> Option<&Charter> {
        self.charters.iter().find(|c| c.holder == holder && c.until.is_none())
    }

    /// The mandates in force a principal gave.
    pub fn of(&self, principal: Party) -> impl Iterator<Item = &Mandate> {
        self.mandates.iter().filter(move |m| m.principal == principal && m.until.is_none())
    }
}

impl crate::society::Society {
    pub fn charters(&self) -> Charters {
        self.world.resource::<Charters>().clone()
    }
}
