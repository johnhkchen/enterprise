//! A host's content pack (`docs/architecture.md`: "content packs are data, checked by the core"): its goods and its
//! day. What the core makes of it is [`Content`]: the mod's trades, conversions, animals and villagers' offers follow
//! from the goods by name, as [`Content::new`] has them.

use serde::{Deserialize, Serialize};

use crate::content::{Content, Good};

/// The host's content: how long its day is, in ticks, and the goods the society trades, each numbered by its place
/// here (a [`super::GoodId`]); then the rest of what the game brings, by name: the kinds of comfort its people want,
/// the trades its villagers can take, what those trades make of one good in another, the animals its towns keep, and
/// the dials it sets for its world. Everything the core makes of the game's names is checked by [`Content::from_pack`]
/// before a society is made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContentPack {
    pub day_ticks: u64,
    pub goods: Vec<PackGood>,
    /// The kinds of comfort, in the order a good's `comfort` names them (the mod's tags under `enterprise:comforts/`).
    pub comforts: Vec<String>,
    /// The trades a villager can have, by name without the game's namespace (`farmer`, not `minecraft:farmer`). The
    /// Merchant's (`merchant`) and the two with no work (`none`, `nitwit`) are the core's own to name and must be here;
    /// the core knows which lead a caravan or go with one by their names.
    pub trades: Vec<String>,
    /// What the trades make: the town's workshops.
    pub recipes: Vec<PackRecipe>,
    /// The animals a town keeps.
    pub animals: Vec<PackAnimal>,
    /// The dials the game sets for a new world, where it isn't the core's default (also what `reset` puts one back to).
    pub dials: Vec<PackDial>,
}

impl ContentPack {
    /// A pack of these goods and nothing more: the core's own five kinds of comfort and sixteen trades, no recipes, no
    /// animals, every dial at the core's default.
    pub fn new(day_ticks: u64, goods: Vec<PackGood>) -> Self {
        ContentPack {
            day_ticks,
            goods,
            comforts: Content::COMFORTS.iter().map(|k| k.to_string()).collect(),
            trades: crate::roster::trades().into_iter().map(|t| t.name).collect(),
            recipes: Vec::new(),
            animals: Vec::new(),
            dials: Vec::new(),
        }
    }
}

/// A good: its name in the host's game, what one is worth (in emeralds), how many make a stack (a wagon's slot holds a
/// stack), the food points and saturation one is worth if it's food, the kind of comfort it is if any (one of the
/// pack's `comforts`, by name), what else it is, and what a full stack weighs (a pallet and a carrier hold so much,
/// `docs/pallets.md`: 1 for a plain good, so that weight equals slots and never binds first).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackGood {
    pub name: String,
    pub worth: f64,
    pub stack: u32,
    pub food: f64,
    pub saturation: f64,
    pub comfort: Option<String>,
    pub flags: Vec<GoodFlag>,
    pub weight: f64,
}

/// A number of a good, by its name in the pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackAmount {
    pub good: String,
    pub count: f64,
}

/// What a town's workshop makes: any hand of one of the `trades` turns the `inputs` into `makes` of `output`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackRecipe {
    pub trades: Vec<String>,
    pub inputs: Vec<PackAmount>,
    pub output: String,
    pub makes: f64,
}

/// An animal a town keeps: what it eats (the goods in the order they're taken) and how much a day, the meat a
/// slaughtered one gives and how much, the wool a Shepherd shears from it if it's sheep, the hide it gives if it's a
/// cow. The core names its kinds `sheep`, `cow`, `pig` and `chicken`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackAnimal {
    pub name: String,
    pub feed: Vec<String>,
    pub feed_per_day: f64,
    pub meat: Option<String>,
    pub meat_each: f64,
    pub wool: Option<String>,
    pub hide: Option<String>,
}

/// A dial set to `value` for a new world, by the name `/enterprise dial` lists it under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackDial {
    pub name: String,
    pub value: f64,
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
    /// A good worth `worth`, `stack` to a stack, a stack weighing 1, and nothing else.
    pub fn new(name: &str, worth: f64, stack: u32) -> Self {
        PackGood { name: name.to_string(), worth, stack, food: 0.0, saturation: 0.0, comfort: None, flags: Vec::new(), weight: 1.0 }
    }
}

impl Content {
    /// The core's content for a host's pack, or everything wrong with it, each in its own words: a day of no ticks; no
    /// goods; a good with no name, or another's; a worth that isn't a number above nothing; a stack of none; food
    /// points, saturation or weight that isn't a number, or below nothing (a weight of nothing too); a comfort the pack
    /// doesn't list; a kind of comfort or a trade named twice, with no name or with a namespace; no Merchant, `none` or
    /// `nitwit` among the trades; a recipe or animal naming a good or trade the pack lacks, a recipe with nothing in
    /// or out; an amount that isn't a number above nothing; a dial the core doesn't have or won't take the value of.
    pub fn from_pack(pack: &ContentPack) -> Result<Content, String> {
        let mut wrong = Vec::new();
        if pack.day_ticks == 0 {
            wrong.push("a day of no ticks".to_string());
        }
        if pack.goods.is_empty() {
            wrong.push("no goods".to_string());
        }

        // Kinds of comfort and trades first: a good names a comfort, a recipe names trades.
        let mut comforts: Vec<String> = Vec::new();
        for (i, kind) in pack.comforts.iter().enumerate() {
            if kind.trim().is_empty() {
                wrong.push(format!("comfort {i}: no name"));
            } else if comforts.contains(kind) {
                wrong.push(format!("comfort {i} ({kind:?}): named as another is"));
            }
            comforts.push(kind.clone());
        }
        let mut trades: Vec<crate::roster::Trade> = Vec::with_capacity(pack.trades.len());
        for (i, name) in pack.trades.iter().enumerate() {
            let mut bad = |what: &str| wrong.push(format!("trade {i} ({name:?}): {what}"));
            if name.trim().is_empty() {
                bad("no name");
            } else if name.contains(':') {
                bad("named with a namespace (a trade is `farmer`, not `minecraft:farmer`)");
            } else if trades.iter().any(|t| &t.name == name) {
                bad("named as another is");
            }
            trades.push(crate::roster::trade_named(name));
        }
        for needed in [crate::roster::MERCHANT, crate::roster::NONE, crate::roster::NITWIT] {
            if !trades.iter().any(|t| t.name == needed) {
                wrong.push(format!("no trade called {needed:?}, which the core needs"));
            }
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
            if !(g.weight.is_finite() && g.weight > 0.0) {
                bad(format!("a stack weighing {}, not a number above nothing", g.weight));
            }
            let comfort = match &g.comfort {
                None => None,
                Some(kind) => match comforts.iter().position(|k| k == kind) {
                    Some(k) => Some(k),
                    None => {
                        bad(format!("a comfort the pack doesn't list ({kind:?}; it lists {comforts:?})"));
                        None
                    }
                },
            };
            let mut good = Good::new(&g.name, g.worth, g.stack).food(g.food).saturation(g.saturation);
            good.comfort = comfort;
            // A stack weighs `weight`; the core weighs a piece.
            good.weight = g.weight / g.stack.max(1) as f64;
            for flag in &g.flags {
                good = match flag {
                    GoodFlag::Crop => good.crop(),
                    GoodFlag::Meat => good.meat(),
                };
            }
            goods.push(good);
        }

        // What the rest names, by the names the pack gave: exactly, so a mistyped one is found out here.
        let good_at = |name: &str| pack.goods.iter().position(|g| g.name == name);
        let trade_at = |name: &str| trades.iter().position(|t| t.name == name);
        let amount = |x: f64| x.is_finite() && x > 0.0;

        let mut conversions = Vec::with_capacity(pack.recipes.len());
        for (i, r) in pack.recipes.iter().enumerate() {
            let mut bad = |what: String| wrong.push(format!("recipe {i} ({:?}): {what}", r.output));
            if r.trades.is_empty() {
                bad("no trade makes it".to_string());
            }
            let mut by = Vec::with_capacity(r.trades.len());
            for t in &r.trades {
                match trade_at(t) {
                    Some(t) => by.push(t),
                    None => bad(format!("no trade called {t:?}")),
                }
            }
            if r.inputs.is_empty() {
                bad("nothing goes in".to_string());
            }
            let mut inputs = Vec::with_capacity(r.inputs.len());
            for a in &r.inputs {
                match good_at(&a.good) {
                    Some(g) => inputs.push((g, a.count)),
                    None => bad(format!("no good called {:?} to put in", a.good)),
                }
                if !amount(a.count) {
                    bad(format!("{} of {:?} in, not a number above nothing", a.count, a.good));
                }
            }
            let output = good_at(&r.output);
            if output.is_none() {
                bad(format!("no good called {:?} to make", r.output));
            }
            if !amount(r.makes) {
                bad(format!("makes {}, not a number above nothing", r.makes));
            }
            if let Some(output) = output {
                conversions.push(crate::workshop::Conversion { trades: by, inputs, output, out: r.makes });
            }
        }

        let mut animals = Vec::with_capacity(pack.animals.len());
        for (i, a) in pack.animals.iter().enumerate() {
            let mut bad = |what: String| wrong.push(format!("animal {i} ({:?}): {what}", a.name));
            if a.name.trim().is_empty() {
                bad("no name".to_string());
            } else if pack.animals[..i].iter().any(|b| b.name == a.name) {
                bad("named as another is".to_string());
            }
            if a.feed.is_empty() {
                bad("eats nothing".to_string());
            }
            let mut feed = Vec::with_capacity(a.feed.len());
            for f in &a.feed {
                match good_at(f) {
                    Some(g) => feed.push(g),
                    None => bad(format!("no good called {f:?} to feed it")),
                }
            }
            if !(a.feed_per_day.is_finite() && a.feed_per_day >= 0.0) {
                bad(format!("eats {} a day", a.feed_per_day));
            }
            if !(a.meat_each.is_finite() && a.meat_each >= 0.0) {
                bad(format!("gives {} meat", a.meat_each));
            }
            let mut by_name = |what: &str, name: &Option<String>| match name {
                None => None,
                Some(n) => {
                    let g = good_at(n);
                    if g.is_none() {
                        bad(format!("no good called {n:?} for its {what}"));
                    }
                    g
                }
            };
            let (meat, wool, hide) = (by_name("meat", &a.meat), by_name("wool", &a.wool), by_name("hide", &a.hide));
            animals.push(crate::herds::Animal { name: a.name.clone(), feed, feed_per_day: a.feed_per_day, meat,
                meat_each: a.meat_each, wool, hide });
        }

        let mut dials = crate::dials::Dials::default();
        for (i, d) in pack.dials.iter().enumerate() {
            if pack.dials[..i].iter().any(|e| e.name == d.name) {
                wrong.push(format!("dial {i} ({:?}): set twice", d.name));
            } else if let Err(why) = dials.set(&d.name, d.value) {
                wrong.push(format!("dial {i} ({:?}): {why}", d.name));
            }
        }

        if !wrong.is_empty() {
            return Err(wrong.join("; "));
        }
        let mut content = Content::new(goods, pack.day_ticks);
        content.comforts = comforts;
        content.trades = trades;
        content.conversions = conversions;
        content.animals = animals;
        content.dial_defaults = dials.clone();
        content.dials = dials;
        // The villagers' trades, vanilla's, for the pack's goods and trades (`lockers.rs`).
        crate::lockers::default_offers(&mut content);
        Ok(content)
    }
}
