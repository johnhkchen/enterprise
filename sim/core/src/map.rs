//! The ground caravans cross, hex by hex, and the cheapest way over it.
//!
//! Ported from the mod:
//! - `territory/Terrain.java`: `crossingFactor` (water and relief multiply; `IMPASSABLE`, `RELIEF_PER_FACTOR`,
//!   `CLIFF`). The mod surveys chunks for `waterShare` and `relief`; here the host sets them per hex (`Ground`), and a
//!   hex nobody set is dry and flat (the mod's guess for an unsurveyed plains chunk).
//! - `caravan/Caravans.java`: `terrain` (a bridged hex that's mostly water is open ground: a bridge carries caravans
//!   over water, not over a cliff), `plan(..., boat)` and `BOAT_FACTOR` (a Fisherman sails an impassable water hex at
//!   0.8 of open ground), `perHex` (road food both ways across a hex), `TICKS_PER_BLOCK`, `BLOCKS_PER_FOOD`.
//! - `caravan/RoutePlanner.java`: `plan` (A* within `MARGIN` hexes of the straight line; a step costs `perHex` times
//!   the next hex's factor plus its toll; the start and goal are always open ground and never avoided), `postToll`
//!   (a town's toll post costs its rate on the coins carried home plus its passage fee, not again at a post of the
//!   same town next to it, never for a town in `free`).
//! - `territory/Works.java` `bridged`: a finished bridge on the hex (the flag here; building it is `works`).
//!
//! **The heuristic.** The mod's A* guesses the rest of the way as `distance × perHex`, which never overestimates on
//! land (every hex costs at least open ground). By boat a water hex costs 0.8 of open ground, so that guess can
//! overestimate and A* could miss the cheapest way. Here the guess is scaled by the cheapest a hex can cost (0.8 when
//! sailing, else 1), which keeps it consistent (it never drops by more than a step's cost), so the first time the goal
//! leaves the queue its cost is the least.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::hex::{Hex, ACROSS};
use crate::messages::TownId;

/// A hex more than this share under water can't be crossed by a caravan.
pub const IMPASSABLE: f64 = 0.5;
/// Every this many blocks of relief in a hex make it as hard again to cross as open ground.
pub const RELIEF_PER_FACTOR: f64 = 8.0;
/// A hex with this much relief or more (a cliff, a mountain's flank) can't be crossed by a caravan.
pub const CLIFF: f64 = 32.0;
/// Sailing a water hex costs this much a hex of open ground: boats are quick.
pub const BOAT_FACTOR: f64 = 0.8;
/// Hexes either side of the straight line the search may stray.
pub const MARGIN: i32 = 6;
/// Blocks a food point carries a caravan.
pub const BLOCKS_PER_FOOD: f64 = 10.0;
/// Ticks a caravan takes a block: 1,200 blocks in a 24,000-tick day.
pub const TICKS_PER_BLOCK: f64 = 20.0;
/// Blocks a caravan covers in a day.
pub const BLOCKS_PER_DAY: f64 = 24_000.0 / TICKS_PER_BLOCK;
/// What a food point is worth when the host has no bread: the mod's `perHex` rate is 1/30 of an emerald a point.
pub const FOOD_POINT_VALUE: f64 = 1.0 / 30.0;

/// Food points a caravan eats crossing a hex of open ground both ways.
pub fn food_per_hex() -> f64 {
    2.0 * (ACROSS / BLOCKS_PER_FOOD)
}

/// Road food both ways across a hex of open ground, at `point` a food point (`Caravans.perHex`).
pub fn per_hex(point: f64) -> f64 {
    food_per_hex() * point
}

/// Road food both ways across a hex of open ground, in emeralds, at bread's worth a food point (the mod's 1/30 of an
/// emerald when the host has no bread with food points): what `Caravans.perHex` prices a hex at.
pub fn per_hex_at_bread(content: &crate::content::Content) -> f64 {
    let point = content
        .good("bread")
        .map(|b| &content.goods[b])
        .filter(|b| b.food > 0.0)
        .map_or(FOOD_POINT_VALUE, |b| b.value / b.food);
    per_hex(point)
}

/// What a hex's ground gives a town's hands: the mod's biome tags (`Labour.hauls`, `Hunting.game`) as one kind a hex
/// (`kit.rs` holds the gathering tables by kind). A hex nobody described is plains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Land {
    /// Open ground: plains, meadows and the like (`Labour.hauls`' last case).
    #[default]
    Plains,
    /// Forest, swamp, jungle and savanna (the mod gives each its own haul; here they share the forest's logs).
    Woods,
    /// Taiga and snow.
    Taiga,
    /// Desert and badlands.
    Desert,
    /// Beach.
    Shore,
    /// Hills and mountains.
    Hills,
    /// Ocean and river.
    Water,
}

/// A hex's ground: its share under water (0 to 1), its relief in blocks (highest surface less lowest), whether a
/// finished bridge stands on it, and what its land gives (`Land`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    pub water_share: f64,
    pub relief: f64,
    pub bridged: bool,
    pub land: Land,
}

impl Ground {
    /// How much harder the hex is to cross than open ground on foot: 1 on dry, flat land, up to 2 with water to skirt,
    /// as hard again every 8 blocks of relief, infinite when mostly water or a cliff; a bridged hex that's mostly
    /// water is open ground.
    pub fn crossing_factor(&self) -> f64 {
        if self.bridged && self.water_share > IMPASSABLE {
            return 1.0;
        }
        if self.water_share > IMPASSABLE || self.relief >= CLIFF {
            return f64::INFINITY;
        }
        (1.0 + 2.0 * self.water_share) * (1.0 + self.relief / RELIEF_PER_FACTOR)
    }

    /// As [`Ground::crossing_factor`]; `boat`: a hex a caravan can't walk is sailed at [`BOAT_FACTOR`].
    pub fn factor(&self, boat: bool) -> f64 {
        let hard = self.crossing_factor();
        if boat && hard.is_infinite() {
            BOAT_FACTOR
        } else {
            hard
        }
    }
}

/// The ground of every hex the host has described; the rest are dry and flat.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Map {
    pub ground: BTreeMap<Hex, Ground>,
}

/// A town's toll post on a hex: whose, its rate on the coins carried home, and its passage fee.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Post {
    pub owner: TownId,
    pub rate: f64,
    pub fee: f64,
}

/// Toll posts by hex.
pub type Posts = BTreeMap<Hex, Post>;

/// A path, start to goal both included, and what it costs: road food both ways times each hex's factor, plus tolls.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub hexes: Vec<Hex>,
    pub cost: f64,
}

impl Path {
    /// Blocks travelled: a hex's width a step (what the mod's `Route.distance` gives for towns at hex centres: the
    /// straight distance plus a hex's width for each hex a detour adds).
    pub fn blocks(&self) -> f64 {
        (self.hexes.len().saturating_sub(1)) as f64 * ACROSS
    }
}

/// The open set as the mod keeps it (`RoutePlanner.plan`'s `PriorityQueue<Node>`, compared on `f` alone): Java's
/// binary heap in an array, so of entries with the same guess the one that comes out first is whichever the heap's
/// sifting leaves on top (`PriorityQueue.offer`'s `siftUp` stops at a parent no greater; `poll`'s `siftDown` takes
/// the right child only when the left is greater, and stops at a child no less), not the first queued. Equal paths
/// across open ground go the mod's way.
#[derive(Default)]
struct OpenSet(Vec<(f64, Hex)>);

impl OpenSet {
    fn push(&mut self, x: (f64, Hex)) {
        let mut k = self.0.len();
        self.0.push(x);
        while k > 0 {
            let parent = (k - 1) >> 1;
            if x.0.total_cmp(&self.0[parent].0).is_ge() {
                break;
            }
            self.0[k] = self.0[parent];
            k = parent;
        }
        self.0[k] = x;
    }

    fn pop(&mut self) -> Option<(f64, Hex)> {
        let last = self.0.pop()?;
        if self.0.is_empty() {
            return Some(last);
        }
        let top = self.0[0];
        let n = self.0.len();
        let (mut k, half) = (0, n >> 1);
        while k < half {
            let mut child = 2 * k + 1;
            if child + 1 < n && self.0[child].0.total_cmp(&self.0[child + 1].0).is_gt() {
                child += 1;
            }
            if last.0.total_cmp(&self.0[child].0).is_le() {
                break;
            }
            self.0[k] = self.0[child];
            k = child;
        }
        self.0[k] = last;
        Some(top)
    }
}

impl Map {
    /// A hex's ground (dry and flat if never set).
    pub fn ground(&self, hex: Hex) -> Ground {
        self.ground.get(&hex).copied().unwrap_or_default()
    }

    /// Sets a hex's water share (clamped to 0..1) and relief (at least 0), keeping its bridge.
    pub fn set_ground(&mut self, hex: Hex, water_share: f64, relief: f64) {
        let g = self.ground.entry(hex).or_default();
        g.water_share = water_share.clamp(0.0, 1.0);
        g.relief = relief.max(0.0);
    }

    /// What the hex's land gives its town's hands, keeping the rest of its ground.
    pub fn set_land(&mut self, hex: Hex, land: Land) {
        self.ground.entry(hex).or_default().land = land;
    }

    /// What a hex's land gives (plains if never set).
    pub fn land(&self, hex: Hex) -> Land {
        self.ground.get(&hex).map_or(Land::Plains, |g| g.land)
    }

    /// A finished bridge on the hex.
    pub fn bridge(&mut self, hex: Hex) {
        self.ground.entry(hex).or_default().bridged = true;
    }

    pub fn crossing_factor(&self, hex: Hex) -> f64 {
        self.ground(hex).crossing_factor()
    }

    /// The cheapest path from `start` to `goal`, or `None` when there is no way through.
    ///
    /// `per_hex`: road food both ways across a hex of open ground; `value`: the coins the caravan expects to carry home
    /// (tolls are a share of it); `posts`: toll posts; `free`: towns that charge this caravan nothing (the route's own
    /// ends); `boat`: it can sail water; `avoid`: hexes treated as impassable (the start and goal never are).
    #[allow(clippy::too_many_arguments)]
    pub fn plan(&self, start: Hex, goal: Hex, per_hex: f64, value: f64, posts: &Posts, free: &BTreeSet<TownId>,
            boat: bool, avoid: &BTreeSet<Hex>) -> Option<Path> {
        let direct = start.distance(goal);
        // The cheapest a hex can cost, a share of open ground: the heuristic's scale (see the module's note).
        let cheapest = if boat { BOAT_FACTOR.min(1.0) } else { 1.0 };
        let guess = |hex: Hex| hex.distance(goal) as f64 * per_hex * cheapest;
        let mut open = OpenSet::default();
        let mut best: BTreeMap<Hex, f64> = BTreeMap::new();
        let mut came_from: BTreeMap<Hex, Hex> = BTreeMap::new();
        best.insert(start, 0.0);
        open.push((guess(start), start));
        while let Some((f, hex)) = open.pop() {
            if hex == goal {
                let mut hexes = vec![goal];
                let mut at = goal;
                while let Some(&prev) = came_from.get(&at) {
                    hexes.push(prev);
                    at = prev;
                }
                hexes.reverse();
                return Some(Path { hexes, cost: best[&goal] });
            }
            let g = best[&hex];
            if f > g + guess(hex) + 1e-9 {
                continue; // stale entry
            }
            for next in hex.neighbors() {
                if next.distance(start) + next.distance(goal) > direct + 2 * MARGIN {
                    continue;
                }
                let end = next == goal || next == start;
                if !end && avoid.contains(&next) {
                    continue;
                }
                let hard = if end { 1.0 } else { self.ground(next).factor(boat) };
                if hard.is_infinite() {
                    continue;
                }
                let cost = g + per_hex * hard + post_toll(posts, hex, next, free, value);
                if cost < best.get(&next).copied().unwrap_or(f64::MAX) {
                    best.insert(next, cost);
                    came_from.insert(next, hex);
                    open.push((cost + guess(next), next));
                }
            }
        }
        None
    }
}

/// The toll for stepping from `here` into `next`: charged on reaching a town's toll post, not again at a post of the
/// same town next to it, and never by a town in `free` (`RoutePlanner.postToll`).
pub fn post_toll(posts: &Posts, here: Hex, next: Hex, free: &BTreeSet<TownId>, value: f64) -> f64 {
    let Some(post) = posts.get(&next) else { return 0.0 };
    if free.contains(&post.owner) || posts.get(&here).map(|p| p.owner) == Some(post.owner) {
        return 0.0;
    }
    post.rate * value + post.fee
}
