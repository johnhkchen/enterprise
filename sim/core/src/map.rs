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
//! **The climb.** Not in the mod: a wagon climbs gently. The host sends each hex's ground height; a step from one hex
//! to the next is as hard again every [`GRADE_PER_FACTOR`] of grade between them and has no way past [`MAX_GRADE`]
//! ([`Map::climb_factor`]), so routes follow valleys and gentle flanks, and a plateau's middle has no wagon road. A
//! route's own towns are always reached and left (their step only harder going); a hex of unknown height counts no
//! climb.
//!
//! **Carriers.** Not in the mod: what carries a road's goods picks its own way ([`Carrier`], [`Gait`]). A wagon holds
//! the most but takes only gentle ground: no step steeper than [`MAX_GRADE`], no cliff, and no river (a hex the host
//! found cut in two by water: [`Ground::river`]) unless a bridge stands on it. A string of llamas carries less, but
//! climbs twice as steeply, picks its way over rougher ground and fords a river. A boat (a Fisherman's) goes a wagon's
//! way on land and sails what's water.
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
/// Every this many blocks of relief in a hex make it as hard again to cross as open ground. The mod's is 8; here 24,
/// since the climb between hexes is counted too ([`GRADE_PER_FACTOR`]): at 8, a hex of rolling country (17 blocks'
/// relief, the median round seed 8675309's villages) cost three times open ground, and no load paid a road's food.
pub const RELIEF_PER_FACTOR: f64 = 24.0;
/// A hex with this much relief or more (a cliff, a mountain's flank) can't be crossed by a caravan.
pub const CLIFF: f64 = 32.0;
/// A wagon climbs from one hex to the next at most this steeply (the rise between their grounds over the hex's width,
/// 80 blocks): 16 blocks. Steeper, there's no way from the one to the other for a wagon; a valley or a gentler flank
/// may still lead round.
pub const MAX_GRADE: f64 = 0.2;
/// Every this much grade (16 blocks over a hex) makes the step as hard again as open ground: the steepest step is
/// twice as hard.
pub const GRADE_PER_FACTOR: f64 = 0.2;
/// Wagons' wear on a hex fades by half in this many days.
pub const WEAR_HALF_LIFE_DAYS: f64 = 20.0;
/// A hex worn by this many crossings at once (a round trip crosses it twice) is a trail, for good: a desire path the
/// host lays in dirt. A wagon every four days wears one in five or six round trips, three weeks or so.
pub const TRAIL_WEAR: f64 = 8.0;
/// A trail is two blocks wide, and a block wider each time its wear doubles past [`TRAIL_WEAR`], up to four.
pub const TRAIL_WIDTH: u32 = 2;
pub const TRAIL_WIDEST: u32 = 4;
/// A trail takes off this share of what its ground and its climb add to open ground's cost: the way is trodden level.
pub const TRAIL_EASE: f64 = 0.5;
/// Sailing a water hex costs this much a hex of open ground: boats are quick.
pub const BOAT_FACTOR: f64 = 0.8;
/// A ship sails a hex of water at this much a hex of open ground: the wind and the current do a team's work, so its
/// crew's food is the road's whole cost (John: "Are water trade routes ready yet?").
pub const SHIP_FACTOR: f64 = 0.5;
/// A hex this share under water or more, or cut by a river, is water a ship sails: a coast's hexes, a lake's, a
/// river's (a quarter of 80 blocks across is a wide river's run through it).
pub const SAIL_WATER: f64 = 0.25;
/// A ship sails this many blocks to a wagon's one: its road's blocks, and so its days and its crew's food, are the
/// way's over this (John: "boat transit can win, because it could require less food or can carry more").
pub const SHIP_PACE: f64 = 2.0;
/// A ship's hold carries this many wagons' loads (slots, weight and pallets), and it has no team to feed.
pub const SHIP_HOLD: u32 = 3;
/// A llama climbs from one hex to the next at most this steeply: 32 blocks over a hex, twice a wagon's.
pub const LLAMA_MAX_GRADE: f64 = 0.4;
/// A hex with this much relief or more stops even a llama: a mountain's face.
pub const LLAMA_CLIFF: f64 = 64.0;
/// Fording a river makes its hex this much harder again for llamas: they wade, their packs held high, slowly and with
/// losses. Heavy, so a train takes a bridge a few hexes off its way over the ford (a wagon has no way without one).
pub const FORD_FACTOR: f64 = 4.0;

/// What carries a road's goods, and so the way it can take (see the module's note).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Carrier {
    /// A wagon: the most it carries, on gentle ground, over rivers only by a bridge.
    #[default]
    Wagon,
    /// A string of llamas: less, but steeper, rougher, and through a river's ford.
    Llama,
    /// A wagon's way on land, and water sailed at [`BOAT_FACTOR`].
    Boat,
    /// A ship between two towns' ports (`Society::set_port`): water only ([`Ground::sailable`]), at [`SHIP_FACTOR`].
    Ship,
}

/// How a carrier goes: the steepest step it takes between hexes, the relief that stops it, and how much harder a
/// river makes a hex (infinite: no way without a bridge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gait {
    pub max_grade: f64,
    pub cliff: f64,
    pub ford: f64,
}

impl Carrier {
    pub fn gait(self) -> Gait {
        match self {
            Carrier::Wagon | Carrier::Boat | Carrier::Ship => Gait { max_grade: MAX_GRADE, cliff: CLIFF, ford: f64::INFINITY },
            Carrier::Llama => Gait { max_grade: LLAMA_MAX_GRADE, cliff: LLAMA_CLIFF, ford: FORD_FACTOR },
        }
    }

    /// Blocks it covers to a wagon's one ([`SHIP_PACE`] for a ship).
    pub fn pace(self) -> f64 {
        if self == Carrier::Ship { SHIP_PACE } else { 1.0 }
    }

    /// Its name, as the host and the trace print it.
    pub fn name(self) -> &'static str {
        match self {
            Carrier::Wagon => "wagon",
            Carrier::Llama => "llama",
            Carrier::Boat => "boat",
            Carrier::Ship => "ship",
        }
    }
}
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
    per_hex(point) * content.dials.road_cost_share.max(0.0)
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
/// finished bridge stands on it, what its land gives (`Land`), the height of its ground (none where the host
/// hasn't surveyed it: no climb is counted to or from it), and whether a river cuts it (water parting its land so that
/// a wagon can't go from the one side to the other: no way for one without a bridge).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    pub water_share: f64,
    pub relief: f64,
    pub bridged: bool,
    pub land: Land,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub river: bool,
}

impl Ground {
    /// How much harder the hex is for a wagon to cross than open ground: [`Ground::factor`] for a wagon.
    pub fn crossing_factor(&self) -> f64 {
        self.factor(Carrier::Wagon)
    }

    /// How much harder the hex is for `carrier` to cross than open ground: 1 on dry, flat land, up to 2 with water to
    /// skirt, as hard again every [`RELIEF_PER_FACTOR`] blocks of relief, infinite when mostly water or past its cliff;
    /// a river cutting it is a wagon's end and a llama's ford ([`Gait::ford`]); a bridged hex that's mostly water, or
    /// cut by a river, is open ground. By boat, a hex that can't be walked is sailed at [`BOAT_FACTOR`].
    pub fn factor(&self, carrier: Carrier) -> f64 {
        if carrier == Carrier::Ship {
            return if self.sailable() { SHIP_FACTOR } else { f64::INFINITY };
        }
        if self.bridged && (self.water_share > IMPASSABLE || self.river) {
            return 1.0;
        }
        let gait = carrier.gait();
        let hard = if self.water_share > IMPASSABLE || self.relief >= gait.cliff {
            f64::INFINITY
        } else {
            let ford = if self.river { gait.ford } else { 1.0 };
            (1.0 + 2.0 * self.water_share) * (1.0 + self.relief / RELIEF_PER_FACTOR) * ford
        };
        if carrier == Carrier::Boat && hard.is_infinite() {
            BOAT_FACTOR
        } else {
            hard
        }
    }
}

impl Ground {
    /// Water a ship sails: [`SAIL_WATER`] of it or more under water, or a river cutting it.
    pub fn sailable(&self) -> bool {
        self.water_share >= SAIL_WATER || self.river
    }
}

/// The ground of every hex the host has described (the rest are dry and flat), how worn each is by wagons, and the
/// trails their wear has made.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Map {
    pub ground: BTreeMap<Hex, Ground>,
    #[serde(default)]
    pub wear: BTreeMap<Hex, Wear>,
    #[serde(default)]
    pub trails: BTreeSet<Hex>,
    /// Each trail's width in blocks, as the host was last told (wider as it's worn more).
    #[serde(default)]
    pub widths: BTreeMap<Hex, u32>,
    /// Each road's last departure whose wear is counted (by route), and the day roads were last planned again.
    #[serde(default)]
    pub worn: BTreeMap<usize, u64>,
    #[serde(default)]
    pub replanned: u64,
}

/// A hex's wear by wagons: so many crossings as of a tick, fading by half every [`WEAR_HALF_LIFE_DAYS`] (worked out on
/// touch, never per tick).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Wear {
    pub crossings: f64,
    pub at: u64,
}

impl Wear {
    /// The wear left at `now`.
    pub fn at(&self, now: u64, day_ticks: u64) -> f64 {
        let days = now.saturating_sub(self.at) as f64 / day_ticks.max(1) as f64;
        self.crossings * 0.5f64.powf(days / WEAR_HALF_LIFE_DAYS)
    }
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
    /// Blocks walked hex to hex: a hex's width a step (a settler's walk). A road's length between two towns is
    /// [`road_blocks`], as the mod's `Route.distance` reckons it.
    pub fn blocks(&self) -> f64 {
        (self.hexes.len().saturating_sub(1)) as f64 * ACROSS
    }
}

/// A road's length along `path` between points `from` and `to`, in blocks (`Route.distance`, `Route.java` 191–205):
/// the straight distance between them, rounded up (`straightDistance`), plus a hex's width for each hex the path adds
/// over the straight line of hexes between theirs (`Hex.line`: a detour). Counting hex steps alone would overcharge a
/// road across hex edges: hub to S3 in the network study is 1,201 blocks, not 17 steps of 80.
pub fn road_blocks(path: &Path, from: (f64, f64), to: (f64, f64)) -> f64 {
    let (dx, dz) = (from.0 - to.0, from.1 - to.1);
    let straight = (dx * dx + dz * dz).sqrt().ceil(); // `Math.sqrt`, correctly rounded on every host
    let line = Hex::line(Hex::at(from.0, from.1), Hex::at(to.0, to.1)).len();
    let extra = path.hexes.len().saturating_sub(line);
    straight + (extra as f64 * ACROSS).round()
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

    /// Wagons crossed these hexes (each by `crossings`) at `now`: their wear grows; those worn to [`TRAIL_WEAR`] are
    /// trails from now on, and a trail widens a block each time its wear doubles again (to [`TRAIL_WIDEST`]). Returned
    /// in the order crossed: each hex that became a trail or widened, with its width now.
    pub fn wear(&mut self, hexes: &[Hex], crossings: f64, now: u64, day_ticks: u64) -> Vec<(Hex, u32)> {
        let mut new = Vec::new();
        for &hex in hexes {
            let w = self.wear.entry(hex).or_default();
            *w = Wear { crossings: w.at(now, day_ticks) + crossings, at: now };
            if w.crossings < TRAIL_WEAR {
                continue;
            }
            let width = (TRAIL_WIDTH + (w.crossings / TRAIL_WEAR).log2().floor() as u32).min(TRAIL_WIDEST);
            self.trails.insert(hex);
            if self.widths.get(&hex).is_none_or(|&had| width > had) {
                self.widths.insert(hex, width);
                new.push((hex, width));
            }
        }
        new
    }

    /// Whether wagons have worn a trail across the hex.
    pub fn trail(&self, hex: Hex) -> bool {
        self.trails.contains(&hex)
    }

    /// What a road along `hexes` (start to goal) costs `carrier` in road food over the map as it is now, at `per_hex` a
    /// hex of open ground and no tolls: as [`Map::plan`] prices each step. Infinite where a step has no way now.
    pub fn path_cost(&self, hexes: &[Hex], per_hex: f64, carrier: Carrier) -> f64 {
        let (Some(&start), Some(&goal)) = (hexes.first(), hexes.last()) else { return 0.0 };
        hexes.windows(2).map(|w| per_hex * self.step(start, goal, w[0], w[1], carrier)).sum()
    }

    /// How much a step from `hex` into `next` costs `carrier` a share of open ground, on a road from `start` to `goal`:
    /// the next hex's ground and the climb, a trail easing both; into or out of the road's own ends, only the climb,
    /// and never none (infinite: no way).
    fn step(&self, start: Hex, goal: Hex, hex: Hex, next: Hex, carrier: Carrier) -> f64 {
        let end = next == goal || next == start;
        // A wagon climbs gently, a llama less so: a step too steep has no way (by boat, water is level). Into or out of
        // the route's own ends, only harder going: a town on a hill's edge is reached and left by its steep flank; one
        // in the middle of a plateau isn't.
        let gait = carrier.gait();
        let sailed = carrier == Carrier::Ship
            || carrier == Carrier::Boat && self.ground(next).factor(Carrier::Wagon).is_infinite();
        let climb = if sailed { 1.0 } else { self.climb_factor(hex, next, carrier) };
        let climb = if (end || hex == start) && climb.is_infinite() { 1.0 + gait.max_grade / GRADE_PER_FACTOR } else { climb };
        let hard = if end { climb } else { self.ground(next).factor(carrier) * climb };
        if self.trail(next) && hard.is_finite() && hard > 1.0 {
            1.0 + (hard - 1.0) * (1.0 - TRAIL_EASE)
        } else {
            hard
        }
    }

    /// The height of a hex's ground (none: unknown), keeping the rest of it.
    pub fn set_height(&mut self, hex: Hex, height: Option<f64>) {
        self.ground.entry(hex).or_default().height = height;
    }

    /// How much harder the step from `here` to `next` is for `carrier`'s climb: 1 on the level or where either height
    /// is unknown, as hard again every [`GRADE_PER_FACTOR`] of grade, infinite past its [`Gait::max_grade`].
    pub fn climb_factor(&self, here: Hex, next: Hex, carrier: Carrier) -> f64 {
        let (Some(a), Some(b)) = (self.ground(here).height, self.ground(next).height) else { return 1.0 };
        let grade = (b - a).abs() / ACROSS;
        if grade > carrier.gait().max_grade + 1e-9 {
            f64::INFINITY
        } else {
            1.0 + grade / GRADE_PER_FACTOR
        }
    }

    /// What the hex's land gives its town's hands, keeping the rest of its ground.
    pub fn set_land(&mut self, hex: Hex, land: Land) {
        self.ground.entry(hex).or_default().land = land;
    }

    /// What a hex's land gives (plains if never set).
    pub fn land(&self, hex: Hex) -> Land {
        self.ground.get(&hex).map_or(Land::Plains, |g| g.land)
    }

    /// Whether a river cuts the hex, keeping the rest of its ground.
    pub fn set_river(&mut self, hex: Hex, river: bool) {
        self.ground.entry(hex).or_default().river = river;
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
    /// ends); `carrier`: what goes, and so the way it can take; `avoid`: hexes treated as impassable (the start and goal
    /// never are).
    #[allow(clippy::too_many_arguments)]
    pub fn plan(&self, start: Hex, goal: Hex, per_hex: f64, value: f64, posts: &Posts, free: &BTreeSet<TownId>,
            carrier: Carrier, avoid: &BTreeSet<Hex>) -> Option<Path> {
        let direct = start.distance(goal);
        // The cheapest a hex can cost, a share of open ground: the heuristic's scale (see the module's note).
        let cheapest = match carrier {
            Carrier::Boat => BOAT_FACTOR.min(1.0),
            Carrier::Ship => SHIP_FACTOR.min(1.0),
            _ => 1.0,
        };
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
                let hard = self.step(start, goal, hex, next, carrier);
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

impl Map {
    /// The rivers a wagon's way from `start` to `goal` would want bridged: the cheapest way for a wagon with every
    /// river's hex bridged (no tolls), and each river hex on it with no bridge yet, in the order it's crossed, with the
    /// hexes before and after it (a deck runs between them). Empty when a wagon needs none, or has no way even so.
    pub fn river_crossings(&self, start: Hex, goal: Hex, per_hex: f64) -> Vec<(Hex, Hex, Hex)> {
        let rivers: BTreeSet<Hex> = self.ground.iter().filter(|(_, g)| g.river && !g.bridged).map(|(h, _)| *h).collect();
        if rivers.is_empty() {
            return Vec::new();
        }
        let mut bridged = Map { ground: self.ground.clone(), trails: self.trails.clone(), ..Default::default() };
        for &h in &rivers {
            bridged.bridge(h);
        }
        let Some(path) = bridged.plan(start, goal, per_hex, 0.0, &Posts::new(), &BTreeSet::new(), Carrier::Wagon,
            &BTreeSet::new()) else { return Vec::new() };
        path.hexes.windows(3).filter(|w| rivers.contains(&w[1])).map(|w| (w[1], w[0], w[2])).collect()
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
