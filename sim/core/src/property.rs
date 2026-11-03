//! Property (John, 2026-11-03, "How to Add Society Systems to the Sim": "Add a claim record: owner (household or
//! firm), target (plot bounds, workstation position or building), how it was acquired, and since when"; "Keep a land
//! register per settlement"). The register only, for now: what exists already, as claims; nothing reads it.
//!
//! Each day (`rules/registers.rs`) the register follows what the society holds: each hex a town holds (`territory.rs`)
//! is its claim, made at its founding, grown into, or taken from another town; each hall built (`works.rs`) is the
//! town's, built; each household's home is the household's. One claim a thing: a claim whose thing is lost is struck
//! out, and a thing that changes hands gets a new claim (overlapping claims, disputes, sale, leases and inheritance come
//! with property's own rules).

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
    Bought,
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
}

impl Register {
    /// What a party holds.
    pub fn held_by(&self, owner: Party) -> impl Iterator<Item = (&Target, &Claim)> {
        self.claims.iter().filter(move |(_, c)| c.owner == owner)
    }
}

impl crate::society::Society {
    pub fn land_register(&self) -> Register {
        self.world.resource::<Register>().clone()
    }
}
