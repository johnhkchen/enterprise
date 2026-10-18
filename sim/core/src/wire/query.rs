//! Questions a host asks (`Command::Ask`): what its commands and screens show, read only from these.

use serde::{Deserialize, Serialize};

use super::{Count, GoodNo, GroundView, HandNo, HexAt, MarketLine, Point, RouteLine, TownNo, WorkView};
use crate::fixed::Fixed;

/// A question for the core.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Query {
    /// What a town holds.
    Stock { town: TownNo },
    /// Every trade point and its town.
    Points,
    /// The goods of the society's content, by their place: what a good's number means. A host that loads a save asks
    /// it to map its items to numbers again, since the content is the save's (a pack given at `Create` is kept with
    /// the society), not whatever pack the host would build today.
    Goods,
    /// Every dial: its value now, the value it starts at (what `reset` puts it back to) and what it does.
    Dials,
    /// A town's hands, each by its id with the host's tag: what a host ties its villagers to (after a load too).
    Hands { town: TownNo },
    /// A point's chests as the core takes them to be (what it counted there, with what it moved since).
    Chests { point: Point },
    /// A desk's till as the core takes it to be: the town's treasury, and what its chests and Banker hold.
    Cash { point: Point },
    /// The mint: the spurs an emerald fetches now, and what moves it.
    Mint,
    /// A town's land: the hexes it holds, its culture, its fields' plots and its works.
    Land { town: TownNo },
    /// The ground of these hexes as the core has it (a hex never surveyed is dry, flat plains).
    Ground { hexes: Vec<HexAt> },
    /// A town's lockers: which its Merchant manages, where each stands, and each one's offers with their switches (what
    /// the Merchant's screen shows).
    Lockers { town: TownNo },
    /// A town's routes (from it and to it), the markets it knows and its squad at home: what the Merchant's screen shows
    /// of its caravans, and the route commands.
    Routes { town: TownNo },
}

/// An answer, in the place of its question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Answer {
    /// What a town holds of each good, by the good's place in the content pack, as the core last counted it (at the
    /// last look, or the last happening that touched the town: asking touches nothing).
    Stock { town: TownNo, stock: Vec<f64> },
    /// Every trade point, in the order of their places (dimension, then x, y, z), with its town.
    Points { points: Vec<TownPoint> },
    /// Every good's name, in the order of their numbers (a `GoodNo` is a place here).
    Goods { goods: Vec<String> },
    /// Every dial, in the order the core lists them.
    Dials { dials: Vec<DialView> },
    /// A town's hands in its roster's order (children grown by now), and how many lockers its desk's Merchant may manage
    /// (its level's cap, `Roster::locker_limit`; none without one). Empty for a town that keeps no roster.
    Hands { town: TownNo, lockers: u32, hands: Vec<HandView> },
    /// A point's chests: its town, what the core takes them to hold by good (empty before their first stocktake), their
    /// empty slots at the last, and what they gave up beyond what the town held (`short`, by good; empty if never).
    Chests { point: Point, town: TownNo, held: Vec<f64>, free: u32, short: Vec<f64> },
    /// A desk's till: its town's `treasury`, the whole `emeralds` the core takes its chests to hold, the `coins` spurs
    /// of coin in them and the worth they stand for (`coin_worth`), the Banker's balance in spurs (`None`: no Banker) and
    /// the worth of it (`bank_worth`), and the emeralds its town keeps in reserve.
    Cash { point: Point, town: TownNo, treasury: Fixed, emeralds: u64, coins: u64, coin_worth: Fixed, bank: Option<u64>,
        bank_worth: Fixed, reserve: u64 },
    /// The mint: `rate` spurs an emerald now, the selling `pressure` (emeralds recently sold to it, decayed), the coin
    /// `minted` without backing (decayed) and how far the spur is `debased` (1: not at all).
    Mint { rate: f64, pressure: f64, minted: f64, debased: f64 },
    /// A town's land: the hexes it holds in the order it claimed them (none before its first count), its culture and
    /// what the next hex costs of it, its fields' plots by crop (what's counted in its hexes and the fields it planted
    /// that the host hasn't laid yet), and its works in the order started (bridges, its wall, halls, then the fields it
    /// planted that the host hasn't laid).
    Land { town: TownNo, hexes: Vec<HexAt>, culture: f64, next_hex: f64, plots: Vec<Count>, works: Vec<WorkView> },
    /// The hexes' ground, in the order asked.
    Ground { ground: Vec<GroundView> },
    /// A town's lockers: how many its desk's Merchant may manage (`limit`, none without one), whether its host places
    /// them (`placed`: a hand has one only where `Input::Locker` put it), and every hand with a trade a locker serves
    /// (grown, not the Merchant, not idle), in the roster's order. Empty for a town that keeps no roster.
    Lockers { town: TownNo, limit: u32, placed: bool, lockers: Vec<LockerView> },
    /// A town's routes, from it and to it, in the order they were opened; the markets it knows, nearest first (as its
    /// atlas has them); and the trades of the hands at home who could go with its wagons (`squad`, in the squad's order:
    /// the core's names).
    Routes { town: TownNo, routes: Vec<RouteLine>, markets: Vec<MarketLine>, squad: Vec<String> },
}

/// A hand's locker as the Merchant's screen shows it: the hand, whether the Merchant manages it now and has taken it on
/// as a client, where it stands (`None`: nowhere the host placed, or a town whose host doesn't place lockers), what it
/// was worth to the town a restock at the Merchant's last review (in emeralds), and its offers at the hand's level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LockerView {
    pub hand: HandNo,
    pub managed: bool,
    pub client: bool,
    pub at: Option<Point>,
    pub worth: f64,
    pub offers: Vec<OfferView>,
}

/// One of a hand's offers: its place among the hand's offers (what `Input::Switch` names), the good it buys (`sells`
/// false: `count` of it for `emeralds`) or sells (`emeralds` for `count`), its cost after the Merchant's discount
/// (`price`: goods it takes, or emeralds it charges), its uses a restock and those used since the last, the XP a trade
/// gives the hand, the level it's offered from, vanilla's demand and whether that has pushed its price above its base
/// (`inflated`: the Merchant waits), and whether it's switched on, for a training contract or for what the town's
/// contracts want.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OfferView {
    pub offer: u32,
    pub good: GoodNo,
    pub sells: bool,
    pub count: u32,
    pub emeralds: u32,
    pub price: u32,
    pub uses: u8,
    pub used: u8,
    pub xp: u32,
    pub level: u8,
    pub demand: i32,
    pub inflated: bool,
    pub on: bool,
    pub training: bool,
    pub procured: bool,
}

/// One of a town's hands: its id (stable while it lives, wherever it goes: a place in the list isn't), the host's tag
/// for it (empty if none), its trade by the core's name, level and XP, whether it's a child, away (with a caravan, at
/// war) or the desk's Merchant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HandView {
    pub hand: HandNo,
    pub tag: String,
    pub trade: String,
    pub level: u8,
    pub xp: u32,
    pub baby: bool,
    pub away: bool,
    pub merchant: bool,
}

/// A trade point and the town it's for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TownPoint {
    pub point: Point,
    pub town: TownNo,
}

/// A dial as `/enterprise dial` lists it: its name, its value now and the value it starts the world at, and what it does
/// (for a dial documented with its neighbours, the group's words).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialView {
    pub name: String,
    pub value: f64,
    pub initial: f64,
    pub does: String,
}
