//! What a host's game brings: its goods, how long its day is, and the dials that tune the society.

use serde::{Deserialize, Serialize};
use bevy_ecs::prelude::Resource;

pub use crate::dials::{DialValue, Dials};

/// A good, by its place in [`Content::goods`].
pub type GoodId = usize;

/// A good the society trades: its name in the host's game, what one is worth, how many make a stack (a wagon's slot
/// holds a stack), the kind of comfort it is if any (by its place in [`Content::comforts`]), and the food points one
/// is worth if it's food.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Good {
    pub name: String,
    pub value: f64,
    pub stack: u32,
    pub comfort: Option<usize>,
    pub food: f64,
    /// A crop farmers grow (`Territory.isCrop`): its sales at the town's lockers count as farm sales (`farms.rs`).
    pub crop: bool,
    /// The army (`army.rs`): the saturation one gives, as the host's food holds it (bread: 6 for its 5 food points),
    /// for how hearty it is; and whether it's meat (any cooked meat or stew, `Army.MEATS`: hearty however plain).
    pub saturation: f64,
    pub meat: bool,
    /// Weight of one (`docs/pallets.md`): a stack weighs 1 unless the content says it's heavy.
    pub weight: f64,
    /// How it is packed by default (a package kind, [`crate::pallet::CRATE`]).
    pub package: crate::pallet::PackageKindId,
    /// What it needs of handling: plain by default.
    pub handling: crate::pallet::Handling,
}

impl Good {
    pub fn new(name: &str, value: f64, stack: u32) -> Self {
        Good { name: name.to_string(), value, stack, comfort: None, food: 0.0, crop: false, saturation: 0.0, meat: false,
            weight: 1.0 / stack.max(1) as f64, package: crate::pallet::CRATE, handling: Default::default() }
    }

    /// Weighs `weight` a piece.
    pub fn weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    /// A comfort of the kind at `kind` in [`Content::comforts`].
    pub fn comfort(mut self, kind: usize) -> Self {
        self.comfort = Some(kind);
        self
    }

    /// A crop farmers grow.
    pub fn crop(mut self) -> Self {
        self.crop = true;
        self
    }

    /// Food, worth `points` a piece.
    pub fn food(mut self, points: f64) -> Self {
        self.food = points;
        self
    }

    /// Food giving `saturation` a piece (`army.rs`: its heartiness).
    pub fn saturation(mut self, saturation: f64) -> Self {
        self.saturation = saturation;
        self
    }

    /// Meat: hearty food for an army (`Army.MEATS`).
    pub fn meat(mut self) -> Self {
        self.meat = true;
        self
    }
}

/// The host's goods, the kinds of comfort its people want, its day's length in ticks, and the dials.
#[derive(Clone, Debug, Resource, Serialize, Deserialize)]
pub struct Content {
    pub goods: Vec<Good>,
    pub comforts: Vec<String>,
    pub day_ticks: u64,
    pub dials: Dials,
    /// The trades a town's hands can have (`roster.rs`): the mod's sixteen by default.
    pub trades: Vec<crate::roster::Trade>,
    /// What a town's trades convert (`workshop.rs`): the mod's seven by default, for the goods the host has.
    pub conversions: Vec<crate::workshop::Conversion>,
    /// The animals a town keeps (`herds.rs`): the mod's four by default, for the goods the host has.
    pub animals: Vec<crate::herds::Animal>,
    /// The packing units (`pallet.rs`): loose, the crate, the barrel, the bundle.
    pub packages: Vec<crate::pallet::PackageKind>,
    /// The pallet kinds: the base content's one, the load.
    pub pallet_kinds: Vec<crate::pallet::PalletKind>,
}

impl Content {
    /// The mod's five kinds of comfort.
    pub const COMFORTS: [&'static str; 5] = ["treats", "reading", "clothing", "furnishings", "leisure"];

    pub fn new(goods: Vec<Good>, day_ticks: u64) -> Self {
        let mut content = Content { goods, comforts: Self::COMFORTS.iter().map(|k| k.to_string()).collect(), day_ticks,
            dials: Dials::default(), trades: crate::roster::trades(), conversions: Vec::new(), animals: Vec::new(),
            packages: Vec::new(), pallet_kinds: Vec::new() };
        content.packages = crate::pallet::base_packages();
        content.pallet_kinds = crate::pallet::base_pallet_kinds();
        content.conversions = crate::workshop::mod_conversions(&content);
        content.animals = crate::herds::mod_animals(&content);
        // The mod's villager trades for the goods the host has (`lockers.rs`).
        crate::lockers::default_offers(&mut content);
        content
    }

    /// The kind of comfort called `name`.
    pub fn comfort(&self, name: &str) -> Option<usize> {
        self.comforts.iter().position(|k| k == name)
    }

    /// The good called `name`, if the host has one.
    pub fn good(&self, name: &str) -> Option<GoodId> {
        self.goods.iter().position(|g| g.name == name)
    }

    /// The trade called `name`, with or without the host's namespace (`librarian`, `minecraft:librarian`).
    pub fn trade(&self, name: &str) -> Option<crate::roster::TradeId> {
        let name = name.rsplit(':').next().unwrap_or(name);
        self.trades.iter().position(|t| t.name == name)
    }
}
