//! A battle between factions for a hex (John: "seed a factional battle between horde and alliance, requiring them to
//! hold a hex 30 days before settling is allowed"; "One hex, seeded", "Faction armies, army vs army", "30 days held,
//! holder only"). Factions are peoples grouped (`docs/decisions.md`: "Factions are races"): the Horde the orcs,
//! tauren and goblins; the Alliance the kingdoms of men (Stormwind, Lordaeron, Dalaran), the dwarves and the night
//! elves ([`Faction::of`]).
//!
//! - **Seeded** (`battle_day`, a dial: none at 0): that day the hex between the nearest Horde town and the nearest
//!   Alliance town, the middle of the line between them or the nearest of it that's dry and no town's, is contested.
//! - **Each side's army** (an [`Army`], as the crown's: its soldiers eat and draw their kit at its camp, its town
//!   nearest the hex, and run its risks): each of its faction's towns within `battle_reach` hexes levies
//!   `battle_levy` of its residents (a levy's order: idle hands first), who march to the hex, the road's time at a
//!   wagon's pace; every `battle_reinforce_days` its towns make up their levies.
//! - **They meet** every `skirmish_days` (`war::clash`: each side's score the other's target, one twist for both); the
//!   winner holds the hex, as does a side whose enemy has no one there. The fallen are their towns' grief; the routed
//!   and deserters walk home.
//! - **Held `battle_hold_days` in a row,** the hex is its holder's: the fighting ends, both armies walk home, and only
//!   the holder's settlers may found a town on it ([`Battles::may_settle`]); none may before.

use serde::{Deserialize, Serialize};

use bevy_ecs::prelude::Resource;

use crate::army::{Army, Recruit};
use crate::hex::Hex;
use crate::messages::TownId;

/// A side in the war of peoples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Faction {
    Horde,
    Alliance,
}

impl Faction {
    /// The faction of a people (a town's kingdom, `Input::People`), if it has one.
    pub fn of(people: &str) -> Option<Faction> {
        match people.rsplit(':').next().unwrap_or(people).to_ascii_lowercase().replace(['_', ' '], "").as_str() {
            "orc" | "tauren" | "goblin" | "troll" | "undead" | "forsaken" | "bloodelf" => Some(Faction::Horde),
            "stormwind" | "lordaeron" | "dalaran" | "human" | "dwarf" | "nightelf" | "gnome" | "draenei" => Some(Faction::Alliance),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Faction::Horde => "the Horde",
            Faction::Alliance => "the Alliance",
        }
    }
}

/// A recruit on its way to the battle: there, it's a soldier of its side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marching {
    pub side: usize,
    pub recruit: Recruit,
    pub arrives: u64,
}

/// A battle for a hex: the hex, the factions and their armies, each side's camp (its town nearest the hex: its
/// warehouse feeds and kits its army) and levying towns with each one's levy, those marching to it, who holds it and
/// since when, whose it became (the battle over), when it began, its days fought and its fights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Battle {
    pub hex: Hex,
    pub factions: [Faction; 2],
    pub armies: [Army; 2],
    pub camps: [TownId; 2],
    pub levies: [Vec<(TownId, usize)>; 2],
    pub marching: Vec<Marching>,
    pub holder: Option<usize>,
    pub held_since: Option<u64>,
    pub won_by: Option<Faction>,
    pub began: u64,
    pub days: u64,
    pub fights: u64,
    /// The day it was last fought (once a day).
    #[serde(default)]
    pub reckoned: Option<u64>,
}

impl Battle {
    /// The days its holder has held it, today.
    pub fn held_days(&self, now: u64, day: u64) -> u64 {
        self.held_since.map_or(0, |s| now.saturating_sub(s) / day.max(1))
    }
}

/// The battles for hexes, and whether `battle_day`'s was seeded.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Battles {
    pub battles: Vec<Battle>,
    pub seeded: bool,
    /// The day the barracks towns last drilled (`works::Kind::Barracks`).
    #[serde(default)]
    pub drilled: Option<u64>,
}

impl Battles {
    /// May a town of this faction found a town on `hex`? Not on a hex fought for until a side has held it
    /// `battle_hold_days`; then only the side that did.
    pub fn may_settle(&self, hex: Hex, faction: Option<Faction>) -> bool {
        self.battles.iter().filter(|b| b.hex == hex).all(|b| b.won_by.is_some() && b.won_by == faction)
    }
}

impl crate::society::Society {
    pub fn battles(&self) -> &Battles {
        self.world.resource::<Battles>()
    }
}
