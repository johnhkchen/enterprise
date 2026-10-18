//! A host's content pack (`docs/architecture.md`: "content packs are data, checked by the core"): its goods and its
//! day. What the core makes of it is [`Content`]: the mod's trades, conversions, animals and villagers' offers follow
//! from the goods by name, as [`Content::new`] has them.

use serde::{Deserialize, Serialize};

use crate::content::{Content, Good};

/// The host's content: how long its day is, in ticks, and the goods the society trades, each numbered by its place
/// here (a [`super::GoodId`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContentPack {
    pub day_ticks: u64,
    pub goods: Vec<PackGood>,
}

/// A good: its name in the host's game, what one is worth (in emeralds), how many make a stack (a wagon's slot holds a
/// stack), the food points and saturation one is worth if it's food, the kind of comfort it is if any (one of
/// [`Content::COMFORTS`], by name), and what else it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackGood {
    pub name: String,
    pub worth: f64,
    pub stack: u32,
    pub food: f64,
    pub saturation: f64,
    pub comfort: Option<String>,
    pub flags: Vec<GoodFlag>,
}

/// What else a good is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoodFlag {
    /// A crop farmers grow: its sales at a town's lockers count as farm sales (`farms.rs`).
    Crop,
    /// Meat: hearty food for an army (`army.rs`).
    Meat,
}

impl PackGood {
    /// A good worth `worth`, `stack` to a stack, and nothing else.
    pub fn new(name: &str, worth: f64, stack: u32) -> Self {
        PackGood { name: name.to_string(), worth, stack, food: 0.0, saturation: 0.0, comfort: None, flags: Vec::new() }
    }
}

impl Content {
    /// The core's content for a host's pack, or everything wrong with the pack: a day of no ticks; no goods; a good
    /// with no name, or another's; a worth that isn't a number above nothing; a stack of none; food points or
    /// saturation below nothing or not a number; a comfort the core doesn't know.
    pub fn from_pack(pack: &ContentPack) -> Result<Content, String> {
        let mut wrong = Vec::new();
        if pack.day_ticks == 0 {
            wrong.push("a day of no ticks".to_string());
        }
        if pack.goods.is_empty() {
            wrong.push("no goods".to_string());
        }
        let mut goods = Vec::with_capacity(pack.goods.len());
        for (i, g) in pack.goods.iter().enumerate() {
            let mut bad = |what: String| wrong.push(format!("good {i} ({:?}): {what}", g.name));
            if g.name.trim().is_empty() {
                bad("no name".to_string());
            } else if pack.goods[..i].iter().any(|h| h.name == g.name) {
                bad("named as another is".to_string());
            }
            if !(g.worth.is_finite() && g.worth > 0.0) {
                bad(format!("worth {}, not a number above nothing", g.worth));
            }
            if g.stack == 0 {
                bad("a stack of none".to_string());
            }
            if !(g.food.is_finite() && g.food >= 0.0) {
                bad(format!("food points {}", g.food));
            }
            if !(g.saturation.is_finite() && g.saturation >= 0.0) {
                bad(format!("saturation {}", g.saturation));
            }
            let comfort = match &g.comfort {
                None => None,
                Some(kind) => match Content::COMFORTS.iter().position(|k| k == kind) {
                    Some(k) => Some(k),
                    None => {
                        bad(format!("a comfort the core doesn't know ({kind:?}; it knows {:?})", Content::COMFORTS));
                        None
                    }
                },
            };
            let mut good = Good::new(&g.name, g.worth, g.stack).food(g.food).saturation(g.saturation);
            good.comfort = comfort;
            for flag in &g.flags {
                good = match flag {
                    GoodFlag::Crop => good.crop(),
                    GoodFlag::Meat => good.meat(),
                };
            }
            goods.push(good);
        }
        if !wrong.is_empty() {
            return Err(wrong.join("; "));
        }
        Ok(Content::new(goods, pack.day_ticks))
    }
}
