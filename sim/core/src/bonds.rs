//! Bonds (John, 2026-11-03: "Towns invest by buying municipal bonds/royal treasuries"; "Any town works"; "Crown builds
//! roads"). A town short of the money for a work it wants offers a **municipal bond** for what it lacks, up to
//! `bond_most`: a hall it would build but for its savings, a waystation out on the road, its stop's next level once
//! wagons pass it. A town with money idle beyond its reserve, what opening a road asks and `bond_idle` buys it, and the
//! work goes ahead on the money. The issuer repays what it borrowed and `bond_coupon` more over `bond_term_days`, a
//! share a day, as wagons between the two call; twice the term unpaid, the bond is in default and the holder's loss.
//!
//! The capital offers **royal bonds** (`royal_bond` each, `royal_bonds_most` out at once) for its **king's highways**:
//! what they raise grades and paves the most worn trails of the realm's land roads, a hex a day, and its treasury repays
//! them as a town's does.
//!
//! Nothing crosses distance without a carrier: a bond is bought, and repaid, only where a wagon between the two calls.
//! At the far end the wagon's purse pays for its town's purchases there and its town's dues, and takes home what the town
//! here pays for a bond its town offers and what it owes on one its town holds (`rules/bonds.rs`).

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::Dials;
use crate::fixed::Fixed;
use crate::hex::Hex;
use crate::messages::TownId;
use crate::works::Kind;

/// What a bond pays for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Purpose {
    /// A hall its town would build but for its savings.
    Hall(Kind),
    /// A waystation out on the road.
    Waystation,
    /// Its stop's next level (the stop on this hex).
    StopLevel(Hex),
    /// The crown's king's highways.
    Highways,
}

impl Purpose {
    pub fn name(&self) -> String {
        match self {
            Purpose::Hall(k) => format!("a {}", k.id().replace('_', " ")),
            Purpose::Waystation => "a waystation".into(),
            Purpose::StopLevel(_) => "its stop's next level".into(),
            Purpose::Highways => "the king's highways".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Offered,
    Sold,
    Repaid,
    Lapsed,
    Defaulted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bond {
    pub issuer: TownId,
    pub purpose: Purpose,
    /// What it borrows, and what it repays in all.
    pub principal: Fixed,
    pub face: Fixed,
    pub offered: u64,
    pub sold: Option<u64>,
    pub holder: Option<TownId>,
    pub repaid: Fixed,
    pub state: State,
}

impl Bond {
    /// What's owed on it by `now`: its face a share a day of its term, less what was repaid.
    pub fn due(&self, now: u64, day: u64, dials: &Dials) -> Fixed {
        let Some(sold) = self.sold else { return Fixed::ZERO };
        if self.state != State::Sold {
            return Fixed::ZERO;
        }
        let share = (now.saturating_sub(sold) as f64 / (dials.bond_term_days * day.max(1) as f64)).min(1.0);
        (Fixed::floor(self.face.to_f64() * share) - self.repaid).at_least_zero()
    }

    pub fn open(&self) -> bool {
        matches!(self.state, State::Offered | State::Sold)
    }
}

/// Every bond offered, in the order offered, and what the crown's highways have to spend.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bonds {
    pub bonds: Vec<Bond>,
    /// What royal bonds raised not yet spent on the highways, and the hexes they laid.
    pub highways: Fixed,
    pub highways_laid: u32,
    /// The day the highways were last worked.
    pub highways_day: u64,
}

impl Bonds {
    /// A bond for `principal`, at the dials' coupon.
    pub fn offer(&mut self, issuer: TownId, purpose: Purpose, principal: Fixed, now: u64, dials: &Dials) -> usize {
        let face = Fixed::ceil(principal.to_f64() * (1.0 + dials.bond_coupon));
        self.bonds.push(Bond { issuer, purpose, principal, face, offered: now, sold: None, holder: None,
            repaid: Fixed::ZERO, state: State::Offered });
        self.bonds.len() - 1
    }

    /// The bond a town has open (offered or sold, not yet repaid), but the crown's.
    pub fn open_by(&self, issuer: TownId) -> Option<&Bond> {
        self.bonds.iter().find(|b| b.issuer == issuer && b.open() && b.purpose != Purpose::Highways)
    }

    /// The bonds a town holds, not yet repaid.
    pub fn held_by(&self, holder: TownId) -> usize {
        self.bonds.iter().filter(|b| b.holder == Some(holder) && b.state == State::Sold).count()
    }

    /// Of a bond's purpose, whether its money's in hand: sold, for a hall of this kind.
    pub fn funds_hall(&self, issuer: TownId, kind: Kind) -> bool {
        self.bonds.iter().any(|b| b.issuer == issuer && b.state == State::Sold && b.purpose == Purpose::Hall(kind))
    }
}

impl crate::society::Society {
    /// Every bond offered.
    pub fn bonds(&self) -> Vec<Bond> {
        self.world.resource::<Bonds>().bonds.clone()
    }

    /// Offers a bond for `principal` emeralds (a host's command, a test's).
    pub fn offer_bond(&mut self, issuer: TownId, purpose: Purpose, principal: f64) -> usize {
        let (now, dials) = (self.now(), self.content().dials.clone());
        self.world.resource_mut::<Bonds>().offer(issuer, purpose, Fixed::ceil(principal), now, &dials)
    }

    /// What the crown's highways have to spend, and the hexes they laid.
    pub fn highways(&self) -> (Fixed, u32) {
        let b = self.world.resource::<Bonds>();
        (b.highways, b.highways_laid)
    }
}
