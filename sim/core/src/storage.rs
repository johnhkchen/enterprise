//! Storage (John, 2026-11-03: "it seems like there's some overproduction. Are we charging storage fees on things, so
//! that it's not worth parking excess supply in warehouses?"; "Spoilage", "Storage fees to the owner", "Inventory tax").
//!
//! **Spoilage.** What a town holds beyond what it wants of a good goes off a share a day (`rules/storage.rs` `spoil`),
//! out of the world: raw meat and fish fastest (`spoil_raw`), other food (`spoil_food`), wool, leather and hides
//! slowly (`spoil_fibre`); tools, ingots, armour, paper and the rest keep. It wants `spoil_keep` of each good, and what
//! it set aside for its forward sales: below that nothing spoils. Seed 1717's year ended with Refuge Pointe holding
//! 1,514 wool and 958 cookies nobody bought, its warehouse over full and its Lumberjack felling nothing for want of room.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::{Content, Dials, GoodId};
use crate::messages::TownId;

/// How a good keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keeping {
    /// Raw meat and fish.
    Raw,
    /// Any other food.
    Food,
    /// Wool, leather and hides.
    Fibre,
    /// Everything else.
    Keeps,
}

/// Raw meat and fish, by the host's names.
pub const RAW: [&str; 7] = ["beef", "porkchop", "mutton", "chicken", "rabbit", "cod", "salmon"];
/// Wool, leather and hides.
pub const FIBRE: [&str; 3] = ["white_wool", "leather", "rabbit_hide"];

fn named(content: &Content, g: GoodId, names: &[&str]) -> bool {
    let name = content.goods[g].name.as_str();
    let bare = name.rsplit(':').next().unwrap_or(name);
    names.contains(&bare)
}

/// How a good keeps.
pub fn keeping(content: &Content, g: GoodId) -> Keeping {
    if named(content, g, &RAW) {
        Keeping::Raw
    } else if content.goods[g].food > 0.0 {
        Keeping::Food
    } else if named(content, g, &FIBRE) {
        Keeping::Fibre
    } else {
        Keeping::Keeps
    }
}

/// The share of what's held beyond its keep that goes off a day.
pub fn spoil_rate(d: &Dials, k: Keeping) -> f64 {
    match k {
        Keeping::Raw => d.spoil_raw,
        Keeping::Food => d.spoil_food,
        Keeping::Fibre => d.spoil_fibre,
        Keeping::Keeps => 0.0,
    }
}

/// What spoiled in a town, all told, at worth, and when it last reckoned.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub spoiled: f64,
    pub reckoned: Option<u64>,
}

/// Every town's storage.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Storage {
    pub towns: BTreeMap<TownId, Stored>,
}

impl crate::society::Society {
    /// A town's storage: what spoiled there.
    pub fn stored(&self, town: TownId) -> Stored {
        self.world.resource::<Storage>().towns.get(&town).cloned().unwrap_or_default()
    }
}
