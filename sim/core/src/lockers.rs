//! A town's lockers (decision 2 of `docs/core-plan.md` section 2): a hand's trades, switched on or off, run against the
//! town's warehouse at each restock, two a day, if the desk's Merchant manages that hand's locker. A town with a
//! [`Roster`] keeps a [`Lockers`]: for each hand, its offers (from its trade's in [`Content`], each switched off
//! until the host or the Merchant's review switches it on), and which hands the Merchant manages. Its income from
//! the goods its switched-on offers buy comes from those trades, not from `Trades.uses` (set aside while they're on).
//!
//! Ported from:
//!
//! - `locker/LockerTrading.java` `trade` (138–311): the switched-on offers that aren't inflated, best deal first
//!   (fewest inputs per item received), as many runs as their uses, those that cost emeralds within the town's money;
//!   each run takes the inputs from the warehouse (the Merchant's `supply` fills the locker, `collect` takes the
//!   results back: here both are the warehouse) and pays out; `tradeOnce` (362), `price` (352: the Merchant's
//!   discount, never below 1), `merchantReputation` (358: `MERCHANT_REPUTATION`), `isInflated` (347),
//!   `demandIfUnused` (60) and `floorCooling` (70: a restock nobody traded since cools demand to zero, never below),
//!   `rewardTradeXp` (402) and `rewardMerchantXp` (412: see `roster.rs`).
//! - Vanilla's `Villager.restock` and `MerchantOffer.updateDemand`: demand moves by the uses since the last restock
//!   less those unused (`demand + uses - (maxUses - uses)`), then the uses reset.
//! - `locker/LockerCatchUp.java` `catchUp` (131), `missedRestocks` (170: two a whole day, at most `MAX_DAYS`),
//!   `settledUntil` (149: the clock moves by the whole days replayed, keeping the part-day), `startClock` (163: never
//!   restocked, the clock starts now and nothing is owed).
//! - `locker/Lockers.java` `near` (65–89: the Merchant's lockers, contract-needed first, then its clients, then worth,
//!   then switched-on, then the nearest; here the hand's place in the roster stands for distance), `review` (105: daily,
//!   value each locker, take on clients, swap the weakest for a waiting one worth `SWAP_MARGIN` times as much, switch on
//!   a client's trades that buy goods the town holds), `worth` (181).
//! - `market/Apprentices.java` `neededTrades` (51): the trades a town's contracts need, a hook ([`Lockers::needed`])
//!   the training and sourcing slices fill.
//! - `merchant/MerchantBook.java` `noteSlack`, `addEarned`, `takeEarned`, `takenOn`.
//!
//! Not here yet: the Merchant buying what its contracts want (`Procurement`, its runs capped by what's due soon), herds'
//! feed kept back from crop sales (`feedKept`), the payroll held back from the emeralds a locker pays out; the offers a
//! villager rolls at random (here a trade offers all of its level's).
//! Where a locker stands is its host's to say (`locker_host.rs`, [`Lockers::placed`]): in a town whose host places its
//! lockers (the mod's: a chest touching a workstation, `LockerTrading.hasLocker`), only a hand whose locker stands
//! somewhere has one; in a town the core keeps as numbers every hand with a trade does.
//!
//! Simplified: one restock clock a town, not one a villager (a hand added later shares it); what a town wants, for a
//! selling offer's worth, is what its people use (`Trades.uses > 0`, where `Caravans.wantedAt` also counts the
//! warehouse's, the realm's and the workshop's inputs); the host's `Society::restock` leaves the clock where it is.

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

use crate::cash_book::{post, CashBook, Reason, Side};
use crate::content::{Content, GoodId};
use crate::roster::{merchant_trade_xp, HandId, Roster, TradeId, MAX_LEVEL};
use crate::state::{Ledger, Money, Rng, Stock, Trades};
use crate::workshop::Workshop;

/// Reputation a Merchant brings to locker trades, by its level: prices drop by reputation times the trade's price
/// multiplier (`LockerTrading.MERCHANT_REPUTATION`).
pub const MERCHANT_REPUTATION: [i32; 5] = [0, 30, 55, 80, 110];
/// Restocks a day (vanilla restocks a villager at most twice a day; catch-up replays two a whole day).
pub const RESTOCKS_PER_DAY: u64 = 2;
/// Days of missed restocks a town replays at most (`LockerCatchUp.MAX_DAYS`).
pub const MAX_DAYS: u64 = 60;
/// A waiting locker takes a client's place when worth this many times as much to the town (`Lockers.SWAP_MARGIN`)...
pub const SWAP_MARGIN: f64 = 1.5;
/// ...and at least this many emeralds a restock more (`Lockers.SWAP_GAIN`).
pub const SWAP_GAIN: f64 = 0.5;
/// A new client keeps its place this many days at least (`Lockers.CLIENT_DAYS`).
pub const CLIENT_DAYS: u64 = 2;
/// A locker worth this little a restock is doing nothing for the town (`Lockers.review`'s slack).
pub const SLACK_WORTH: f64 = 0.01;
/// The most of one item a stack holds when the cost is emeralds.
pub const EMERALD_STACK: u32 = 64;

/// The reputation of a Merchant of `level` (clamped to 1..=5).
pub fn merchant_reputation(level: i32) -> i32 {
    MERCHANT_REPUTATION[(level.clamp(1, MAX_LEVEL as i32) - 1) as usize]
}

/// One of a hand's trades: `count` of a good for `emeralds` (it buys the good: `sells` false), or `emeralds` for
/// `count` of it (it sells the good), `uses` times a restock, `xp` a trade to the hand, with vanilla's price
/// multiplier and demand; offered from the hand's `level`; `used` since its last restock; `on` when the Merchant runs
/// it. Every offer starts off.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    pub good: GoodId,
    pub count: u32,
    pub emeralds: u32,
    pub uses: u8,
    pub xp: u32,
    pub multiplier: f64,
    pub level: u8,
    pub sells: bool,
    pub demand: i32,
    pub used: u8,
    pub on: bool,
    /// Switched on by the Merchant for a training contract, and to go off again with it (`MerchantBook.training`;
    /// `training.rs`).
    pub training: bool,
    /// Switched on by the Merchant for what its contracts want of the villager (`Exchange.procured`; `sourcing.rs`),
    /// and to go off again when it isn't wanted; its runs are capped at what's due soon (`Lockers::due`).
    pub procured: bool,
}

impl Offer {
    /// A trade that buys `count` of `good` for `emeralds` (vanilla's `EmeraldForItems`), offered from level 1.
    pub fn buys(good: GoodId, count: u32, emeralds: u32, uses: u8, xp: u32, multiplier: f64) -> Self {
        Offer { good, count, emeralds, uses, xp, multiplier, level: 1, ..Default::default() }
    }

    /// A trade that sells `count` of `good` for `emeralds` (vanilla's `ItemsForEmeralds`), offered from level 1.
    pub fn sells(good: GoodId, emeralds: u32, count: u32, uses: u8, xp: u32, multiplier: f64) -> Self {
        Offer { good, count, emeralds, uses, xp, multiplier, level: 1, sells: true, ..Default::default() }
    }

    /// Offered from `level`.
    pub fn at(mut self, level: u8) -> Self {
        self.level = level;
        self
    }

    /// With `demand` (vanilla's, as a player's trades left it).
    pub fn with_demand(mut self, demand: i32) -> Self {
        self.demand = demand;
        self
    }

    /// What one trade costs before the discount (vanilla's cost A): the goods it buys, or the emeralds it charges.
    pub fn cost(&self) -> u32 {
        if self.sells { self.emeralds } else { self.count }
    }

    /// What one trade gives: the emeralds it pays, or the goods it sells.
    pub fn result(&self) -> u32 {
        if self.sells { self.count } else { self.emeralds }
    }

    /// The cost after the Merchant's discount, never below 1 nor above a stack (`LockerTrading.price`). Vanilla's
    /// multiplier is a float, and so is the product: the discount is taken in `f32` as the mod takes it.
    pub fn price(&self, reputation: i32, stack: u32) -> u32 {
        let discount = (reputation as f32 * self.multiplier as f32).floor() as i64;
        (self.cost() as i64 - discount).clamp(1, stack.max(1) as i64) as u32
    }

    /// Demand has pushed the price above its base: lockers wait for it to cool off (`LockerTrading.isInflated`).
    pub fn inflated(&self) -> bool {
        ((self.cost() as i64 * self.demand as i64) as f32 * self.multiplier as f32).floor() > 0.0
    }

    /// Vanilla's `updateDemand` at a restock, then the uses reset.
    fn restock(&mut self) {
        self.demand = self.demand + self.used as i32 - (self.uses as i32 - self.used as i32);
        self.used = 0;
    }
}

/// What a hand's offers would do for the town a restock, in emeralds (`Lockers.worth`): what its buying trades would
/// pay for the town's goods, as far as it has them (`supply`, in emeralds a good), and half what its selling trades
/// would charge for goods the town wants. Only offers of its `level` or below count.
pub fn worth(offers: &[Offer], level: u8, supply: &[f64], wanted: &[bool]) -> f64 {
    let mut worth = 0.0;
    for o in offers.iter().filter(|o| o.level <= level && o.count > 0 && o.emeralds > 0) {
        if !o.sells {
            worth += (o.uses as f64 * o.emeralds as f64).min(supply.get(o.good).copied().unwrap_or(0.0));
        } else if wanted.get(o.good).copied().unwrap_or(false) {
            worth += 0.5 * o.uses as f64 * o.emeralds as f64;
        }
    }
    worth
}

/// One hand's locker: its trade (when the hand takes another, its locker starts again), its offers, what it was
/// worth to the town at the last review, what it did since, the days it has done nothing (below 0: a grace), when
/// the Merchant took it on as a client, and whose it is (the hand's id, `roster.rs`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Locker {
    pub trade: TradeId,
    pub offers: Vec<Offer>,
    pub worth: f64,
    pub earned: f64,
    pub slack: i32,
    pub taken_on: Option<u64>,
    pub hand: HandId,
}

impl Locker {
    /// A hand of `trade`'s locker: its trade's offers, all off.
    pub fn new(content: &Content, trade: TradeId) -> Self {
        let offers = content.trades.get(trade).map_or_else(Vec::new, |t| t.offers.clone());
        Locker { trade, offers: offers.into_iter().map(|o| Offer { on: false, used: 0, ..o }).collect(),
            ..Default::default() }
    }

    /// Any offer switched on.
    pub fn switched_on(&self) -> bool {
        self.offers.iter().any(|o| o.on)
    }
}

/// A rostered town's lockers, one a hand in the roster's order (each knows its hand's id, and follows it as the roster
/// changes: [`Lockers::sync`]): which the Merchant manages (by their places, worked out again at each sync), its
/// clients (by their hands' ids, in the order it took them on), the trades its contracts need (a hook the training and sourcing slices fill), its restock
/// clock (`None`: never restocked), the day it last reviewed them, the uses of each good its switched-on offers buy
/// that were set aside from `Trades.uses` while they buy it, and what its managed lockers can take of each good a day
/// (the town's `Trades.capacity` for those goods, for the warehouse's par).
#[derive(Component, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lockers {
    pub hands: Vec<Locker>,
    pub managed: Vec<usize>,
    pub clients: Vec<HandId>,
    pub needed: Vec<TradeId>,
    pub restocked_until: Option<u64>,
    pub reviewed: Option<u64>,
    pub kept: Vec<Option<f64>>,
    pub capacity: Vec<f64>,
    /// What the training contracts the town holds ask of its Merchant at a restock (`training.rs`).
    pub lessons: crate::training::Lessons,
    /// What the town's contracts will have it deliver within `pace_days`, of each good, not yet less its stock
    /// (`Procurement.soon`; `sourcing.rs` sets it before each restock): a procured offer runs no more than that.
    pub due: Vec<f64>,
    /// The trades its managed lockers have made, all told (`LockerTrading.trade`'s count; the study's `trades`).
    pub trades: u64,
    /// Where its hands' lockers stand, when its host places them (`locker_host.rs`): a hand has a locker only where its
    /// host says one stands (a workstation with a chest touching it, `docs/decisions.md` "Lockers"). `None` (a town the
    /// core keeps as numbers, the study's): every hand with a trade has one.
    pub placed: Option<std::collections::BTreeMap<HandId, crate::points::Spot>>,
}

/// The books a locker trade writes in: the ledger (goods used and made), the cash book, and the town's dice.
pub struct Books<'a> {
    pub ledger: &'a mut Ledger,
    pub book: &'a mut CashBook,
    pub rng: &'a mut Rng,
}

/// Whether a hand has a locker the Merchant could manage: a grown-up with a trade of its own that isn't the
/// Merchant's (`LockerTrading.hasLocker`: a Merchant's desk is never a locker).
pub fn has_locker(content: &Content, roster: &Roster, i: usize) -> bool {
    let Some(h) = roster.hands.get(i) else { return false };
    !h.is_baby() && roster.merchant != Some(i)
        && content.trades.get(h.trade).is_some_and(|t| !t.idle() && !t.merchant)
}

impl Lockers {
    /// A town's lockers for its roster now: every hand's offers off, never restocked.
    pub fn new(content: &Content, roster: &Roster) -> Self {
        let mut lockers = Lockers::default();
        lockers.sync(content, roster);
        lockers
    }

    /// The hand at `i` left the roster (`settlers::take_hand`): its locker goes with it, and the later hands' lockers
    /// and managed move down one, as the roster's do (the mod keys a locker by its workstation, which stays); a client
    /// it was is one no more. (A sync after the roster changed does the same, by the hands' ids.)
    pub fn take_hand(&mut self, i: usize) {
        if i < self.hands.len() {
            let gone = self.hands.remove(i).hand;
            if gone != 0 {
                self.clients.retain(|&c| c != gone);
            }
        }
        self.managed.retain(|&c| c != i);
        for c in self.managed.iter_mut() {
            if *c > i {
                *c -= 1;
            }
        }
    }

    /// Follows the roster: a locker for every hand, in the roster's order, found by the hand's id (a new hand's, or a
    /// hand that took another trade, starts afresh), none for hands gone; the clients and managed among those still
    /// there. Hands with no id (a roster built outside a society) are matched by place.
    pub fn sync(&mut self, content: &Content, roster: &Roster) {
        let n = content.goods.len();
        self.kept.resize(n, None);
        self.capacity.resize(n, 0.0);
        let aligned = self.hands.len() >= roster.hands.len()
            && roster.hands.iter().zip(&self.hands).all(|(h, l)| l.hand == h.id);
        if aligned {
            // (The common case, and the old one: each locker already at its hand's place.)
            self.hands.truncate(roster.hands.len());
        } else {
            let mut old: Vec<Option<Locker>> = std::mem::take(&mut self.hands).into_iter().map(Some).collect();
            for (i, h) in roster.hands.iter().enumerate() {
                let at = if old.get(i).is_some_and(|l| l.as_ref().is_some_and(|l| l.hand == h.id)) {
                    Some(i)
                } else if h.id != 0 {
                    old.iter().position(|l| l.as_ref().is_some_and(|l| l.hand == h.id))
                } else {
                    None
                };
                self.hands.push(at.and_then(|at| old[at].take()).unwrap_or_else(|| Locker::new(content, h.trade)));
            }
        }
        for (h, l) in roster.hands.iter().zip(self.hands.iter_mut()) {
            if l.trade != h.trade {
                *l = Locker::new(content, h.trade);
            }
            l.hand = h.id;
        }
        if let Some(placed) = self.placed.as_mut() {
            // A hand gone takes its locker's place with it (the point stays the host's to remove, `locker_host.rs`).
            placed.retain(|&id, _| roster.place(id).is_some());
        }
        let clients: Vec<HandId> = std::mem::take(&mut self.clients);
        self.clients = clients.into_iter().filter(|&id| roster.place(id).is_some_and(|i| self.has(content, roster, i))).collect();
        self.managed = self.order(content, roster, roster.locker_limit() as usize);
        self.capacity = self.capacity(content, roster);
    }

    /// Whether the hand at `i` has a locker here: one the Merchant could manage ([`has_locker`]), standing where its
    /// host placed it if the host places this town's lockers ([`Lockers::placed`]).
    pub fn has(&self, content: &Content, roster: &Roster, i: usize) -> bool {
        has_locker(content, roster, i)
            && self.placed.as_ref().is_none_or(|placed| roster.hands.get(i).is_some_and(|h| placed.contains_key(&h.id)))
    }

    /// Whether the hand at `i` is one of the Merchant's clients.
    fn client(&self, roster: &Roster, i: usize) -> Option<usize> {
        let id = roster.hands.get(i)?.id;
        self.clients.iter().position(|&c| c == id && id != 0)
    }

    /// The hands whose lockers the Merchant manages, at most `limit`, in its order (`Lockers.near`): those whose
    /// trade a contract needs first, then its clients in the order it took them on, then worth to the town, then
    /// those with a trade switched on, then the nearest (the roster's order).
    pub fn order(&self, content: &Content, roster: &Roster, limit: usize) -> Vec<usize> {
        let mut all: Vec<usize> = (0..self.hands.len()).filter(|&i| self.has(content, roster, i)).collect();
        // (Each hand's key once, not at every comparison.)
        let keys: Vec<(bool, usize, bool, usize)> = (0..self.hands.len()).map(|i| {
            let needed = !self.needed.contains(&self.hands[i].trade);
            let client = self.client(roster, i).unwrap_or(usize::MAX);
            (needed, client, !self.hands[i].switched_on(), i)
        }).collect();
        all.sort_by(|&a, &b| {
            let (ka, kb) = (keys[a], keys[b]);
            ka.0.cmp(&kb.0).then(ka.1.cmp(&kb.1))
                .then((-self.hands[a].worth).total_cmp(&-self.hands[b].worth))
                .then(ka.2.cmp(&kb.2)).then(ka.3.cmp(&kb.3))
        });
        all.truncate(limit);
        all
    }

    /// What a hand's offers would do for the town a restock (see [`worth`]), at the hand's `level`.
    pub fn worth(&self, hand: usize, level: u8, supply: &[f64], wanted: &[bool]) -> f64 {
        self.hands.get(hand).map_or(0.0, |l| worth(&l.offers, level, supply, wanted))
    }

    /// What the managed lockers' switched-on buying offers can take of each good a day: their price times their
    /// uses (`LockerTrading.trade`'s `canTake`, recorded at each restock, for the warehouse's par: the mod counts one
    /// restock's worth as a day's, so a Librarian's 24 paper x 16 uses is 384 a day, as `docs/decisions.md` has it).
    fn capacity(&self, content: &Content, roster: &Roster) -> Vec<f64> {
        let mut out = vec![0.0; content.goods.len()];
        let rep = merchant_reputation(roster.merchant_level().unwrap_or(1) as i32);
        for &i in &self.managed {
            let level = roster.hands[i].level;
            for o in self.hands[i].offers.iter().filter(|o| o.on && !o.sells && o.level <= level) {
                if let Some(c) = out.get_mut(o.good) {
                    *c += (o.price(rep, content.goods[o.good].stack) as u64 * o.uses as u64) as f64;
                }
            }
        }
        out
    }

    /// Whether a switched-on offer buys `good`: the town's income from it comes from its lockers.
    pub fn buys(&self, good: GoodId) -> bool {
        self.hands.iter().any(|l| l.offers.iter().any(|o| o.on && !o.sells && o.good == good))
    }

    /// Whether a trade the town has switched on takes `good` (`Caravans.wantsAt` 2302–2315: the costs of every
    /// workstation's enabled trades, "a trade it has switched on takes it", whether the Merchant manages that locker or
    /// not): a hand with a locker, of the trade its locker was made for, with a buying offer of `good` switched on at
    /// its level or below (the offers a villager of that level has).
    pub fn takes(&self, content: &Content, roster: &Roster, good: GoodId) -> bool {
        self.hands.iter().enumerate().any(|(i, l)| {
            let Some(h) = roster.hands.get(i) else { return false };
            l.trade == h.trade && self.has(content, roster, i)
                && l.offers.iter().any(|o| o.on && !o.sells && o.good == good && o.level <= h.level)
        })
    }

    /// [`Lockers::takes`] of every good below `n` at once, by good: one pass over the hands' offers, not one a good.
    pub fn taken(&self, content: &Content, roster: &Roster, n: usize) -> Vec<bool> {
        let mut out = vec![false; n];
        for (i, l) in self.hands.iter().enumerate() {
            let Some(h) = roster.hands.get(i) else { continue };
            if l.trade != h.trade || !self.has(content, roster, i) {
                continue;
            }
            for o in l.offers.iter().filter(|o| o.on && !o.sells && o.level <= h.level) {
                if let Some(t) = out.get_mut(o.good) {
                    *t = true;
                }
            }
        }
        out
    }

    /// [`Lockers::buys`] of every good below `n` at once, by good.
    fn bought(&self, n: usize) -> Vec<bool> {
        let mut out = vec![false; n];
        for o in self.hands.iter().flat_map(|l| l.offers.iter()).filter(|o| o.on && !o.sells) {
            if let Some(b) = out.get_mut(o.good) {
                *b = true;
            }
        }
        out
    }

    /// While a switched-on offer buys a good, the town's people don't also buy it at its worth: its `Trades.uses` is
    /// set aside ([`Lockers::kept`]) and its `Trades.capacity` is what its managed lockers can take a day, so its par
    /// follows them (`bargain.rs`), and its workshop makes it up to that par (`workshop.rs`'s `locker_par`); when none
    /// buys it, its uses and capacity are what its people use again, and no locker's par is left for its workshop.
    pub fn set_aside(&mut self, content: &Content, trades: &mut Trades, mut workshop: Option<&mut Workshop>) {
        let n = trades.uses.len();
        self.kept.resize(n, None);
        self.capacity.resize(n, 0.0);
        trades.capacity.resize(n, 0.0);
        if let Some(w) = workshop.as_deref_mut() {
            w.locker_par.resize(n, 0.0);
        }
        let bought = self.bought(n);
        for g in 0..n {
            if bought[g] {
                if self.kept[g].is_none() || trades.uses[g] != 0.0 {
                    self.kept[g] = Some(trades.uses[g]);
                    trades.uses[g] = 0.0;
                }
                trades.capacity[g] = self.capacity[g];
                if let Some(w) = workshop.as_deref_mut() {
                    w.locker_par[g] = crate::bargain::par(content, self.capacity[g], true);
                }
            } else if let Some(kept) = self.kept[g].take() {
                if trades.uses[g] == 0.0 {
                    trades.uses[g] = kept;
                }
                trades.capacity[g] = trades.uses[g];
                if let Some(w) = workshop.as_deref_mut() {
                    w.locker_par[g] = 0.0;
                }
            }
        }
    }
}

/// A town's par for a good a trade it has switched on takes and its people don't use (`Warehouse.par(desk, item,
/// true, now)`, 138–141, as `Caravans.pickCargo` 2366 and `sell` 2480 read it for a wanted good): `shelf_days` of
/// what its lockers can take of it a day (`Trades.capacity`: its managed lockers', recorded at their sessions), rounded
/// up, a stack at least ([`crate::bargain::MIN_PAR`]: a locker the Merchant doesn't manage records no capacity), and
/// what its contracts need. `None` where its people use it (their shelf stands, `rules::shelf`) or no trade there
/// takes it. `lockers`: the town's, with its roster, if it keeps one.
pub fn wanted_par(content: &Content, trades: &Trades, lockers: Option<(&Lockers, &Roster)>, good: GoodId) -> Option<f64> {
    wanted_par_by(content, trades, |g| lockers.is_some_and(|(l, r)| l.takes(content, r, g)), good)
}

/// [`wanted_par`] with what the town's lockers take already known, by good ([`Lockers::taken`]): for a caller asking
/// of every good in turn.
pub fn wanted_par_of(content: &Content, trades: &Trades, taken: Option<&[bool]>, good: GoodId) -> Option<f64> {
    wanted_par_by(content, trades, |g| taken.is_some_and(|t| t.get(g).copied().unwrap_or(false)), good)
}

fn wanted_par_by(content: &Content, trades: &Trades, takes: impl FnOnce(GoodId) -> bool, good: GoodId) -> Option<f64> {
    if trades.uses.get(good).is_some_and(|u| *u > 0.0) {
        return None;
    }
    let capacity = trades.capacity.get(good).copied().unwrap_or(0.0);
    let takes = capacity > 0.0 || takes(good);
    takes.then(|| crate::bargain::par(content, capacity, true).max(trades.inputs.get(good).copied().unwrap_or(0.0)))
}

/// What a town's villagers at the lockers its Merchant manages would buy a day, by good (`CensusTaker.noteDemand`
/// 247–263, as the census counts it for the farm, `Territory.recordDemand`): each buying trade a managed hand has (of
/// its level or below), switched on or not, twice its cost times its uses (vanilla restocks twice a day). Only a villager
/// at a locker the Merchant manages buys from the town.
pub fn census_demand(roster: &Roster, lockers: &Lockers) -> std::collections::BTreeMap<GoodId, f64> {
    let mut out = std::collections::BTreeMap::new();
    for &i in &lockers.managed {
        let (Some(h), Some(l)) = (roster.hands.get(i), lockers.hands.get(i)) else { continue };
        if l.trade != h.trade || h.is_baby() {
            continue;
        }
        for o in l.offers.iter().filter(|o| !o.sells && o.level <= h.level && o.count > 0) {
            *out.entry(o.good).or_insert(0.0) += 2.0 * o.count as f64 * o.uses as f64;
        }
    }
    out
}

/// One restock of every hand with a locker (vanilla's `Villager.restock`, with the mod's hooks): demand moves by the
/// uses since the last, and cools no lower than zero if nobody traded since (`floorCooling`); then the lockers the
/// Merchant manages trade. Hands away don't restock. Returns the trades made.
#[allow(clippy::too_many_arguments)]
pub fn restock(content: &Content, books: &mut Books, now: u64, town: usize, roster: &mut Roster, lockers: &mut Lockers,
        stock: &mut Stock, money: &mut Money) -> u32 {
    lockers.sync(content, roster);
    let mut made = 0;
    for i in 0..lockers.hands.len() {
        if !lockers.has(content, roster, i) || roster.hands[i].away {
            continue;
        }
        let level = roster.hands[i].level;
        let offers = &mut lockers.hands[i].offers;
        let unused = offers.iter().filter(|o| o.level <= level).all(|o| o.used == 0);
        for o in offers.iter_mut().filter(|o| o.level <= level) {
            let before = o.demand;
            o.restock();
            if unused {
                o.demand = o.demand.max(before.min(0));
            }
        }
        // The Merchant's lockers this restock: its level may have risen on the last.
        if roster.merchant.is_some() && lockers.order(content, roster, roster.locker_limit() as usize).contains(&i) {
            // Training, at the villager's restock before its locker trades run (`Apprentices.review`).
            crate::training::review(content, roster, lockers, i, stock, money);
            made += trade(content, books, now, town, roster, lockers, i, stock, money);
        }
    }
    lockers.sync(content, roster);
    made
}

/// A managed hand's switched-on offers run against the warehouse (`LockerTrading.trade`): those not inflated, best
/// deal first (fewest inputs per item received), each up to its uses while the warehouse holds its price, those that
/// cost emeralds within the town's money. Each run levels the hand and the Merchant; its income is posted
/// "lockers: sold <good>", what it costs "lockers: bought <good>". Returns the trades made.
#[allow(clippy::too_many_arguments)]
fn trade(content: &Content, books: &mut Books, now: u64, town: usize, roster: &mut Roster, lockers: &mut Lockers,
        i: usize, stock: &mut Stock, money: &mut Money) -> u32 {
    let Some(m) = roster.merchant else { return 0 };
    let rep = merchant_reputation(roster.hands[m].level as i32);
    let level = roster.hands[i].level;
    let stack = |o: &Offer| if o.sells { EMERALD_STACK } else { content.goods.get(o.good).map_or(64, |g| g.stack) };
    let offers = &lockers.hands[i].offers;
    let mut order: Vec<usize> = (0..offers.len())
        .filter(|&k| offers[k].on && offers[k].level <= level && !offers[k].inflated() && offers[k].good < stock.0.len())
        .collect();
    let per_item = |k: usize| offers[k].price(rep, stack(&offers[k])) as f64 / offers[k].result().max(1) as f64;
    order.sort_by(|&a, &b| per_item(a).total_cmp(&per_item(b)));
    // As many runs as its uses left; those that cost emeralds, within the town's money.
    let mut runs: Vec<u32> = order.iter().map(|&k| (offers[k].uses - offers[k].used.min(offers[k].uses)) as u32).collect();
    // What the Merchant buys of this villager for its contracts is only what's wanted soon (`Procurement.soon`, less what
    // the warehouse holds now): a restock's worth at most, not the whole of a long contract.
    for (r, &k) in runs.iter_mut().zip(&order) {
        let o = &offers[k];
        if o.sells && o.procured && !o.training {
            let need = (lockers.due.get(o.good).copied().unwrap_or(0.0) - stock.0[o.good]).max(0.0).ceil();
            *r = (*r).min((need / o.count.max(1) as f64).ceil() as u32);
        }
    }
    let mut budget = money.0.at_least_zero().millis() / crate::fixed::SCALE; // whole emeralds
    for (r, &k) in runs.iter_mut().zip(&order) {
        if offers[k].sells {
            let price = offers[k].price(rep, stack(&offers[k])) as i64;
            *r = (*r as i64).min(budget.max(0) / price) as u32;
            budget -= *r as i64 * price;
        }
    }
    let mut made = 0;
    for (n, &k) in order.iter().enumerate() {
        let o = lockers.hands[i].offers[k].clone();
        let price = o.price(rep, stack(&o));
        let g = o.good;
        let mut ran = 0u32;
        while ran < runs[n] && lockers.hands[i].offers[k].used < o.uses {
            if !o.sells && stock.0[g] < price as f64 {
                break;
            }
            if o.sells {
                stock.0[g] += o.count as f64;
                books.ledger.made[g] += o.count as f64;
            } else {
                stock.0[g] -= price as f64;
                books.ledger.used[g] += price as f64;
            }
            lockers.hands[i].offers[k].used += 1;
            let levelled = roster.hands[i].reward_trade_xp(o.xp);
            let roll = books.rng.below(crate::roster::MERCHANT_TRADE_XP_ROLL as u64) as u32;
            roster.hands[m].reward_merchant_xp(merchant_trade_xp(roll, levelled));
            // What the client did for the town, for the Merchant's review: sales whole, purchases at half.
            lockers.hands[i].earned += if o.sells { 0.5 * price as f64 } else { o.emeralds as f64 };
            ran += 1;
        }
        if ran > 0 {
            if o.sells && o.training {
                // What the Merchant paid the pupil to make (`Apprentices.afterTrade`, in `rules/training.rs`).
                let trade = lockers.hands[i].trade;
                lockers.lessons.made.push(crate::training::Made { trade, good: g, count: (o.count * ran) as f64 });
            }
            if o.sells {
                // Named as `Reason::Sold` names its good, so a book's lines read alike.
                let name = &content.goods[g].name;
                post(books.ledger, books.book, Side::treasury(town, &mut money.0), Side::outside(),
                    crate::fixed::Fixed::emeralds((price * ran) as i64), Reason::Other(format!("lockers: bought {name}")), now,
                    content.day_ticks);
            } else {
                post(books.ledger, books.book, Side::outside(), Side::treasury(town, &mut money.0),
                    crate::fixed::Fixed::emeralds((o.emeralds * ran) as i64), Reason::Sold(g), now, content.day_ticks);
            }
        }
        made += ran;
    }
    lockers.trades += made as u64;
    made
}

/// The restocks a town missed since its clock, replayed (`LockerCatchUp.catchUp`): two a whole day, at most
/// [`MAX_DAYS`] days, the clock moved by the whole days replayed (past the cap, to now). A town never restocked starts
/// its clock now and owes nothing. Returns the restocks replayed.
#[allow(clippy::too_many_arguments)]
pub fn catch_up(content: &Content, books: &mut Books, now: u64, town: usize, roster: &mut Roster, lockers: &mut Lockers,
        stock: &mut Stock, money: &mut Money) -> u32 {
    let Some(last) = lockers.restocked_until else {
        lockers.restocked_until = Some(now);
        return 0;
    };
    let day = content.day_ticks.max(1);
    let days = now.saturating_sub(last) / day;
    if days == 0 {
        return 0;
    }
    let restocks = days.min(MAX_DAYS) * RESTOCKS_PER_DAY;
    for _ in 0..restocks {
        restock(content, books, now, town, roster, lockers, stock, money);
    }
    lockers.restocked_until = Some(if days > MAX_DAYS { now } else { last + days * day });
    restocks as u32
}

/// The Merchant's daily review of its lockers (`Lockers.review`): each valued by what its trades would do for the town
/// a restock with the goods in the warehouse now, or by what it did since the last review (a restock's share) if more;
/// its clients the first of its order up to its limit, the weakest (not one a contract needs, nor one taken on less
/// than [`CLIENT_DAYS`] ago) let go for the best waiting one if that's worth [`SWAP_MARGIN`] times as much and
/// [`SWAP_GAIN`] more (its trades switched off); every client's trades that buy goods the town holds (an emerald's
/// worth or more) switched on. `wanted`: the goods the town wants (for selling offers). Returns the hand let go.
pub fn review(content: &Content, now: u64, roster: &Roster, lockers: &mut Lockers, stock: &Stock, wanted: &[bool])
        -> Option<usize> {
    lockers.sync(content, roster);
    roster.merchant?;
    let supply: Vec<f64> = stock.0.iter().zip(&content.goods).map(|(n, g)| n * g.value).collect();
    // The order as the last review left it; then each valued afresh.
    let all = lockers.order(content, roster, usize::MAX);
    for &i in &all {
        let w = lockers.worth(i, roster.hands[i].level, &supply, wanted).max(lockers.hands[i].earned / 2.0);
        let l = &mut lockers.hands[i];
        l.worth = w;
        if w <= SLACK_WORTH {
            l.slack += 1;
        } else if l.slack >= 0 {
            l.slack = 0;
        }
    }
    for l in &mut lockers.hands {
        l.earned = 0.0;
    }
    let limit = roster.locker_limit() as usize;
    let mut taken: Vec<usize> = all.iter().take(limit).copied().collect();
    let waiting: Vec<usize> = all.iter().skip(limit).copied().collect();
    let worth = |i: usize| lockers.hands[i].worth;
    // The first of the most worth, and the first of the least (Java's `max` and `min` keep the first of equals).
    let best = waiting.iter().copied().reduce(|a, b| if worth(b) > worth(a) { b } else { a });
    let weakest = taken.iter().copied()
        .filter(|&i| !lockers.needed.contains(&lockers.hands[i].trade)
            && lockers.hands[i].taken_on.is_none_or(|t| now.saturating_sub(t) >= CLIENT_DAYS * content.day_ticks))
        .reduce(|a, b| if worth(b) < worth(a) { b } else { a });
    let mut let_go = None;
    if let (Some(best), Some(weakest)) = (best, weakest) {
        let (gain, loss) = (worth(best), worth(weakest));
        if gain >= SWAP_MARGIN * loss && gain - loss >= SWAP_GAIN {
            for o in &mut lockers.hands[weakest].offers {
                o.on = false;
            }
            let at = taken.iter().position(|&i| i == weakest).unwrap();
            taken[at] = best;
            lockers.hands[best].taken_on = Some(now);
            let_go = Some(weakest);
        }
    }
    lockers.clients = taken.iter().map(|&i| roster.hands[i].id).collect();
    // Taking a client on is serving it: its trades that would buy goods the town holds are switched on.
    for &i in &taken {
        let level = roster.hands[i].level;
        for o in &mut lockers.hands[i].offers {
            if !o.sells && o.level <= level && supply.get(o.good).copied().unwrap_or(0.0) >= 1.0 {
                o.on = true;
            }
        }
    }
    lockers.sync(content, roster);
    let_go
}

/// Vanilla's buying trades (`VillagerTrades`, `EmeraldForItems`: the good, how many for an emerald, uses, XP), by
/// trade and the level they're offered at: a villager rolls two of a level's; here it offers them all. The multiplier
/// is vanilla's 0.05.
const BUYS: &[(&str, u8, &str, u32, u8, u32)] = &[
    ("farmer", 1, "wheat", 20, 16, 2), ("farmer", 1, "potato", 26, 16, 2), ("farmer", 1, "carrot", 22, 16, 2),
    ("farmer", 1, "beetroot", 15, 16, 2), ("farmer", 2, "pumpkin", 6, 12, 10), ("farmer", 3, "melon", 4, 12, 20),
    ("fisherman", 1, "string", 20, 16, 2), ("fisherman", 1, "coal", 10, 16, 2), ("fisherman", 2, "cod", 15, 16, 10),
    ("fisherman", 3, "salmon", 13, 16, 20), ("fisherman", 4, "tropical_fish", 6, 12, 30),
    ("fisherman", 5, "pufferfish", 4, 12, 30),
    ("shepherd", 1, "white_wool", 18, 16, 2), ("shepherd", 1, "brown_wool", 18, 16, 2),
    ("shepherd", 1, "black_wool", 18, 16, 2), ("shepherd", 1, "gray_wool", 18, 16, 2),
    ("shepherd", 2, "white_dye", 12, 16, 10), ("shepherd", 2, "gray_dye", 12, 16, 10),
    ("shepherd", 2, "black_dye", 12, 16, 10), ("shepherd", 2, "light_blue_dye", 12, 16, 10),
    ("shepherd", 2, "lime_dye", 12, 16, 10), ("shepherd", 3, "yellow_dye", 12, 16, 20),
    ("shepherd", 3, "orange_dye", 12, 16, 20), ("shepherd", 3, "red_dye", 12, 16, 20),
    ("shepherd", 4, "brown_dye", 12, 16, 30), ("shepherd", 4, "blue_dye", 12, 16, 30),
    ("fletcher", 1, "stick", 32, 16, 2), ("fletcher", 2, "flint", 26, 12, 10), ("fletcher", 3, "string", 14, 16, 20),
    ("fletcher", 4, "feather", 24, 16, 30), ("fletcher", 5, "tripwire_hook", 8, 12, 30),
    ("librarian", 1, "paper", 24, 16, 2), ("librarian", 2, "book", 4, 12, 10), ("librarian", 3, "ink_sac", 5, 12, 20),
    ("librarian", 4, "writable_book", 2, 12, 30),
    ("cartographer", 1, "paper", 24, 16, 2), ("cartographer", 2, "glass_pane", 11, 16, 10),
    ("cartographer", 3, "compass", 1, 12, 20),
    ("cleric", 1, "rotten_flesh", 32, 16, 2), ("cleric", 2, "gold_ingot", 3, 12, 10),
    ("cleric", 3, "rabbit_foot", 2, 12, 20), ("cleric", 4, "turtle_scute", 4, 12, 30),
    ("cleric", 4, "glass_bottle", 9, 12, 30), ("cleric", 5, "nether_wart", 22, 12, 30),
    ("armorer", 1, "coal", 15, 16, 2), ("armorer", 2, "iron_ingot", 4, 12, 10), ("armorer", 3, "lava_bucket", 1, 12, 20),
    ("armorer", 3, "diamond", 1, 12, 20),
    ("weaponsmith", 1, "coal", 15, 16, 2), ("weaponsmith", 2, "iron_ingot", 4, 12, 10),
    ("weaponsmith", 3, "flint", 24, 12, 20), ("weaponsmith", 4, "diamond", 1, 12, 30),
    ("toolsmith", 1, "coal", 15, 16, 2), ("toolsmith", 2, "iron_ingot", 4, 12, 10), ("toolsmith", 3, "flint", 30, 12, 20),
    ("toolsmith", 4, "diamond", 1, 12, 30),
    ("butcher", 1, "chicken", 14, 16, 2), ("butcher", 1, "porkchop", 7, 16, 2), ("butcher", 1, "rabbit", 4, 16, 2),
    ("butcher", 2, "coal", 15, 16, 2), ("butcher", 3, "mutton", 7, 16, 20), ("butcher", 3, "beef", 10, 16, 20),
    ("butcher", 4, "dried_kelp_block", 10, 12, 30), ("butcher", 5, "sweet_berries", 10, 12, 30),
    ("leatherworker", 1, "leather", 6, 16, 2), ("leatherworker", 2, "flint", 26, 12, 10),
    ("leatherworker", 3, "rabbit_hide", 9, 12, 20), ("leatherworker", 4, "turtle_scute", 4, 12, 30),
    ("mason", 1, "clay_ball", 10, 16, 2), ("mason", 2, "stone", 20, 16, 10), ("mason", 3, "granite", 16, 16, 20),
    ("mason", 3, "andesite", 16, 16, 20), ("mason", 3, "diorite", 16, 16, 20), ("mason", 4, "quartz", 12, 12, 30),
];

/// A selling trade in the table: trade, level, good, emeralds, how many, uses, XP, multiplier.
type Sells = (&'static str, u8, &'static str, u32, u32, u8, u32, f64);

/// Vanilla's selling trades for goods the mod prices (`ItemsForEmeralds`: emeralds, how many, uses, XP, multiplier),
/// by trade and level.
const SELLS: &[Sells] = &[
    ("farmer", 1, "bread", 1, 6, 16, 1, 0.05), ("farmer", 2, "pumpkin_pie", 1, 4, 12, 5, 0.05),
    ("farmer", 2, "apple", 1, 4, 16, 5, 0.05), ("farmer", 3, "cookie", 3, 18, 12, 10, 0.05),
    ("farmer", 4, "cake", 1, 1, 12, 15, 0.05),
    ("butcher", 2, "cooked_porkchop", 1, 5, 16, 5, 0.05), ("butcher", 2, "cooked_chicken", 1, 8, 16, 5, 0.05),
    ("fletcher", 1, "arrow", 1, 16, 12, 1, 0.05), ("fletcher", 2, "bow", 2, 1, 12, 5, 0.05),
    ("librarian", 1, "bookshelf", 9, 1, 12, 1, 0.05), ("librarian", 2, "lantern", 1, 1, 12, 5, 0.05),
    ("librarian", 4, "compass", 4, 1, 12, 15, 0.05),
    ("cleric", 2, "lapis_lazuli", 1, 1, 12, 5, 0.05),
    ("armorer", 1, "iron_leggings", 7, 1, 12, 1, 0.2), ("armorer", 1, "iron_boots", 4, 1, 12, 1, 0.2),
    ("armorer", 1, "iron_helmet", 5, 1, 12, 1, 0.2), ("armorer", 1, "iron_chestplate", 9, 1, 12, 1, 0.2),
    ("armorer", 2, "chainmail_boots", 1, 1, 12, 5, 0.2), ("armorer", 2, "chainmail_leggings", 3, 1, 12, 5, 0.2),
    ("armorer", 3, "chainmail_helmet", 1, 1, 12, 10, 0.2), ("armorer", 3, "chainmail_chestplate", 4, 1, 12, 10, 0.2),
    ("armorer", 3, "shield", 5, 1, 12, 10, 0.2),
    ("weaponsmith", 1, "iron_axe", 3, 1, 12, 1, 0.2), ("weaponsmith", 2, "bell", 36, 1, 12, 5, 0.2),
    ("toolsmith", 1, "stone_axe", 1, 1, 12, 1, 0.2), ("toolsmith", 1, "stone_shovel", 1, 1, 12, 1, 0.2),
    ("toolsmith", 1, "stone_pickaxe", 1, 1, 12, 1, 0.2),
    ("shepherd", 2, "white_carpet", 1, 4, 16, 5, 0.05), ("shepherd", 5, "painting", 2, 3, 12, 30, 0.05),
    ("leatherworker", 1, "leather_leggings", 3, 1, 12, 1, 0.2), ("leatherworker", 1, "leather_chestplate", 7, 1, 12, 1, 0.2),
    ("leatherworker", 2, "leather_helmet", 5, 1, 12, 5, 0.2), ("leatherworker", 2, "leather_boots", 4, 1, 12, 5, 0.2),
    ("leatherworker", 5, "saddle", 6, 1, 12, 30, 0.2),
    ("mason", 1, "brick", 1, 10, 16, 1, 0.05), ("mason", 4, "terracotta", 1, 1, 12, 15, 0.05),
];

/// The mod's trades get vanilla's offers for the goods the host has, by name with or without its namespace
/// (`paper`, `minecraft:paper`). Called by [`Content::new`].
pub fn default_offers(content: &mut Content) {
    let good = |content: &Content, name: &str| content.goods.iter().position(|g| g.name.rsplit(':').next() == Some(name));
    for &(trade, level, name, count, uses, xp) in BUYS {
        if let (Some(t), Some(g)) = (content.trade(trade), good(content, name)) {
            content.trades[t].offers.push(Offer::buys(g, count, 1, uses, xp, 0.05).at(level));
        }
    }
    for &(trade, level, name, emeralds, count, uses, xp, mult) in SELLS {
        if let (Some(t), Some(g)) = (content.trade(trade), good(content, name)) {
            content.trades[t].offers.push(Offer::sells(g, emeralds, count, uses, xp, mult).at(level));
        }
    }
}

// The host's calls for lockers.
impl crate::society::Society {
    /// A town's lockers as they are now (following its roster), if it keeps a roster.
    pub fn lockers(&self, town: usize) -> Option<Lockers> {
        let e = self.world.entity(self.towns[town]);
        let roster = e.get::<Roster>()?;
        let mut lockers = e.get::<Lockers>().cloned().unwrap_or_default();
        lockers.sync(self.content(), roster);
        Some(lockers)
    }

    fn put_lockers(&mut self, town: usize, lockers: Lockers) {
        self.world.entity_mut(self.towns[town]).insert(lockers);
    }

    fn lockers_now(&self, town: usize) -> Lockers {
        self.lockers(town).unwrap_or_else(|| panic!("town {town} keeps no roster, so no lockers"))
    }

    /// Switches one of a hand's offers (by its place in the hand's offers) on or off, as the Merchant's screen does.
    pub fn switch(&mut self, town: usize, hand: usize, offer: usize, on: bool) {
        let mut lockers = self.lockers_now(town);
        lockers.hands[hand].offers[offer].on = on;
        let roster = self.world.get::<Roster>(self.towns[town]).unwrap().clone();
        lockers.sync(self.content(), &roster);
        self.put_lockers(town, lockers);
    }

    /// Gives a hand these offers in place of its trade's (a host whose villagers rolled their own; a test's fixture).
    pub fn set_offers(&mut self, town: usize, hand: usize, offers: Vec<Offer>) {
        let mut lockers = self.lockers_now(town);
        lockers.hands[hand].offers = offers;
        let roster = self.world.get::<Roster>(self.towns[town]).unwrap().clone();
        lockers.sync(self.content(), &roster);
        self.put_lockers(town, lockers);
    }

    /// The trades a town's contracts need its Merchant to tend first (`Apprentices.neededTrades`), by name.
    pub fn set_needed(&mut self, town: usize, trades: &[&str]) {
        let mut lockers = self.lockers_now(town);
        lockers.needed = trades.iter().map(|t| self.content().trade(t).unwrap_or_else(|| panic!("no trade called {t}")))
            .collect();
        let roster = self.world.get::<Roster>(self.towns[town]).unwrap().clone();
        lockers.sync(self.content(), &roster);
        self.put_lockers(town, lockers);
    }

    /// Sets a town's restock clock (`None`: never restocked), as a host loading an older save would.
    pub fn set_restock_clock(&mut self, town: usize, at: Option<u64>) {
        let mut lockers = self.lockers_now(town);
        lockers.restocked_until = at;
        self.put_lockers(town, lockers);
    }

    /// One restock of the town's hands now (vanilla's `restock`, as the mod hooks it); the clock stays. Returns the
    /// trades made.
    pub fn restock(&mut self, town: usize) -> u32 {
        self.procure(town);
        self.plan_lessons(town);
        let made = self.with_lockers(town, restock);
        self.finish_lessons(town);
        made
    }

    /// Replays the restocks the town missed since its clock (`LockerCatchUp.catchUp`). Returns the restocks.
    pub fn catch_up_lockers(&mut self, town: usize) -> u32 {
        self.procure(town);
        self.plan_lessons(town);
        let made = self.with_lockers(town, catch_up);
        self.finish_lessons(town);
        made
    }

    /// The Merchant's review of a town's lockers now (as its day's first look does). Returns the hand let go.
    pub fn review_lockers(&mut self, town: usize) -> Option<usize> {
        let e = self.towns[town];
        let mut lockers = self.lockers_now(town);
        let now = self.now();
        let ent = self.world.entity(e);
        let (roster, stock, trades) = (ent.get::<Roster>().unwrap(), ent.get::<Stock>().unwrap(),
            ent.get::<crate::state::Trades>().unwrap());
        let wanted: Vec<bool> = trades.uses.iter().map(|u| *u > 0.0).collect();
        let out = review(self.content(), now, roster, &mut lockers, stock, &wanted);
        lockers.reviewed = Some(now / self.content().day_ticks.max(1));
        self.put_lockers(town, lockers);
        out
    }

    #[allow(clippy::type_complexity)]
    fn with_lockers(&mut self, town: usize,
            f: fn(&Content, &mut Books, u64, usize, &mut Roster, &mut Lockers, &mut Stock, &mut Money) -> u32) -> u32 {
        let e = self.towns[town];
        let mut lockers = self.lockers_now(town);
        let now = self.now();
        let mut roster = self.world.get::<Roster>(e).unwrap().clone();
        let mut stock = self.world.get::<Stock>(e).unwrap().clone();
        let mut money = *self.world.get::<Money>(e).unwrap();
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        let mut book = std::mem::take(&mut *self.world.resource_mut::<CashBook>());
        let seed = self.world.resource::<Rng>().clone();
        let mut dice = std::mem::take(&mut *self.world.resource_mut::<crate::state::Dice>());
        let made = {
            let mut books = Books { ledger: &mut ledger, book: &mut book, rng: dice.of(town, &seed) };
            f(self.content(), &mut books, now, town, &mut roster, &mut lockers, &mut stock, &mut money)
        };
        *self.world.resource_mut::<Ledger>() = ledger;
        *self.world.resource_mut::<CashBook>() = book;
        *self.world.resource_mut::<crate::state::Dice>() = dice;
        self.world.entity_mut(e).insert((roster, stock, money, lockers));
        made
    }
}
