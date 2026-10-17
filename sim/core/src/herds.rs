//! A town's herds, on the ledger: sheep, cows, pigs and chickens, counted per town (fractions carried) and tended on
//! touch. Ported from `territory/Husbandry.java`: `KINDS` (37), `seed` (69), `feedDemand` (84), `capacity` (94),
//! `tend` (101-140) and its constants.
//!
//! A town's first Shepherd brings 3 sheep, its first Leatherworker 2 cows, its first Butcher 2 pigs and 2 chickens,
//! once only (a flock that starves away isn't given back). The animals eat from the warehouse (the farms ask for the
//! feed: [`feed_demand`]), breed while fed into the room the pastures hold, and dwindle while hungry. A Shepherd shears
//! the sheep; a Butcher culls any kind above the breeding stock for meat, a Leatherworker its cows (for beef and
//! leather), so a culled herd grows to its pastures first.
//!
//! **Logistic growth with carrying capacity** (`docs/core-math.md`): a fed herd grows by `breed_per_day` of itself a
//! day, but never beyond the room its pastures hold (`pasture_animals` each, and a herd not yet fenced has one's room);
//! the cull keeps `cull_share` of the room for the kinds present.
//!
//! The products are owed with the farms' harvest (`Farm.pending`) and the feed joins the farm's demand and needs
//! (`Farm.feed`), where the town's farm is set up; a town without one has its products straight into stock.
//!
//! **Not yet as the mod:** the pasture's fence and the entities that show the ledger (the host's); the mod skips a
//! holding's first tending (`lastTend` -1), here the herds are tended from the town's founding.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::farms::{Farm, MAX_DAYS};
use crate::roster::Roster;
use crate::society::Society;
use crate::state::{Clock, Ledger, Stock, Town};

/// An animal kept: what it eats and how much a day, what slaughtering one gives, what it gives alive (`Husbandry.Kind`).
/// Matched to the host's goods by name; a good the host lacks is left out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animal {
    pub name: String,
    /// What it eats, in the order it's taken.
    pub feed: Vec<GoodId>,
    pub feed_per_day: f64,
    pub meat: Option<GoodId>,
    pub meat_each: f64,
    /// Wool a Shepherd shears from it a day, if it is sheep (`WOOL_PER_SHEEP_DAY` of the `wool_per_sheep_day` dial).
    pub wool: Option<GoodId>,
    /// Leather a slaughtered one gives, if it is a cow (`LEATHER_PER_COW`).
    pub hide: Option<GoodId>,
}

/// A slaughtered cow gives this much leather (`Husbandry.LEATHER_PER_COW`).
pub const LEATHER_PER_COW: f64 = 1.0;

/// The mod's four kinds: name, feed, feed a day, meat, meat each (`Husbandry.KINDS`).
#[allow(clippy::type_complexity)]
pub const MOD_ANIMALS: [(&str, &[&str], f64, &str, f64); 4] = [
    ("sheep", &["wheat"], 0.25, "mutton", 1.5),
    ("cow", &["wheat"], 0.25, "beef", 2.0),
    ("pig", &["carrot", "potato", "beetroot"], 0.25, "porkchop", 2.0),
    ("chicken", &["wheat"], 0.1, "chicken", 1.0),
];

/// The mod's kinds for the goods the host has, in the mod's order. A kind with none of its feed goods is left out.
pub fn mod_animals(content: &Content) -> Vec<Animal> {
    MOD_ANIMALS
        .iter()
        .filter_map(|&(name, feed, feed_per_day, meat, meat_each)| {
            let feed: Vec<GoodId> = feed.iter().filter_map(|f| content.good(f)).collect();
            if feed.is_empty() {
                return None;
            }
            Some(Animal {
                name: name.to_string(),
                feed,
                feed_per_day,
                meat: content.good(meat),
                meat_each,
                wool: if name == "sheep" { content.good("white_wool") } else { None },
                hide: if name == "cow" { content.good("leather") } else { None },
            })
        })
        .collect()
}

/// Which herding trades a town's roster has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Keepers {
    pub shepherd: bool,
    pub butcher: bool,
    pub leatherworker: bool,
}

impl Keepers {
    pub fn of(content: &Content, roster: &Roster) -> Keepers {
        let has = |name: &str| content.trade(name).is_some_and(|t| roster.count(t) > 0);
        Keepers { shepherd: has("shepherd"), butcher: has("butcher"), leatherworker: has("leatherworker") }
    }
}

/// A town's herds: how many of each kind (by place in [`Content::animals`]), the feed each has eaten less than its
/// share, which kinds have been seeded, how many pastures are fenced, and when it was last tended.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Herds {
    pub count: Vec<f64>,
    pub owed: Vec<f64>,
    pub seeded: Vec<bool>,
    pub pastures: u32,
    pub last: u64,
}

impl Herds {
    pub fn count_of(&self, kind: usize) -> f64 {
        self.count.get(kind).copied().unwrap_or(0.0)
    }

    fn fit(&mut self, kinds: usize) {
        self.count.resize(kinds, 0.0);
        self.owed.resize(kinds, 0.0);
        self.seeded.resize(kinds, false);
    }

    /// What `pastures` hold, at least one's room (`Husbandry.capacity`).
    pub fn capacity(&self, content: &Content) -> f64 {
        content.dials.pasture_animals as f64 * self.pastures.max(1) as f64
    }
}

fn kind_named(content: &Content, name: &str) -> Option<usize> {
    content.animals.iter().position(|a| a.name == name)
}

/// The town's first Shepherd brings 3 sheep, its first Leatherworker 2 cows, its first Butcher 2 pigs and 2 chickens.
/// Once only (`Husbandry.seed`).
pub fn seed(content: &Content, herds: &mut Herds, keepers: Keepers) {
    herds.fit(content.animals.len());
    let (sheep, cow, pig, chicken) = (kind_named(content, "sheep"), kind_named(content, "cow"),
        kind_named(content, "pig"), kind_named(content, "chicken"));
    let give = |herds: &mut Herds, kind: Option<usize>, n: f64| {
        if let Some(k) = kind {
            herds.count[k] = n;
            herds.seeded[k] = true;
        }
    };
    let seeded = |herds: &Herds, kind: Option<usize>| kind.is_some_and(|k| herds.seeded[k]);
    if keepers.shepherd && sheep.is_some() && !seeded(herds, sheep) {
        give(herds, sheep, 3.0);
    }
    if keepers.leatherworker && cow.is_some() && !seeded(herds, cow) {
        give(herds, cow, 2.0);
    }
    if keepers.butcher && (pig.is_some() || chicken.is_some()) && !seeded(herds, pig) && !seeded(herds, chicken) {
        give(herds, pig, 2.0);
        give(herds, chicken, 2.0);
    }
}

/// What the herds eat a day, by good (the first of each kind's feed): the village asks its farmers for it
/// (`Husbandry.feedDemand`).
pub fn feed_demand(content: &Content, herds: &Herds) -> Vec<f64> {
    let mut out = vec![0.0; content.goods.len()];
    for (k, a) in content.animals.iter().enumerate() {
        out[a.feed[0]] += herds.count_of(k) * a.feed_per_day;
    }
    out
}

/// Whole units in `x`, forgiving the dust floats leave.
fn whole(x: f64) -> f64 {
    (x + 1e-9).floor()
}

/// Tends the herds for `days`: they eat from the warehouse, breed into the room their pastures have or dwindle, are
/// shorn and culled (`Husbandry.tend`). Feed is eaten whole: `owed` carries what each kind has eaten less than its
/// share, between tendings (a herd tended many times a day eats no more for it). A Butcher (a Leatherworker its cows)
/// culls only once a kind exceeds its share of the room the cull leaves, so a culled herd grows to its pastures first.
/// Feed is `used` and wool, meat and leather `made` in the ledger, straight into stock.
pub fn tend(content: &Content, herds: &mut Herds, keepers: Keepers, days: f64, stock: &mut Stock, ledger: &mut Ledger) {
    for (g, n) in tend_for(content, herds, keepers, days, stock, ledger) {
        stock.0[g] += n;
        ledger.made[g] += n;
    }
}

/// [`tend`], the products returned (wool, meat and leather, in the order made) rather than put in stock: what the mod
/// adds to `produce`, owed to the warehouse with the farms' harvest. Feed is taken from stock and `used`.
pub fn tend_for(content: &Content, herds: &mut Herds, keepers: Keepers, days: f64, stock: &mut Stock,
        ledger: &mut Ledger) -> Vec<(GoodId, f64)> {
    let mut produce = Vec::new();
    if days <= 0.0 {
        return produce;
    }
    let d = &content.dials;
    herds.fit(content.animals.len());
    let capacity = herds.capacity(content);
    let mut total: f64 = herds.count.iter().sum();
    let kept = herds.count.iter().filter(|&&c| c >= 1.0).count();
    // Each kind's share of the room the cull leaves: the herd fills that much before it's culled.
    let keep = d.breeding_stock.max(d.cull_share * capacity / kept.max(1) as f64);
    for (k, a) in content.animals.iter().enumerate() {
        let mut count = herds.count[k];
        if count <= 0.0 {
            herds.owed[k] = 0.0;
            continue;
        }
        let need = count * a.feed_per_day * days;
        let mut due = herds.owed[k] + need;
        let wanted = whole(due);
        let mut eaten = 0.0;
        for &g in &a.feed {
            if eaten >= wanted {
                break;
            }
            let take = (wanted - eaten).min(whole(stock.0[g])).max(0.0);
            stock.0[g] -= take;
            ledger.used[g] += take;
            eaten += take;
        }
        due -= eaten;
        if due < 1.0 {
            let room = (capacity - total).max(0.0);
            let born = room.min(count * d.breed_per_day * days);
            count += born;
            total += born;
        } else {
            // Short of feed: it dwindles by how much of this tending's share went uneaten, and doesn't eat it back.
            let fed = (1.0 - (due - 1.0) / need).max(0.0);
            let lost = count * d.starve_per_day * (1.0 - fed) * days;
            count -= lost;
            total -= lost;
            due = due.min(1.0);
        }
        // (Eaten whole, forgiving float dust, can leave a hair below nought owed: none owed.)
        herds.owed[k] = due.max(0.0);
        if let (Some(wool), true) = (a.wool, keepers.shepherd) {
            produce.push((wool, count * d.wool_per_sheep_day * days));
        }
        // A Butcher culls any herd for meat; a Leatherworker culls cows for their hides (and beef).
        let culls = keepers.butcher || a.name == "cow" && keepers.leatherworker;
        if culls && count > keep {
            let surplus = count - keep;
            let slaughtered = surplus.min(surplus * d.slaughter_per_day * days);
            count -= slaughtered;
            total -= slaughtered;
            if let Some(meat) = a.meat {
                produce.push((meat, slaughtered * a.meat_each));
            }
            if let Some(hide) = a.hide {
                produce.push((hide, slaughtered * LEATHER_PER_COW));
            }
        }
        herds.count[k] = count.max(0.0);
    }
    produce
}

/// Every town: its herds tended for the time since (at most [`MAX_DAYS`], as `Territory.touch` caps it), then a new
/// Shepherd, Butcher or Leatherworker on its roster seeds its kinds (a town with no roster has no keepers, and its
/// herds are still fed, grown and starved). Seeding follows tending so new animals aren't tended for time before they
/// came. Where the town's farm is set up, the products are owed with its harvest (`Farm.pending`, handed over in Pay
/// as whole items) and the herds' feed is the farm's to grow (`Farm.feed`); otherwise they go straight into stock.
#[allow(clippy::type_complexity)]
pub fn tend_towns(clock: Res<Clock>, content: Res<Content>, mut ledger: ResMut<Ledger>,
        mut towns: Query<(&Town, &mut Herds, &mut Stock, Option<&Roster>, Option<&mut Farm>)>) {
    if content.animals.is_empty() {
        return;
    }
    // In town order, so the ledger's sums come out the same however the archetypes lie.
    let mut order: Vec<_> = towns.iter_mut().collect();
    order.sort_by_key(|(town, ..)| town.id);
    for (_, mut herds, mut stock, roster, farm) in order {
        let days = MAX_DAYS.min(clock.now.saturating_sub(herds.last) as f64 / content.day_ticks as f64);
        herds.last = clock.now;
        let keepers = roster.map(|r| Keepers::of(&content, r)).unwrap_or_default();
        let produce = tend_for(&content, &mut herds, keepers, days, &mut stock, &mut ledger);
        seed(&content, &mut herds, keepers);
        match farm.filter(|f| f.set_up) {
            Some(mut farm) => {
                for (g, n) in produce {
                    *farm.pending.entry(g).or_insert(0.0) += n;
                }
                let feed: BTreeMap<GoodId, f64> = feed_demand(&content, &herds).into_iter().enumerate()
                    .filter(|&(_, n)| n > 0.0).collect();
                if farm.feed != feed {
                    farm.feed = feed;
                }
            }
            None => {
                for (g, n) in produce {
                    stock.0[g] += n;
                    ledger.made[g] += n;
                }
            }
        }
    }
}

impl Society {
    /// Sets the mod's four kinds again from the goods the host has now (after it added goods).
    pub fn rebuild_animals(&mut self) {
        let list = mod_animals(self.content());
        self.content_mut().animals = list;
    }

    /// A town's herds as they are now.
    pub fn herds(&self, town: usize) -> Herds {
        self.world.get::<Herds>(self.towns[town]).cloned().unwrap_or_default()
    }

    /// How many pastures a town has fenced (a herd not yet fenced in has the room of one).
    pub fn set_pastures(&mut self, town: usize, pastures: u32) {
        self.world.get_mut::<Herds>(self.towns[town]).expect("a town has its herds").pastures = pastures;
    }

    /// Puts `count` of a kind (by name) in a town's herd, as a host that already has animals would.
    pub fn set_herd(&mut self, town: usize, kind: &str, count: f64) -> bool {
        let Some(k) = kind_named(self.content(), kind) else { return false };
        let n = self.content().animals.len();
        let mut herds = self.world.get_mut::<Herds>(self.towns[town]).expect("a town has its herds");
        herds.fit(n);
        herds.count[k] = count.max(0.0);
        herds.seeded[k] = true;
        true
    }

    /// What a town's herds eat a day, by good: the farms' demand for feed.
    pub fn feed_demand(&self, town: usize) -> Vec<f64> {
        feed_demand(self.content(), &self.herds(town))
    }
}
