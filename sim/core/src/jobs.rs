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
//! [`Hand`] here, and a Nitwit is never idle for a stall (the mod takes villagers with no profession only).
//! The draw uses the world's seeded [`Rng`], not the level's.

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
pub const TRADES: [(&str, &[&str]); 13] = [
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
];

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
}

/// A good by name, with or without the host's namespace (`paper`, `minecraft:paper`).
pub fn good_named(content: &Content, name: &str) -> Option<GoodId> {
    content.goods.iter().position(|g| g.name.rsplit(':').next() == Some(name))
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

/// A town's whole bag for its idle hand now: the markets' signals and the comforts it lacks, then [`bag`].
pub fn town_bag(content: &Content, signals: &Signals, town: usize, stock: &Stock, farm: &Farm, met: &[f64],
        savings: f64, roster: &Roster) -> Marbles {
    let mut all = signals.of(town);
    for (t, m) in comfort_marbles(content, met, savings, roster) {
        put(&mut all, t, m);
    }
    bag(content, &supply(content, &farm.per_day, stock), roster, &farm.idle, signals.build_work_of(town), &all)
}

/// A town's place for its stalls: the borrowed pieces of it (the system and the host's call each lend them).
pub struct Place<'a> {
    pub town: usize,
    pub money: &'a mut f64,
    pub purse: &'a mut f64,
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

/// A hand whose trade has done nothing for the town for `slack_days` reviews, in a town with no roads (`Jobs.slack`).
fn slack(content: &Content, place: &Place) -> Option<usize> {
    if place.roads {
        return None;
    }
    let days = content.dials.slack_days as i32;
    place.lockers.hands.iter().enumerate().find_map(|(i, l)| {
        let h = place.roster.hands.get(i)?;
        let t = content.trades.get(h.trade)?;
        (l.slack >= days && !h.is_baby() && !h.away && place.roster.merchant != Some(i) && !t.merchant && !t.idle()
            && !KEPT.contains(&t.name.as_str())).then_some(i)
    })
}

/// The review of a town's idle hands (`Jobs.employ`): an unemployed adult, or failing one a slack hand, takes a trade,
/// a contract's first, else drawn from the bag, if the town can pay for its stall. At most one a call. A town without a
/// desk (a Merchant) puts up no stalls.
pub fn employ(content: &Content, books: &mut Books, bills: &Bills, signals: &Signals, now: u64, place: &mut Place)
        -> Option<Hired> {
    place.roster.merchant?;
    place.lockers.sync(content, place.roster);
    let idle = place.roster.hands.iter()
        .position(|h| !h.is_baby() && !h.away && content.trades.get(h.trade).is_some_and(|t| t.none));
    let retraining = idle.is_none();
    let hand = idle.or_else(|| slack(content, place))?;
    let stall = content.dials.stall_emeralds;
    let reserve = crate::rules::farms::reserve(content, place.trades, Some(place.farm));
    let cash = *place.money - reserve;
    // Short of coin for the stall, the treasury may borrow its households' savings.
    if cash + on_call(content, *place.purse, place.deposits.owed) < stall {
        return None;
    }
    let trade = for_contracts(content, place).or_else(|| {
        let marbles = town_bag(content, signals, place.town, place.stock, place.farm, place.met, *place.purse, place.roster);
        draw(&marbles, books.rng)
    })?;
    if retraining && place.roster.count(trade) > 0 {
        return None; // nothing it needs that the town hasn't: the hand keeps its trade
    }
    // Out of its own money, the Accountant's say; on its households' savings, theirs.
    if cash >= stall {
        let free = (*place.money - crate::rules::trading_capital(content, place.trades)
            - bills.due_by(place.town, now + (content.dials.budget_days * content.day_ticks as f64) as u64)).max(0.0);
        if !Accountant::approve(content, books.book, place.town, now, free, place.budget, Reason::Other("stall".into()),
                stall) {
            return None;
        }
    } else {
        borrow(content, books.ledger, books.book, place.town, place.money, place.purse, place.deposits, stall - cash, now);
    }
    let paid = stall.min(*place.money);
    post(books.ledger, books.book, Side::treasury(place.town, place.money), Side::outside(), paid,
        Reason::Other("stall".into()), now, content.day_ticks);
    // The hand takes the job at once: a locker of the trade's, its trades that buy goods for emeralds switched on, and
    // time to get going (its herd to breed, its first goods) before it is judged slack.
    place.roster.hands[hand] = Hand::new(content, trade, 1);
    place.lockers.sync(content, place.roster);
    let locker = &mut place.lockers.hands[hand];
    for o in locker.offers.iter_mut().filter(|o| !o.sells && o.level <= 1) {
        o.on = true;
    }
    locker.slack = -2 * content.dials.slack_days as i32;
    Some(Hired { hand, trade, retrained: retraining })
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

    /// The bag an idle hand of the town would draw from now, by trade name.
    pub fn job_bag(&self, town: usize) -> Vec<(String, f64)> {
        let e = self.world.entity(self.towns[town]);
        let roster = self.roster(town);
        let (Some(stock), Some(farm), Some(people)) = (e.get::<Stock>(), e.get::<Farm>(), e.get::<People>()) else {
            return Vec::new();
        };
        let content = self.content();
        town_bag(content, self.world.resource::<Signals>(), town, stock, farm, &people.comfort_met, people.purse, &roster).into_iter()
            .map(|(t, m)| (content.trades[t].name.clone(), m)).collect()
    }

    /// Whether a route starts or ends at the town.
    pub fn has_roads(&self, town: usize) -> bool {
        self.routes.iter().filter_map(|&r| self.world.get::<Route>(r))
            .any(|r| r.from_id == town || r.to_id == town)
    }

    /// The review of a town's idle hands now, whatever the day (as its look does once a day). Returns who took which
    /// trade.
    pub fn review_jobs(&mut self, town: usize) -> Option<(usize, String, bool)> {
        let e = self.towns[town];
        let now = self.now();
        let roads = self.has_roads(town);
        let mut roster = self.world.get::<Roster>(e)?.clone();
        let mut lockers = self.world.get::<Lockers>(e)?.clone();
        let (mut money, mut people, mut budget) = (*self.world.get::<Money>(e)?, self.world.get::<People>(e)?.clone(),
            *self.world.get::<Budget>(e)?);
        let (stock, trades, farm) = (self.world.get::<Stock>(e)?.clone(), self.world.get::<Trades>(e)?.clone(),
            self.world.get::<Farm>(e)?.clone());
        let mut deposits = self.world.get::<Deposits>(e).copied().unwrap_or_default();
        let signals = self.world.resource::<Signals>().clone();
        let bills = self.world.resource::<Bills>().clone();
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        let mut book = std::mem::take(&mut *self.world.resource_mut::<crate::cash_book::CashBook>());
        let mut rng = std::mem::take(&mut *self.world.resource_mut::<Rng>());
        let hired = {
            let mut books = Books { ledger: &mut ledger, book: &mut book, rng: &mut rng };
            let (met, mut purse) = (people.comfort_met.clone(), people.purse);
            let mut place = Place { town, money: &mut money.0, purse: &mut purse, deposits: &mut deposits,
                budget: &mut budget, stock: &stock, trades: &trades, farm: &farm, met: &met,
                roster: &mut roster, lockers: &mut lockers, roads };
            let hired = employ(self.content(), &mut books, &bills, &signals, now, &mut place);
            people.purse = purse;
            hired
        };
        *self.world.resource_mut::<Ledger>() = ledger;
        *self.world.resource_mut::<crate::cash_book::CashBook>() = book;
        *self.world.resource_mut::<Rng>() = rng;
        self.world.entity_mut(e).insert((roster, lockers, money, people, budget, deposits));
        let content = self.content();
        hired.map(|h| (h.hand, content.trades[h.trade].name.clone(), h.retrained))
    }
}
