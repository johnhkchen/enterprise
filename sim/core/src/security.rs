//! Security (John, 2026-11-03: "introduce a new dimension of spending: security. This is an example of where smiths,
//! fletchers, miners, masons all can contribute"; "Bandits on the roads", "Raids on rich towns", "Monsters at night",
//! "Theft from within", "Soldiers to hold/contest a territory"; what's taken goes "Out of the world").
//!
//! A town keeps a **watch**: watchmen paid `guard_pay` a day, as its posts' guards are, wearing the guards' kit (swords
//! and bows from its smiths and Fletchers, chestplates from its Armorer, of its miners' iron: `kit.rs`'s guards count
//! them). Its **defence** is its watch and the idle hands standing with it (a militia, half a watchman each), better
//! armed and armoured as its kit covers them, and its walls (a whole wall a full watch's worth) and watchtower (half).
//! Against a town of its size ([`needed`]: a watchman for `watch_per_villagers` villagers) that gives its
//! **protection**, defence over needed defence plus one: none undefended, a half at par, two thirds at twice par.
//!
//! What it's protected from, each day (`rules/security.rs`), the rest of protection's share falling on it:
//! - **Raids** on a rich town: a chance up to `raid_chance` a day as its wealth (treasury and warehouse at worth) nears
//!   `raid_wealth`; a raid takes `raid_take` of its treasury and goods and frightens it (`raid_mood` for `fear_days`).
//! - **Theft** from within: `theft_rate` of its warehouse and its households' purses a day.
//! - **Bandits on the roads:** a wagon home again by land was robbed with a chance of `bandit_chance` for each lonely
//!   hex it crossed, there and back (beyond its ends' doorsteps, held by no town that keeps a watch or posts, no stop within 2 hexes), less its
//!   escort (its home's watch, by how well armed: half at best); robbers take `bandit_take` of its purse and its load.
//! - **Monsters at night:** a chance of `night_chance` a day, less its walls and the light of its lanterns (one for
//!   four villagers), frightens it (`night_mood`).
//!
//! A hamlet of fewer than `secure_villagers` keeps no watch and suffers none of it. What's taken leaves the world. A town sets its watch each day: a watchman for each `watch_per_villagers`, twice that
//! for `fear_days` after a raid, as far as it can pay `watch_days` of their pay beyond its reserve; it changes by one
//! a day.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::{Content, Dials};
use crate::fixed::Fixed;
use crate::messages::TownId;

/// A watchtower's worth of defence, against a full watch's.
pub const TOWER_DEFENCE: f64 = 0.5;
/// A militiaman (an idle hand) against a watchman.
pub const MILITIA: f64 = 0.5;
/// Villagers a lantern lights the night for.
pub const PER_LANTERN: f64 = 4.0;

/// A town's watch and what befell it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Watch {
    pub watchmen: u32,
    /// Its defence and protection as last reckoned.
    pub defence: f64,
    pub protection: f64,
    /// When it was last raided, and the days it reckoned.
    pub raided: Option<u64>,
    pub kept_at: Option<u64>,
    /// What it lost, all told: to raids, to thieves, and the nights it was attacked; raids suffered.
    pub raided_of: Fixed,
    pub stolen: Fixed,
    pub raids: u32,
    pub nights: u32,
    /// Its wagons robbed on the roads, and what they lost.
    #[serde(default)]
    pub robbed: u32,
    #[serde(default)]
    pub robbed_of: Fixed,
}

/// Every town's watch.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Security {
    pub towns: BTreeMap<TownId, Watch>,
}

impl Security {
    pub fn watchmen(&self, town: TownId) -> usize {
        self.towns.get(&town).map_or(0, |w| w.watchmen as usize)
    }
}

/// The defence a town of `villagers` needs to be half protected: a watchman for `watch_per_villagers`.
pub fn needed(d: &Dials, villagers: f64) -> f64 {
    (villagers / d.watch_per_villagers.max(1.0)).max(1.0)
}

/// A town's defence: its watch and militia, by how well they're armed and armoured, and its walls and watchtower.
pub fn defence(d: &Dials, villagers: f64, watchmen: usize, militia: usize, armed: f64, armoured: f64, walled: f64,
        tower: bool) -> f64 {
    let men = watchmen as f64 + MILITIA * militia as f64;
    let par = needed(d, villagers);
    men * (0.5 + 0.5 * armed) * (1.0 + 0.5 * armoured) + par * (walled + if tower { TOWER_DEFENCE } else { 0.0 })
}

/// Protection from a defence against what's needed: defence over needed plus one.
pub fn protection(defence: f64, needed: f64) -> f64 {
    let r = defence / needed.max(1e-9);
    r / (1.0 + r)
}

/// A town frightened: `mood` off where its mood heads for `days` (`People::fear`).
pub fn frighten(content: &Content, people: &mut crate::state::People, now: u64, mood: f64, days: f64) {
    people.fear.retain(|(until, _)| *until > now);
    people.fear.push((now + (days * content.day_ticks as f64).round() as u64, mood));
}

/// What frightens a town now, at most `fear_most`.
pub fn fear(d: &crate::content::Dials, people: &crate::state::People) -> f64 {
    people.fear.iter().filter(|(until, _)| people.last < *until).map(|(_, m)| m).sum::<f64>().min(d.fear_most)
}

impl crate::society::Society {
    /// A town's watch and what befell it.
    pub fn watch(&self, town: TownId) -> Watch {
        self.world.resource::<Security>().towns.get(&town).cloned().unwrap_or_default()
    }
}
