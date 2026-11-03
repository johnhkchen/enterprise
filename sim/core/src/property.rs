//! Property (John, 2026-11-03, "How to Add Society Systems to the Sim": "Add a claim record: owner (household or
//! firm), target (plot bounds, workstation position or building), how it was acquired, and since when"; "Keep a land
//! register per settlement"; then "Property turns one profession into three social positions. A farmer can own land,
//! rent it, or work it for wages").
//!
//! Each day (`rules/registers.rs`) the register follows what the society holds: each hex a town holds (`territory.rs`)
//! is its claim, made at its founding, grown into, or taken from another town; each hall built (`works.rs`) is the
//! town's, built; each household's home is the household's. One claim a thing: a claim whose thing is lost is struck
//! out, and a thing that changes hands gets a new claim.
//!
//! **Fields** are property (`rules/property.rs`, daily). A town's farm is its estate, the commons' (`farms.rs`), its
//! Farmers on wages for a share of the harvest; a field a household holds is a freehold, whose Farmers keep at least
//! [`crate::farms::FREEHOLD_SHARE`] of what it yields. Each freehold is a [`Field`], one household's claim:
//! - **Bought:** the households' purse buys a field of the town (`rules/farms.rs`), for the Farmer household with room
//!   for it that has most, which pays its price out of its own share.
//! - **Homesteaded:** estate land left untended, a field's worth of plots for `homestead_days`, is taken by a Farmer
//!   household with room for it; any other that could have is left with a dispute ([`Contest::Homestead`]).
//! - **Worked, let or idle:** a household works as many of its fields as it has grown Farmers (a household holds no more
//!   fields than `fields_per_farmer` a Farmer of it, as it takes them: claims capped by its labour); the rest it lets
//!   to a Farmer household with room ([`Lease`]: a tenant pays `lease_share` of what its field earns). A household with
//!   a Farmer and no field to work is a labourer's, on the estate's wages.
//! - **Inherited:** a household left empty passes its fields to the household its last hand came from, its kin; with no
//!   kin left, the town takes them back into its estate, and a tenant working one is left with a dispute
//!   ([`Contest::Inheritance`]).
//! - **Sold:** a field its household hasn't worked for `idle_sale_days` (let or lying idle) is sold to a Farmer household
//!   with room that can pay `land_price_days` of what it earns (its tenant first), out of its share into the seller's.
//!
//! What a field earns is its share of the Farmers' wages the town paid (the plots it holds of the plots tended): the
//! households' shares follow it, from all of them (as the purse's gains were shared by their earners) to the household
//! that works it, and of that its rent to its owner. Nothing passes through the ledger: the households' shares of
//! their one purse move, and still add up to it. Disputes are left open, a backlog for the leaders to come.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::charters::Party;
use crate::hex::Hex;
use crate::households::HouseholdId;
use crate::messages::TownId;
use crate::works::Kind;

pub type ClaimId = u32;

/// What a claim is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Target {
    /// A hex of land.
    Hex(Hex),
    /// A town's hall of a kind.
    Hall(TownId, Kind),
    /// A household's home.
    Home(HouseholdId),
    /// A field of a town's farm.
    Field(FieldId),
}

pub type FieldId = u32;

/// A field held as a freehold: its town, crop and plots, who works it, and since when its household hasn't.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub town: TownId,
    pub crop: crate::content::GoodId,
    pub plots: u32,
    pub worked_by: Option<HouseholdId>,
    pub idle_since: Option<u64>,
}

/// A field let to a tenant for a share of what it earns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    pub tenant: HouseholdId,
    pub share: f64,
    pub since: u64,
}

/// Why a claim is contested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Contest {
    /// Its holder's household died out with no kin, and another worked it.
    Inheritance,
    /// Untended land homesteaded by one when others could have.
    Homestead,
}

/// A contested claim, left open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dispute {
    pub id: u32,
    pub town: TownId,
    pub target: Target,
    pub claimants: Vec<Party>,
    pub why: Contest,
    pub since: u64,
}

/// A town's farm as the register last followed it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Estate {
    /// The Farmers' wages paid, all told, when last read (`Farm::wages_paid`).
    pub wages_seen: crate::fixed::Fixed,
    /// Since when it has had a field's worth of plots untended.
    pub untended_since: Option<u64>,
    pub reckoned: Option<u64>,
}

/// How a claim was come by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Acquired {
    /// Held from the town's founding (the register's first day).
    Founding,
    /// Grown into, as a town's culture spreads, or garrisoned.
    Grown,
    /// Taken from another.
    Taken,
    Built,
    Homesteaded,
    Inherited,
    /// Bought of the town, from its estate.
    Bought,
    /// Bought of another household.
    Purchased,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub id: ClaimId,
    pub owner: Party,
    pub how: Acquired,
    pub since: u64,
}

/// The land register: each thing claimed and its claim, and the next claim's id.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Register {
    pub claims: BTreeMap<Target, Claim>,
    pub next: ClaimId,
    pub reckoned: Option<u64>,
    #[serde(default)]
    pub fields: BTreeMap<FieldId, Field>,
    #[serde(default)]
    pub next_field: FieldId,
    #[serde(default)]
    pub leases: BTreeMap<FieldId, Lease>,
    #[serde(default)]
    pub disputes: Vec<Dispute>,
    #[serde(default)]
    pub next_dispute: u32,
    #[serde(default)]
    pub estates: BTreeMap<TownId, Estate>,
}

impl Register {
    /// What a party holds.
    pub fn held_by(&self, owner: Party) -> impl Iterator<Item = (&Target, &Claim)> {
        self.claims.iter().filter(move |(_, c)| c.owner == owner)
    }

    /// Who holds a field.
    pub fn field_owner(&self, f: FieldId) -> Option<Party> {
        self.claims.get(&Target::Field(f)).map(|c| c.owner)
    }

    /// A town's fields.
    pub fn fields_of(&self, town: TownId) -> impl Iterator<Item = (&FieldId, &Field)> {
        self.fields.iter().filter(move |(_, f)| f.town == town)
    }
}

impl crate::society::Society {
    pub fn land_register(&self) -> Register {
        self.world.resource::<Register>().clone()
    }
}

/// The Farmer's household with room for another field (fewer than `fields_per_farmer` a grown Farmer of it) that holds
/// most of the households' purse, and what it holds, if a town has one.
pub fn richest_farmer(content: &crate::content::Content, roster: &crate::roster::Roster, register: &Register,
        town: TownId, list: &[crate::households::Household]) -> Option<(HouseholdId, crate::fixed::Fixed)> {
    let farmer = content.trade("farmer")?;
    list.iter().filter(|h| {
        let farmers = h.members.iter().filter(|id| roster.hands.iter()
            .any(|x| x.id == **id && x.trade == farmer && !x.is_baby())).count();
        let held = register.fields_of(town).filter(|(f, _)| register.field_owner(**f) == Some(Party::Household(h.id))).count();
        farmers as f64 * content.dials.fields_per_farmer > held as f64
    }).max_by_key(|h| (h.purse, std::cmp::Reverse(h.id))).map(|h| (h.id, h.purse))
}

/// Where a household stands on the land: working a field it holds, working one it rents, letting fields and working
/// none, a Farmer's household working none (on the estate's wages), or none of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Standing {
    Owner,
    Tenant,
    Landlord,
    Labourer,
    Other,
}

impl Standing {
    pub fn name(self) -> &'static str {
        match self {
            Standing::Owner => "owner",
            Standing::Tenant => "tenant",
            Standing::Landlord => "landlord",
            Standing::Labourer => "labourer",
            Standing::Other => "other",
        }
    }
}

impl crate::society::Society {
    /// Each of a town's households and where it stands on the land.
    pub fn standings(&self, town: TownId) -> Vec<(HouseholdId, Standing)> {
        let r = self.world.resource::<Register>();
        let roster = self.roster(town);
        let farmer = self.content().trade("farmer");
        self.households(town).iter().map(|h| {
            let mine = Party::Household(h.id);
            let works = |f: &FieldId| r.fields.get(f).is_some_and(|x| x.worked_by == Some(h.id));
            let holds: Vec<FieldId> = r.fields_of(town).map(|(f, _)| *f).filter(|f| r.field_owner(*f) == Some(mine)).collect();
            let standing = if holds.iter().any(works) {
                Standing::Owner
            } else if r.fields_of(town).any(|(f, _)| works(f)) {
                Standing::Tenant
            } else if holds.iter().any(|f| r.leases.contains_key(f)) {
                Standing::Landlord
            } else if h.members.iter().any(|id| roster.hands.iter().any(|x| x.id == *id && Some(x.trade) == farmer && !x.is_baby())) {
                Standing::Labourer
            } else {
                Standing::Other
            };
            (h.id, standing)
        }).collect()
    }
}
