//! What a Merchant knows (`docs/decisions.md` "News travels with carriers: each desk keeps an atlas", "Towns
//! commission their own caravans", "The town's shop, the Merchant's books, and its reasons"): the markets it has heard
//! of, the last stocktake of each market that a carrier brought back, its books of what was sent, sold, bought and
//! brought home, and the tally of its reasons.
//!
//! - **The atlas.** A town knows every placed town within word of mouth (`word_of_mouth`, 400 blocks; a market with a
//!   caravanserai is heard of from 800), and those it has learned of: both ends of a route learn each other as its
//!   wagon leaves, and each town whose land a wagon's road crossed learns both its ends when the wagon is home.
//! - **Stocktakes.** What a market held when last counted, and when: a town's own as each of its wagons leaves (with no
//!   time, as the mod's `recordStocktake(desk, totals)` takes it: it never makes the books fresh), and a far end's when
//!   a Librarian is aboard a wagon there, after the sale (or, for a home that keeps no roster, at every visit). A
//!   town with a roster plans its wagons' loads on the far end's stocktake while it is fresh (`stale_books_days`), less
//!   what other wagons on their way there carry; with none, or a stale one, on the far end's stock as it is (today's
//!   rule). Stale books want a Librarian along ([`trip_wants`]).
//! - **Books.** Each town's entries by day: what it sent (`Sent`), what came home (`Home`), what it bought at its desk
//!   from a wagon (`Bought`), and, from its wagons' logs handed in at home, what they sold there (`Sold`). They go back
//!   10 days with a Merchant alone, 60 with a Librarian in town, 365 with a library ([`keeps`]). A Merchant values a
//!   good at a market by the last price its books show there ([`Books::last_price`]).
//! - **Reasons** ([`Whys`]): why each good went or stayed, why the far end wouldn't buy the rest, why no wagon left and
//!   why one went. A tally (how often, how many, the last day), never a page; a reason in a wagon's log is tallied
//!   when it's handed in.
//! - **Commissions.** At its reviews, a town with no route of its own, short of some kind of comfort and
//!   `commission_spare` beyond its reserve, opens a route to the nearest market it knows, within a leg's food, whose
//!   last stocktake holds that kind ([`choose`]).
//!
//! Ported from:
//!
//! - `caravan/Atlas.java`: `WORD_OF_MOUTH`, `learn` (42), `learnBoth`, `known` (57: heard of, and neighbours within
//!   reach, nearest first), `reach` (`Buildings.CARAVANSERAI_REACH`), `markets`.
//! - `merchant/Warehouse.java`: `recordStocktake` (186), `stocktakeTime`, `onHand` (203).
//! - `caravan/CaravanServices.java`: `booksStale` (113), `copyBooks` (116); `Caravans.STALE_BOOKS_TICKS`.
//! - `merchant/Books.java`: `MERCHANT_DAYS`, `LIBRARIAN_DAYS`, `LIBRARY_DAYS`, `keeps`, `enter` (113), `why` (122),
//!   `tally`, `whys`, `file` (156), `lastPrice` (185), `Kind`, `Entry`.
//! - `caravan/Caravans.java`: `pickCargo` (2361: the reasons), `dispatch` 476–493 (comforts' reasons), 498 (home's
//!   stocktake), 510 (`went:`), 562 (`learnBoth`), 570 (`SENT`); `arriveAtDestination` 696–703 (`BOUGHT`, the log's
//!   `SOLD`); `sell`'s unsold reasons (2496–2532, `logUnsold`); `arriveHome` 1160–1192 (`HOME`, the log filed, towns
//!   crossed learn both ends); `poll` 183 (`no trip:`).
//! - `caravan/Commissions.java`: `COMMISSION_SPARE`, `consider` (46); `merchant/CommissionPayload.java` `commission`
//!   (43: only a market it knows, within a leg's food).
//!
//! **Where the core differs.** The mod's stocktake is one per market, as here, but a far end with no stocktake reads
//! as holding nothing (its wagons haul its whole par); the core reads its stock as it is, so towns without Librarians
//! trade as before, and a stocktake gone stale is read the same way rather than for ever. Reasons are tallied when a
//! wagon leaves, or once a day while it stays (the mod tallies at each try, four a day). A page of the books takes no
//! stationery yet (`Paperwork.drawn(desk, 1, LEDGER)`). A commissioned route carries a shopping purse like any town's wagon
//! (`desires.rs`), and a Cartographer's news isn't brought home yet (`services`).

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::hex::Hex;
use crate::messages::{RouteId, TownId};
use crate::roster::{Roster, TradeId};
use crate::society::Society;
use crate::state::*;

/// A market with a caravanserai is heard of this far (`Buildings.CARAVANSERAI_REACH`).
pub const CARAVANSERAI_REACH: f64 = 800.0;
/// Days a town's books go back with a Merchant alone, with a Librarian in town, with a library (`Books.MERCHANT_DAYS`,
/// `LIBRARIAN_DAYS`, `LIBRARY_DAYS`).
pub const MERCHANT_DAYS: u64 = 10;
pub const LIBRARIAN_DAYS: u64 = 60;
pub const LIBRARY_DAYS: u64 = 365;
/// The trade that copies the far end's books.
pub const LIBRARIAN: &str = "librarian";

/// The markets each town has heard of, and when (`Atlas.known`); the towns' commissions' clocks (the day each last
/// considered one) and the routes they commissioned this look, opened once it's done (`Society::advance`).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Atlas {
    pub known: BTreeMap<TownId, BTreeMap<TownId, u64>>,
    pub considered: BTreeMap<TownId, u64>,
    pub pending: Vec<(TownId, TownId)>,
}

impl Atlas {
    /// The town hears of the market `other` (`Atlas.learn`): itself, never.
    pub fn learn(&mut self, town: TownId, other: TownId, now: u64) {
        if town != other {
            self.known.entry(town).or_default().entry(other).or_insert(now);
        }
    }

    /// Each hears of the other (`Atlas.learnBoth`).
    pub fn learn_both(&mut self, a: TownId, b: TownId, now: u64) {
        self.learn(a, b, now);
        self.learn(b, a, now);
    }

    /// The markets the town has heard of, without its neighbours.
    pub fn heard(&self, town: TownId) -> impl Iterator<Item = TownId> + '_ {
        self.known.get(&town).into_iter().flat_map(|m| m.keys().copied())
    }
}

/// What a market held when last counted (`Warehouse.stocktake`), and when: none for a town's own as its wagon leaves
/// (the mod's -1: it never makes the books fresh).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stocktake {
    pub stock: Vec<f64>,
    pub when: Option<u64>,
}

/// Each market's last stocktake (`Warehouse.recordStocktake`, one per market as the mod keeps it).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stocktakes(pub BTreeMap<TownId, Stocktake>);

impl Stocktakes {
    pub fn record(&mut self, market: TownId, stock: &[f64], when: Option<u64>) {
        self.0.insert(market, Stocktake { stock: stock.to_vec(), when });
    }

    /// How many of a good the market held at its last stocktake (`Warehouse.onHand`): none if never taken.
    pub fn on_hand(&self, market: TownId, good: GoodId) -> f64 {
        self.0.get(&market).and_then(|s| s.stock.get(good)).copied().unwrap_or(0.0)
    }

    /// Are the market's books stale (`CaravanServices.booksStale`): never copied, or over `stale_books_days` old?
    pub fn stale(&self, content: &Content, market: TownId, now: u64) -> bool {
        self.fresh(content, market, now).is_none()
    }

    /// The market's stocktake, if a carrier copied it within `stale_books_days`.
    pub fn fresh(&self, content: &Content, market: TownId, now: u64) -> Option<&Stocktake> {
        let stale = (content.dials.stale_books_days * content.day_ticks as f64) as u64;
        self.0.get(&market).filter(|s| s.when.is_some_and(|w| now.saturating_sub(w) <= stale))
    }
}

/// What kind of dealing a line of the books is (`Books.Kind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Bought,
    Sold,
    Sent,
    Home,
    Shop,
    Why,
}

/// Why the far end wants a good (`Caravans.wantsAt`, as far as the core knows it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Want {
    /// Its people use it.
    Uses,
    /// A locker of its takes it.
    Locker,
    /// It's an input to goods the town has contracted to sell (`Trades.inputs`; `Caravans.wantsAt`'s first reason).
    Inputs,
    /// A player's shop there ordered it (`Caravans.wantsThere`, `orders.rs`).
    Ordered,
}

impl Want {
    pub fn of(trades: &Trades, good: GoodId) -> Want {
        if trades.inputs.get(good).is_some_and(|n| *n > 0.0) {
            Want::Inputs
        } else if trades.capacity.get(good).is_some_and(|c| *c > 0.0) { Want::Locker } else { Want::Uses }
    }

    fn label(self) -> &'static str {
        match self {
            Want::Uses => "its people use it",
            Want::Locker => "a trade it has switched on takes it",
            Want::Inputs => "an input to goods it's contracted to sell",
            Want::Ordered => "a player's shop there ordered it",
        }
    }
}

/// A reason (`Books.why`): the Merchant's own reckoning, tallied, never a page.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Why {
    /// The far end holds its par or more of it, as last seen (`pickCargo`).
    HoldTheirFill(Want),
    /// Home holds none.
    HaveNone(Want),
    /// What home holds is set aside for a forward sale.
    SetAside(Want),
    /// What home holds is its own shelf (the core keeps it: `plan_sale`).
    KeepOurShelf(Want),
    /// The far end wouldn't deal at any price (`bargain.rs`).
    NoDeal(Want),
    /// The wagon is full.
    NoRoomAboard,
    Sent(Want),
    /// The far end is short of a kind of comfort home holds no more of than its own par (`dispatch` 476–483), or none.
    ShortKeepOurPar(usize),
    ShortHaveNone(usize),
    /// A comfort sent where they're short of its kind.
    SentComfort(usize),
    /// Why the far end didn't buy the rest (`sell`, in the wagon's log): it holds its fill (of a kind of comfort), it
    /// can't pay, it has no use for it.
    UnsoldFull(Option<usize>),
    UnsoldCantPay,
    UnsoldNoUse,
    /// No wagon left today, and why (`poll` 183: "no trip: " and the route's status).
    NoTrip(String),
    /// A wagon left (`dispatch` 510): full, or on its day; by porter.
    Went { full: bool, porter: bool },
    /// Any other, in words.
    Note(String),
}

impl Why {
    /// The reason in words, as the mod's books print it.
    pub fn label(&self, content: &Content) -> String {
        let kind = |k: &usize| content.comforts.get(*k).cloned().unwrap_or_else(|| "it".into());
        match self {
            Why::HoldTheirFill(w) => format!("not sent: they hold their fill ({})", w.label()),
            Why::HaveNone(w) => format!("not sent: we have none ({})", w.label()),
            Why::SetAside(w) => format!("not sent: set aside for a forward sale ({})", w.label()),
            Why::KeepOurShelf(w) => format!("not sent: we keep our own shelf ({})", w.label()),
            Why::NoDeal(w) => format!("not sent: they wouldn't deal ({})", w.label()),
            Why::NoRoomAboard => "not sent: no room aboard".into(),
            Why::Sent(w) => format!("sent: {}", w.label()),
            Why::ShortKeepOurPar(k) => format!("not sent: they're short of {}, but we keep our own par", kind(k)),
            Why::ShortHaveNone(k) => format!("not sent: they're short of {}, and we have none", kind(k)),
            Why::SentComfort(k) => format!("sent: they're short of {}", kind(k)),
            Why::UnsoldFull(Some(k)) => format!("unsold: they hold their fill of {}", kind(k)),
            Why::UnsoldFull(None) => "unsold: they hold their fill".into(),
            Why::UnsoldCantPay => "unsold: they can't pay".into(),
            Why::UnsoldNoUse => "unsold: they've no use for it".into(),
            Why::NoTrip(s) => format!("no trip: {s}"),
            Why::Went { full, porter } => format!("went: {}{}", if *full { "full" } else { "on its day" },
                if *porter { ", by porter" } else { "" }),
            Why::Note(s) => s.clone(),
        }
    }
}

/// The words for why a wagon stayed home (the mod's route status).
pub fn stay_label(stay: crate::road::Stay) -> &'static str {
    use crate::road::Stay::*;
    match stay {
        OutOfRange => "the cheapest path is too long for one leg's food",
        CantFund => "the hub can't fund the trip",
        WaitingForALoad => "waiting for a fuller load",
        NoSquad => "no squad at home",
        NoMerchant => "no Merchant at the hub",
    }
}

/// One line of a town's books (`Books.Entry`): on `day`, `count` of `good` for `money` (emeralds' worth, the lot),
/// with `with` (none: the town itself); a reason says `why`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub day: u64,
    pub kind: Kind,
    pub with: Option<TownId>,
    pub good: Option<GoodId>,
    pub count: f64,
    pub money: crate::fixed::Fixed,
    pub why: Option<Why>,
}

impl Entry {
    pub fn new(day: u64, kind: Kind, with: TownId, good: GoodId, count: f64, money: crate::fixed::Fixed) -> Self {
        Entry { day, kind, with: Some(with), good: Some(good), count, money, why: None }
    }

    /// A reason, for a wagon's log (`Entry.why`).
    pub fn why(day: u64, with: TownId, good: Option<GoodId>, why: Why, count: f64) -> Self {
        Entry { day, kind: Kind::Why, with: Some(with), good, count, money: crate::fixed::Fixed::ZERO, why: Some(why) }
    }
}

/// How often a reason was given, how many items it was about in all, and the last day it was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tally {
    pub times: u64,
    pub items: f64,
    pub last: u64,
}

/// A reason's key in a town's tally: with whom, which good (none: the trip as a whole), why.
pub type WhyKey = (Option<TownId>, Option<GoodId>, Why);

/// Each town's reasons (`Books.whys`), and the day each route last told why it stayed (once a day).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Whys {
    pub tally: BTreeMap<TownId, BTreeMap<WhyKey, Tally>>,
    pub noted: BTreeMap<RouteId, u64>,
}

impl Whys {
    /// A reason, in the town's tally (`Books.why`): why `count` of `good` went (or didn't) with `with`.
    pub fn why(&mut self, town: TownId, with: Option<TownId>, good: Option<GoodId>, why: Why, count: f64, day: u64) {
        let t = self.tally.entry(town).or_default().entry((with, good, why)).or_default();
        t.times += 1;
        t.items += count.max(0.0);
        t.last = t.last.max(day);
    }
}

/// Each town's books (`Books.books`), oldest first.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Books(pub BTreeMap<TownId, Vec<Entry>>);

impl Books {
    /// Lines filed in the town's books (a wagon's log handed in, or its own dealings: `Books.file`): its reasons go in
    /// the tally, no page; the rest are kept as far back as it `keeps` days.
    pub fn file(&mut self, whys: &mut Whys, town: TownId, today: u64, keeps: u64, lines: Vec<Entry>) {
        let mut pages = Vec::new();
        for e in lines {
            if e.kind == Kind::Why {
                whys.why(town, e.with, e.good, e.why.clone().unwrap_or(Why::Note(String::new())), e.count, e.day);
            } else {
                pages.push(e);
            }
        }
        if pages.is_empty() {
            return;
        }
        let book = self.0.entry(town).or_default();
        book.extend(pages);
        let oldest = today.saturating_sub(keeps);
        book.retain(|e| e.day >= oldest);
    }

    /// A line in the town's books (`Books.enter`): nothing for nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn enter(&mut self, whys: &mut Whys, town: TownId, today: u64, keeps: u64, kind: Kind, with: TownId,
            good: GoodId, count: f64, money: crate::fixed::Fixed) {
        if count <= 0.0 && money <= crate::fixed::Fixed::ZERO {
            return;
        }
        self.file(whys, town, today, keeps, vec![Entry::new(today, kind, with, good, count, money)]);
    }

    /// The last price the town's books show for one of `good` sold to or bought from `market` (`Books.lastPrice`).
    pub fn last_price(&self, town: TownId, market: TownId, good: GoodId) -> Option<f64> {
        self.0.get(&town)?.iter().rev()
            .find(|e| e.good == Some(good) && e.with == Some(market) && e.count > 0.0 && e.money.is_positive()
                && matches!(e.kind, Kind::Sold | Kind::Bought))
            .map(|e| e.money.to_f64() / e.count)
    }
}

/// Days a town's books go back (`Books.keeps`): a year with a library, 60 days with a Librarian among its people, else
/// the Merchant's 10.
pub fn keeps(content: &Content, roster: Option<&Roster>, works: Option<&crate::works::Works>) -> u64 {
    if works.is_some_and(|w| w.built(crate::works::Kind::Library)) {
        return LIBRARY_DAYS;
    }
    let librarian = content.trade(LIBRARIAN);
    if roster.is_some_and(|r| r.hands.iter().any(|h| Some(h.trade) == librarian)) { LIBRARIAN_DAYS } else { MERCHANT_DAYS }
}

/// Does the hand's trade copy books (a Librarian)?
pub fn copies_books(content: &Content, trade: TradeId) -> bool {
    content.trade(LIBRARIAN) == Some(trade)
}

/// The trades a route's trip wants of its squad (`Services.wantedAbroad`): the host's (`RouteState.wanted_trades`)
/// and a Librarian while the far end's books are stale.
pub fn trip_wants(content: &Content, state: &RouteState) -> Vec<TradeId> {
    let mut wanted = state.wanted_trades.clone();
    // And the trades whose services there the ledgers say the far end would want (`services.rs`).
    for &t in &state.services_wanted {
        if !wanted.contains(&t) {
            wanted.push(t);
        }
    }
    if state.books_stale {
        if let Some(l) = content.trade(LIBRARIAN) {
            if !wanted.contains(&l) {
                wanted.push(l);
            }
        }
    }
    wanted
}

/// A wagon's takings for a good at the far end this visit (`rules/arrive.rs::sell`), for both Merchants' books.
pub fn paid(wagon: &mut Wagon, good: GoodId, money: crate::fixed::Fixed) {
    if wagon.paid_here.len() <= good {
        wagon.paid_here.resize(good + 1, crate::fixed::Fixed::ZERO);
    }
    wagon.paid_here[good] += money;
}

/// What the atlas reads of the towns: where each stands (if placed) and how far word of it carries, and the routes'
/// lengths between towns (for towns not both placed).
#[derive(Clone, Debug, Default)]
pub struct Places {
    pub hexes: Vec<Option<Hex>>,
    pub reach: Vec<f64>,
    pub roads: BTreeMap<(TownId, TownId), u32>,
}

impl Places {
    /// How far apart two towns are, in blocks (`Atlas.known`'s `distSqr`): between their hexes' centres if both are
    /// placed, else a route's length between them, else not known.
    pub fn distance(&self, a: TownId, b: TownId) -> Option<u32> {
        match (self.hexes.get(a).copied().flatten(), self.hexes.get(b).copied().flatten()) {
            (Some(x), Some(y)) => {
                let (dx, dz) = (x.center_x() - y.center_x(), x.center_z() - y.center_z());
                Some((dx * dx + dz * dz).sqrt().ceil() as u32)
            }
            _ => self.roads.get(&(a, b)).or_else(|| self.roads.get(&(b, a))).copied(),
        }
    }

    /// The markets a town knows (`Atlas.known`), nearest first (then by number): those it has heard of, and every
    /// placed town within word of mouth of it (further for one with a caravanserai).
    pub fn known(&self, content: &Content, atlas: &Atlas, town: TownId) -> Vec<TownId> {
        let mut out: Vec<TownId> = atlas.heard(town).filter(|&o| o != town && o < self.hexes.len()).collect();
        if self.hexes.get(town).copied().flatten().is_some() {
            for other in 0..self.hexes.len() {
                if other == town || out.contains(&other) || self.hexes[other].is_none() {
                    continue;
                }
                let reach = self.reach.get(other).copied().unwrap_or(content.dials.word_of_mouth);
                if self.distance(town, other).is_some_and(|d| d as f64 <= reach) {
                    out.push(other);
                }
            }
        }
        out.sort_by_key(|&o| (self.distance(town, o).unwrap_or(u32::MAX), o));
        out
    }
}

/// How far word of a market carries (`Atlas.reach`): further from a town with a caravanserai.
pub fn reach(content: &Content, works: Option<&crate::works::Works>) -> f64 {
    if works.is_some_and(|w| w.built(crate::works::Kind::Caravanserai)) { CARAVANSERAI_REACH } else { content.dials.word_of_mouth }
}

/// What a town considering a commission is (`Commissions.consider`'s inputs): whether it has a route of its own,
/// how well each kind of comfort was met, and its money beyond its reserve.
pub struct Considering<'a> {
    pub town: TownId,
    pub has_route: bool,
    pub comfort_met: &'a [f64],
    pub spare: f64,
}

/// The market a town commissions a route to (`Commissions.consider`): with no route of its own, short of some kind of
/// comfort (met below 1), with `commission_spare` beyond its reserve, the nearest market it knows within a leg's food
/// whose last stocktake holds a good of a kind it's short of.
pub fn choose(content: &Content, atlas: &Atlas, stocktakes: &Stocktakes, places: &Places, town: &Considering) -> Option<TownId> {
    let short: Vec<usize> = town.comfort_met.iter().enumerate().filter(|(_, m)| **m < 1.0).map(|(k, _)| k).collect();
    if town.has_route || short.is_empty() || town.spare < content.dials.commission_spare {
        return None;
    }
    places.known(content, atlas, town.town).into_iter().find(|&other| {
        places.distance(town.town, other).is_some_and(|d| crate::road::in_range(content, d))
            && (0..content.goods.len()).any(|g| content.goods[g].comfort.is_some_and(|k| short.contains(&k))
                && stocktakes.on_hand(other, g) > 0.0)
    })
}

/// A line of a town's reasons, as a host shows it: "with | good | why", how often, how many items, the last day.
#[derive(Clone, Debug, PartialEq)]
pub struct WhyLine {
    pub line: String,
    pub times: u64,
    pub items: f64,
    pub last: u64,
}

impl Society {
    /// What the atlas reads of the towns now.
    pub(crate) fn places(&self) -> Places {
        let n = self.towns.len();
        let mut places = Places { hexes: vec![None; n], reach: vec![0.0; n], roads: BTreeMap::new() };
        for t in 0..n {
            let e = self.world.entity(self.towns[t]);
            places.hexes[t] = e.get::<Town>().and_then(|t| t.hex);
            places.reach[t] = reach(self.content(), e.get::<crate::works::Works>());
        }
        for r in self.routes.iter().filter_map(|&e| self.world.get::<Route>(e)) {
            places.roads.entry((r.from_id, r.to_id)).and_modify(|b| *b = (*b).min(r.blocks)).or_insert(r.blocks);
        }
        places
    }

    /// The markets a town knows, nearest first (`Atlas.known`).
    pub fn known(&self, town: TownId) -> Vec<TownId> {
        self.places().known(self.content(), self.world.resource::<Atlas>(), town)
    }

    /// A route's two ends: its home, and its far end.
    pub fn route_ends(&self, route: RouteId) -> (TownId, TownId) {
        let r = self.world.get::<Route>(self.routes[route]).expect("a route");
        (r.from_id, r.to_id)
    }

    /// The town hears of a market (`Atlas.learn`): a host's news, or a test's.
    pub fn learn(&mut self, town: TownId, other: TownId) {
        let now = self.now();
        self.world.resource_mut::<Atlas>().learn(town, other, now);
    }

    /// A market's last stocktake, and when it was taken (none: as its own wagon left).
    pub fn stocktake(&self, market: TownId) -> Option<Stocktake> {
        self.world.resource::<Stocktakes>().0.get(&market).cloned()
    }

    /// Records a market's stocktake (`Warehouse.recordStocktake`): what it holds of each good named, taken at `when`.
    pub fn record_stocktake(&mut self, market: TownId, held: &[(GoodId, f64)], when: Option<u64>) {
        let mut stock = vec![0.0; self.content().goods.len()];
        for &(g, n) in held {
            stock[g] += n;
        }
        self.world.resource_mut::<Stocktakes>().record(market, &stock, when);
    }

    /// A town's books, oldest first.
    pub fn books(&self, town: TownId) -> Vec<Entry> {
        self.world.resource::<Books>().0.get(&town).cloned().unwrap_or_default()
    }

    /// The last price a town's books show for one of a good at a market (`Books.lastPrice`).
    pub fn last_price(&self, town: TownId, market: TownId, good: GoodId) -> Option<f64> {
        self.world.resource::<Books>().last_price(town, market, good)
    }

    /// Lines filed in a town's books (`Books.file`): a host's, or a wagon's log in a test.
    pub fn file(&mut self, town: TownId, lines: Vec<Entry>) {
        let today = self.now() / self.content().day_ticks.max(1);
        let e = self.world.entity(self.towns[town]);
        let keeps = keeps(self.content(), e.get::<Roster>(), e.get::<crate::works::Works>());
        self.world.resource_scope(|world, mut books: bevy_ecs::prelude::Mut<Books>| {
            let mut whys = world.resource_mut::<Whys>();
            books.file(&mut whys, town, today, keeps, lines);
        });
    }

    /// A town's reasons, most often first (`Books.whys`), "with | good | why" as the mod's books print them.
    pub fn whys(&self, town: TownId) -> Vec<WhyLine> {
        let content = self.content();
        let name = |t: Option<TownId>| t.and_then(|t| self.towns.get(t))
            .and_then(|&e| self.world.get::<Town>(e)).map_or_else(|| "the town".to_string(), |t| t.name.clone());
        let mut out: Vec<WhyLine> = self.world.resource::<Whys>().tally.get(&town).into_iter().flatten()
            .map(|((with, good, why), t)| WhyLine {
                line: format!("{} | {} | {}", name(*with), good.map_or("-", |g| content.goods[g].name.as_str()),
                    why.label(content)),
                times: t.times,
                items: t.items,
                last: t.last,
            })
            .collect();
        out.sort_by(|a, b| b.times.cmp(&a.times));
        out
    }

    /// Commissions a route from a town to a market (`CommissionPayload.commission`): refused for a market the town
    /// doesn't know, one beyond a leg's food, or one it already sends to. Opened as [`Society::open_route`] does.
    pub fn commission(&mut self, from: TownId, to: TownId) -> Result<RouteId, String> {
        if from == to {
            return Err("a market doesn't send caravans to itself".into());
        }
        if self.has_route(from, Some(to)) {
            return Err("it already sends caravans there".into());
        }
        if !self.known(from).contains(&to) {
            return Err("not a market it knows".into());
        }
        let places = self.places();
        let Some(blocks) = places.distance(from, to) else { return Err("nobody knows how far it is".into()) };
        if !crate::road::in_range(self.content(), blocks) {
            return Err(format!("{blocks} blocks is too far for a leg's food"));
        }
        self.open_route(from, to).ok_or_else(|| "no way there".into())
    }

    /// The town considers a caravan of its own now (`Commissions.consider`), as its reviews do: the route opened, if
    /// it commissioned one.
    pub fn consider_commission(&mut self, town: TownId) -> Option<RouteId> {
        let e = self.towns[town];
        let has_route = self.has_route(town, None);
        let content = self.content().clone();
        let entity = self.world.entity(e);
        let met = entity.get::<People>()?.comfort_met.clone();
        let spare = entity.get::<Money>()?.0
            - crate::rules::farms::reserve(&content, entity.get::<Trades>()?, entity.get::<crate::farms::Farm>());
        let places = self.places();
        let to = choose(&content, self.world.resource::<Atlas>(), self.world.resource::<Stocktakes>(), &places,
            &Considering { town, has_route, comfort_met: &met, spare: spare.to_f64() })?;
        self.open_route(town, to)
    }

    /// Does a town send caravans (to `to`, or anywhere)?
    fn has_route(&self, from: TownId, to: Option<TownId>) -> bool {
        self.routes.iter().filter_map(|&e| self.world.get::<Route>(e))
            .any(|r| r.from_id == from && to.is_none_or(|t| r.to_id == t))
    }

    /// A route from one town to another, a wagon every `wagon_wait_days` (a dial): a road over the map if both are placed and
    /// there's a way; else a bare road at their distance (straight, or a route's between them), its food a hex's for
    /// each hex of it.
    pub fn open_route(&mut self, from: TownId, to: TownId) -> Option<RouteId> {
        let every = self.content().dials.wagon_wait_days;
        if self.town_hex(from).is_some() && self.town_hex(to).is_some() {
            if let Some(id) = self.add_road(from, to, every) {
                return Some(id);
            }
        }
        let blocks = self.places().distance(from, to)?;
        let hexes = (blocks as f64 / crate::hex::ACROSS).ceil().max(1.0);
        let cost = self.per_hex() * hexes;
        Some(self.add_route(from, to, blocks.max(1) as f64 / crate::map::BLOCKS_PER_DAY, cost, every))
    }

    /// The routes towns commissioned at this look's reviews, opened (a town that has a route by now opens none).
    pub(crate) fn open_commissions(&mut self) {
        let pending = std::mem::take(&mut self.world.resource_mut::<Atlas>().pending);
        for (from, to) in pending {
            if !self.has_route(from, None) {
                self.open_route(from, to);
            }
        }
    }
}
