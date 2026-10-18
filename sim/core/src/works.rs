//! A town's public works: bridges, a wall, and halls, built by masons for wages. Ported from the mod:
//!
//! - `territory/Works.java`: `work` (333: masons' wages and blocks for the days since the last touch, one project at a
//!   time; the first call only starts the clock; unpaid masons don't lay), `help` (308: a visiting Mason's blocks),
//!   `underway` (303), `start` (230), `startWall` (259), `startBuilding` (267), `finish` (243), `walled`, `builtOf`,
//!   `consider` (the bridge payback, then a wall), `deck` (599) and `ring` (509: only how many blocks they make),
//!   `MASONS`, `MASON_PAY_PER_DAY`, `BLOCKS_PER_MASON_DAY`, `STONE_PER_HEX_DAY`, `PAYBACK_DAYS`, `WALL_MOOD`,
//!   `WALL_CULTURE_PER_DAY`, `WALL_START_EMERALDS`.
//! - `territory/Buildings.java`: `Kind` (sizes, `partnered`: any one partner trade at the level, or every one for a
//!   counting house), `COUNTING_HOUSE`, `SMITHY`, `STORES`, `has`, `SQUARE_MOOD`, `FLOWER_MOOD`,
//!   `LIBRARY_CULTURE_PER_DAY`, `CARAVANSERAI_CULTURE_PER_DAY`, `EMERALDS_PER_LOG`.
//! - `service/BuildServices.java`: `next` (the first hall a town lacks whose partners it has, when it is content and
//!   holds [`SAVINGS`] times its cost beyond its reserve), `wanted`, `cost`, `work` (what the halls it wants would
//!   cost: the Mason's marbles in the job draw), `hire` (builders for a town without a Mason, up to [`HIRED_SKILL`]),
//!   `start` (materials bought through the Accountant, logs in the warehouse standing in for some), `helpWanted` and
//!   `help` (a visiting Mason's day's work).
//! - `service/MasterServices.java`: `flowersWanted`, `plantFlowers` (a master Farmer's beds on the square).
//! - `territory/Territory.java` `moodTarget` (1587: wall, square, flowers), and its culture count (736: the wall's
//!   share, a library, a caravanserai).
//! - `cargo/Paperwork.java` `settle` and `money/Accountant.java` `free`, `today`: a counting house halves the
//!   stationery a document takes, charges a clerk's 0.02 for one it lacks it for, doubles the days of bills held back
//!   from the free money and widens the budget share by half.
//!
//! **Where the core differs.** The mod's masons are paid at each touch of the desk and the town reviews its toll for a
//! bridge and a wall (and hired builders a hall) at its toll reviews; the core pays once a day and weighs those every
//! `toll_review_days`, the first look only starting the clock, on a clock of its own. A project's blocks
//! are numbers: a bridge's deck and a wall's ring are counted from the same geometry as the mod's ([`deck_size`],
//! [`ring_size`]) and nothing is laid in a world; a hall's site is always found (a stone dock needs the host to say
//! there is a pier: [`Works::dock`]). A town without land holds no stone and starts nothing. The effects that belong
//! to later slices (a temple's double blessings, a smithy's mine and kit, a watchtower's archers, stone docks' catch,
//! the caravanserai's reach, a hub's discount desk) are read off [`Works::built`] by them.

use std::collections::BTreeSet;

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::hex::Hex;

/// A square brings this much mood; with a master Farmer's flower beds, this much more (`Buildings.SQUARE_MOOD`,
/// `FLOWER_MOOD`).
pub const SQUARE_MOOD: f64 = 0.5;
pub const FLOWER_MOOD: f64 = 0.5;
/// A wall whole brings this much mood and this much culture a day, in proportion to the share standing
/// (`Works.WALL_MOOD`, `WALL_CULTURE_PER_DAY`).
pub const WALL_MOOD: f64 = 1.0;
/// A town starts a wall or a hall at this mood or better (`Works.WALL_MOOD_TO_START`).
pub const MOOD_TO_START: f64 = 0.5;
/// Each log in the warehouse saves this much of a hall's materials, in emeralds (`Buildings.EMERALDS_PER_LOG`).
pub const EMERALDS_PER_LOG: f64 = 0.125;
/// Flowers a master Farmer needs for the square's beds (`MasterServices.FLOWERS`) and the Farmer level for it.
pub const FLOWERS: f64 = 8.0;
pub const FLOWER_FARMER_LEVEL: u8 = 5;
/// The wall's radius and height (`Works.WALL_RADIUS`, `WALL_HEIGHT`).
pub const WALL_RADIUS: i32 = 40;
pub const WALL_HEIGHT: u32 = 3;
/// A deck runs this far past the hex's edges (`Works.LANDING`).
const LANDING: f64 = 6.0;
/// At most this many days of work are counted at once (`Works.work`).
pub const MAX_DAYS: f64 = 60.0;
/// Flowers a town's beds can be planted from, by the host's names (`MasterServices.FLOWER_ITEMS`).
pub const FLOWER_ITEMS: [&str; 8] = ["dandelion", "poppy", "blue_orchid", "allium", "azure_bluet", "oxeye_daisy",
    "cornflower", "lily_of_the_valley"];
/// Logs a town might hold, by the host's names (`BuildServices.LOGS`).
pub const LOGS: [&str; 8] = ["oak_log", "spruce_log", "birch_log", "jungle_log", "acacia_log", "dark_oak_log",
    "mangrove_log", "cherry_log"];

/// A kind of hall, in the order a town builds them (`Buildings.KINDS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Kind {
    Square,
    Granary,
    RootCellar,
    CountingHouse,
    Temple,
    Library,
    Smithy,
    Watchtower,
    StoneDocks,
    Caravanserai,
}

/// Every kind, in the order a town builds them.
pub const KINDS: [Kind; 10] = [Kind::Square, Kind::Granary, Kind::RootCellar, Kind::CountingHouse, Kind::Temple,
    Kind::Library, Kind::Smithy, Kind::Watchtower, Kind::StoneDocks, Kind::Caravanserai];

impl Kind {
    /// Its name (`Kind.id`).
    pub fn id(self) -> &'static str {
        match self {
            Kind::Square => "square",
            Kind::Granary => "granary",
            Kind::RootCellar => "root_cellar",
            Kind::CountingHouse => "counting_house",
            Kind::Temple => "temple",
            Kind::Library => "library",
            Kind::Smithy => "smithy",
            Kind::Watchtower => "watchtower",
            Kind::StoneDocks => "stone_docks",
            Kind::Caravanserai => "caravanserai",
        }
    }

    /// Its footprint (a square this many blocks a side) and height (`Kind.size`, `height`).
    pub fn size(self) -> u32 {
        match self {
            Kind::Square | Kind::Caravanserai => 9,
            Kind::Granary | Kind::CountingHouse | Kind::Temple | Kind::Library | Kind::Smithy => 7,
            Kind::RootCellar => 5,
            Kind::Watchtower | Kind::StoneDocks => 3,
        }
    }

    pub fn height(self) -> u32 {
        match self {
            Kind::Square | Kind::StoneDocks => 1,
            Kind::RootCellar => 3,
            Kind::Watchtower => 7,
            _ => 4,
        }
    }

    /// The Mason's level it needs (`Kind.masonLevel`).
    pub fn mason_level(self) -> u8 {
        match self {
            Kind::Square | Kind::Granary | Kind::RootCellar => 2,
            Kind::StoneDocks => 4,
            Kind::Caravanserai => 5,
            _ => 3,
        }
    }

    /// The trades it serves, by name (`Kind.partners`; none for a square).
    pub fn partners(self) -> &'static [&'static str] {
        match self {
            Kind::Square => &[],
            Kind::Granary | Kind::RootCellar => &["farmer"],
            Kind::CountingHouse => &["librarian", "merchant"],
            Kind::Temple => &["cleric"],
            Kind::Library => &["librarian"],
            Kind::Smithy => &["toolsmith", "armorer", "weaponsmith"],
            Kind::Watchtower => &["fletcher"],
            Kind::StoneDocks => &["fisherman"],
            Kind::Caravanserai => &["merchant"],
        }
    }

    /// The level one of its partners must have at home (`Kind.partnerLevel`).
    pub fn partner_level(self) -> u8 {
        match self {
            Kind::Square => 0,
            Kind::Granary | Kind::RootCellar => 2,
            _ => 3,
        }
    }

    /// Its materials, in emeralds (`Kind.emeralds`).
    pub fn emeralds(self) -> f64 {
        match self {
            Kind::Square | Kind::RootCellar | Kind::Watchtower => 8.0,
            Kind::Granary | Kind::Smithy | Kind::StoneDocks => 12.0,
            Kind::CountingHouse | Kind::Temple | Kind::Library => 16.0,
            Kind::Caravanserai => 24.0,
        }
    }

    /// Does it need every one of its partner trades, not just one (`Kind.everyPartner`: the counting house)?
    pub fn every_partner(self) -> bool {
        self == Kind::CountingHouse
    }

    /// Its blocks (`Kind.blocks`): size by size by height.
    pub fn blocks(self) -> u32 {
        self.size() * self.size() * self.height()
    }

    /// Does the town have the trades it serves, at the level: one of them, or (a hall two trades share) every one?
    /// `skills` are the residents' best levels by trade name; `merchant`, the town Merchant's level
    /// (`Kind.partnered`).
    pub fn partnered(self, skills: &std::collections::BTreeMap<String, u8>, merchant: u8) -> bool {
        let has = |trade: &&str| {
            if *trade == crate::roster::MERCHANT {
                merchant >= self.partner_level()
            } else {
                skills.get(*trade).copied().unwrap_or(0) >= self.partner_level()
            }
        };
        let partners = self.partners();
        partners.is_empty() || if self.every_partner() { partners.iter().all(has) } else { partners.iter().any(has) }
    }

    /// The crop store it is, if it is one (`Buildings.STORES`).
    pub fn store(self) -> Option<crate::farms::Store> {
        match self {
            Kind::Granary => Some(crate::farms::Store::Granary),
            Kind::RootCellar => Some(crate::farms::Store::RootCellar),
            _ => None,
        }
    }

    /// Culture a day it brings (`Buildings.LIBRARY_CULTURE_PER_DAY`, `CARAVANSERAI_CULTURE_PER_DAY`).
    pub fn culture(self) -> f64 {
        match self {
            Kind::Library => crate::territory::LIBRARY_CULTURE,
            Kind::Caravanserai => crate::territory::CARAVANSERAI_CULTURE,
            _ => 0.0,
        }
    }
}

/// The blocks of a bridge's deck (`Works.deck(...).size()`): from the hex's centre toward each neighbour it runs
/// between, to half the way there and [`LANDING`] past, every half block, five wide; the cells counted once.
pub fn deck_size(hex: Hex, from: Hex, to: Hex) -> u32 {
    let (cx, cz) = (hex.center_x(), hex.center_z());
    let mut cells: BTreeSet<(i64, i64)> = BTreeSet::new();
    for end in [from, to] {
        let (ex, ez) = (end.center_x() - cx, end.center_z() - cz);
        let length = crate::fixed::hypot(ex, ez);
        let (ux, uz) = (ex / length, ez / length);
        let reach = length / 2.0 + LANDING;
        let mut t = 0.0;
        while t <= reach {
            for w in -2i64..=2 {
                let x = (cx + ux * t - uz * w as f64).floor() as i64;
                let z = (cz + uz * t + ux * w as f64).floor() as i64;
                cells.insert((x, z));
            }
            t += 0.5;
        }
    }
    cells.len() as u32
}

/// The columns of a wall `radius` blocks round the bell, gates three wide at the four points (`Works.ring(...).size()`).
pub fn ring_size(radius: i32) -> u32 {
    let mut n = 0;
    for dx in -radius - 1..=radius + 1 {
        for dz in -radius - 1..=radius + 1 {
            let round = (crate::fixed::hypot(dx as f64, dz as f64) + 0.5).floor() as i32;
            if round == radius && dx.abs() > 1 && dz.abs() > 1 {
                n += 1;
            }
        }
    }
    n
}

/// What a project is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum What {
    /// A bridge on a hex, its deck running between two neighbours.
    Bridge { hex: Hex, from: Hex, to: Hex },
    Wall,
    Hall(Kind),
}

impl What {
    /// The order the mod works its projects in: bridges, then the wall, then the halls (`Works.projectsOf`).
    fn rank(&self) -> u8 {
        match self {
            What::Bridge { .. } => 0,
            What::Wall => 1,
            What::Hall(_) => 2,
        }
    }

    /// Does starting `other` replace this one (the same bridge hex, the wall, the same kind of hall)?
    fn same_as(&self, other: &What) -> bool {
        match (self, other) {
            (What::Bridge { hex: a, .. }, What::Bridge { hex: b, .. }) => a == b,
            (What::Wall, What::Wall) => true,
            (What::Hall(a), What::Hall(b)) => a == b,
            _ => false,
        }
    }
}

/// Something masons build for a town: wages and stone go in, blocks laid come out (`Works.Project`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub what: What,
    pub blocks_needed: u32,
    pub laid: f64,
    /// Emeralds owed the masons, fractions carried: a claim accruing each day, paid in whole emeralds (the money moved in
    /// thousandths, `fixed.rs`).
    pub wage_owed: f64,
    pub wages_paid: crate::fixed::Fixed,
    pub done: bool,
    /// When it was last worked on (the first touch only starts the clock).
    pub started: Option<u64>,
    /// Its effect on the world applied (a bridge marked on the map, a store set up, a counting house's flags).
    pub applied: bool,
}

impl Project {
    pub fn new(what: What, blocks_needed: u32) -> Self {
        Project { what, blocks_needed, laid: 0.0, wage_owed: 0.0, wages_paid: crate::fixed::Fixed::ZERO, done: false, started: None, applied: false }
    }

    /// How much of it stands, 0 to 1 (`Project.progress`).
    pub fn progress(&self) -> f64 {
        if self.blocks_needed == 0 { 1.0 } else { (self.laid / self.blocks_needed as f64).min(1.0) }
    }

    /// Lays `blocks`; done when all are (`Works.help`).
    pub fn lay(&mut self, blocks: f64) {
        self.laid = (self.laid + blocks).min(self.blocks_needed as f64);
        if self.laid >= self.blocks_needed as f64 {
            self.done = true;
        }
    }

    /// Finished at once (`Works.finish`).
    pub fn finish(&mut self) {
        self.laid = self.blocks_needed as f64;
        self.done = true;
    }
}

/// A town's public works: its projects in the order they were started, whether a master Farmer has planted the
/// square's flower beds, and whether the host says the town has a pier for stone docks. A component on every town.
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Works {
    pub projects: Vec<Project>,
    pub flowers: bool,
    pub dock: bool,
    /// Culture a day its works have added to the town's holding so far (`Holding.extra_culture` is moved by the change).
    pub culture_applied: f64,
    /// When it last reviewed what to build, and when it last looked for a bridge (at its toll reviews, in the mod).
    pub reviewed: Option<u64>,
    pub bridges_looked: Option<u64>,
}

impl Works {
    /// The project being built now, if any: bridges first, then the wall, then halls (`Works.underway`).
    pub fn underway(&self) -> Option<usize> {
        (0..self.projects.len()).filter(|&i| !self.projects[i].done).min_by_key(|&i| (self.projects[i].what.rank(), i))
    }

    /// Is a project of this town still being built (`Works.busy`)?
    pub fn busy(&self) -> bool {
        self.projects.iter().any(|p| !p.done)
    }

    /// Starts a project; the same bridge, the wall or the same hall started again replaces the old one.
    pub fn start(&mut self, what: What, blocks_needed: u32) {
        let project = Project::new(what, blocks_needed);
        match self.projects.iter().position(|p| p.what.same_as(&what)) {
            Some(i) => self.projects[i] = project,
            None => self.projects.push(project),
        }
    }

    /// The wall, if it was started.
    pub fn wall(&self) -> Option<&Project> {
        self.projects.iter().find(|p| p.what == What::Wall)
    }

    /// How much of the wall stands, 0 to 1 (`Works.walled`).
    pub fn walled(&self) -> f64 {
        self.wall().map_or(0.0, |w| w.progress())
    }

    /// Is a hall of this kind finished (`Works.built`)?
    pub fn built(&self, kind: Kind) -> bool {
        self.projects.iter().any(|p| p.done && p.what == What::Hall(kind))
    }

    /// Is a hall of this kind started or finished (`Works.buildingsOf(...).containsKey`)?
    pub fn has_started(&self, kind: Kind) -> bool {
        self.projects.iter().any(|p| p.what == What::Hall(kind))
    }

    /// The halls finished, in the order they were started (`Works.builtOf`).
    pub fn built_of(&self) -> Vec<Kind> {
        self.projects.iter().filter(|p| p.done).filter_map(|p| match p.what {
            What::Hall(k) => Some(k),
            _ => None,
        }).collect()
    }

    /// Is a bridge on this hex finished (`Works.bridged`)?
    pub fn bridged(&self, hex: Hex) -> bool {
        self.projects.iter().any(|p| p.done && matches!(p.what, What::Bridge { hex: h, .. } if h == hex))
    }

    /// Has a bridge on this hex been started or finished?
    pub fn has_bridge(&self, hex: Hex) -> bool {
        self.projects.iter().any(|p| matches!(p.what, What::Bridge { hex: h, .. } if h == hex))
    }

    /// Where the mood is pushed by the town's works: the wall in proportion to its share standing, a square, and its
    /// flower beds (`Territory.moodTarget`).
    pub fn mood(&self) -> f64 {
        let square = if self.built(Kind::Square) { SQUARE_MOOD + if self.flowers { FLOWER_MOOD } else { 0.0 } } else { 0.0 };
        WALL_MOOD * self.walled() + square
    }

    /// Culture a day beyond the people's: the wall's share, a library, a caravanserai (`Territory.afterCensus`).
    pub fn culture(&self) -> f64 {
        crate::territory::WALL_CULTURE * self.walled() + self.built_of().iter().map(|k| k.culture()).sum::<f64>()
    }

    /// A counting house stands: its clerks keep the papers, and its books widen the budget.
    pub fn counting(&self) -> bool {
        self.built(Kind::CountingHouse)
    }
}

/// Lays masons' work on a town's project for the days since it was last worked (`Works.work`): the wages for the days
/// due, whole emeralds, `treasury` of which is what it can pay; blocks by the stone its land gives (`hexes` held),
/// in proportion to the wages paid. Returns the emeralds paid. The first call only starts the clock.
pub fn work(project: &mut Project, d: &crate::dials::Dials, hexes: usize, now: u64, day_ticks: u64,
        treasury: crate::fixed::Fixed) -> crate::fixed::Fixed {
    let Some(last) = project.started else {
        project.started = Some(now);
        return crate::fixed::Fixed::ZERO;
    };
    let days = ((now - last) as f64 / day_ticks as f64).min(MAX_DAYS);
    project.started = Some(now);
    project.wage_owed += d.masons as f64 * d.mason_pay * days;
    let whole = project.wage_owed.floor();
    let mut share = 1.0;
    let mut paid = crate::fixed::Fixed::ZERO;
    if whole > 0.0 {
        // Whole emeralds owed (fractions carried), as far as the treasury goes.
        let owed = crate::fixed::Fixed::emeralds(whole as i64);
        paid = owed.min(treasury.at_least_zero());
        project.wage_owed -= whole;
        project.wages_paid += paid;
        share = paid.ratio(owed); // unpaid masons don't work, and aren't owed
    }
    let per_day = per_day(d, hexes);
    project.laid += per_day * days * share;
    if project.laid >= project.blocks_needed as f64 {
        project.laid = project.blocks_needed as f64;
        project.done = true;
    }
    paid
}

/// Blocks a day a town's masons lay: their own hands, or the stone its land gives, whichever is less.
pub fn per_day(d: &crate::dials::Dials, hexes: usize) -> f64 {
    (d.masons as f64 * d.blocks_per_mason_day).min(hexes as f64 * d.stone_per_hex_day)
}

/// A hall's builders' wages, in emeralds (`BuildServices.cost`).
pub fn wages_for(d: &crate::dials::Dials, blocks: u32) -> f64 {
    blocks as f64 / (d.masons as f64 * d.blocks_per_mason_day) * d.masons as f64 * d.mason_pay
}

/// What a project's finishing does to the world, once (`Works.finish`, the effects read where they matter): a bridge
/// marks its hex on the map, a granary or root cellar sets up the farm's store.
pub fn apply(works: &mut Works, map: &mut crate::map::Map, mut farm: Option<&mut crate::farms::Farm>) {
    for p in works.projects.iter_mut().filter(|p| p.done && !p.applied) {
        p.applied = true;
        match p.what {
            What::Bridge { hex, .. } => map.bridge(hex),
            What::Hall(kind) => {
                if let (Some(store), Some(farm)) = (kind.store(), farm.as_deref_mut()) {
                    if !farm.stores.contains(&store) {
                        farm.stores.push(store);
                    }
                }
            }
            What::Wall => {}
        }
    }
}

/// The counting house's effect on the Accountant, set on the town's budget and trades (`Accountant.free`, `today`).
pub fn flags(works: &Works, trades: &mut crate::state::Trades, budget: &mut crate::state::Budget) {
    trades.counting_house = works.counting();
    budget.counting_house = works.counting();
}

/// The best level of each of a town's trades among its adults, by name (`Services.residents`).
pub fn skills(content: &crate::content::Content, roster: Option<&crate::roster::Roster>) -> std::collections::BTreeMap<String, u8> {
    let mut out = std::collections::BTreeMap::new();
    if let Some(r) = roster {
        for h in r.hands.iter().filter(|h| !h.is_baby()) {
            if let Some(t) = content.trades.get(h.trade) {
                let best = out.entry(t.name.clone()).or_insert(0u8);
                *best = (*best).max(h.level);
            }
        }
    }
    out
}

/// The best level among a town's Masons at home, if it has a Mason at all (`Crafts.hasOwn`): `Some(None)` when they
/// are all away.
pub fn mason(content: &crate::content::Content, roster: Option<&crate::roster::Roster>) -> Option<Option<u8>> {
    let (r, trade) = (roster?, content.trade("mason")?);
    if r.count(trade) == 0 {
        return None;
    }
    Some(r.hands.iter().filter(|h| h.trade == trade && !h.is_baby() && !h.away).map(|h| h.level).max())
}

/// Logs in a stock, and the materials still to buy for a hall: logs stand in for some of the timber
/// (`BuildServices.logs`, `materials`).
pub fn logs(content: &crate::content::Content, stock: &[f64]) -> f64 {
    LOGS.iter().filter_map(|n| crate::jobs::good_named(content, n)).map(|g| stock[g].floor().max(0.0)).sum()
}

pub fn materials(content: &crate::content::Content, stock: &[f64], kind: Kind) -> f64 {
    (kind.emeralds() - logs(content, stock) * EMERALDS_PER_LOG).max(0.0)
}

/// A hall's materials still to buy and its builders' wages, in emeralds (`BuildServices.cost`).
pub fn cost(content: &crate::content::Content, stock: &[f64], kind: Kind) -> f64 {
    materials(content, stock, kind) + wages_for(&content.dials, kind.blocks())
}

/// Host calls.
impl crate::society::Society {
    /// A town's works as they stand.
    pub fn works(&self, town: crate::messages::TownId) -> Works {
        self.world.get::<Works>(self.towns[town]).cloned().unwrap_or_default()
    }

    /// How much of the town's wall stands, 0 to 1 (`Works.walled`).
    pub fn walled(&self, town: crate::messages::TownId) -> f64 {
        self.works(town).walled()
    }

    /// Is a hall of this kind finished for the town (`Works.built`)?
    pub fn built(&self, town: crate::messages::TownId, kind: Kind) -> bool {
        self.works(town).built(kind)
    }

    /// Where the town's mood is heading (`Territory.moodTarget`): its comforts met and its works, less its size, people
    /// without beds and unpaid wages.
    pub fn mood_target(&self, town: crate::messages::TownId) -> f64 {
        let e = self.world.entity(self.towns[town]);
        crate::people::mood_target(&self.content().dials, e.get::<crate::state::People>().expect("a town"), e.get::<Works>())
    }

    /// The town's works written back, their effects applied (`Works.finish`'s: a bridge on the map, a store on the farm,
    /// the counting house on the budget).
    fn commit_works(&mut self, town: crate::messages::TownId, mut works: Works) {
        let e = self.towns[town];
        let mut map = std::mem::take(&mut *self.world.resource_mut::<crate::map::Map>());
        let mut farm = self.world.get::<crate::farms::Farm>(e).cloned();
        apply(&mut works, &mut map, farm.as_mut());
        *self.world.resource_mut::<crate::map::Map>() = map;
        if let Some(farm) = farm {
            *self.world.get_mut::<crate::farms::Farm>(e).expect("a town") = farm;
        }
        let (mut trades, mut budget) = (self.world.get::<crate::state::Trades>(e).cloned().expect("a town"),
            *self.world.get::<crate::state::Budget>(e).expect("a town"));
        flags(&works, &mut trades, &mut budget);
        let mut town = self.world.entity_mut(e);
        town.insert((works, trades, budget));
    }

    /// Starts a project for the town (`Works.start`, `startWall`, `startBuilding`): a bridge on a hex, its deck between
    /// two neighbours; the wall; a hall. Its blocks are counted from the mod's geometry. The same one started again
    /// begins afresh.
    pub fn start_project(&mut self, town: crate::messages::TownId, what: What) {
        let blocks = match what {
            What::Bridge { hex, from, to } => deck_size(hex, from, to),
            What::Wall => self.content().dials.wall_blocks,
            What::Hall(kind) => kind.blocks(),
        };
        let mut works = self.works(town);
        works.start(what, blocks);
        self.commit_works(town, works);
    }

    /// Finishes a project at once (`Works.finish`: for hosts and commands).
    pub fn finish_project(&mut self, town: crate::messages::TownId, what: What) {
        let mut works = self.works(town);
        if let Some(p) = works.projects.iter_mut().find(|p| p.what == what) {
            p.finish();
        }
        self.commit_works(town, works);
    }

    /// A visiting Mason lays `blocks` on the town's project under way (`Works.help`). Returns whether there was one.
    pub fn help_project(&mut self, town: crate::messages::TownId, blocks: f64) -> bool {
        let mut works = self.works(town);
        let Some(i) = works.underway() else { return false };
        works.projects[i].lay(blocks);
        self.commit_works(town, works);
        true
    }

    /// Does the town want a visiting Mason of `skill` (`BuildServices.helpWanted`): it has no Mason of its own, a
    /// project is under way, and the visitor's market can pay `mason_pay` a level (`budget`, its day's budget).
    pub fn masonry_wanted(&self, town: crate::messages::TownId, skill: u8, budget: f64) -> bool {
        let roster = self.world.get::<crate::roster::Roster>(self.towns[town]);
        mason(self.content(), roster).is_none()
            && self.works(town).busy()
            && budget >= self.content().dials.mason_pay * skill as f64
    }

    /// A visiting Mason of `skill` lays a day's work on the town's project, `blocks_per_mason_day` blocks a level
    /// (`BuildServices.help`). Returns the day's wage the visitor's market owes, `mason_pay` a level.
    pub fn masonry_abroad(&mut self, town: crate::messages::TownId, skill: u8) -> f64 {
        let d = &self.content().dials;
        let (blocks, fee) = (d.blocks_per_mason_day * skill as f64, d.mason_pay * skill as f64);
        self.help_project(town, blocks);
        fee
    }

    /// Whether the town has a pier stone docks could be laid on (the host's: the mod's `Territory.dock`).
    pub fn set_dock(&mut self, town: crate::messages::TownId, dock: bool) {
        self.world.get_mut::<Works>(self.towns[town]).expect("a town").dock = dock;
    }
}
