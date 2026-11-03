//! Work for a town's idle hands, ported from `territory/Jobs.java`: `TRADES` (41), `supply` (71), `buys` (79), `bag`
//! (95), `comfortMarbles` (122), `draw` (154), `employ` (179), `forContracts`, `slack` and `leaveTrade`, and
//! `docs/decisions.md` "Work for idle hands", "A bag of marbles", "Comforts in the job draw", "A trade with nothing to
//! do gives way".
//!
//! A town with a desk and an idle adult puts up a stall for a trade drawn from a **bag of marbles**: each trade as
//! likely as its share of the marbles, which are emeralds of demand it would meet. A trade's marbles are what its
//! villagers would buy of the town's supply (4 days of its farms' output and its warehouse, at the goods' worth),
//! shared with the hands of the trade already there; goods that markets have posted as wanted ([`Signals`], filled by
//! other slices); four days of what the untended plots would yield, for a Farmer, if that is half an emerald a day;
//! the halls it wants, for a Mason it lacks ([`Signals::build_work`]); and the comforts the town lacks: for each kind
//! short of its ration, a share of the households' savings (`comfort_marbles`) for the trade whose workshop makes
//! it, if the town has none. A trade a contract needs (`Lockers::needed`) comes first, unasked. The stall costs
//! `stall_emeralds` beyond the reserve (the households' savings on call can make up the difference); the hand takes
//! the trade at once and its trades that buy goods for emeralds are switched on.
//!
//! With no idle hand, in a town no road comes to or goes from, a hand whose locker has done nothing for `slack_days`
//! reviews takes up a trade the town lacks instead (never a Farmer's, Mason's or Cleric's, whose work isn't in their
//! locker, nor the Merchant's); a new hand gets twice that long to get going.
//!
//! **Not as the mod:** there is no ground to find a site on, so a stall needs none; the mod's `Villager` is a
//! [`Hand`] here. The caller gates it as the mod's `Territory.grow` does (see `rules/jobs.rs`); `review_jobs` doesn't.
//! The draw uses the town's own seeded dice ([`Dice`]), not the level's.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bills::Bills;
use crate::cash_book::{post, Accountant, Reason, Side};
use crate::content::{Content, GoodId};
use crate::farms::{borrow, on_call, Deposits, Farm};
use crate::lockers::{Books, Lockers};
use crate::roster::{Hand, Roster, TradeId};
use crate::state::*;

/// The trades a town can set up and the goods their villagers buy (`Jobs.TRADES`), in the mod's order.
pub const TRADES: [(&str, &[&str]); 14] = [
    ("farmer", &["wheat", "potato", "carrot", "beetroot", "pumpkin", "melon"]),
    ("librarian", &["paper", "book", "ink_sac"]),
    ("cartographer", &["paper", "glass_pane", "compass"]),
    ("fletcher", &["stick", "flint", "string", "feather"]),
    ("fisherman", &["string", "coal"]),
    ("shepherd", &["white_wool"]),
    ("cleric", &["rotten_flesh", "gold_ingot", "glass_bottle", "nether_wart"]),
    ("butcher", &["chicken", "porkchop", "rabbit", "coal", "sweet_berries"]),
    ("leatherworker", &["leather", "flint"]),
    ("mason", &["clay_ball", "stone"]),
    ("armorer", &["coal", "iron_ingot"]),
    ("toolsmith", &["coal", "iron_ingot", "flint"]),
    ("weaponsmith", &["coal", "iron_ingot", "flint"]),
    ("lumberjack", &["oak_sapling", "spruce_sapling"]),
];

/// The trades whose work burns or builds with wood: a Librarian's shelves, a Mason's halls, the smiths' fires and a
/// Butcher's smokehouse (charcoal where there's no coal). A town with none of them wants no Lumberjack for its woods.
pub const USE_WOOD: [&str; 6] = ["librarian", "mason", "armorer", "toolsmith", "weaponsmith", "butcher"];

/// A town's woods unfelled, a Lumberjack's marbles (as unfished grounds are a Fisherman's): four days of what a Novice
/// would fell of the logs its town uses a day (`wood_per_user_day` a hand of a trade that uses wood, and a Novice's
/// felling while a home or a hall waits on timber), as far as its woodlots give them, beyond its Lumberjacks at home, while a trade
/// of its uses wood ([`USE_WOOD`]), it holds fewer than `crate::kit::KEEP` logs and its warehouse has room for more
/// (`crate::kit::room`'s slots: a town whose warehouse is full drew Lumberjack after Lumberjack, each a stall's fee,
/// who felled nothing). And what waits on timber (`waiting`: a home's or a hall's worth, John: "make lumber demand
/// sufficiently high such that they show up early to settlements"), in full while it has no Lumberjack. None without
/// a Lumberjack trade or land.
pub fn woodland_marbles(content: &Content, holding: Option<&crate::territory::Holding>, home: Option<crate::hex::Hex>,
        map: &crate::map::Map, roster: &Roster, stock: &Stock, waiting: f64) -> Option<(TradeId, f64)> {
    let lumberjack = content.trade(crate::roster::LUMBERJACK)?;
    let uses = waiting > 0.0
        || USE_WOOD.iter().filter_map(|t| content.trade(t)).any(|t| crate::kit::at_home(roster, Some(t)).next().is_some());
    let logs: f64 = crate::works::LOGS.iter().filter_map(|l| content.good(l)).map(|g| stock.0[g]).sum();
    let slots = crate::kit::free_slots(content, &stock.0, 0) > content.dials.free_slots as i64 + crate::kit::SPARE_SLOTS;
    if !uses || logs >= crate::kit::KEEP || !slots {
        return None;
    }
    let d = &content.dials;
    let (lots, _) = crate::kit::woodland(map, holding, home);
    let could = lots as f64 / d.woodlot_grow_days.max(1e-9) * d.logs_per_lot;
    let felled = crate::kit::levels(roster, Some(lumberjack)) * d.lots_felled_per_level_day * d.logs_per_lot;
    // Only the wood its town burns and builds with: `wood_per_user_day` for each hand of a trade that uses it, and a
    // Novice's day's felling more while a home or a hall waits on timber. Not the logs its Lumberjacks' own trades buy
    // back for emeralds, which kept every town short of logs and drew Lumberjack after Lumberjack (seed 1717's year:
    // 363 of 845 villagers, earning 0.38 emeralds a day each; John: "Go").
    let novice = d.lots_felled_per_level_day * d.logs_per_lot;
    let users = USE_WOOD.iter().filter_map(|t| content.trade(t)).map(|t| crate::kit::at_home(roster, Some(t)).count())
        .sum::<usize>() as f64;
    let need = users * d.wood_per_user_day + if waiting > 0.0 { novice } else { 0.0 };
    // (Whatever the wood they like: logs are all worth alike.)
    let log = crate::works::LOGS.iter().find_map(|l| content.good(l))?;
    let unfelled = (could.min(need) - felled).max(0.0).min(novice);
    let wait = if roster.count(lumberjack) == 0 { waiting } else { 0.0 };
    (unfelled > 0.0).then(|| (lumberjack, unfelled * content.goods[log].value * 4.0 + wait))
}

/// What else a trade takes, a glut of which trains a hand to it ([`for_glut`]; not as the mod): the Butcher cooks beef and
/// mutton, the Armorer smelts ore. Every town held hundreds of beef, its cows' as their leather was taken, with no
/// Butcher drawn for it. Not in the job draw's bag: a glut the trade can't use up would draw hand after hand to it.
pub const TAKES: [(&str, &[&str]); 2] = [("butcher", &["beef", "mutton"]), ("armorer", &["raw_iron", "raw_gold"])];

/// Trades whose work isn't in their locker's trades (fields, the desk, halls, blessings): never slack by them
/// (`Jobs.KEPT`).
pub const KEPT: [&str; 3] = ["farmer", "mason", "cleric"];

/// Marbles a trade has in the bag, in the order they were first put in.
pub type Marbles = Vec<(TradeId, f64)>;

/// What other slices tell the job draw, by town: goods that markets the town knows have posted as wanted, as the trade
/// that would supply them and emeralds of demand (`Jobs.signals`; `crates-and-papers` fills it), and the halls the
/// town wants and could pay for now, in emeralds (`BuildServices.work`; `buildings` fills it).
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Signals {
    pub trades: BTreeMap<usize, BTreeMap<TradeId, f64>>,
    pub build_work: BTreeMap<usize, f64>,
}

impl Signals {
    /// A town's signals, as marbles in the trades' order.
    pub fn of(&self, town: usize) -> Marbles {
        self.trades.get(&town).map_or_else(Vec::new, |m| m.iter().map(|(&t, &e)| (t, e)).collect())
    }

    pub fn build_work_of(&self, town: usize) -> f64 {
        self.build_work.get(&town).copied().unwrap_or(0.0)
    }
}

/// The day each town's idle hands were last looked at: one a day.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hiring {
    pub reviewed: BTreeMap<usize, u64>,
    /// The day each town's review last left an idle adult without work (no stall it could pay for, nothing drawn): a
    /// hand the squad may train that day (`rules/squad.rs` `train`).
    #[serde(default)]
    pub left_idle: BTreeMap<usize, u64>,
    /// Hands on the road to the town that hired them from another (`rules/jobs.rs` `relocate`).
    #[serde(default)]
    pub movers: Vec<Mover>,
}

/// A hand hired away by a town down its roads (John: "hire-relocated-worker"): its trade and level kept, on the road from
/// `from` to `to`, there at `arrives`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mover {
    pub hand: crate::roster::Hand,
    pub from: usize,
    pub to: usize,
    pub arrives: u64,
}

impl crate::society::Society {
    /// The hands hired away that have come to their new town: on its roster, a resident, its stall paid for (put up
    /// once the host's villager for it is named, `Society::tag_hand`).
    pub(crate) fn land_movers(&mut self) {
        let now = self.now();
        let content = self.content().clone();
        let here: Vec<Mover> = {
            let mut hiring = self.world.resource_mut::<Hiring>();
            let (here, rest) = std::mem::take(&mut hiring.movers).into_iter().partition(|m| m.arrives <= now);
            hiring.movers = rest;
            here
        };
        for m in here {
            let Some(&e) = self.towns.get(m.to) else { continue };
            let mut q = self.world.query::<(&mut Roster, &mut Lockers)>();
            let Ok((mut roster, mut lockers)) = q.get_mut(&mut self.world, e) else { continue };
            let id = m.hand.id;
            roster.add(&content, m.hand);
            if let Some(i) = roster.hands.iter().position(|h| h.id == id) {
                open_stall(&content, &mut lockers, &roster, i);
            }
            let residents = roster.residents() as f64;
            if let Some(mut people) = self.world.get_mut::<crate::state::People>(e) {
                people.villagers = residents;
            }
        }
    }
}

/// A good by name, with or without the host's namespace (`paper`, `minecraft:paper`).
pub fn good_named(content: &Content, name: &str) -> Option<GoodId> {
    // (`g.name.rsplit(':').next() == Some(name)`, without splitting every good's name: the name after its last colon.)
    if name.contains(':') {
        return None;
    }
    content.goods.iter().position(|g| {
        g.name.strip_suffix(name).is_some_and(|before| before.is_empty() || before.ends_with(':'))
    })
}

/// A trade a town can set up: its place in [`TRADES`], by its place in the content.
pub fn job_trade(content: &Content, trade: TradeId) -> Option<usize> {
    let name = &content.trades.get(trade)?.name;
    TRADES.iter().position(|(n, _)| n == name)
}

/// Adds `marbles` to a trade's, putting it last if it had none.
fn put(marbles: &mut Marbles, trade: TradeId, m: f64) {
    match marbles.iter_mut().find(|(t, _)| *t == trade) {
        Some((_, have)) => *have += m,
        None => marbles.push((trade, m)),
    }
}

/// What the town has to sell, in emeralds by good: 4 days of its farms' output and what its warehouse holds, each at
/// the good's worth (`Jobs.supply`).
pub fn supply(content: &Content, per_day: &BTreeMap<GoodId, f64>, stock: &Stock) -> Vec<f64> {
    let mut worth: Vec<f64> = stock.0.iter().zip(&content.goods).map(|(n, g)| n * g.value).collect();
    worth.resize(content.goods.len(), 0.0);
    for (&g, &n) in per_day {
        if let Some(w) = worth.get_mut(g) {
            *w += n * 4.0 * content.goods[g].value;
        }
    }
    worth
}

/// The worth of the town's supply a job trade's villagers would buy, in emeralds (`Jobs.buys`).
pub fn buys(content: &Content, job: usize, supply: &[f64]) -> f64 {
    TRADES[job].1.iter().filter_map(|n| good_named(content, n)).map(|g| supply.get(g).copied().unwrap_or(0.0)).sum()
}

/// The bag of marbles an idle hand draws its trade from (`Jobs.bag`): what each trade's villagers would buy of the
/// town's supply, shared with the hands of the trade already there; the markets' `signals`; four days of what the
/// untended plots would yield (`idle_per_day`, items a day by crop), a Farmer's, if it comes to `farmer_idle_emeralds`
/// a day; and the halls wanted (`build_work`), a Mason's, until the town has one.
pub fn bag(content: &Content, supply: &[f64], roster: &Roster, idle_per_day: &BTreeMap<GoodId, f64>, build_work: f64,
        signals: &[(TradeId, f64)]) -> Marbles {
    let mut bag = Marbles::new();
    for (job, (name, _)) in TRADES.iter().enumerate() {
        let Some(trade) = content.trade(name) else { continue };
        let m = buys(content, job, supply) / (1 + roster.count(trade)) as f64;
        if m > 0.0 {
            put(&mut bag, trade, m);
        }
    }
    for &(trade, e) in signals {
        put(&mut bag, trade, e);
    }
    let idle: f64 = idle_per_day.iter().map(|(&g, &n)| n * content.goods.get(g).map_or(0.0, |g| g.value)).sum();
    if idle >= content.dials.farmer_idle_emeralds {
        if let Some(farmer) = content.trade("farmer") {
            put(&mut bag, farmer, idle * 4.0);
        }
    }
    if let Some(mason) = content.trade("mason") {
        if build_work > 0.0 && roster.count(mason) == 0 {
            put(&mut bag, mason, build_work);
        }
    }
    bag
}

/// Marbles for the comforts the town lacks (`Jobs.comfortMarbles`): each kind short of its ration (`met` below 1, by
/// kind) puts in a share (`comfort_marbles`) of what the households have saved, for each job trade whose workshop
/// makes it that the town has none of.
pub fn comfort_marbles(content: &Content, met: &[f64], savings: f64, roster: &Roster) -> Marbles {
    let mut marbles = Marbles::new();
    for c in &content.conversions {
        let Some(kind) = content.goods.get(c.output).and_then(|g| g.comfort) else { continue };
        let lacking = 1.0 - met.get(kind).copied().unwrap_or(0.0);
        if lacking <= 0.01 {
            continue;
        }
        for &t in &c.trades {
            if job_trade(content, t).is_some() && roster.count(t) == 0 {
                put(&mut marbles, t, lacking * savings * content.dials.comfort_marbles);
            }
        }
    }
    marbles
}

/// Marbles for an input the comforts the town lacks wait on (not as the mod): each kind short of its ration whose
/// maker the town has, short of an input none of its trades make, puts in its share (as [`comfort_marbles`]) for the
/// trade that would make the input ([`maker_of`]) where the town has none of it. Seed 1717's first town, its Librarian
/// with 2,000 paper and no leather for books, drew no Leatherworker once its clothing was met: a Leatherworker's
/// marbles were clothing's alone.
pub fn input_marbles(content: &Content, met: &[f64], savings: f64, roster: &Roster, stock: &Stock) -> Marbles {
    let mut marbles = Marbles::new();
    let recipes = crate::services::recipes(content);
    let made_here = |g: GoodId| content.conversions.iter().any(|c| c.output == g && crate::workshop::can_do(c, roster));
    for c in &content.conversions {
        let Some(kind) = content.goods.get(c.output).and_then(|g| g.comfort) else { continue };
        let lacking = 1.0 - met.get(kind).copied().unwrap_or(0.0);
        if lacking <= 0.01 || !crate::workshop::can_do(c, roster) {
            continue;
        }
        for &(g, n) in &c.inputs {
            if stock.0.get(g).copied().unwrap_or(0.0) >= n || made_here(g) {
                continue;
            }
            let Some(t) = maker_of(content, &recipes, g).filter(|&t| job_trade(content, t).is_some()) else { continue };
            if roster.count(t) == 0 {
                put(&mut marbles, t, lacking * savings * content.dials.comfort_marbles);
            }
        }
    }
    marbles
}

/// A marble drawn from the bag: each trade as likely as its share of the marbles; none from an empty bag
/// (`Jobs.draw`).
pub fn draw(bag: &[(TradeId, f64)], rng: &mut Rng) -> Option<TradeId> {
    let total: f64 = bag.iter().map(|(_, m)| m).sum();
    if total <= 0.0 {
        return None;
    }
    let mut marble = (rng.next() >> 11) as f64 / (1u64 << 53) as f64 * total;
    let mut last = None;
    for &(trade, m) in bag {
        last = Some(trade);
        marble -= m;
        if marble < 0.0 {
            return Some(trade);
        }
    }
    last
}

/// A port's fishing grounds unfished, a Fisherman's marbles (as untended plots are a Farmer's): four days of the cod
/// its waters would yield (`COD_PER_WATER_HEX` a water hex it holds, the harbour's own counting as one) beyond what its
/// Fishermen at home land (John: "the use of the water as a fishing plot can act as a substitute for farm spread").
/// None without a harbour.
pub fn fishing_marbles(content: &Content, works: Option<&crate::works::Works>, holding: Option<&crate::territory::Holding>,
        map: &crate::map::Map, roster: &Roster) -> Option<(TradeId, f64)> {
    works.filter(|w| w.port.is_some())?;
    let (fisherman, cod) = (content.trade(crate::squad::FISHERMAN)?, content.good("cod")?);
    let grounds = holding.map_or(0, |h| h.hexes.iter().filter(|&&x| map.land(x) == crate::map::Land::Water).count())
        .max(1) as f64;
    let landed: f64 = roster.hands.iter().filter(|h| h.trade == fisherman && !h.away)
        .map(|h| h.level as f64 * content.dials.cod_per_fisher_level).sum();
    let unfished = (grounds * crate::works::COD_PER_WATER_HEX - landed).max(0.0);
    (unfished > 0.0).then(|| (fisherman, unfished * content.goods[cod].value * 4.0))
}

/// A town's whole bag for its idle hand now: the markets' signals and the comforts it lacks, then [`bag`].
pub fn town_bag(content: &Content, signals: &Signals, town: usize, stock: &Stock, farm: &Farm, met: &[f64],
        savings: f64, roster: &Roster) -> Marbles {
    let mut all = signals.of(town);
    for (t, m) in comfort_marbles(content, met, savings, roster) {
        put(&mut all, t, m);
    }
    for (t, m) in input_marbles(content, met, savings, roster, stock) {
        put(&mut all, t, m);
    }
    bag(content, &supply(content, &farm.per_day, stock), roster, &farm.idle, signals.build_work_of(town), &all)
}

/// The trade that would make a good for a neighbour: one whose people sell it, else one that makes it of `recipes`
/// (`rules/sourcing.rs` `supplier_trade_in`), else the keeper of the herd it comes of (`herds.rs`: a Shepherd's wool, a
/// Leatherworker's cows' leather, a Butcher's meat), else, for game, the Fletcher, who hunts ([`crate::kit::game`]). A
/// crop is the Farmer's: its people only buy crops, so no offer of theirs sells one.
pub fn maker_of(content: &Content, recipes: &[crate::services::Recipe], good: GoodId) -> Option<TradeId> {
    if content.goods.get(good).is_some_and(|g| g.crop) {
        return content.trade("farmer");
    }
    crate::rules::sourcing::supplier_trade_in(content, recipes, good).or_else(|| {
        let keeper = crate::herds::mod_animals(content).iter().find_map(|a| {
            if a.wool == Some(good) {
                Some("shepherd")
            } else if a.hide == Some(good) {
                Some("leatherworker")
            } else if a.meat == Some(good) {
                Some("butcher")
            } else {
                None
            }
        });
        keeper.and_then(|k| content.trade(k))
    }).or_else(|| {
        let name = content.goods.get(good)?.name.rsplit(':').next()?;
        let lands = [crate::map::Land::Plains, crate::map::Land::Woods, crate::map::Land::Taiga, crate::map::Land::Desert,
            crate::map::Land::Hills, crate::map::Land::Shore, crate::map::Land::Water];
        lands.iter().any(|&l| crate::kit::game(l).iter().any(|(n, _)| *n == name)).then(|| content.trade("fletcher")).flatten()
    })
}

/// Marbles for what the towns down a town's roads want (`demands`: each one's villagers' buying a day, by good, at the
/// lockers its Merchant manages, [`crate::lockers::census_demand`]): the trade that would make each good for them
/// ([`maker_of`]), four days of it (no more than a stack) at its worth, shared with the hands of the trade the town has.
/// A town makes what its neighbours buy, as its fields grow it (`rules/farms.rs`).
pub fn far_marbles<'a>(content: &Content, recipes: &[crate::services::Recipe], roster: &Roster,
        demands: impl IntoIterator<Item = &'a BTreeMap<GoodId, f64>>) -> Marbles {
    let mut marbles = Marbles::new();
    for demand in demands {
        for (&g, &per_day) in demand {
            let Some(trade) = maker_of(content, recipes, g).filter(|&t| job_trade(content, t).is_some()) else { continue };
            let good = &content.goods[g];
            let m = (per_day * 4.0).min(good.stack.max(1) as f64) * good.value / (1 + roster.count(trade)) as f64;
            if m > 0.0 {
                put(&mut marbles, trade, m);
            }
        }
    }
    marbles
}

/// A town's place for its stalls: the borrowed pieces of it (the system and the host's call each lend them).
pub struct Place<'a> {
    pub town: usize,
    pub money: &'a mut crate::fixed::Fixed,
    pub purse: &'a mut crate::fixed::Fixed,
    pub deposits: &'a mut Deposits,
    pub budget: &'a mut Budget,
    pub stock: &'a Stock,
    pub trades: &'a Trades,
    pub farm: &'a Farm,
    /// How well each kind of comfort was met last time.
    pub met: &'a [f64],
    pub roster: &'a mut Roster,
    pub lockers: &'a mut Lockers,
    /// Whether a route starts or ends at the town.
    pub roads: bool,
    /// Whether one does, carrying or not: a market, not a village (`settlers`: a village has no Merchant and no route).
    pub routed: bool,
    /// What the towns down its roads want, as marbles ([`far_marbles`]).
    pub far: Marbles,
    /// Its herds, how many of each kind (by place in [`Content::animals`]).
    pub herd: &'a [f64],
    /// What waits on timber (a home's or a hall's worth, `People::timber_wait`, `Works::timber_wait`).
    pub timber_wait: f64,
}

/// A hand that took up a trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hired {
    pub hand: usize,
    pub trade: TradeId,
    /// It left a trade that had done nothing for the town, rather than being idle.
    pub retrained: bool,
}

/// A trade the town's contracts call for and it hasn't (`Jobs.forContracts`, from the hook `Lockers::needed`).
fn for_contracts(content: &Content, place: &Place) -> Option<TradeId> {
    place.lockers.needed.iter().copied().find(|&t| job_trade(content, t).is_some() && place.roster.count(t) == 0)
}

/// The maker of a comfort the town lacks (met under half) and holds a batch's makings of, where it has no hand of it
/// and can draw one: the first such in the conversions' order.
pub fn for_comfort(content: &Content, stock: &Stock, met: &[f64], roster: &Roster) -> Option<TradeId> {
    content.conversions.iter().find_map(|c| {
        let kind = content.goods.get(c.output).and_then(|g| g.comfort)?;
        if met.get(kind).copied().unwrap_or(1.0) >= 0.5 || crate::workshop::can_do(c, roster)
            || !c.inputs.iter().all(|&(g, n)| stock.0.get(g).copied().unwrap_or(0.0) >= n) {
            return None;
        }
        c.trades.iter().copied().find(|&t| job_trade(content, t).is_some())
    })
}

/// The trade that takes what the town holds most of, worth `glut_emeralds` or more, where it has no hand of it: trained
/// to it, not drawn (John: "train a villager who specifically takes an abundant resource"; not as the mod). In the
/// trades' order where two hold as much.
pub fn for_glut(content: &Content, stock: &Stock, roster: &Roster, herd: &[f64]) -> Option<TradeId> {
    let held: Vec<f64> = stock.0.iter().zip(&content.goods).map(|(n, g)| n.max(0.0) * g.value).collect();
    let of = |goods: &[&str]| -> f64 { goods.iter().filter_map(|n| good_named(content, n)).map(|g| held[g]).sum() };
    let mut best: Option<(TradeId, f64)> = None;
    for (name, goods) in TRADES {
        let Some(trade) = content.trade(name).filter(|&t| roster.count(t) == 0) else { continue };
        // And the animals of its herds beyond the breeding stock, at their meat and hide: a herd with no keeper.
        let animals: f64 = content.animals.iter().zip(herd).filter(|(a, _)| crate::herds::keeper_of(&a.name) == name)
            .map(|(a, &n)| (n - content.dials.breeding_stock).max(0.0) * crate::herds::worth_each(content, a)).sum();
        let worth = of(goods) + TAKES.iter().filter(|(t, _)| *t == name).map(|(_, g)| of(g)).sum::<f64>() + animals;
        if worth >= content.dials.glut_emeralds && best.is_none_or(|(_, w)| worth > w) {
            best = Some((trade, worth));
        }
    }
    best.map(|(t, _)| t)
}

/// Does a road carry, for its towns' job draws ([`slack`]): a wagon on it, or one gone down it within `slack_days`?
/// One that has carried nothing that long is no reason to keep a trade that does nothing.
pub fn carries(content: &Content, state: &RouteState, now: u64) -> bool {
    let lately = (content.dials.slack_days as u64 * content.day_ticks).max(1);
    state.wagon.is_some() || state.last_departure.is_some_and(|t| now.saturating_sub(t) < lately)
}

/// Does a trade keep a herd the town has: breeding it up is its work, though its locker does nothing (Dustfell's
/// Leatherworker, its two cows breeding, was slack by day 7 and turned Mason; its cows bred to 48 with no one to take
/// their hides, and the town went unclothed for 90 days).
fn keeps_a_herd(content: &Content, herd: &[f64], trade: &str) -> bool {
    content.animals.iter().zip(herd).any(|(a, &n)| n >= 1.0 && crate::herds::keeper_of(&a.name) == trade)
}

/// A hand whose trade has done nothing for the town for `slack_days` reviews, in a town with no roads (`Jobs.slack`),
/// or none that have carried anything lately (`rules/jobs.rs`).
fn slack(content: &Content, place: &Place) -> Option<usize> {
    if place.roads {
        return None;
    }
    let days = content.dials.slack_days as i32;
    place.lockers.hands.iter().enumerate().find_map(|(i, l)| {
        let h = place.roster.hands.get(i)?;
        let t = content.trades.get(h.trade)?;
        (l.slack >= days && !h.is_baby() && !h.away && place.roster.merchant != Some(i) && !t.merchant && !t.idle()
            && !KEPT.contains(&t.name.as_str()) && !keeps_a_herd(content, place.herd, &t.name)).then_some(i)
    })
}

/// A market that has lost its Merchant (seed 1717's Pyrewood's suffocated at its conversion, its desk then idle for
/// good: no one hired, no wagon sent) puts an idle adult at its desk for a caravan lead's signing fee (John: "Jobless
/// takes the desk"): signed on as a squad signs one, it keeps the trade with no workstation of its own but the desk.
fn to_the_desk(content: &Content, books: &mut Books, now: u64, place: &mut Place) -> Option<Hired> {
    let trade = content.trade(crate::roster::MERCHANT)?;
    let hand = place.roster.hands.iter()
        .position(|h| !h.is_baby() && !h.away && content.trades.get(h.trade).is_some_and(|t| t.none))?;
    let fee = crate::fixed::Fixed::ceil(content.dials.signing_emeralds);
    if *place.money < fee {
        return None;
    }
    post(books.ledger, books.book, Side::treasury(place.town, place.money), Side::outside(), fee,
        crate::rules::squad::signing(), now, content.day_ticks);
    crate::squad::sign_on(content, place.roster, Some(&mut *place.lockers), hand, trade);
    place.roster.hands[hand].xp = 1;
    place.roster.merchant = Some(hand);
    Some(Hired { hand, trade, retrained: false })
}

/// The review of a town's idle hands (`Jobs.employ`): an unemployed adult, or failing one a slack hand, takes a trade,
/// a contract's first, else drawn from the bag, if the town can pay for its stall. At most one a call. A town without a
/// desk (a Merchant) puts up no stalls.
pub fn employ(content: &Content, books: &mut Books, bills: &Bills, signals: &Signals, now: u64, place: &mut Place)
        -> Option<Hired> {
    if place.roster.merchant.is_none() && place.routed {
        return to_the_desk(content, books, now, place);
    }
    place.roster.merchant?;
    place.lockers.sync(content, place.roster);
    // An adult with no trade, else a Nitwit: a Nitwit takes a trade as any idle adult does (John: "Why don't we just get
    // rid of nitwits?": seed 1717's Refuge Pointe, its idle all Nitwits, could staff nothing).
    let free = |nitwit: bool| place.roster.hands.iter()
        .position(|h| !h.is_baby() && !h.away && content.trades.get(h.trade).is_some_and(|t| if nitwit { t.nitwit } else { t.none }));
    let idle = free(false).or_else(|| free(true));
    let retraining = idle.is_none();
    // A home or a hall waiting on timber in a town with no Lumberjack signs one on first (John: "make lumber demand
    // sufficiently high such that they show up early to settlements rather than as a later role"); with no hand idle
    // or slack, it retrains its least-trained hand whose work isn't the town's mainstay (not its Merchant, nor a trade
    // kept: seed 1717's Refuge Pointe, its idle all Nitwits and no beds for a child, waited on timber 25 days).
    let timber = content.trade(crate::roster::LUMBERJACK)
        .filter(|&l| place.timber_wait > 0.0 && place.roster.count(l) == 0);
    // And a comfort it lacks and holds the makings of, with no hand of its maker: from its idle or slack first, else, once
    // in `comfort_retrain_days`, retrained as for timber (John: "Export the creations instead of rapidly retraining.
    // Allow a long cooldown retraining"; seed 1717's Refuge Pointe held 1,000 paper and leather 37 days, no hand free).
    let comfort = for_comfort(content, place.stock, place.met, place.roster);
    let cooled = place.roster.comfort_retrained.is_none_or(|d| {
        now.saturating_sub(d) as f64 >= content.dials.comfort_retrain_days * content.day_ticks as f64
    });
    let free_hand = idle.or_else(|| slack(content, place));
    let retrain_for_comfort = free_hand.is_none() && timber.is_none() && comfort.is_some() && cooled;
    let first = timber.or(comfort.filter(|_| retrain_for_comfort || free_hand.is_some()));
    let hand = free_hand.or_else(|| first.and_then(|_| {
        place.roster.hands.iter().enumerate()
            .filter(|&(i, h)| Some(i) != place.roster.merchant && !h.is_baby() && !h.away
                && content.trades.get(h.trade).is_some_and(|t| !t.idle() && !t.merchant && !KEPT.contains(&t.name.as_str())))
            .min_by_key(|&(i, h)| (h.level, std::cmp::Reverse(i))).map(|(i, _)| i)
    }))?;
    let stall = crate::fixed::Fixed::ceil(content.dials.stall_emeralds);
    let reserve = crate::rules::farms::build_reserve(content, place.trades, Some(place.farm));
    let cash = *place.money - reserve;
    // Short of coin for the stall, the treasury may borrow its households' savings.
    if cash + on_call(content, *place.purse, place.deposits.owed) < stall {
        return None;
    }
    let trade = first.or_else(|| for_contracts(content, place)).or_else(|| for_glut(content, place.stock, place.roster, place.herd)).or_else(|| {
        let mut marbles = town_bag(content, signals, place.town, place.stock, place.farm, place.met, place.purse.to_f64(),
            place.roster);
        for &(t, m) in &place.far {
            put(&mut marbles, t, m);
        }
        draw(&marbles, books.rng)
    })?;
    if retraining && place.roster.count(trade) > 0 {
        return None; // nothing it needs that the town hasn't: the hand keeps its trade
    }
    // A caravan's lead (drawn for its roads that stayed home with no squad, `rules/jobs.rs`): signed on as the squad
    // signs one (`squad.rs` `sign_on`, the signing fee), no stall: it keeps its trade with no workstation.
    if content.trades.get(trade).is_some_and(|t| t.merchant) {
        let fee = crate::fixed::Fixed::ceil(content.dials.signing_emeralds);
        if retraining || cash < fee {
            return None;
        }
        post(books.ledger, books.book, Side::treasury(place.town, place.money), Side::outside(), fee,
            crate::rules::squad::signing(), now, content.day_ticks);
        crate::squad::sign_on(content, place.roster, Some(&mut *place.lockers), hand, trade);
        place.roster.hands[hand].xp = 1;
        return Some(Hired { hand, trade, retrained: false });
    }
    // Out of its own money, the Accountant's say; on its households' savings, theirs. What today's budget hasn't left of
    // it the savings lend: a town of 10 emeralds and no trade, its budget half of them, never had a stall's 6 to hand
    // (seed 1717's Tarren Mill, 9 jobless for 23 days, earning nothing to save up the rest).
    if cash >= stall {
        let free = crate::rules::free(content, place.trades, &Money(*place.money), bills, place.town, now);
        let own = Accountant::left(content, now, free, place.budget).min(stall).at_least_zero();
        let lent = stall - own;
        let short = lent > on_call(content, *place.purse, place.deposits.owed);
        if !Accountant::approve(content, books.book, place.town, now, free, place.budget, Reason::Other("stall".into()),
                if short { stall } else { own }) || short {
            return None;
        }
        if lent > crate::fixed::Fixed::ZERO {
            borrow(content, books.ledger, books.book, place.town, place.money, place.purse, place.deposits, lent, now);
        }
    } else {
        borrow(content, books.ledger, books.book, place.town, place.money, place.purse, place.deposits, stall - cash, now);
    }
    let paid = stall.min(*place.money);
    post(books.ledger, books.book, Side::treasury(place.town, place.money), Side::outside(), paid,
        Reason::Other("stall".into()), now, content.day_ticks);
    // The hand takes the job at once, a locker of the trade's (`open_stall`): the same villager in a new trade, its id
    // stays.
    place.roster.hands[hand] = Hand { id: place.roster.hands[hand].id, ..Hand::new(content, trade, 1) };
    open_stall(content, place.lockers, place.roster, hand);
    if retrain_for_comfort && Some(trade) == comfort {
        place.roster.comfort_retrained = Some(now);
    }
    Some(Hired { hand, trade, retrained: retraining })
}

/// A hand's stall paid for: its locker in the books until the host puts the stall up and places it (`Lockers::stalls`);
/// a locker it had stood at its old trade's workstation, which the host takes down (`Jobs.leaveTrade`). Its trades that
/// buy goods for emeralds switched on, and time to get going (its herd to breed, its first goods) before it is judged
/// slack.
pub(crate) fn open_stall(content: &Content, lockers: &mut Lockers, roster: &Roster, hand: usize) {
    let id = roster.hands[hand].id;
    if let Some(placed) = lockers.placed.as_mut() {
        placed.remove(&id);
    }
    lockers.stalls.insert(id);
    lockers.sync(content, roster);
    let locker = &mut lockers.hands[hand];
    for o in locker.offers.iter_mut().filter(|o| !o.sells && o.level <= 1) {
        o.on = true;
    }
    locker.slack = -2 * content.dials.slack_days as i32;
}

// The host's calls for jobs.
impl crate::society::Society {
    /// Tells the job draw a town's markets want goods a trade would supply, in emeralds (`crates-and-papers` fills it).
    pub fn set_signal(&mut self, town: usize, trade: &str, emeralds: f64) {
        let t = self.content().trade(trade).unwrap_or_else(|| panic!("no trade called {trade}"));
        self.world.resource_mut::<Signals>().trades.entry(town).or_default().insert(t, emeralds.max(0.0));
    }

    /// Tells the job draw what building the town wants and could pay for now (`buildings` fills it).
    pub fn set_build_work(&mut self, town: usize, emeralds: f64) {
        self.world.resource_mut::<Signals>().build_work.insert(town, emeralds.max(0.0));
    }

    /// What the towns down a town's roads (either way) buy a day at their managed lockers, by good, each once.
    pub fn far_demands(&self, town: usize) -> Vec<BTreeMap<GoodId, f64>> {
        let mut far: Vec<usize> = self.routes.iter().filter_map(|&r| self.world.get::<Route>(r))
            .filter_map(|r| if r.from_id == town { Some(r.to_id) } else if r.to_id == town { Some(r.from_id) } else { None })
            .collect();
        far.sort_unstable();
        far.dedup();
        far.into_iter().filter_map(|t| {
            let e = self.world.entity(self.towns[t]);
            Some(crate::lockers::census_demand(e.get::<Roster>()?, e.get::<Lockers>()?))
        }).collect()
    }

    /// The bag an idle hand of the town would draw from now, by trade name.
    pub fn job_bag(&self, town: usize) -> Vec<(String, f64)> {
        let e = self.world.entity(self.towns[town]);
        let roster = self.roster(town);
        let (Some(stock), Some(farm), Some(people)) = (e.get::<Stock>(), e.get::<Farm>(), e.get::<People>()) else {
            return Vec::new();
        };
        let content = self.content();
        let mut bag = town_bag(content, self.world.resource::<Signals>(), town, stock, farm, &people.comfort_met,
            people.purse.to_f64(), &roster);
        let recipes = crate::services::recipes(content);
        for (t, m) in far_marbles(content, &recipes, &roster, &self.far_demands(town)) {
            put(&mut bag, t, m);
        }
        let unmanned: f64 = self.routes.iter().filter_map(|&r| Some((self.world.get::<crate::state::Route>(r)?,
                self.world.get::<crate::state::RouteState>(r)?)))
            .filter(|(r, st)| r.from_id == town && st.stayed == Some(crate::road::Stay::NoSquad))
            .filter_map(|(r, _)| {
                let (home, far) = (self.world.entity(r.from), self.world.entity(r.to));
                Some(crate::squad::wanted_worth(content, &home.get::<Stock>()?.0, far.get::<crate::state::Trades>()?,
                    far.get::<crate::lockers::Lockers>().zip(far.get::<crate::roster::Roster>())))
            })
            .sum();
        if let (true, Some(merchant)) = (unmanned > 0.0, content.trade(crate::roster::MERCHANT)) {
            put(&mut bag, merchant, unmanned);
        }
        if let Some((t, m)) = fishing_marbles(content, e.get::<crate::works::Works>(),
                self.world.resource::<crate::territory::Territory>().holding(town), self.map(), &roster) {
            put(&mut bag, t, m);
        }
        if let Some((t, m)) = woodland_marbles(content, self.world.resource::<crate::territory::Territory>().holding(town),
                e.get::<crate::state::Town>().and_then(|t| t.hex), self.map(), &roster, stock,
                people.timber_wait + e.get::<crate::works::Works>().map_or(0.0, |w| w.timber_wait)) {
            put(&mut bag, t, m);
        }
        bag.into_iter().map(|(t, m)| (content.trades[t].name.clone(), m)).collect()
    }

    /// Whether a route starts or ends at the town, carrying or not.
    pub fn has_routes(&self, town: usize) -> bool {
        self.routes.iter().filter_map(|&r| self.world.get::<Route>(r)).any(|r| r.from_id == town || r.to_id == town)
    }

    /// Whether a route starts or ends at the town.
    pub fn has_roads(&self, town: usize) -> bool {
        let now = self.now();
        self.routes.iter().filter_map(|&r| Some((self.world.get::<Route>(r)?, self.world.get::<RouteState>(r)?)))
            .any(|(r, st)| (r.from_id == town || r.to_id == town) && carries(self.content(), st, now))
    }

    /// The review of a town's idle hands now, whatever the day (as its look does once a day). Returns who took which
    /// trade.
    pub fn review_jobs(&mut self, town: usize) -> Option<(usize, String, bool)> {
        let e = self.towns[town];
        let now = self.now();
        let roads = self.has_roads(town);
        let routed = self.has_routes(town);
        let far_demands = self.far_demands(town);
        let mut roster = self.world.get::<Roster>(e)?.clone();
        let mut lockers = self.world.get::<Lockers>(e)?.clone();
        let (mut money, mut people, mut budget) = (*self.world.get::<Money>(e)?, self.world.get::<People>(e)?.clone(),
            *self.world.get::<Budget>(e)?);
        let (stock, trades, farm) = (self.world.get::<Stock>(e)?.clone(), self.world.get::<Trades>(e)?.clone(),
            self.world.get::<Farm>(e)?.clone());
        let mut deposits = self.world.get::<Deposits>(e).copied().unwrap_or_default();
        let herd = self.world.get::<crate::herds::Herds>(e).map(|h| h.count.clone()).unwrap_or_default();
        let signals = self.world.resource::<Signals>().clone();
        let bills = self.world.resource::<Bills>().clone();
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        let mut book = std::mem::take(&mut *self.world.resource_mut::<crate::cash_book::CashBook>());
        let seed = self.world.resource::<Rng>().clone();
        let mut dice = std::mem::take(&mut *self.world.resource_mut::<Dice>());
        let hired = {
            let mut books = Books { ledger: &mut ledger, book: &mut book, rng: dice.of(town, &seed) };
            let (met, mut purse) = (people.comfort_met.clone(), people.purse);
            let far = far_marbles(self.content(), &crate::services::recipes(self.content()), &roster, &far_demands);
            let herd = herd.clone();
            let mut place = Place { town, money: &mut money.0, purse: &mut purse, deposits: &mut deposits,
                budget: &mut budget, stock: &stock, trades: &trades, farm: &farm, met: &met,
                roster: &mut roster, lockers: &mut lockers, roads, routed, far, herd: &herd,
                timber_wait: people.timber_wait + self.world.get::<crate::works::Works>(e).map_or(0.0, |w| w.timber_wait) };
            let hired = employ(self.content(), &mut books, &bills, &signals, now, &mut place);
            people.purse = purse;
            hired
        };
        *self.world.resource_mut::<Ledger>() = ledger;
        *self.world.resource_mut::<crate::cash_book::CashBook>() = book;
        *self.world.resource_mut::<Dice>() = dice;
        self.world.entity_mut(e).insert((roster, lockers, money, people, budget, deposits));
        let content = self.content();
        hired.map(|h| (h.hand, content.trades[h.trade].name.clone(), h.retrained))
    }
}
