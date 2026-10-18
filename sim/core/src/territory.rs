//! Whose land is whose, and what holding it costs: a town claims its home hex at its first count and grows by culture,
//! out to two rings; a held hex stays with its owner; posts are held hexes with a guard, who eats and is paid or
//! deserts; hands without a trade stand watch as militia for their food; wagons pay tolls to the posted land they cross.
//!
//! Ported from the mod:
//! - `territory/Territory.java`: `claim` (707), `afterCensus` (727: claim the home hex, add culture for the days since
//!   the last count, `grow`), `grow(GlobalPos, Hex)` (833: spend culture on the unclaimed neighbour of a held hex, within
//!   `MAX_RINGS` of home, nearest the bell, then lowest q, then r), `nextHexCost` (232: `CULTURE_PER_HEX` times the
//!   hexes held, at least 1), `cultureFactor` (1734), `tollRate` (1787), `postOwner` (1797), `setPosts` (1806),
//!   `setGuards` (1816), `oweToll` (1453), `upkeepGuards` (1865: food first, then pay; unpaid guards on tolling posts
//!   wait up to `GUARD_CREDIT_DAYS`; the guards posted first stay longest).
//! - `settlement/Census.java` `value` (24): free villagers count 1, confined ones `CONFINED_WEIGHT` (0.1).
//! - `caravan/Caravans.java` `payTolls` (1912) and `ends` (2040): per owner crossed, in path order, the route's own two
//!   towns excepted, the toll is `min(carried, floor(carried * rate + fee))` when the owner has a post on the path.
//! - `territory/Labour.java` `hands` (116): the idle hands stand watch as militia, for food and no pay.
//!
//! **Where the core differs.** The mod counts a town's culture at each census and its guards' upkeep at each touch of
//! its desk; the core does both once a day (a review, a pay), the guards' food in whole items with the surplus
//! credited to the next day. The bell's position isn't kept, only its hex: growth picks the unclaimed neighbour
//! nearest the home hex's centre. Wall, library and caravanserai culture comes in as a daily figure the host sets
//! (`set_culture_extras`). The mod's `tollIncomeYesterday` is the tolls taken since the day before last's pay. Tolls are
//! reckoned in spurs (the coins a wagon carries at the mint's rate, the fee in spurs, the toll floored in spurs, as in
//! the mod) and paid out of a wagon's purse at once (the mod owes them until the next touch).

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::hex::Hex;
use crate::messages::TownId;
use crate::society::Society;

/// What a confined villager counts for in a town's culture (`Census.CONFINED_WEIGHT`).
pub const CONFINED_WEIGHT: f64 = 0.1;
/// The most a town's toll rate can be (`Territory.MAX_TOLL_RATE`).
pub const MAX_TOLL_RATE: f64 = 0.25;
/// At most this many days of upkeep are counted at once (`Territory.MAX_DAYS`).
pub const MAX_DAYS: f64 = 60.0;
/// Culture a day a wall, library and caravanserai each give (`Works.WALL_CULTURE_PER_DAY`,
/// `Buildings.LIBRARY_CULTURE_PER_DAY`, `Buildings.CARAVANSERAI_CULTURE_PER_DAY`).
pub const WALL_CULTURE: f64 = 2.0;
pub const LIBRARY_CULTURE: f64 = 3.0;
pub const CARAVANSERAI_CULTURE: f64 = 1.0;

/// Culture for a mood: a quarter more per point, between a quarter and double (`Territory.cultureFactor`).
pub fn culture_factor(mood: f64) -> f64 {
    (1.0 + 0.25 * mood).clamp(0.25, 2.0)
}

/// What a town holds: its hexes in the order it claimed them, its culture, its posts (held hexes with a guard, in the
/// order posted), its toll, its guards' upkeep.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Holding {
    pub hexes: Vec<Hex>,
    pub culture: f64,
    /// Culture a day beyond its people's: walls, a library, a caravanserai (set by the host).
    pub extra_culture: f64,
    /// Villagers the census counts as confined (they count for a tenth).
    pub confined: u32,
    /// When it was last reviewed (culture counted), in ticks.
    pub reviewed: Option<u64>,
    pub posts: Vec<Hex>,
    /// Its toll as a share of the coins a wagon carries; the default dial if never set.
    pub toll_rate: Option<f64>,
    /// A handling fee a wagon pays at its posts whatever it carries, in spurs.
    pub passage_fee: f64,
    /// Guards who deserted, in all.
    pub deserted: u32,
    /// Posts held by idle hands this pay, unpaid.
    pub militia: u32,
    /// When the guards were last fed and paid.
    pub upkept: Option<u64>,
    /// Food points and coins owed the guards (below 0: credit): claims accruing each look, fractions carried, paid in
    /// whole points and whole emeralds (the money moved in thousandths, `fixed.rs`).
    pub food_owed: f64,
    pub pay_owed: f64,
    /// Tolls taken since the last pay, and in the period before it.
    pub toll_today: crate::fixed::Fixed,
    pub toll_before: crate::fixed::Fixed,
    pub tolls_taken: crate::fixed::Fixed,
    /// Chokepoints (`chokepoints.rs`): what the town has seen of the routes crossing its land, in the order first
    /// seen; when it last reviewed its tolls; the plan it settled on; the coins carried across its land since the last
    /// review and a day over the last period, in spurs.
    pub crossings: Vec<crate::chokepoints::Crossing>,
    pub last_toll_review: Option<u64>,
    pub toll_plan: crate::chokepoints::TollPlan,
    pub traffic_since: f64,
    pub traffic_per_day: f64,
    /// Shares of its guards armed (0 to 1) and armoured (the chestplates' kit share: to 1.5 with enchanted plates, as the
    /// mod's, so an armoured toll's cap may pass `ARMOURED_TOLL_RATE`): armed ones ask more of a detour, armoured ones a
    /// higher toll.
    /// Set at each look from the kit in the warehouse (`kit.rs`, `Smithing.armed` and the armour share); a host with
    /// none of the guards' kit goods sets them itself (`Society::set_guard_kit`).
    pub armed: f64,
    pub armoured: f64,
    /// Share of its hunters (Fletchers) with a bow (`Smithing.hunting`, `kit.rs`).
    pub hunting: f64,
    /// Its mine, once a Toolsmith has dug one (`Territory.mine`), and whether it is sunk deep.
    pub mine: Option<crate::kit::Mine>,
}

/// Whose every claimed hex is, and what each town holds.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Territory {
    pub owners: BTreeMap<Hex, TownId>,
    pub holdings: BTreeMap<TownId, Holding>,
}

impl Territory {
    pub fn owner(&self, hex: Hex) -> Option<TownId> {
        self.owners.get(&hex).copied()
    }

    pub fn holding(&self, town: TownId) -> Option<&Holding> {
        self.holdings.get(&town)
    }

    /// Claims a hex for a town if nobody holds it (`Territory.claim`).
    pub fn claim(&mut self, town: TownId, hex: Hex) -> bool {
        if self.owners.contains_key(&hex) {
            return false;
        }
        self.owners.insert(hex, town);
        self.holdings.entry(town).or_default().hexes.push(hex);
        true
    }

    /// What the next hex costs a town in culture (`Territory.nextHexCost`).
    pub fn next_hex_cost(&self, town: TownId, culture_per_hex: f64) -> f64 {
        culture_per_hex * self.holdings.get(&town).map_or(1, |h| h.hexes.len().max(1)) as f64
    }

    /// A town's count (`Territory.afterCensus`): claims its home hex if nobody holds it; if home is its own, adds
    /// culture for `days` (its free villagers 1 each, confined ones a tenth, its works' daily figure) times its mood's
    /// factor, and grows.
    #[allow(clippy::too_many_arguments)]
    pub fn count(&mut self, town: TownId, home: Hex, villagers: f64, mood: f64, days: f64, now: u64,
            culture_per_hex: f64, rings: i32) {
        self.claim(town, home);
        self.holdings.entry(town).or_default().reviewed = Some(now);
        if self.owner(home) != Some(town) {
            return;
        }
        let h = self.holdings.get_mut(&town).unwrap();
        let confined = (h.confined as f64).min(villagers);
        let value = (villagers - confined) + CONFINED_WEIGHT * confined;
        h.culture += (value + h.extra_culture) * days * culture_factor(mood);
        self.grow(town, home, culture_per_hex, rings);
    }

    /// Spends a town's culture on the nearest unclaimed neighbouring hexes, one at a time (`Territory.grow`): within
    /// `rings` of `home`, nearest home's centre, then lowest q, then lowest r.
    pub fn grow(&mut self, town: TownId, home: Hex, culture_per_hex: f64, rings: i32) {
        loop {
            let cost = self.next_hex_cost(town, culture_per_hex);
            let Some(h) = self.holdings.get(&town) else { return };
            if h.culture < cost {
                return;
            }
            let next = h.hexes.iter().flat_map(|x| x.neighbors())
                .filter(|x| x.distance(home) <= rings && !self.owners.contains_key(x))
                .min_by(|a, b| {
                    let near = |x: &Hex| crate::fixed::hypot(x.center_x() - home.center_x(), x.center_z() - home.center_z());
                    near(a).total_cmp(&near(b)).then(a.q.cmp(&b.q)).then(a.r.cmp(&b.r))
                });
            let Some(next) = next else { return };
            self.claim(town, next);
            self.holdings.get_mut(&town).unwrap().culture -= cost;
        }
    }

    /// The toll posts as a wagon's planner sees them (`RoutePlanner.postToll` over `Territory.postOwner`): each town's
    /// posts on hexes it holds, at its rate (the default until it sets one) and its passage fee.
    pub fn toll_posts(&self, default_rate: f64) -> crate::map::Posts {
        let mut posts = crate::map::Posts::new();
        for (&town, h) in &self.holdings {
            for &hex in &h.posts {
                if self.owner(hex) == Some(town) {
                    posts.insert(hex, crate::map::Post { owner: town, rate: h.toll_rate.unwrap_or(default_rate), fee: h.passage_fee });
                }
            }
        }
        posts
    }

    /// The toll each owner takes of a wagon carrying `carried` spurs over `path`, in the order it crosses their land
    /// (`Caravans.payTolls`): the route's own `ends` take none; an owner is paid once, if one of its posts is on the
    /// path, `min(carried, floor(carried * rate + fee))` of what's left after the owners before it.
    pub fn tolls_due(&self, ends: &[TownId], path: &[Hex], carried: f64, default_rate: f64) -> Vec<(TownId, f64)> {
        let mut crossed: Vec<(TownId, Vec<Hex>)> = Vec::new();
        for &hex in path {
            let Some(owner) = self.owner(hex).filter(|o| !ends.contains(o)) else { continue };
            match crossed.iter_mut().find(|(o, _)| *o == owner) {
                Some((_, hexes)) => hexes.push(hex),
                None => crossed.push((owner, vec![hex])),
            }
        }
        let mut left = carried;
        let mut out = Vec::new();
        for (owner, hexes) in crossed {
            let Some(h) = self.holdings.get(&owner) else { continue };
            if !hexes.iter().any(|x| h.posts.contains(x)) {
                continue;
            }
            let rate = h.toll_rate.unwrap_or(default_rate);
            let fee = if h.posts.is_empty() { 0.0 } else { h.passage_fee };
            let toll = left.min((left * rate + fee).floor());
            if toll > 0.0 {
                out.push((owner, toll));
                left -= toll;
            }
        }
        out
    }
}

/// Host calls.
impl Society {
    /// The town claims the hex it stands in if nobody holds it. False if it isn't placed or the hex is taken. If it
    /// was never counted, this is its first count: a day's culture, and the hexes it buys (`Territory.afterCensus`).
    pub fn claim(&mut self, town: TownId) -> bool {
        let Some(home) = self.town_hex(town) else { return false };
        let now = self.now();
        let (villagers, mood) = {
            let p = self.world.get::<crate::state::People>(self.towns[town]).expect("a town");
            (p.villagers, p.mood)
        };
        let (per_hex, rings) = (self.content().dials.culture_per_hex, self.content().dials.max_rings as i32);
        let mut t = self.world.resource_mut::<Territory>();
        let claimed = t.claim(town, home);
        if t.holdings.get(&town).and_then(|h| h.reviewed).is_none() {
            t.count(town, home, villagers, mood, 1.0, now, per_hex, rings);
        }
        claimed
    }

    /// The town claims a given hex if nobody holds it (a host's grant, a rival's land).
    pub fn claim_hex(&mut self, town: TownId, hex: Hex) -> bool {
        self.world.resource_mut::<Territory>().claim(town, hex)
    }

    /// Who holds a hex.
    pub fn hex_owner(&self, hex: Hex) -> Option<TownId> {
        self.world.resource::<Territory>().owner(hex)
    }

    pub fn territory(&self) -> &Territory {
        self.world.resource::<Territory>()
    }

    /// What a town holds (nothing, if it never has).
    pub fn holding(&self, town: TownId) -> Holding {
        self.territory().holding(town).cloned().unwrap_or_default()
    }

    /// The town's toll posts, one guard each (`Territory.setPosts`). The guards start eating from now.
    pub fn set_posts(&mut self, town: TownId, hexes: &[Hex]) {
        let now = self.now();
        let mut t = self.world.resource_mut::<Territory>();
        let h = t.holdings.entry(town).or_default();
        if h.posts.is_empty() {
            h.upkept = Some(now);
        }
        let mut posts: Vec<Hex> = Vec::new();
        for &x in hexes {
            if !posts.contains(&x) {
                posts.push(x);
            }
        }
        h.posts = posts;
    }

    /// Guards on the first `n` of the town's hexes by q, then r (`Territory.setGuards` over `hexes`).
    pub fn set_guards(&mut self, town: TownId, n: usize) {
        let mut hexes = self.holding(town).hexes;
        hexes.sort_by_key(|x| (x.q, x.r));
        hexes.truncate(n);
        self.set_posts(town, &hexes);
    }

    /// The guards a town has: one a post.
    pub fn guards(&self, town: TownId) -> usize {
        self.holding(town).posts.len()
    }

    /// Guards who have deserted, in all.
    pub fn deserted(&self, town: TownId) -> u32 {
        self.holding(town).deserted
    }

    /// Posts held by idle hands, as of the last pay.
    pub fn militia(&self, town: TownId) -> u32 {
        self.holding(town).militia
    }

    /// Sets the town's toll, a share of the coins a wagon carries (0 to 25%).
    pub fn set_toll(&mut self, town: TownId, rate: f64) {
        self.world.resource_mut::<Territory>().holdings.entry(town).or_default().toll_rate = Some(rate.clamp(0.0, MAX_TOLL_RATE));
    }

    /// The town's toll rate.
    pub fn toll_rate(&self, town: TownId) -> f64 {
        let default = self.content().dials.default_toll_rate;
        self.holding(town).toll_rate.unwrap_or(default)
    }

    /// Sets the town's handling fee at its posts, in spurs.
    pub fn set_passage_fee(&mut self, town: TownId, fee: f64) {
        self.world.resource_mut::<Territory>().holdings.entry(town).or_default().passage_fee = fee.max(0.0);
    }

    /// How many of the town's villagers the census counts as confined.
    pub fn set_confined(&mut self, town: TownId, confined: u32) {
        self.world.resource_mut::<Territory>().holdings.entry(town).or_default().confined = confined;
    }

    /// Culture a day the host's own works add beyond its people's and beyond the core's works (`works.rs`: a wall 2, a
    /// library 3, a caravanserai 1), which stay counted.
    pub fn set_culture_extras(&mut self, town: TownId, per_day: f64) {
        let works = self.towns.get(town).and_then(|&e| self.world.get::<crate::works::Works>(e))
            .map_or(0.0, |w| w.culture_applied);
        self.world.resource_mut::<Territory>().holdings.entry(town).or_default().extra_culture = per_day.max(0.0) + works;
    }

    /// The tolls, in spurs, a wagon carrying `carried` spurs over `path` between two towns would pay.
    pub fn tolls_for(&self, from: TownId, to: TownId, path: &[Hex], carried: f64) -> Vec<(TownId, f64)> {
        self.territory().tolls_due(&[from, to], path, carried, self.content().dials.default_toll_rate)
    }
}
