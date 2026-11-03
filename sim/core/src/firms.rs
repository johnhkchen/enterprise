//! Firms (John, 2026-11-03, "How to Add Society Systems to the Sim": "Start a firm as a thin wrapper around what a
//! merchant or smith already does, then let it hire"). The record only, for now: no firm is founded yet.
//!
//! A firm holds its owners and their shares, a treasury, assets (its claims, `property.rs`, owned by
//! [`Party::Firm`](crate::charters::Party)), its employees on wages, what it makes and its reputation. When firms are
//! founded their treasuries join the ledger as accounts of their own (`Side`), as towns' and households' are: till then
//! every firm's is nothing, and the books don't count it.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::charters::Party;
use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::roster::HandId;

pub type FirmId = u32;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Firm {
    pub id: FirmId,
    pub town: TownId,
    pub name: String,
    /// Its owners and each one's share (they add up to 1).
    pub owners: Vec<(Party, f64)>,
    pub treasury: Fixed,
    /// Its hands on wages, and what each is paid a day.
    pub employees: Vec<(HandId, Fixed)>,
    /// What it makes: a recipe's output good, by name.
    pub recipe: Option<String>,
    pub reputation: f64,
    pub founded: u64,
    /// The days its treasury has been below nothing (it's wound up after enough).
    pub broke_days: u32,
}

/// Every firm, and the next firm's id.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Firms {
    pub firms: Vec<Firm>,
    pub next: FirmId,
}

impl crate::society::Society {
    pub fn firms(&self) -> Vec<Firm> {
        self.world.resource::<Firms>().firms.clone()
    }
}
