//! Questions a host asks (`Command::Ask`): what its commands and screens show, read only from these.

use serde::{Deserialize, Serialize};

use super::{Count, GoodNo, GroundView, HandNo, HexAt, ManifestLine, MarketLine, PalletView, Point, RouteLine, RouteNo, TownNo, WorkView};
use super::{BookLine, ContractLine, ForTotal, ForwardLine, NameCount, ProjectLine, RentLine, RouteBook, SummaryLine, TrustLine, WarTown, WhyView};
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
    /// A town's pallets: those in its warehouse, and those made out to it elsewhere (on wagons, in a hub's custody, in
    /// another town's warehouse): what the Merchant's desk lists and `/enterprise core pallets` prints.
    Pallets { town: TownNo },
    /// One pallet, by its id: as it is now, or gone (opened, lost).
    Pallet { id: u64 },
    /// The pallets of a desk's town to put in its chests now, whole, with `free` of their slots empty: a chest takes a
    /// pallet whole or not at all, and the core says which (`pallet_host.rs`).
    Stow { point: Point, free: u32 },
    /// A route's wagon's manifest: the pallets aboard, by line, and whether it needs a superbill of its own (two lines or
    /// more).
    Manifest { route: RouteNo },
    /// A town's cash book for the last `days` days, today's included (`/enterprise cashbook`), and what its Accountant
    /// held back from its spenders in them.
    CashBook { town: TownNo, days: u32 },
    /// A market's books summed, with its reasons (`/enterprise books`).
    Books { town: TownNo },
    /// A market's reasons alone (`/enterprise whys`).
    Whys { town: TownNo },
    /// Trust between each pair of towns that have dealt (`/enterprise trust`).
    Trust,
    /// Contracts: all of them, or those a town is a party to (`/enterprise contracts`).
    Contracts { town: Option<TownNo> },
    /// A town's territory report: its land, farms, treasury, mood, people, tolls and posts (`/enterprise territory`).
    Territory { town: TownNo },
    /// The war: the sovereign's phase, treasury and minting, the contracts that supply it, the army and each town's part
    /// (`/enterprise war`, `war tally`).
    War,
    /// Every route's books, for the studies' per-day rows (`dev/experiments/network_study.py`).
    RouteBooks,
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
    /// A town's pallets: those `held` in its warehouse (in arrival order, the order a wagon loads them) and those made out
    /// to it elsewhere (`inbound`, by id); and the registry's tallies: pallets `received` by their consignees, pallets
    /// `lost` whole, and the goods lost off pallets (`lost_goods`, by good: booked as used, reason lost), in the world.
    Pallets { town: TownNo, held: Vec<PalletView>, inbound: Vec<PalletView>, received: u64, lost: u64, lost_goods: Vec<Count> },
    /// A pallet as it is now, or `None`: gone (opened by its consignee, taken apart by its holder, every package lost).
    Pallet { id: u64, pallet: Option<PalletView> },
    /// The pallets to put in a desk's chests now, whole, in the order to put them: those of its town's warehouse whose
    /// packages are nowhere the core knows of (never put in chests, or moved or changed since), in arrival order, each
    /// taken whole if its `items` fit in what's left of the free slots, else passed over. The host puts each in and says
    /// so (`Input::Stowed`).
    Stow { point: Point, pallets: Vec<PalletView> },
    /// A route's wagon's manifest, by line (none while it's home), and whether it needs a superbill: two lines or more,
    /// a document drawn at its home as it leaves (one pallet's bill is its own manifest).
    Manifest { route: RouteNo, lines: Vec<ManifestLine>, superbill: bool },
    /// A town's cash book: its name, the day it is, the lines by day then what for (oldest first), and what the
    /// Accountant held back, by day and what for; then the sums the core makes of them: the money in and out of the
    /// treasury by what for (`totals`, `held_totals`, by name) and all that came in (`received`) and went out (`paid`, a
    /// positive sum): the sums of the per-cause totals above and below nothing.
    CashBook { town: TownNo, name: String, today: u64, lines: Vec<BookLine>, held: Vec<BookLine>, totals: Vec<ForTotal>,
        held_totals: Vec<ForTotal>, received: Fixed, paid: Fixed },
    /// A market's books: its name, the days its books go back, the dealings summed and its reasons (most often first).
    Books { town: TownNo, name: String, keeps: u64, lines: Vec<SummaryLine>, whys: Vec<WhyView> },
    /// A market's reasons, most often first.
    Whys { town: TownNo, name: String, whys: Vec<WhyView> },
    /// Every pair's record, in the order of the pair.
    Trust { records: Vec<TrustLine> },
    /// The contracts asked for, and the rents of the towns asked about (all, or those a town is a party to), in the order of
    /// their numbers.
    Contracts { contracts: Vec<ContractLine>, rents: Vec<RentLine> },
    /// A town's territory report. Money is in emeralds, the toll's figures in spurs where the mod counted spurs (at the
    /// mint's rate now, `rate`). `tolls_owed` is what the town's posts are owed and not yet paid: the core's wagons pay a
    /// toll at once, so it is none.
    Territory { town: TownNo, name: String, rate: f64, hexes: Vec<HexAt>, culture: f64, next_hex: f64, farms: Vec<NameCount>,
        idle: Vec<NameCount>, sold_forward: Vec<ForwardLine>, wage_per_day: f64, labor_share: f64, crop_sales_per_day: f64,
        wages_paid: Fixed, wages_unpaid: Fixed, harvested: f64, deposits: f64, focus: String, purse: Fixed, credit: Fixed,
        freeholds: u32, tightness: f64, treasury: Fixed, reserve: u64, mood: f64, villagers: f64, beds: f64,
        comforts: Vec<NameCount>, yield_factor: f64, culture_factor: f64, bridges: Vec<ProjectLine>, wall: Option<ProjectLine>,
        buildings: Vec<ProjectLine>, hunting: f64, armed: f64, homes_built: u32, born: u32, toll_rate: f64, passage_fee: f64,
        toll_income: f64, tolls_taken: Fixed, tolls_owed: Fixed, guards: u32, posts: Vec<HexAt>, traffic: f64, deserted: u32,
        militia: u32, trades: u64 },
    /// The war. `treasury` is the capital's, in emeralds (`treasury_spurs`: at the mint's rate); `contracts` and `delivered`
    /// those the capital buys goods under; `bills` the emeralds of bills outstanding; `depot` what the capital holds of the
    /// crown's stockpile goods; `unmet` the goods it lacks; `phase` and `theatre` in words; each town's part in `towns`.
    War { phase: String, theatre: String, days: f64, capital: Option<TownNo>, treasury: Fixed, treasury_spurs: f64,
        minted: f64, contracts: u32, delivered: f64, serving: u32, waiting: u32, raised: u32, fallen: u32, bills: f64,
        rate: f64, debasement: f64, levies: u32, hired: u32, awarded: u32, depot: Vec<NameCount>, unmet: Vec<String>,
        towns: Vec<WarTown>, issued: Vec<NameCount>, eaten: f64, heartiness: f64 },
    /// Every route's books (`rate`: the spurs an emerald fetches now), in the order of the routes' numbers.
    RouteBooks { rate: f64, routes: Vec<RouteBook> },
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
