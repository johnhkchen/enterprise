//! Storage (John, 2026-11-03: "it seems like there's some overproduction. Are we charging storage fees on things, so
//! that it's not worth parking excess supply in warehouses?"; "Spoilage", "Storage fees to the owner", "Inventory tax").
//!
//! **Spoilage.** What a town holds beyond what it wants of a good goes off a share a day (`rules/storage.rs` `spoil`),
//! out of the world: raw meat and fish fastest (`spoil_raw`), other food (`spoil_food`), wool, leather and hides
//! slowly (`spoil_fibre`); tools, ingots, armour, paper and the rest keep. It wants `spoil_keep` of each good, and what
//! it set aside for its forward sales: below that nothing spoils. Seed 1717's year ended with Refuge Pointe holding
//! 1,514 wool and 958 cookies nobody bought, its warehouse over full and its Lumberjack felling nothing for want of room.
//!
//! **Storehouses** (John: "Armory", "Granary (all food)", "Wool and cloth store"). A hall that holds a kind of goods
//! out of the warehouse, `STORE_SLOTS` stacks of them: the armory weapons and armour (each a stack of one), the granary
//! every food (and food goes off at half the rate in a town with one), the storehouse wool, carpets, leather, hides and
//! string (and they go off at half the rate). A town builds one once its kind takes `STORE_WANTED` slots of its
//! warehouse (`rules/works.rs`); the warehouse's free slots count only what its storehouses don't hold ([`relief`]).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::{Content, Dials, GoodId};
use crate::messages::TownId;
use crate::works::{Kind, Works};

/// The stacks a storehouse holds.
pub const STORE_SLOTS: u32 = 27;
/// The stacks of its kind in the warehouse that want one built.
pub const STORE_WANTED: u32 = 8;
/// Weapons and armour, by the host's names' endings.
pub const ARMS: [&str; 11] = ["_sword", "bow", "crossbow", "arrow", "shield", "_helmet", "_chestplate", "_leggings",
    "_boots", "_horse_armor", "trident"];
/// Wool and cloth, by the host's names' endings.
pub const CLOTH: [&str; 7] = ["_wool", "_carpet", "_banner", "leather", "rabbit_hide", "string", "feather"];

fn bare(content: &Content, g: GoodId) -> &str {
    let name = content.goods[g].name.as_str();
    name.rsplit(':').next().unwrap_or(name)
}

/// The storehouse that holds a good, if one does.
pub fn store_of(content: &Content, g: GoodId) -> Option<Kind> {
    let name = bare(content, g);
    if ARMS.iter().any(|e| name.ends_with(e)) {
        Some(Kind::Armory)
    } else if content.goods[g].food > 0.0 {
        Some(Kind::Granary)
    } else if CLOTH.iter().any(|e| name.ends_with(e)) {
        Some(Kind::Storehouse)
    } else {
        None
    }
}

/// The storehouses a town has built.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stores {
    pub granary: bool,
    pub armory: bool,
    pub storehouse: bool,
}

impl Stores {
    pub fn of(works: Option<&Works>) -> Stores {
        let built = |k| works.is_some_and(|w| w.built(k));
        Stores { granary: built(Kind::Granary), armory: built(Kind::Armory), storehouse: built(Kind::Storehouse) }
    }

    pub fn has(self, kind: Kind) -> bool {
        match kind {
            Kind::Granary => self.granary,
            Kind::Armory => self.armory,
            Kind::Storehouse => self.storehouse,
            _ => false,
        }
    }
}

/// The warehouse stacks of each storehouse's kind of goods: the granary's, the armory's, the storehouse's.
pub fn kind_stacks(content: &Content, stock: &[f64]) -> [u32; 3] {
    let mut out = [0; 3];
    for (g, &n) in stock.iter().enumerate() {
        let i = match store_of(content, g) {
            Some(Kind::Granary) => 0,
            Some(Kind::Armory) => 1,
            Some(Kind::Storehouse) => 2,
            _ => continue,
        };
        out[i] += crate::cargo::warehouse_stacks(content, g, n);
    }
    out
}

/// The stacks a town's storehouses hold out of its warehouse.
pub fn relief(content: &Content, stock: &[f64], stores: Stores) -> u32 {
    let stacks = kind_stacks(content, stock);
    [Kind::Granary, Kind::Armory, Kind::Storehouse].iter().zip(stacks)
        .filter(|(k, _)| stores.has(**k)).map(|(_, n)| n.min(STORE_SLOTS)).sum()
}

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
    names.contains(&bare(content, g))
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

/// The share of what's held beyond its keep that goes off a day: half in a town whose storehouse holds it.
pub fn spoil_rate(d: &Dials, k: Keeping, stores: Stores) -> f64 {
    match k {
        Keeping::Raw => d.spoil_raw * if stores.granary { 0.5 } else { 1.0 },
        Keeping::Food => d.spoil_food * if stores.granary { 0.5 } else { 1.0 },
        Keeping::Fibre => d.spoil_fibre * if stores.storehouse { 0.5 } else { 1.0 },
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
