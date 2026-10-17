//! What each trade does for a town (`docs/decisions.md` "Services", "The metal chain"): its own town, at home, or the
//! town its wagon reaches, abroad. A service is registered by name: the trade that does it, where, what it does in
//! plain words, whether the town served wants it now, and doing it. Each is done by the most skilled of its trade who
//! is there (at home a resident not away; abroad one of the wagon's crew), and skill (level) sets how much.
//!
//! - **At home** (`rules/services.rs`, `Work`, after the workshop): a Cleric blesses its own town (a day at a time),
//!   a Cartographer charts the country around (markets within 800 blocks, every 10 days), and the trades make what the
//!   town wants from its stock ([`Recipe`]s: bookbinding while reading is short, smelting, kit for its wearers...).
//! - **Abroad** (`Services`, once the wagon has sold): each crew trade's services the far end wants and can pay for,
//!   from its money beyond its reserve, into the wagon's purse; each paid service raises the two towns' trust by
//!   `trust_per_service`. A Cleric blesses a town without one (+`blessing_mood` to where its mood heads, 2 days and a
//!   day a level, 2 emeralds), a Farmer brings in a day's harvest from fields no one tends (as many plots as a Farmer
//!   of its level, for the town's farmhands' share of its worth), a Cartographer swaps maps (each town learns the
//!   markets the other knows; the one that learns more pays 0.25 a market) and surveys the road (the towns within two
//!   hexes of it are heard of at home), a Shepherd drives up to 8 of home's animals to sell where the pasture is short
//!   (2 a villager), and the trades make for a town without them, so much an item. What the crew learn rides home
//!   with the wagon (`Wagon.news`).
//! - **Before a trip** (`Plan`): each trade at home is asked whether its services would be wanted at the far end, from
//!   the ledgers alone (the far end's last stocktake, not its stock): those trades go along (`trip_wants`).
//!
//! Ported from:
//!
//! - `service/Service.java` (`Where`, `road`, `passive`); `service/Services.java`: `TRUST_PER_SERVICE`, `all`, `of`
//!   (52–61), `skills`, `residents`, `wantedAbroad` (85), `abroad` (103: the budget read again before each service,
//!   the fee at most the budget, trust for a paid one, the ledger noted either way), `home` (139).
//! - `service/Visit.java`: `planning` (no stock to see), `abroad`, `home`, `budget` (never below 0), `skill` (at least
//!   1), `hear` (never the worker's own town).
//! - `service/TownServices.java`: `register` (49: the order), `hasOwn`, `blessingWanted` (100), `bless` (109),
//!   `harvestWanted` (118), `helpHarvest` (132), `marketsKnown`, `swapWanted` (160), `swapMaps` (166),
//!   `chartingWanted`, `chart`; the constants.
//! - `service/Crafts.java`: `register` (44: the recipes and their order), `Recipe`, `recipe` (151: home, and
//!   `_abroad` for a town without the trade), `count`, `spare`, `batches` (178), `wanted` (194), `make` (201), `kit`
//!   (121), `contractInputs` (247), `contracted`, `comfortWanted`, `foodWanted`, `saddlesWanted`, `booksWanted` (303).
//! - `caravan/CaravanServices.java`: `register`, `drovingWanted` (49), `drove` (63), `surveyWanted` (94), `survey` (99);
//!   `caravan/Cartography.java` `townsAlong` (32), `ALONG`; `caravan/Squad.java` `HERD`, `ANIMAL_VALUE`, `herd`,
//!   `pasture`; `Caravans.ANIMALS_PER_VILLAGER`, `STALE_MAP_TICKS`, `dispatch` 383–384 and 563 (the herd driven when a
//!   Shepherd goes), `arriveHome` 1186 and 1199 (the news learned, the unsold animals home).
//! - `territory/Territory.java` `bless` (1685), `owe` (1712), `moodTarget`'s blessing (`BLESSING_MOOD`, twice with a
//!   temple), `idlePerDay`, `spareFarmPlots`, `workers`, `guards`; `territory/Smithing.java` `KITS`, `wearers`, `share`.
//! - `service/ServiceLedger.java`: `record`, `last`, `tallies`, `recent`.
//!
//! **Where the core differs.** A Merchant's deals (`arrive::draft`), a Librarian's copy of the books
//! (`atlas::copy_books`, wanted through `atlas::want_books`) and the road's services (a Leatherworker's packs,
//! porters: `squad.rs`) are listed here but worked by their own slices; a Fisherman's boat and catch, a Toolsmith's
//! mine and mending, Fletchers' hunting, the master Armorer's blast furnace and the master services are later slices'
//! (`mines-and-kit`), so a town has no miners and no pickaxes are wanted yet. The core has no entities: a Shepherd
//! drives whole animals from home's herd (`herds.rs`), and the far end's pasture is its herd, the animals it buys at
//! most the room its pastures have (the mod turns them out by the bell with no room to keep). The Cartographer brings
//! no map item home (Minecraft's), only the towns it saw. Recipes are matched to the host's goods by name at each
//! look, so a host that renames goods needs nothing rebuilt; one with a good missing has no such recipe. Crafts take
//! no warehouse room.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::atlas::{Atlas, Places, Stocktakes};
use crate::content::{Content, Dials, GoodId};
use crate::contracts::Book;
use crate::farms::Farm;
use crate::herds::Herds;
use crate::hex::Hex;
use crate::messages::{RouteId, TownId};
use crate::roster::{Roster, TradeId};
use crate::society::Society;
use crate::state::*;

/// A resident Cleric's blessing lasts a day (`TownServices.HOME_BLESSING_DAYS`).
pub const HOME_BLESSING_DAYS: f64 = 1.0;
/// A visiting Farmer brings in a day's harvest (`TownServices.HELP_DAYS`).
pub const HELP_DAYS: f64 = 1.0;
/// A Cartographer at home charts the country this far around, every so many days (`CHART_RANGE`, `CHART_TICKS`);
/// a master twice as far.
pub const CHART_RANGE: f64 = 800.0;
pub const CHART_DAYS: f64 = 10.0;
/// A road's map is stale this many days after a Cartographer walked it (`Caravans.STALE_MAP_TICKS`).
pub const STALE_MAP_DAYS: f64 = 10.0;
/// A Cartographer notes towns this many hexes either side of the road (`Cartography.ALONG`).
pub const ALONG: i32 = 2;
/// A Shepherd drives at most this many animals (`Squad.HERD`).
pub const HERD: usize = 8;
/// A town keeps this many grown animals a villager in its pasture; below that it buys them (`ANIMALS_PER_VILLAGER`).
pub const ANIMALS_PER_VILLAGER: f64 = 2.0;
/// What a driven animal is worth, by kind (`Squad.ANIMAL_VALUE`).
pub const ANIMAL_VALUE: [(&str, f64); 4] = [("sheep", 1.0), ("cow", 1.0), ("pig", 0.5), ("chicken", 0.25)];
/// Smelting: this much raw ore and a coal make as many ingots (`Smithing.RAW_PER_COAL`).
pub const RAW_PER_COAL: u32 = 8;
/// The ledger keeps the latest so many services done (`ServiceLedger.RECENT`).
pub const RECENT: usize = 40;

/// Where a service is done (`Service.Where`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Where {
    /// For its own town, whenever the town is worked.
    Home,
    /// For the town its wagon reaches.
    Abroad,
    /// Worked by the wagon itself as it travels.
    Road,
}

/// Who wears a kind of kit (`Smithing.Wearer`): bows are for the hunters (Fletchers) and the guards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wearer {
    Farmers,
    Miners,
    Guards,
    HuntersAndGuards,
}

/// A kind of kit (`Smithing.Kit`): the good, who wears it, how many days one lasts, what it's made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kit {
    pub good: &'static str,
    pub wearer: Wearer,
    pub days: f64,
    pub inputs: &'static [(&'static str, u32)],
}

/// The mod's five (`Smithing.KITS`).
pub const KITS: [Kit; 5] = [
    Kit { good: "iron_hoe", wearer: Wearer::Farmers, days: 16.0, inputs: &[("iron_ingot", 2)] },
    Kit { good: "iron_pickaxe", wearer: Wearer::Miners, days: 16.0, inputs: &[("iron_ingot", 3)] },
    Kit { good: "iron_sword", wearer: Wearer::Guards, days: 20.0, inputs: &[("iron_ingot", 2)] },
    Kit { good: "iron_chestplate", wearer: Wearer::Guards, days: 40.0, inputs: &[("iron_ingot", 8)] },
    Kit { good: "bow", wearer: Wearer::HuntersAndGuards, days: 20.0, inputs: &[("stick", 3), ("string", 3)] },
];

/// How many wear a kind of kit (`Smithing.wearers`).
pub fn wearers(wearer: Wearer, farmers: usize, miners: usize, guards: usize, hunters: usize) -> usize {
    match wearer {
        Wearer::Farmers => farmers,
        Wearer::Miners => miners,
        Wearer::Guards => guards,
        Wearer::HuntersAndGuards => hunters + guards,
    }
}

/// The best plain kit covers this share of its wearers (`Smithing.BEST`, enchanted kit counting for more).
pub const BEST: f64 = 1.5;

/// The share of its wearers a town's plain kit covers (`Smithing.share`, every piece of quality 1): 0 with no wearers.
pub fn kit_share(held: f64, wearers: usize) -> f64 {
    if wearers == 0 {
        return 0.0;
    }
    (whole(held).min(wearers as f64) / wearers as f64).min(BEST)
}

/// What a recipe's output is wanted for (`Crafts`' `wanted` functions), by the goods' names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Want {
    /// Books while the town is short of reading, up to its par (`booksWanted`).
    Books,
    /// All the raw ore it holds of this good (smelting).
    Ore(&'static str),
    /// What it has contracted to sell and must still deliver (`contracted`).
    Contracted,
    /// Kit for each wearer who lacks one, and what it has contracted to sell (`kit`).
    Kit(Wearer),
    /// Comforts of a kind while short, up to its par (`comfortWanted`).
    Comfort(&'static str),
    /// Cooked meat while the town's food is short, and what it has contracted to sell (`foodWanted`).
    Food,
    /// A saddle for each of its routes, less those it has (`saddlesWanted`).
    Saddles,
}

/// One of the mod's recipes, by names: (name, trade, at home, abroad, inputs, output, out, per level, fee each, wanted
/// for, least level).
pub type RecipeRow = (&'static str, &'static str, &'static str, &'static str, &'static [(&'static str, u32)],
    &'static str, u32, u32, f64, Want, u8);

/// The mod's recipes in its order (`Crafts.register`), less the master Armorer's blast smelting (no blast furnace
/// yet). Smoking: five meats, each from the Butcher at level 5.
pub const MOD_RECIPES: [RecipeRow; 19] = [
    ("bookbinding", "librarian", "binds books from paper and leather while its town is short of reading",
        "binds books for a town short of reading, from its paper and leather", &[("paper", 3), ("leather", 1)], "book",
        1, 16, 0.05, Want::Books, 1),
    ("smelting", "armorer", "smelts its town's ore: raw iron and coal into iron",
        "smelts a town's ore, for one with a mine and no Armorer", &[("raw_iron", RAW_PER_COAL), ("coal", 1)],
        "iron_ingot", RAW_PER_COAL, 4, 0.02, Want::Ore("raw_iron"), 1),
    ("gold_smelting", "armorer", "smelts its town's gold ore: raw gold and coal into gold",
        "smelts a town's gold ore, for one with a mine and no Armorer", &[("raw_gold", RAW_PER_COAL), ("coal", 1)],
        "gold_ingot", RAW_PER_COAL, 4, 0.02, Want::Ore("raw_gold"), 1),
    ("arrows", "fletcher", "fletches arrows of flint, sticks and feathers, for what its town has contracted to sell",
        "fletches arrows for a town's contracts from its flint, sticks and feathers",
        &[("flint", 1), ("stick", 1), ("feather", 1)], "arrow", 4, 4, 0.01, Want::Contracted, 1),
    ("stone_swords", "weaponsmith", "makes stone swords, for what its town has contracted to sell",
        "makes stone swords for a town's contracts", &[("cobblestone", 2), ("stick", 1)], "stone_sword", 1, 2, 0.05,
        Want::Contracted, 1),
    ("wooden_swords", "weaponsmith", "makes wooden swords, for what its town has contracted to sell",
        "makes wooden swords for a town's contracts", &[("oak_log", 1), ("stick", 2)], "wooden_sword", 2, 2, 0.02,
        Want::Contracted, 1),
    ("shields", "armorer", "makes shields of wood and iron, for what its town has contracted to sell",
        "makes shields for a town's contracts from its logs and iron", &[("oak_log", 2), ("iron_ingot", 1)], "shield",
        1, 2, 0.1, Want::Contracted, 1),
    ("hoes", "toolsmith", "forges hoes for its town's farmers: they tend more",
        "forges hoes for a town's farmers from its iron", KITS[0].inputs, "iron_hoe", 1, 2, 0.1,
        Want::Kit(Wearer::Farmers), 1),
    ("pickaxes", "toolsmith", "forges pickaxes for its town's miners: more ore",
        "forges pickaxes for a town's miners from its iron", KITS[1].inputs, "iron_pickaxe", 1, 2, 0.15,
        Want::Kit(Wearer::Miners), 1),
    ("swords", "weaponsmith", "arms its town's guards: the town can ask more of what going around would cost",
        "arms a town's guards from its iron", KITS[2].inputs, "iron_sword", 1, 2, 0.1, Want::Kit(Wearer::Guards), 1),
    ("armour", "armorer", "armours its town's guards: the town can ask a higher toll",
        "armours a town's guards from its iron", KITS[3].inputs, "iron_chestplate", 1, 2, 0.4,
        Want::Kit(Wearer::Guards), 1),
    ("banners", "shepherd", "weaves banners while its town is short of furnishings: a banner furnishes like twelve carpets",
        "weaves banners for a town short of furnishings, from its wool and sticks", &[("white_wool", 6), ("stick", 1)],
        "white_banner", 1, 2, 0.1, Want::Comfort("furnishings"), 5),
    ("smoking_beef", "butcher", "smokes beef in its town's smokehouse while food is short, or to sell: coal for fuel",
        "smokes a town's beef while its food is short", &[("beef", 8), ("coal", 1)], "cooked_beef", 8, 2, 0.01,
        Want::Food, 5),
    ("smoking_porkchop", "butcher",
        "smokes porkchop in its town's smokehouse while food is short, or to sell: coal for fuel",
        "smokes a town's porkchop while its food is short", &[("porkchop", 8), ("coal", 1)], "cooked_porkchop", 8, 2,
        0.01, Want::Food, 5),
    ("smoking_chicken", "butcher",
        "smokes chicken in its town's smokehouse while food is short, or to sell: coal for fuel",
        "smokes a town's chicken while its food is short", &[("chicken", 8), ("coal", 1)], "cooked_chicken", 8, 2,
        0.01, Want::Food, 5),
    ("smoking_mutton", "butcher", "smokes mutton in its town's smokehouse while food is short, or to sell: coal for fuel",
        "smokes a town's mutton while its food is short", &[("mutton", 8), ("coal", 1)], "cooked_mutton", 8, 2, 0.01,
        Want::Food, 5),
    ("smoking_rabbit", "butcher", "smokes rabbit in its town's smokehouse while food is short, or to sell: coal for fuel",
        "smokes a town's rabbit while its food is short", &[("rabbit", 8), ("coal", 1)], "cooked_rabbit", 8, 2, 0.01,
        Want::Food, 5),
    ("saddles", "leatherworker", "makes saddles of leather and iron: its caravans ride, a third faster",
        "makes saddles for a town's caravans from its leather and iron", &[("leather", 5), ("iron_ingot", 1)],
        "saddle", 1, 1, 0.25, Want::Saddles, 5),
    ("bows", "fletcher", "makes bows of sticks and string: for its hunters, then its guards (armed, as with swords)",
        "makes bows for a town's hunters and guards from its sticks and string", KITS[4].inputs, "bow", 1, 2, 0.1,
        Want::Kit(Wearer::HuntersAndGuards), 1),
];

/// `inputs` make `out` of `output`, at most `per_level` batches a level each time, while the town wants more; abroad
/// at `fee_each` an item (`Crafts.Recipe`), matched to the host's goods and trades.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub name: &'static str,
    pub trade: TradeId,
    pub does: &'static str,
    pub does_abroad: &'static str,
    pub inputs: Vec<(GoodId, u32)>,
    pub output: GoodId,
    pub out: u32,
    pub per_level: u32,
    pub fee_each: f64,
    pub want: Want,
    pub min_level: u8,
    /// Kit for the town's own hands: its inputs come before what its lockers sell.
    pub kit: bool,
}

/// The mod's recipes the host has every good and the trade for, in the mod's order.
pub fn recipes(content: &Content) -> Vec<Recipe> {
    MOD_RECIPES.iter().filter_map(|&(name, trade, does, does_abroad, inputs, output, out,
            per_level, fee_each, want, min_level)| {
        let trade = content.trade(trade)?;
        let inputs: Option<Vec<(GoodId, u32)>> = inputs.iter().map(|&(g, n)| content.good(g).map(|g| (g, n))).collect();
        let kit = KITS.iter().any(|k| k.good == output);
        Some(Recipe { name, trade, does, does_abroad, inputs: inputs?, output: content.good(output)?, out, per_level,
            fee_each, want, min_level, kit })
    }).collect()
}

/// What a service does, for the runners to dispatch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Work {
    Droving,
    Deals,
    Survey,
    Books,
    Packs,
    Boat,
    Fishing,
    Blessing,
    HarvestHelp,
    MapSwap,
    HomeBlessing,
    Charting,
    /// A recipe, by its place in [`recipes`].
    Craft(usize),
    Militia,
    Porter,
}

/// A service (`Service`): its name, the trade that does it, where, what it does in plain words, and its work.
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub name: String,
    pub trade: TradeId,
    pub where_: Where,
    pub does: String,
    pub work: Work,
}

impl Service {
    /// Worked elsewhere (by its own slice, or the road): listed for the readout, never run by the runners here.
    pub fn passive(&self) -> bool {
        matches!(self.work, Work::Deals | Work::Books | Work::Packs | Work::Boat | Work::Fishing | Work::Militia | Work::Porter)
    }
}

/// Every service, in the order they're done (`Services.all`: `CaravanServices.register`, then `TownServices.register`
/// with its `Crafts.register`, each recipe at home then abroad), for the trades the host has.
pub fn all(content: &Content) -> Vec<Service> {
    let mut out = Vec::new();
    let mut add = |name: &str, trade: &str, where_: Where, does: &str, work: Work| {
        if let Some(t) = content.trade(trade) {
            out.push(Service { name: name.to_string(), trade: t, where_, does: does.to_string(), work });
        }
    };
    add("droving", "shepherd", Where::Abroad, "drives animals from the market pen to sell to a town short of them",
        Work::Droving);
    add("deals", "merchant", Where::Abroad,
        "strikes deals: haggles at its level, and drafts standing orders for goods that sold well", Work::Deals);
    add("survey", "cartographer", Where::Abroad, "maps the road: the towns along it are known at home", Work::Survey);
    add("books", "librarian", Where::Abroad, "copies the books: what the town holds, so the next caravan knows what to bring",
        Work::Books);
    add("packs", "leatherworker", Where::Road, "carries packs: 9 more stacks of cargo", Work::Packs);
    add("boat", "fisherman", Where::Road, "captains a boat across water, past bridges and their tolls", Work::Boat);
    add("fishing", "fisherman", Where::Road, "fishes the water it crosses: cod a hex, sold where food is short",
        Work::Fishing);
    add("blessing", "cleric", Where::Abroad, "blesses a town without a Cleric of its own: happier for a few days",
        Work::Blessing);
    add("harvest_help", "farmer", Where::Abroad,
        "helps bring in the harvest from fields no one tends, for a farmhand's share", Work::HarvestHelp);
    add("map_swap", "cartographer", Where::Abroad, "swaps maps: each town learns of the markets the other knows",
        Work::MapSwap);
    add("home_blessing", "cleric", Where::Home, "blesses its own town: happier while it stays", Work::HomeBlessing);
    add("charting", "cartographer", Where::Home, "charts the country around: its town learns of markets within 800 blocks",
        Work::Charting);
    for (i, r) in recipes(content).into_iter().enumerate() {
        let trade = content.trades[r.trade].name.clone();
        add(r.name, &trade, Where::Home, r.does, Work::Craft(i));
        add(&format!("{}_abroad", r.name), &trade, Where::Abroad, &format!("{}, {} emeralds each", r.does_abroad, r.fee_each),
            Work::Craft(i));
    }
    add("militia", "nitwit", Where::Home, "stands watch at the town's posts for its food, without pay", Work::Militia);
    add("porter", "nitwit", Where::Road, "carries 9 stacks out with a caravan, then walks home", Work::Porter);
    out
}

/// What a trade does, everywhere (`Services.of`).
pub fn of(content: &Content, trade: TradeId) -> Vec<Service> {
    all(content).into_iter().filter(|s| s.trade == trade).collect()
}

/// A town as a service sees it: whether it's a settlement (a realm off the map isn't), what it holds (while planning,
/// its last stocktake), its trades and people, its roster, farm and herds, its guards and its routes out.
#[derive(Clone, Copy)]
pub struct Served<'a> {
    pub id: TownId,
    pub settlement: bool,
    pub stock: &'a [f64],
    pub trades: &'a Trades,
    pub people: &'a People,
    pub roster: Option<&'a Roster>,
    pub farm: Option<&'a Farm>,
    pub herds: Option<&'a Herds>,
    pub guards: usize,
    pub routes_out: usize,
}

impl Served<'_> {
    /// The best level of each trade among its grown residents at home, at least 1 (`Services.residents`).
    pub fn residents(&self) -> BTreeMap<TradeId, u8> {
        residents(self.roster)
    }

    /// Its grown residents at home by trade (`Territory.workers`).
    pub fn workers(&self, trade: Option<TradeId>) -> usize {
        let Some(t) = trade else { return 0 };
        self.roster.map_or(0, |r| r.hands.iter().filter(|h| h.trade == t && !h.away && !h.is_baby()).count())
    }

    /// Has it a resident of this trade at home? Then it does this for itself (`hasOwn`).
    pub fn has_own(&self, trade: TradeId) -> bool {
        self.settlement && self.residents().contains_key(&trade)
    }
}

/// The best level of each trade among a roster's grown hands at home, at least 1 (`Services.residents`).
pub fn residents(roster: Option<&Roster>) -> BTreeMap<TradeId, u8> {
    let mut out: BTreeMap<TradeId, u8> = BTreeMap::new();
    for h in roster.into_iter().flat_map(|r| r.hands.iter()).filter(|h| !h.away && !h.is_baby()) {
        let e = out.entry(h.trade).or_insert(0);
        *e = (*e).max(h.level.max(1));
    }
    out
}

/// The best level of each trade among these hands, at least 1 (`Services.skills`: a squad's).
pub fn skills<'a>(hands: impl IntoIterator<Item = &'a crate::roster::Hand>) -> BTreeMap<TradeId, u8> {
    let mut out: BTreeMap<TradeId, u8> = BTreeMap::new();
    for h in hands {
        let e = out.entry(h.trade).or_insert(0);
        *e = (*e).max(h.level.max(1));
    }
    out
}

/// The trip a visit is part of, abroad or while planning one: the route, when its road was last mapped and its path,
/// home's spare plots (none: home isn't a settlement) and the animals at home a Shepherd could drive, the wagon's herd,
/// and the two Merchants' levels.
#[derive(Clone, Copy, Default)]
pub struct Trip<'a> {
    pub route: RouteId,
    pub mapped_at: Option<u64>,
    pub path: &'a [Hex],
    pub home_spare_plots: Option<u32>,
    pub home_animals: usize,
    pub herd: &'a [usize],
    pub seller_level: u8,
    pub buyer_level: u8,
}

/// One trade's hand doing a service for a town (`Visit`): who it comes from, the town served, how skilled it is,
/// whether a trip is only being planned, what the town can spend, and what the ledgers say.
#[derive(Clone, Copy)]
pub struct Visit<'a> {
    pub content: &'a Content,
    pub from: TownId,
    pub at: TownId,
    pub skill: u8,
    pub now: u64,
    pub planning: bool,
    pub budget: f64,
    pub town: Served<'a>,
    pub trip: Option<Trip<'a>>,
    pub book: &'a Book,
    pub atlas: &'a Atlas,
    pub places: &'a Places,
    pub ledger: &'a ServiceLedger,
    /// The recipes, as [`recipes`] matched them to the host's goods this look.
    pub recipes: &'a [Recipe],
}

impl Visit<'_> {
    pub fn abroad(&self) -> bool {
        self.from != self.at
    }

    fn day(&self) -> u64 {
        self.content.day_ticks.max(1)
    }

    fn count(&self, good: GoodId) -> f64 {
        whole(self.town.stock.get(good).copied().unwrap_or(0.0))
    }

    /// What it holds less what its lockers want of it (`Crafts.spare`): a good a locker takes is kept to its par.
    fn spare(&self, good: GoodId) -> f64 {
        let capacity = self.town.trades.capacity.get(good).copied().unwrap_or(0.0);
        let kept = if capacity > 0.0 { crate::bargain::par(self.content, capacity, true) } else { 0.0 };
        (self.count(good) - kept).max(0.0)
    }

    /// What it has contracted to sell of `good` and must still deliver, less what it holds (`Crafts.contracted`).
    fn contracted(&self, good: GoodId) -> f64 {
        let owed = to_deliver(self.book, self.at, good);
        if owed <= 0.0 { 0.0 } else { (owed - if self.planning { 0.0 } else { self.count(good) }).max(0.0) }
    }

    /// Comforts of `output`'s kind it could use, as items of `output`: while short, up to its par (`comfortWanted`).
    fn comfort_wanted(&self, kind: Option<usize>, output: GoodId, planned: f64) -> f64 {
        let Some(kind) = kind else { return 0.0 };
        if self.planning {
            return if self.town.people.comfort_met.get(kind).copied().unwrap_or(0.0) < 1.0 { planned } else { 0.0 };
        }
        let held: f64 = self.content.goods.iter().enumerate().filter(|(_, g)| g.comfort == Some(kind))
            .map(|(i, g)| self.town.stock.get(i).copied().unwrap_or(0.0) * g.value).sum();
        let short = crate::people::comfort_par(self.content, self.town.people) - held;
        if short <= 0.0 { 0.0 } else { (short / self.content.goods[output].value.max(1e-9) - 1e-9).ceil() }
    }

    /// The markets a town knows, and itself (`marketsKnown`).
    fn markets_known(&self, town: TownId) -> Vec<TownId> {
        let mut known = self.places.known(self.content, self.atlas, town);
        if !known.contains(&town) {
            known.push(town);
        }
        known
    }
}

/// What a town has contracted to sell of a good and must still deliver (`ContractBook.toDeliver`): every contract in
/// force it knows of, its whole quantity less what's delivered.
pub fn to_deliver(book: &Book, seller: TownId, good: GoodId) -> f64 {
    book.open.iter().filter_map(|&id| book.contracts.get(id))
        .filter(|c| c.in_force() && c.seller == seller && c.known_by_seller && c.good == good && c.training.is_none())
        .map(|c| (c.quantity * c.installments as f64 - c.delivered).max(0.0))
        .sum()
}

/// What a town needs of each input to make what it has contracted to sell, by the first recipe that makes each good
/// (`Crafts.contractInputs`): what the wagons that serve it should bring it.
pub fn contract_inputs(content: &Content, book: &Book, town: TownId) -> BTreeMap<GoodId, f64> {
    let recipes = recipes(content);
    let mut goods: Vec<GoodId> = Vec::new();
    for c in book.open.iter().filter_map(|&id| book.contracts.get(id)) {
        if c.in_force() && c.seller == town && c.known_by_seller && c.training.is_none() && !goods.contains(&c.good) {
            goods.push(c.good);
        }
    }
    let mut needs = BTreeMap::new();
    for g in goods {
        let owed = to_deliver(book, town, g);
        if let Some(r) = recipes.iter().find(|r| r.output == g) {
            let batches = (owed / r.out as f64 - 1e-9).ceil().max(0.0);
            for &(input, n) in &r.inputs {
                *needs.entry(input).or_insert(0.0) += batches * n as f64;
            }
        }
    }
    needs
}

/// What doing a service did, for the runner to put into the world: the fee the town served owes (the runner takes
/// at most its budget), goods taken from its stock and made into it, a blessing, produce owed it with the harvest,
/// markets it learned (`taught`) and the worker's town learned (`heard`: at once at home, with the wagon abroad), the
/// road mapped and the towns seen along it, and the animals it bought from the wagon's herd (by their places in it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Done {
    pub fee: f64,
    pub used: Vec<(GoodId, f64)>,
    pub made: Vec<(GoodId, f64)>,
    pub blessed_until: Option<u64>,
    pub owed: Vec<(GoodId, f64)>,
    pub taught: Vec<TownId>,
    pub heard: Vec<TownId>,
    pub mapped: Option<u32>,
    pub bought: Vec<usize>,
}

impl Done {
    fn hear(&mut self, visit: &Visit, market: TownId) {
        if market != visit.from && !self.heard.contains(&market) {
            self.heard.push(market);
        }
    }
}

fn whole(x: f64) -> f64 {
    (x + 1e-9).floor()
}

fn animal_value(content: &Content, kind: usize) -> f64 {
    let name = content.animals.get(kind).map_or("", |a| a.name.as_str());
    ANIMAL_VALUE.iter().find(|(n, _)| *n == name).map_or(0.0, |(_, v)| *v)
}

/// The whole animals in a herd a Shepherd could drive, up to [`HERD`], a kind each, in the content's order (`Squad.herd`).
pub fn drivable(content: &Content, herds: &Herds) -> Vec<usize> {
    let mut out = Vec::new();
    for k in 0..content.animals.len() {
        if animal_value(content, k) <= 0.0 {
            continue;
        }
        let n = whole(herds.count_of(k)) as usize;
        let take = n.min(HERD - out.len());
        out.extend((0..take).map(|_| k));
        if out.len() >= HERD {
            break;
        }
    }
    out
}

/// A Cleric's blessing to where a town's mood heads (`Territory.moodTarget`): `blessing_mood` while it's blessed (as of
/// its last count), twice with a temple.
pub fn blessing_mood(d: &Dials, people: &People, works: Option<&crate::works::Works>) -> f64 {
    if people.blessed_until.is_some_and(|until| until >= people.last) {
        d.blessing_mood * if works.is_some_and(|w| w.built(crate::works::Kind::Temple)) { 2.0 } else { 1.0 }
    } else {
        0.0
    }
}

/// Batches a recipe would make now (`Crafts.batches`): as many as are wanted, its skill allows and the inputs cover
/// (kit from all the town holds, anything else from what its lockers leave), a stack's worth at most, and abroad
/// what the town can pay for.
pub fn batches(recipe: &Recipe, visit: &Visit) -> u32 {
    let out = recipe.out.max(1) as f64;
    let mut batches = (want(recipe, visit) / out - 1e-9).ceil().max(0.0).min((recipe.per_level * visit.skill as u32) as f64);
    for &(g, n) in &recipe.inputs {
        let have = if recipe.kit { visit.count(g) } else { visit.spare(g) };
        batches = batches.min((have / n as f64).floor());
    }
    batches = batches.min((64.0 / out).floor());
    if visit.abroad() && !visit.planning {
        batches = batches.min((visit.budget / (recipe.fee_each * out)).floor());
    }
    batches.max(0.0) as u32
}

/// How many of a recipe's output the town wants (`Crafts`' `wanted` functions).
pub fn want(recipe: &Recipe, visit: &Visit) -> f64 {
    let c = visit.content;
    let output = recipe.output;
    match recipe.want {
        Want::Books => visit.comfort_wanted(c.comfort("reading"), output, 16.0),
        Want::Ore(name) => c.good(name).map_or(0.0, |g| visit.count(g)),
        Want::Contracted => visit.contracted(output),
        Want::Kit(wearer) => {
            // No mines yet (`mines-and-kit`): a town has no miners.
            let farmers = visit.town.workers(c.trade("farmer"));
            let hunters = visit.town.workers(c.trade("fletcher"));
            let n = wearers(wearer, farmers, 0, visit.town.guards, hunters) as f64;
            (n + visit.contracted(output) - visit.count(output)).max(0.0)
        }
        Want::Comfort(kind) => visit.comfort_wanted(c.comfort(kind), output, 4.0),
        Want::Food => {
            let food = if visit.planning {
                0.0
            } else {
                let points = crate::people::food_points(c, &Stock(visit.town.stock.to_vec()));
                ((2.0 * crate::workshop::FOOD_PAR_POINTS - points.floor()).max(0.0) / 6.0).floor()
            };
            food + visit.contracted(output)
        }
        Want::Saddles => (visit.town.routes_out as f64 - visit.count(output)).max(0.0),
    }
}

/// Would the town served have a use for the service now (`Service.wanted`)? Asked before a trip too, from the ledgers
/// alone (`planning`). A service worked elsewhere is never wanted here.
pub fn wanted(service: &Service, visit: &Visit) -> bool {
    let c = visit.content;
    let d = &c.dials;
    let day = visit.day();
    let town = &visit.town;
    match service.work {
        Work::Blessing => {
            if !town.settlement || c.trade("cleric").is_some_and(|t| town.has_own(t)) {
                return false;
            }
            let lasting = town.people.blessed_until.is_some_and(|u| u > visit.now + day);
            !lasting && (visit.planning || visit.budget >= d.blessing_emeralds)
        }
        Work::HomeBlessing => town.settlement && town.people.blessed_until.is_none_or(|u| u < visit.now + day / 2),
        Work::HarvestHelp => {
            let idle = town.farm.map_or(0.0, |f| f.idle.values().sum::<f64>());
            if !town.settlement || idle <= 0.0 {
                return false;
            }
            if visit.planning {
                // Only a Farmer its own fields can spare: home tends all it has with a Farmer's plots left over.
                return visit.trip.and_then(|t| t.home_spare_plots)
                    .is_none_or(|spare| spare >= crate::farms::PLOTS_PER_FARMER_LEVEL);
            }
            visit.budget > 0.0
        }
        Work::MapSwap => {
            let ours = visit.markets_known(visit.from);
            let theirs = visit.markets_known(visit.at);
            !theirs.iter().all(|m| ours.contains(m)) || !ours.iter().all(|m| theirs.contains(m))
        }
        Work::Charting => visit.ledger.last("charting", visit.at)
            .is_none_or(|last| visit.now.saturating_sub(last) >= (CHART_DAYS * day as f64) as u64),
        Work::Survey => visit.trip.is_some_and(|t| {
            t.mapped_at.is_none_or(|m| visit.now.saturating_sub(m) > (STALE_MAP_DAYS * day as f64) as u64)
        }),
        Work::Droving => {
            if !town.settlement {
                return false;
            }
            match visit.trip {
                Some(t) if visit.planning => t.home_animals > 0,
                Some(t) => !t.herd.is_empty() && visit.budget > 0.0,
                None => false,
            }
        }
        Work::Craft(i) => {
            let Some(recipe) = visit.recipes.get(i) else { return false };
            if !town.settlement || visit.abroad() && town.has_own(recipe.trade) || visit.skill < recipe.min_level {
                return false;
            }
            batches(recipe, visit) > 0
        }
        Work::Deals | Work::Books | Work::Packs | Work::Boat | Work::Fishing | Work::Militia | Work::Porter => false,
    }
}

/// Does the service (`Service.perform`): what it did, and what the town served owes for it (abroad; nothing at home).
pub fn perform(service: &Service, visit: &Visit) -> Done {
    let c = visit.content;
    let d = &c.dials;
    let day = visit.day() as f64;
    let town = &visit.town;
    let mut done = Done::default();
    match service.work {
        Work::Blessing | Work::HomeBlessing => {
            let days = if visit.abroad() { d.blessing_days + visit.skill as f64 } else { HOME_BLESSING_DAYS };
            done.blessed_until = Some(visit.now + (days * day) as u64);
            done.fee = if visit.abroad() { d.blessing_emeralds } else { 0.0 };
        }
        Work::HarvestHelp => {
            let Some(farm) = town.farm else { return done };
            let idle_plots = farm.idle.values().sum::<f64>() / crate::farms::YIELD_PER_PLOT_PER_DAY;
            if idle_plots <= 0.0 {
                return done;
            }
            let share = (crate::farms::PLOTS_PER_FARMER_LEVEL as f64 * visit.skill as f64 / idle_plots).min(1.0);
            let mut produce: Vec<(GoodId, f64)> = farm.idle.iter().map(|(&g, &per_day)| (g, per_day * share * HELP_DAYS)).collect();
            let value: f64 = produce.iter().map(|&(g, n)| n * c.goods[g].value).sum();
            let mut fee = value * farm.labor_share;
            if fee > visit.budget {
                // It pays for what it can: the rest stays in the fields.
                let scale = if fee > 0.0 { visit.budget / fee } else { 0.0 };
                for p in produce.iter_mut() {
                    p.1 *= scale;
                }
                fee = visit.budget;
            }
            done.owed = produce;
            done.fee = fee;
        }
        Work::MapSwap => {
            let ours = visit.markets_known(visit.from);
            let theirs = visit.markets_known(visit.at);
            let taught: Vec<TownId> = ours.iter().copied().filter(|m| !theirs.contains(m)).collect();
            let learned: Vec<TownId> = theirs.iter().copied().filter(|m| !ours.contains(m)).collect();
            for &m in &learned {
                done.hear(visit, m); // our copy of their map rides home
            }
            done.fee = d.map_emeralds * (taught.len() as f64 - learned.len() as f64).max(0.0);
            done.taught = taught;
        }
        Work::Charting => {
            let range = CHART_RANGE * if visit.skill >= 5 { 2.0 } else { 1.0 };
            // Markets on the map within range of its own (`Atlas.markets`, by where their desks stand).
            if visit.places.hexes.get(visit.at).copied().flatten().is_some() {
                for m in 0..visit.places.hexes.len() {
                    if m != visit.at && visit.places.hexes[m].is_some()
                        && visit.places.distance(visit.at, m).is_some_and(|b| b as f64 <= range) {
                        done.hear(visit, m);
                    }
                }
            }
        }
        Work::Survey => {
            let Some(trip) = visit.trip else { return done };
            let towns = towns_along(visit.places, trip.path, visit.from, visit.at);
            for &m in &towns {
                done.hear(visit, m);
            }
            done.mapped = Some(towns.len() as u32);
        }
        Work::Droving => {
            let Some(trip) = visit.trip else { return done };
            // Its pasture counts the kinds a Shepherd drives (`Squad.pasture`: `ANIMAL_VALUE`'s); the room, every kind.
            let pasture: f64 = town.herds.map_or(0.0, |h| {
                h.count.iter().enumerate().filter(|&(k, _)| animal_value(c, k) > 0.0).map(|(_, n)| n).sum::<f64>()
            });
            let room = town.herds.map_or(0.0, |h| h.capacity(c) - h.count.iter().sum::<f64>());
            let mut wanted = (whole(town.people.villagers) * ANIMALS_PER_VILLAGER - whole(pasture)).min(whole(room));
            let mut owed = 0.0;
            for (i, &kind) in trip.herd.iter().enumerate() {
                if wanted <= 0.0 {
                    break;
                }
                let value = animal_value(c, kind);
                let each = if d.bargain {
                    crate::bargain::price(value, crate::bargain::shortage(0.0, 1.0), trip.seller_level, trip.buyer_level)
                        .unwrap_or(0.0)
                } else {
                    value
                };
                if each <= 0.0 || owed + each > visit.budget {
                    continue;
                }
                owed += each;
                wanted -= 1.0;
                done.bought.push(i);
            }
            done.fee = owed;
        }
        Work::Craft(i) => {
            let Some(recipe) = visit.recipes.get(i) else { return done };
            let n = batches(recipe, visit) as f64;
            if n <= 0.0 {
                return done;
            }
            done.used = recipe.inputs.iter().map(|&(g, k)| (g, n * k as f64)).collect();
            let items = n * recipe.out as f64;
            done.made = vec![(recipe.output, items)];
            done.fee = if visit.abroad() { items * recipe.fee_each } else { 0.0 };
        }
        Work::Deals | Work::Books | Work::Packs | Work::Boat | Work::Fishing | Work::Militia | Work::Porter => {}
    }
    done
}

/// The towns within [`ALONG`] hexes of a road's path, but not its ends (`Cartography.townsAlong`): placed ones, by
/// number.
pub fn towns_along(places: &Places, path: &[Hex], from: TownId, to: TownId) -> Vec<TownId> {
    (0..places.hexes.len())
        .filter(|&t| t != from && t != to)
        .filter(|&t| places.hexes[t].is_some_and(|at| path.iter().any(|h| h.distance(at) <= ALONG)))
        .collect()
}

/// A service done, as the ledger keeps it: when, its name, the worker's town, the town served, what was paid.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub time: u64,
    pub service: String,
    pub from: TownId,
    pub at: TownId,
    pub paid: f64,
}

/// Services done, on the ledger (`ServiceLedger`): how many times each worker's town did each for each town and what
/// it was paid, when each was last done for each town (by anyone), and the latest few.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct ServiceLedger {
    pub tallies: BTreeMap<(String, TownId, TownId), (u32, f64)>,
    pub last: BTreeMap<(String, TownId), u64>,
    pub recent: Vec<Record>,
}

impl ServiceLedger {
    pub fn record(&mut self, service: &str, from: TownId, at: TownId, paid: f64, now: u64) {
        let t = self.tallies.entry((service.to_string(), from, at)).or_insert((0, 0.0));
        t.0 += 1;
        t.1 += paid;
        self.last.insert((service.to_string(), at), now);
        self.recent.insert(0, Record { time: now, service: service.to_string(), from, at, paid });
        self.recent.truncate(RECENT);
    }

    /// When `service` was last done for the town `at`, by anyone; none if never.
    pub fn last(&self, service: &str, at: TownId) -> Option<u64> {
        self.last.get(&(service.to_string(), at)).copied()
    }

    /// How many times the town `from` did `service` for `at`.
    pub fn times(&self, service: &str, from: TownId, at: TownId) -> u32 {
        self.tallies.get(&(service.to_string(), from, at)).map_or(0, |t| t.0)
    }
}

/// A service as a host sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceView {
    pub name: String,
    pub trade: String,
    pub where_: Where,
    pub does: String,
}

/// What a service done for a test or a host did: the fee owed (at most the budget), and the markets the worker's town
/// learned of (abroad, they'd ride home).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DoneView {
    pub fee: f64,
    pub heard: Vec<TownId>,
}

impl Society {
    /// Every service, in the order they're done (`Services.all`).
    pub fn services(&self) -> Vec<ServiceView> {
        let c = self.content();
        all(c).into_iter().map(|s| ServiceView { name: s.name, trade: c.trades[s.trade].name.clone(), where_: s.where_,
            does: s.does }).collect()
    }

    /// What a trade does, everywhere (`Services.of`), by its name.
    pub fn services_of(&self, trade: &str) -> Vec<ServiceView> {
        let Some(t) = self.content().trade(trade) else { return Vec::new() };
        self.services().into_iter().filter(|s| self.content().trade(&s.trade) == Some(t)).collect()
    }

    /// The services done by or for a town, the latest first (`ServiceLedger.recent`).
    pub fn done(&self, town: TownId) -> Vec<Record> {
        self.world.resource::<ServiceLedger>().recent.iter().filter(|r| r.from == town || r.at == town).cloned().collect()
    }

    /// How many times `from`'s hands did `service` for `at`.
    pub fn times_done(&self, service: &str, from: TownId, at: TownId) -> u32 {
        self.world.resource::<ServiceLedger>().times(service, from, at)
    }

    /// When a town's blessing lasts until; none if it was never blessed.
    pub fn blessed_until(&self, town: TownId) -> Option<u64> {
        self.world.get::<People>(self.towns[town]).and_then(|p| p.blessed_until)
    }

    /// What a town's untended plots would yield a day, by crop, and the plots its Farmers could tend beyond those they
    /// work: what a visiting Farmer brings in (`Territory.setIdleForTest`; the farm's plan sets them each review).
    pub fn set_idle(&mut self, town: TownId, idle: &[(GoodId, f64)], spare_plots: u32) {
        let mut farm = self.world.get_mut::<Farm>(self.towns[town]).expect("a town has its farm");
        farm.idle = idle.iter().copied().collect();
        farm.spare_plots = spare_plots;
    }

    /// Produce owed a town with its harvest, not yet handed over, by good.
    pub fn owed_produce(&self, town: TownId) -> BTreeMap<GoodId, f64> {
        self.world.get::<Farm>(self.towns[town]).map(|f| f.pending.clone()).unwrap_or_default()
    }

    /// A route's road as its Cartographers left it: when it was last mapped, the most towns seen along it, the animals
    /// its Shepherds sold at the far end, and what its squads' services earned in all.
    pub fn route_services(&self, route: RouteId) -> (Option<u64>, u32, u32, f64) {
        let s = self.world.get::<RouteState>(self.routes[route]).expect("a route");
        (s.mapped_at, s.towns_mapped, s.animals_sold, s.service_paid)
    }

    /// The trades whose services the far end of a route would want this trip, by name, as the last plan asked them.
    pub fn services_wanted(&self, route: RouteId) -> Vec<String> {
        let s = self.world.get::<RouteState>(self.routes[route]).expect("a route");
        s.services_wanted.iter().map(|&t| self.content().trades[t].name.clone()).collect()
    }

    /// The animals on a route's wagon, by kind's name.
    pub fn wagon_herd(&self, route: RouteId) -> Vec<String> {
        let s = self.world.get::<RouteState>(self.routes[route]).expect("a route");
        s.wagon.as_ref().map_or_else(Vec::new, |w| w.herd.iter().map(|&k| self.content().animals[k].name.clone()).collect())
    }

    /// Would the named service be wanted at `at` from a hand of `from` of this level (`Service.wanted`), with
    /// `budget` to spend, or only `planning` a trip (on `route`, if it's part of one)?
    #[allow(clippy::too_many_arguments)]
    pub fn service_wanted(&mut self, name: &str, from: TownId, at: TownId, skill: u8, planning: bool, budget: f64,
            route: Option<RouteId>) -> bool {
        self.visit(name, from, at, skill, planning, budget, route, wanted).unwrap_or(false)
    }

    /// Does the named service at `at` from a hand of `from` of this level, as a test does it (`perform` on a `Visit`):
    /// what it did goes into the world (the goods, the blessing, the produce owed, what the town served learned), and
    /// what it owes, at most `budget`, is returned with what the worker's town heard of; no money moves, and a wagon's
    /// part (a herd sold, a road mapped) is the runner's (`rules/services.rs`), not done here.
    pub fn do_service(&mut self, name: &str, from: TownId, at: TownId, skill: u8, budget: f64, route: Option<RouteId>)
            -> Option<DoneView> {
        let done = self.visit(name, from, at, skill, false, budget, route, perform)?;
        let now = self.now();
        let e = self.towns[at];
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        {
            let mut ent = self.world.entity_mut(e);
            let mut stock = ent.get_mut::<Stock>().expect("a town").0.clone();
            let mut people = ent.get::<People>().cloned().expect("a town");
            let mut farm = ent.get::<Farm>().cloned();
            put(&mut ledger, &done, &mut stock, &mut people, farm.as_mut());
            ent.get_mut::<Stock>().unwrap().0 = stock;
            *ent.get_mut::<People>().unwrap() = people;
            if let Some(f) = farm {
                *ent.get_mut::<Farm>().unwrap() = f;
            }
        }
        *self.world.resource_mut::<Ledger>() = ledger;
        let mut atlas = self.world.resource_mut::<Atlas>();
        for &m in &done.taught {
            atlas.learn(at, m, now);
        }
        if from == at {
            for &m in &done.heard {
                atlas.learn(from, m, now);
            }
        }
        Some(DoneView { fee: done.fee.min(budget.max(0.0)), heard: done.heard })
    }

    #[allow(clippy::too_many_arguments)]
    fn visit<R>(&mut self, name: &str, from: TownId, at: TownId, skill: u8, planning: bool, budget: f64,
            route: Option<RouteId>, f: impl FnOnce(&Service, &Visit) -> R) -> Option<R> {
        let content = self.content().clone();
        let service = all(&content).into_iter().find(|s| s.name == name)?;
        let recipes = recipes(&content);
        let places = self.places();
        let routes_out = self.routes_out(at);
        let guards = self.guards(at);
        let w = &self.world;
        let e = w.entity(self.towns[at]);
        let held = if planning {
            let st = w.resource::<Stocktakes>();
            (0..content.goods.len()).map(|g| st.on_hand(at, g)).collect()
        } else {
            e.get::<Stock>().expect("a town").0.clone()
        };
        let trades = e.get::<Trades>().expect("a town");
        let farm = e.get::<Farm>();
        let town = Served {
            id: at,
            settlement: !e.contains::<crate::realms::Realm>() && !e.contains::<crate::realms::OffMap>(),
            stock: &held,
            trades,
            people: e.get::<People>().expect("a town"),
            roster: e.get::<Roster>(),
            farm,
            herds: e.get::<Herds>(),
            guards,
            routes_out,
        };
        let state = route.and_then(|r| w.get::<RouteState>(self.routes[r]));
        let path = route.and_then(|r| w.get::<RoutePath>(self.routes[r])).map_or(&[][..], |p| &p.0[..]);
        let home = w.entity(self.towns[from]);
        let herd: &[usize] = state.and_then(|s| s.wagon.as_ref()).map_or(&[][..], |wg| &wg.herd[..]);
        let trip = route.map(|r| Trip {
            route: r,
            mapped_at: state.and_then(|s| s.mapped_at),
            path,
            // (as the runner's `plan`: a home off the map has no fields to spare a Farmer from)
            home_spare_plots: (!home.contains::<crate::realms::OffMap>()).then(|| home.get::<Farm>().map_or(0, |f| f.spare_plots)),
            home_animals: home.get::<Herds>().map_or(0, |h| drivable(&content, h).len()),
            herd,
            seller_level: crate::bargain::merchant_level(home.get::<Roster>()),
            buyer_level: crate::bargain::merchant_level(e.get::<Roster>()),
        });
        let visit = Visit { content: &content, from, at, skill: skill.max(1), now: w.resource::<Clock>().now, planning,
            budget: if planning { 0.0 } else { budget.max(0.0) }, town, trip, book: w.resource::<Book>(),
            atlas: w.resource::<Atlas>(), places: &places, ledger: w.resource::<ServiceLedger>(), recipes: &recipes };
        Some(f(&service, &visit))
    }
}

/// What a service did, put into the town served (and the ledger): goods taken and made, a blessing (the longer
/// stands), produce owed with its harvest (straight into stock where its farm isn't set up).
pub fn put(ledger: &mut Ledger, done: &Done, stock: &mut [f64], people: &mut People, farm: Option<&mut Farm>) {
    for &(g, n) in &done.used {
        stock[g] -= n;
        ledger.used[g] += n;
    }
    for &(g, n) in &done.made {
        stock[g] += n;
        ledger.made[g] += n;
    }
    if let Some(until) = done.blessed_until {
        people.blessed_until = Some(people.blessed_until.map_or(until, |u| u.max(until)));
    }
    if !done.owed.is_empty() {
        match farm.filter(|f| f.set_up) {
            Some(farm) => {
                for &(g, n) in &done.owed {
                    *farm.pending.entry(g).or_insert(0.0) += n;
                }
            }
            None => {
                for &(g, n) in &done.owed {
                    let n = whole(n);
                    stock[g] += n;
                    ledger.made[g] += n;
                }
            }
        }
    }
}
