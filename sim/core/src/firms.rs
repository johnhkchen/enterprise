//! Firms (John, 2026-11-03, "How to Add Society Systems to the Sim": "Start a firm as a thin wrapper around what a
//! merchant or smith already does, then let it hire"; step 3, "Firms, plus 2–3 processing jobs with threshold and
//! range"; 2026-11-04, "Allow capital investments for higher quality goods", "Baker, Weaver, Tanner. Higher quality
//! goods", "Town hands of its trade").
//!
//! A firm is a household's capital put into a workshop of one of the [`KINDS`]: a bakery, a weaver's or a tannery. It
//! makes the finer tier of what the town's own workshop makes plain (`People::fine`: each piece worth `fine_quality`
//! plain ones), of the town's goods, with hands of its trade it hires away from the town's wages. It opens only where
//! enough people live within its range (the kind's threshold and range, the guide's central places: a bakery in the
//! bigger town and not the small one), its town wants what it makes, and what a hand makes it earns more than the hand
//! earns now; it's wound up once it hasn't paid its hands in full for `firm_broke_days`.
//!
//! A firm holds its owners and their shares, a treasury (on the ledger, `Account::Firm`), its employees on wages, what
//! it makes and its reputation. Its claims (`property.rs`, owned by [`Party::Firm`]) are for later steps.

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::charters::Party;
use crate::content::{Content, GoodId};
use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::roster::{HandId, TradeId};
use crate::workshop::Conversion;

pub type FirmId = u32;

/// The kinds of firm: name, the trade it hires, what it makes, the people within its range it needs (threshold) and
/// that range in blocks, and the capital its workshop takes, in emeralds (the guide's starting values: a baker 40 and
/// 150; the weaver's and the tanner's the smith's 300 blocks, and its 60 people cut to 50: seed 1717's towns are 55 at
/// most, and no Shepherd's or Leatherworker's had 60 within its range all year).
#[allow(clippy::type_complexity)]
pub const KINDS: [(&str, &str, &str, f64, f64, f64); 3] = [
    ("bakery", "farmer", "bread", 40.0, 150.0, 30.0),
    ("weaver", "shepherd", "white_carpet", 50.0, 300.0, 40.0),
    ("tannery", "leatherworker", "leather_boots", 50.0, 300.0, 40.0),
];

/// A kind of firm as the host's content has it: its trade and the town workshop's conversion it makes the finer tier
/// of. `None` where the host lacks the trade or the conversion.
#[derive(Clone, Debug, PartialEq)]
pub struct Kind {
    pub name: &'static str,
    pub trade: TradeId,
    pub conversion: Conversion,
    pub threshold: f64,
    pub range: f64,
    pub capital: f64,
}

/// Each kind the host's content has.
pub fn kinds(content: &Content) -> Vec<Kind> {
    KINDS.iter().filter_map(|&(name, trade, output, threshold, range, capital)| {
        let trade = content.trade(trade)?;
        let output = content.good(output)?;
        let conversion = content.conversions.iter().find(|c| c.output == output && c.trades.contains(&trade))?.clone();
        Some(Kind { name, trade, conversion, threshold, range, capital })
    }).collect()
}

/// What a batch of its finer pieces is worth less its inputs, in emeralds: what a hand's batch adds.
pub fn added(content: &Content, c: &Conversion) -> f64 {
    c.out * content.goods[c.output].value * content.dials.fine_quality - crate::workshop::inputs_worth(content, c)
}

/// How many more of its finer pieces a town wants of a firm: what its own workshop keeps it at of the output
/// (`workshop::wanted`'s pars: a comfort's rations for `comfort_days`, food at twice the food par, else a locker's
/// par), less the finer pieces it holds. Its people use the finer pieces first, so a firm keeps their stock up and the
/// town's workshop makes plain only what the firm can't.
pub fn fine_wanted(content: &Content, people: &crate::state::People, output: GoodId,
        locker_par: f64) -> f64 {
    let g = &content.goods[output];
    let par = if g.comfort.is_some() {
        crate::people::comfort_par(content, people) / content.comfort_worth(output).max(1e-9)
    } else if g.food > 0.0 {
        2.0 * crate::workshop::FOOD_PAR_POINTS / g.food.max(1e-9)
    } else {
        locker_par
    };
    let fine = people.fine.get(&output).copied().unwrap_or(0.0);
    (par - fine).max(0.0).floor()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Firm {
    pub id: FirmId,
    pub town: TownId,
    /// Its kind's name ([`KINDS`]).
    pub name: String,
    /// Its owners and each one's share (they add up to 1).
    pub owners: Vec<(Party, f64)>,
    pub treasury: Fixed,
    /// Its hands on wages, and what each is paid a day.
    pub employees: Vec<(HandId, Fixed)>,
    /// What it makes: its output good, by name.
    pub recipe: Option<String>,
    pub reputation: f64,
    pub founded: u64,
    /// The days running it hasn't paid its hands in full (it's wound up after `firm_broke_days`).
    pub broke_days: u32,
    /// The pieces it has made, all told.
    #[serde(default)]
    pub made: f64,
    /// The day it last ran (its day's work done once a day).
    #[serde(default)]
    pub reckoned: Option<u64>,
}

impl Firm {
    pub fn output(&self, content: &Content) -> Option<GoodId> {
        self.recipe.as_deref().and_then(|r| content.good(r))
    }
}

/// Every firm, the next firm's id, and the firms wound up (with the day).
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Firms {
    pub firms: Vec<Firm>,
    pub next: FirmId,
    #[serde(default)]
    pub wound_up: Vec<(Firm, u64)>,
}

impl Firms {
    /// The hands of a town working for its firms.
    pub fn employed(&self, town: TownId) -> BTreeSet<HandId> {
        self.firms.iter().filter(|f| f.town == town).flat_map(|f| f.employees.iter().map(|e| e.0)).collect()
    }

    pub fn get_mut(&mut self, id: FirmId) -> Option<&mut Firm> {
        self.firms.iter_mut().find(|f| f.id == id)
    }
}

impl crate::society::Society {
    pub fn firms(&self) -> Vec<Firm> {
        self.world.resource::<Firms>().firms.clone()
    }

    /// The firms wound up, each with the day.
    pub fn firms_wound_up(&self) -> Vec<(Firm, u64)> {
        self.world.resource::<Firms>().wound_up.clone()
    }
}
