//! Training as a contract, and the tenders that find it work (`contract/Training.java`, `contract/ContractBook.java`
//! `reviewTraining` (449), `trained` (496), `at` (501), `trainingFor` (509), `noteOfftake`; `market/Apprentices.java`
//! `target` (40), `neededTrades` (51), `afterTrade` (78), `collectable` (125), `pupil` (146), `review` (156);
//! `market/Tenders.java` `worth` (161), `read` (142), `consider` (195), `take` (229), `terms` (248), `post` (103),
//! `on` (119), `lapse` (127); `docs/decisions.md` "Training is a contract", "The Merchant trains only under
//! contract", "The Merchant trains smarter").
//!
//! **A training contract** ([`Training`] on a [`Contract`]; no goods, quantity 0, its worth the bonus) has a seller bring
//! one of its hands of a trade up to a level, and the buyer pay the bonus when it has: judged on the seller's roster by
//! the contract's day, a third of it up front (the advance) where the two trust each other. It is for a *new* master: the
//! seller had `already` of the trade at the level when it took the work on, and is done when it has one more
//! ([`mastered`]). Late by two days' grace it is a miss, and as long again after that it breaks (`Book::review_training`).
//!
//! **The Merchant trains only under one.** [`plan`] (each look, before the lockers restock) tells the town's lockers
//! what its training contracts ask ([`Lessons`]): the trade at the level, whether the buyer takes the pupil's work, the
//! advances held and the town's reserve; and sets the trades in [`Lockers::needed`], so the Merchant manages them first
//! and the job draw takes the trade up if the town lacks it. At each of the pupil's restocks ([`review`], called from
//! `lockers::restock` before the trades run) its buying offers go on, flagged as training; with none of what it buys in
//! the warehouse, its cheapest work is bought instead (what the town wants first), if the work has somewhere to go and
//! the town can spare the money (with what it was paid up front); at the level, all go off again. Under an offtake
//! ([`finish`], after the restocks) the work bought is crated for the buyer, paid for already, with a bill drawn on the
//! buyer for its worth at the offtake's share.
//!
//! **Tenders** are work a buyer posts on its town's board for any seller ([`Tenders`]): "a master Weaponsmith, 32
//! emeralds, by day 40". A wagon between two towns lets each one's Merchant read the other's board ([`Tenders::read`]); a
//! Merchant takes a tender worth it to its town ([`worth`]: the bonus less a stall for a trade it hasn't and
//! `level_cost` a level, if the levels fit in the time), or, with roads to towns of its own that don't know the board, carries
//! it on, posting it on its own board for `relay_margin` less and two days sooner, and contracting with the buyer once a
//! town of its takes it up. A tender nobody takes lapses; a buyer that reposts gets it back for a quarter more.
//!
//! **Stood in for:** the mod's census is the town's [`Roster`]; its warehouse slots are `warehouse_slots` stacks (the core
//! has no slots), `FREE_SLOTS` the dial `free_slots`; what a town wants is what its people use below its shelf, and what its
//! contracts have to deliver; the cold store, the atlas (a town knows the board's if a road joins them) and freight hubs
//! (the work is collectable only where a wagon of the buyer's calls or a hub runs to the buyer: `stranded.rs`). The
//! per-restock review is called with the plan made at the look (or at the host's restock), not at each of a catch-up's
//! restocks. A lapsed tender is reposted here at its bonus times 1.25 for its span; the mod's `Sovereign.offerTraining`
//! recounts the realm's shortfall and raises a per-trade counter (the crown's to port; `training_bonus` waits for it).
//!
//! **Differs from the mod, on purpose:** `Tenders.lapse` clears a fully taken or carried tender at once, so a later
//! lapse of its carrier has nothing to give back to and a later take drafts no contract with the first buyer; here it
//! stays while a tender carrying it on is posted. And `Apprentices.afterTrade` takes the pupil's work out of stock before
//! it asks whether the buyer can collect, losing it when it can't; here the work stays the town's.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::bills::{Bills, Held};
use crate::cargo::{stacks_of, Paperwork, Shipments, Shipper};
use crate::cash_book::CashBook;
use crate::content::{Content, GoodId};
use crate::contracts::{Book, Contract, Incoterm, State, GRACE_DAYS, MIN_TRUST};
use crate::farms::Farm;
use crate::lockers::{merchant_reputation, Lockers, Offer, EMERALD_STACK};
use crate::pallet::{At, Lading, Pallets, Payment};
use crate::roster::{Roster, TradeId};
use crate::state::*;

/// A tender nobody took is posted again for this much more (`Sovereign.offerTraining`'s `Math.pow(1.25, raised)`).
pub const REPOST_RAISE: f64 = 1.25;

/// The training a contract asks of its seller (`contract/Training.java`): bring a hand of `trade` up to `level`, for
/// `bonus` emeralds; with an `offtake`, the buyer takes the pupil's paid work too, at that share of its worth. The seller
/// had `already` of the trade at the level when it took this on. The contract's `drafted` day and the contract it is
/// `via` (a Merchant carrying a tender on is done when the town it passed it to is) are the mod's, kept with the term.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Training {
    pub trade: TradeId,
    pub level: u8,
    /// What the contract pays for the new master, in thousandths (`fixed.rs`).
    pub bonus: crate::fixed::Fixed,
    pub offtake: f64,
    pub already: u32,
    pub drafted: u64,
    pub via: Option<usize>,
}

impl Training {
    pub fn new(trade: TradeId, level: u8, bonus: f64, offtake: f64) -> Self {
        Training { trade, level, bonus: crate::fixed::Fixed::round(bonus), offtake, ..Default::default() }
    }

    /// The same work, for a seller that already has `n` of the trade at the level (`Training.forSeller`).
    pub fn for_seller(mut self, n: u32) -> Self {
        self.already = n;
        self
    }
}

/// The seller's hands of a trade at a level or above, adults, by its roster (`ContractBook.at`).
pub fn mastered(roster: Option<&Roster>, trade: TradeId, level: u8) -> u32 {
    roster.map_or(0, |r| r.hands.iter().filter(|h| !h.is_baby() && h.trade == trade && h.level >= level).count() as u32)
}

impl Book {
    /// A training contract drafted at the seller's (or at both's, `both`): due `due`, the buyer to pay a third of the
    /// bonus up front if `advance_share` is above 0 (`Tenders.terms`, the buyer trusted). Judged by
    /// [`Book::review_training`].
    #[allow(clippy::too_many_arguments)]
    pub fn draft_training(&mut self, seller: usize, buyer: usize, work: Training, due: u64, now: u64, day: u64,
            advance_share: f64, both: bool) -> usize {
        let id = self.draft(seller, buyer, 0, 0.0, 0.0, due, day, 1, Incoterm::Exw, true, both);
        let c = &mut self.contracts[id];
        c.training = Some(Training { drafted: now, ..work });
        c.advance_share = advance_share;
        id
    }

    /// Training contracts this seller holds in force and knows of: what its Merchant is to train
    /// (`ContractBook.trainingFor`).
    pub fn training_for(&self, seller: usize) -> impl Iterator<Item = (&Contract, &Training)> {
        self.open.iter().map(|&id| &self.contracts[id])
            .filter(move |c| c.in_force() && c.seller == seller && c.known_by_seller)
            .filter_map(|c| c.training.as_ref().map(|t| (c, t)))
    }

    /// The training contracts in force are judged (`ContractBook.reviewTraining`): done once the seller's census shows
    /// one more hand of the trade at the level than it had (`mastered` says how many), or, carried on, once the contract
    /// it was passed to is: the bonus less what came up front is owed, the pair keep one more installment. Not done by
    /// its day plus grace, a miss (lightly while the two are new to each other), and as long again after that, broken.
    pub fn review_training(&mut self, now: u64, day: u64, mastered: impl Fn(&Contract, &Training) -> u32) {
        let grace = (GRACE_DAYS * day as f64) as u64;
        for i in self.open.clone() {
            let c = &self.contracts[i];
            let Some(t) = c.training.filter(|_| c.in_force()) else { continue };
            let done = match t.via {
                Some(via) => self.contracts.get(via).is_some_and(|v| v.state == State::Completed),
                None => mastered(c, &t) > t.already,
            };
            let (seller, buyer, start, judged) = (c.seller, c.buyer, c.start, c.judged);
            if done {
                let c = &mut self.contracts[i];
                c.delivered = 1.0;
                let recoup = c.recoup_from(t.bonus, 1.0);
                c.recouped += recoup;
                c.paid += recoup;
                self.owe(i, t.bonus - recoup);
                self.kept(seller, buyer, 1.0, now, day);
                self.contracts[i].state = State::Completed;
                self.tidy();
                continue;
            }
            let due = start + grace;
            if now > due && judged == 0 {
                self.contracts[i].judged = 1;
                self.contracts[i].missed += 1;
                self.missed(seller, buyer, 1.0, now, day);
            }
            // As long again as it was given.
            if now > due + start.saturating_sub(t.drafted) {
                self.contracts[i].state = State::Broken;
                self.broken(i);
                self.tidy();
            }
        }
    }
}

/// What a town's training contracts ask of its lockers, made at each look (`plan`) and read at each restock (`review`),
/// then the pupil's paid work waiting to be crated (`finish`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lessons {
    /// Each trade it is contracted to train, to the highest level asked, and the offtake of the first contract with one.
    pub targets: Vec<Target>,
    /// Paid up front and not yet worked off: money the work may spend.
    pub held: crate::fixed::Fixed,
    /// What the town keeps back (`Territory.reserve`).
    pub reserve: crate::fixed::Fixed,
    /// The goods the town wants (its work first).
    pub wanted: Vec<bool>,
    /// Crates its warehouse holds, in its slots.
    pub crates: u32,
    /// Work the Merchant paid a pupil for at the restocks since, for `finish` to crate.
    pub made: Vec<Made>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub trade: TradeId,
    pub level: u8,
    pub offtake: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Made {
    pub trade: TradeId,
    pub good: GoodId,
    pub count: f64,
}

/// The trades each town's training contracts put in its lockers' `needed` (so the Merchant manages them first), apart
/// from those the host set: what [`plan`] added, to take out again when the contract is gone.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Apprenticeships {
    pub added: BTreeMap<usize, Vec<TradeId>>,
}

/// A town's pupil in a trade: its hand furthest along short of `target`, the first of equals (`Apprentices.pupil`).
pub fn pupil(roster: &Roster, trade: TradeId, target: u8) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, h) in roster.hands.iter().enumerate() {
        if h.is_baby() || h.trade != trade || h.level >= target {
            continue;
        }
        if best.is_none_or(|b| h.level > roster.hands[b].level) {
            best = Some(i);
        }
    }
    best
}

/// Free warehouse slots: what the stock's stacks and the crates leave (`MerchantStock.freeSlots`; a crate takes a slot).
fn free_slots(content: &Content, stock: &Stock, crates: u32) -> i64 {
    let used: i64 = stock.0.iter().enumerate().map(|(g, n)| stacks_of(content, g, *n) as i64).sum::<i64>() + crates as i64;
    content.dials.warehouse_slots as i64 - used
}

/// Room for `extra` of a good without dipping into the `free_slots` kept free; merging into its stacks is fine
/// (`MerchantStock.hasRoomFor`).
fn room(content: &Content, stock: &Stock, crates: u32, good: GoodId, extra: f64) -> bool {
    let more = stacks_of(content, good, stock.0[good] + extra) as i64 - stacks_of(content, good, stock.0[good]) as i64;
    (free_slots(content, stock, crates) - content.dials.free_slots as i64).max(0) >= more
}

/// At a pupil's restock, before its locker trades run (`Apprentices.review`): trains it or stops training it.
pub fn review(content: &Content, roster: &Roster, lockers: &mut Lockers, i: usize, stock: &Stock, money: &Money) {
    let Some(hand) = roster.hands.get(i) else { return };
    let (trade, level) = (hand.trade, hand.level);
    let target = lockers.lessons.targets.iter().find(|t| t.trade == trade).copied();
    let training = target.is_some_and(|t| level < t.level && pupil(roster, trade, t.level).is_none_or(|p| p == i));
    if !training {
        for o in lockers.hands[i].offers.iter_mut().filter(|o| o.training) {
            o.on = false;
            o.training = false;
        }
        return;
    }
    let target = target.unwrap();
    let rep = merchant_reputation(roster.merchant_level().unwrap_or(1) as i32);
    let (held, reserve, crates) = (lockers.lessons.held, lockers.lessons.reserve, lockers.lessons.crates);
    let stack = |o: &Offer| if o.sells { EMERALD_STACK } else { content.goods.get(o.good).map_or(64, |g| g.stack) };
    // Every trade by which it buys is switched on; can the warehouse meet one?
    let mut can_buy = false;
    let wants: Vec<bool> = (0..content.goods.len()).map(|g| lockers.lessons.wanted.get(g).copied().unwrap_or(false)).collect();
    for o in lockers.hands[i].offers.iter_mut().filter(|o| !o.sells && o.level <= level) {
        if !o.on {
            o.on = true;
            o.training = true;
        }
        can_buy |= stock.0.get(o.good).copied().unwrap_or(0.0) >= o.price(rep, content.goods.get(o.good).map_or(64, |g| g.stack)) as f64;
    }
    // Nothing to buy: its work is paid for instead, if it has somewhere to go: bought by the contract's buyer (an
    // offtake), or wanted here, or at least with room to keep a restock's worth; what's wanted first, then the cheapest.
    let offers = &lockers.hands[i].offers;
    let work = (0..offers.len())
        .filter(|&k| {
            let o = &offers[k];
            o.sells && o.level <= level && o.count > 0 && o.good < stock.0.len()
                && (target.offtake > 0.0 || wants[o.good] || room(content, stock, crates, o.good, (o.count * o.uses as u32) as f64))
        })
        .min_by_key(|&k| (!wants[offers[k].good], offers[k].price(rep, stack(&offers[k]))));
    let Some(k) = work else {
        // Nowhere for its work to go: none bought.
        for o in lockers.hands[i].offers.iter_mut().filter(|o| o.sells && o.training) {
            o.on = false;
            o.training = false;
        }
        return;
    };
    // As far as it can spare (with what it was paid up front): a lesson at least.
    let spare = money.0 - reserve + held;
    let price = offers[k].price(rep, stack(&offers[k])) as f64;
    let pay = !can_buy && spare >= crate::fixed::Fixed::emeralds(price as i64);
    let o = &mut lockers.hands[i].offers[k];
    if pay && !o.on {
        o.on = true;
        o.training = true;
    } else if !pay && o.training {
        o.on = false;
        o.training = false;
    }
}

/// Tells a town's lockers what its training contracts ask (each look, before the lockers restock): the trades and levels,
/// the advances held, the reserve, what the town wants, the crates it holds; and keeps the trades in `needed`
/// (`Apprentices.neededTrades`: the Merchant manages their lockers first; the job draw takes the trade up).
#[allow(clippy::too_many_arguments)]
pub fn plan(content: &Content, book: &Book, apprenticeships: &mut Apprenticeships, town: usize,
        lockers: &mut Lockers, stock: &Stock, trades: &Trades, farm: Option<&Farm>, crates: u32) {
    let mut targets: Vec<Target> = Vec::new();
    for (_, t) in book.training_for(town) {
        match targets.iter_mut().find(|x| x.trade == t.trade) {
            Some(x) => {
                x.level = x.level.max(t.level);
                if x.offtake <= 0.0 {
                    x.offtake = t.offtake;
                }
            }
            None => targets.push(Target { trade: t.trade, level: t.level, offtake: t.offtake }),
        }
    }
    let n = content.goods.len();
    let mut wanted = vec![false; n];
    if !targets.is_empty() {
        for g in 0..n {
            let shelf = crate::rules::shelf(content, trades, g);
            wanted[g] = trades.uses[g] > 0.0 && stock.0[g] < shelf;
        }
        for &id in &book.open {
            let c = &book.contracts[id];
            if c.seller == town && c.in_force() && c.training.is_none() && c.quantity > 0.0 {
                let left = (c.quantity * c.installments as f64 - c.counted()).max(0.0);
                wanted[c.good] |= left > stock.0[c.good];
            }
        }
    }
    let lessons = &mut lockers.lessons;
    lessons.held = if targets.is_empty() { crate::fixed::Fixed::ZERO } else { book.advances_held(town) };
    lessons.reserve = crate::rules::farms::reserve(content, trades, farm);
    lessons.crates = crates;
    lessons.wanted = wanted;
    // The trades in `needed`: those the host set, and those a contract adds.
    let mine = apprenticeships.added.entry(town).or_default();
    lockers.needed.retain(|t| !mine.contains(t));
    mine.clear();
    for t in &targets {
        if !lockers.needed.contains(&t.trade) {
            lockers.needed.push(t.trade);
            mine.push(t.trade);
        }
    }
    if mine.is_empty() {
        apprenticeships.added.remove(&town);
    }
    lockers.lessons.targets = targets;
}

/// Can the buyer get the work, and the town its money: a wagon of the buyer's calls at the town (`Stranded.calls`), or the
/// town's wagons reach a freight hub that runs to the buyer (it will buy the bill and forward the goods)
/// (`Apprentices.collectable`; see [`crate::stranded::collectable`]).
pub fn collectable(net: &crate::freight::Network, desk: usize, buyer: usize) -> bool {
    crate::stranded::collectable(net, desk, buyer)
}

/// After the restocks: what the Merchant paid the pupil to make, under a training contract with an offtake, is the
/// buyer's. It is crated, made out to the buyer with a bill of lading (stored, not stock, until the buyer's wagon comes for
/// it), and the Merchant draws a bill of exchange on the buyer for it at the agreed share of its worth, to be paid when
/// the buyer's wagon is next here (`Apprentices.afterTrade`). With no wagon of the buyer's calling, the work is the town's.
#[allow(clippy::too_many_arguments)]
pub fn finish(content: &Content, book: &Book, bills: &mut Bills, ledger: &mut Ledger, cash: &mut CashBook,
        pallets: &mut Pallets, net: &crate::freight::Network, now: u64, town: usize, lockers: &mut Lockers, stock: &mut Stock,
        money: &mut Money, trades: &Trades, farm: Option<&Farm>, store: &mut Shipments, paperwork: &mut Paperwork) {
    for made in std::mem::take(&mut lockers.lessons.made) {
        let Some((c, t)) = book.training_for(town).find(|(_, t)| t.trade == made.trade && t.offtake > 0.0) else { continue };
        if !collectable(net, town, c.buyer) {
            continue;
        }
        let n = made.count.min(stock.0[made.good].floor());
        if n <= 0.0 {
            continue;
        }
        let spare = money.0 - crate::rules::farms::build_reserve(content, trades, farm);
        let mut shipper = Shipper { town, money: &mut money.0, spare, store: &mut *store, paperwork: &mut *paperwork };
        // Paid for as it's crated (the buyer's bill for it drawn at once, naming them), stored, not stock, until the buyer's
        // wagon fetches it: a pallet each pallet's worth, a bill of lading each (`pallet-caps`).
        let lading = Lading { contract: Some(c.id), payment: Payment::Paid, ..Lading::new(town, c.buyer, now) };
        let ids = crate::cargo::pack_pallets(content, ledger, cash, now, &mut shipper, pallets, &lading, made.good, n, At::Town(town));
        if ids.is_empty() {
            continue;
        }
        let count = crate::cargo::count_of(pallets, &ids);
        stock.0[made.good] -= count;
        let worth = count * content.goods[made.good].value * t.offtake;
        let bill = bills.draw(c.buyer, town, crate::fixed::Fixed::ceil(worth), now, content.day_ticks);
        bills.bills[bill].held = Held::Town(town);
        bills.bills[bill].pallets.extend(ids);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Tenders

/// A posted tender: its board, its buyer, the work, how many are wanted still, and when they're due. `relay_for` is
/// minus the id of the tender this carries on into the board's region (or 0); `origin` who first wanted it (the buyer of
/// the tender this one carries on, at the root); `reposts`, whether the buyer posts it again, for more, if it lapses;
/// `span`, the time it was given.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tender {
    pub id: usize,
    pub board: usize,
    pub buyer: usize,
    pub work: Training,
    pub open: u32,
    pub due: u64,
    pub relay_for: i64,
    pub origin: usize,
    pub reposts: bool,
    pub span: u64,
}

/// Every tender posted, on every town's board (`market/Tenders.java`).
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tenders {
    pub tenders: Vec<Tender>,
    pub last_id: usize,
}

/// What a town's desk can do for a tender: its roster (a town without one has no hands to train: a hub), money, stock and
/// crates, the figures its reserve comes from.
pub struct Desk<'a> {
    pub roster: Option<&'a Roster>,
    pub money: crate::fixed::Fixed,
    pub stock: &'a Stock,
    pub trades: &'a Trades,
    pub farm: Option<&'a Farm>,
    /// The crates it holds packed (its pallets, in its warehouse).
    pub crates: u32,
}

/// What this tender is worth to the desk's own town: its bonus, less what training would cost (a stall for a trade it
/// hasn't, `level_cost` for each level to go); `None` if the town can't do it: no hand of the trade and no idle hands,
/// not by the day, no money for it (beyond its reserve, with what's paid up front), or no room in its warehouse for what
/// the pupil makes and learns on (`Tenders.worth`).
pub fn worth(content: &Content, now: u64, desk: &Desk, tender: &Tender) -> Option<f64> {
    let roster = desk.roster?;
    let d = &content.dials;
    // A new one: the best of the trade not yet at the level (a master it has already doesn't count).
    let best = roster.hands.iter().filter(|h| !h.is_baby() && h.trade == tender.work.trade && h.level < tender.work.level)
        .map(|h| h.level.max(1)).max().unwrap_or(0);
    let idle = roster.hands.iter().any(|h| !h.is_baby() && content.trades.get(h.trade).is_some_and(|t| t.none));
    if best == 0 && !idle {
        return None;
    }
    let levels = tender.work.level.saturating_sub(best.max(1)) as f64;
    let days = if levels == 0.0 { 0.0 } else { levels * d.days_per_level + d.supply_days + if best == 0 { 1.0 } else { 0.0 } };
    if now as f64 + days * content.day_ticks as f64 > tender.due as f64 {
        return None;
    }
    let cost = if best == 0 { d.stall_emeralds } else { 0.0 } + levels * d.level_cost;
    let money = (desk.money - crate::rules::farms::reserve(content, desk.trades, desk.farm)).to_f64()
        + tender.work.bonus.to_f64() * d.advance_share;
    let free = free_slots(content, desk.stock, desk.crates);
    if money < cost || free < d.free_slots as i64 {
        return None;
    }
    Some(tender.work.bonus.to_f64() - cost)
}

/// What a reader's Merchant sees of the world as it reads: the dials, the clock, the routes and every town's desk.
pub struct Env<'a, D: Fn(usize) -> Option<Desk<'a>>> {
    pub content: &'a Content,
    pub now: u64,
    pub routes: &'a [Route],
    pub desk: D,
}

impl Tenders {
    /// Posts `count` of `work` on `board`'s board, for `buyer`, due by `due` (`Tenders.post`). Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub fn post(&mut self, board: usize, buyer: usize, work: Training, count: u32, due: u64, now: u64, relay_for: i64) -> usize {
        let origin = if relay_for < 0 {
            self.tenders.iter().find(|t| t.id as i64 == -relay_for).map_or(buyer, |t| t.origin)
        } else {
            buyer
        };
        self.last_id += 1;
        self.tenders.push(Tender { id: self.last_id, board, buyer, work, open: count, due, relay_for, origin, reposts: false,
            span: due.saturating_sub(now) });
        self.last_id
    }

    pub fn get(&self, id: usize) -> Option<&Tender> {
        self.tenders.iter().find(|t| t.id == id)
    }

    /// Open tenders first wanted by `origin`, wherever they're posted now (carried on or not) (`Tenders.openFor`).
    pub fn open_for(&self, origin: usize, trade: TradeId) -> u32 {
        self.tenders.iter().filter(|t| t.origin == origin && t.work.trade == trade).map(|t| t.open).sum()
    }

    /// Tenders open on a town's board (`Tenders.on`).
    pub fn on(&self, board: usize) -> Vec<&Tender> {
        self.tenders.iter().filter(|t| t.board == board && t.open > 0).collect()
    }

    /// Tenders past their day lapse, two days sooner for the supply (`supply_days`); returns those that lapsed untaken
    /// (or partly), for their buyers to think again. A tender carried on that no town took is given back to the one it
    /// carried (`Tenders.lapse`). Those a buyer `reposts` are posted again for a quarter more, as long as they were given.
    pub fn lapse(&mut self, content: &Content, now: u64) -> Vec<Tender> {
        let before = (content.dials.supply_days * content.day_ticks as f64) as u64;
        let lapses = |t: &Tender| now + before >= t.due;
        let lapsed: Vec<usize> = self.tenders.iter().filter(|t| t.open > 0 && lapses(t)).map(|t| t.id).collect();
        for id in &lapsed {
            let t = self.get(*id).unwrap().clone();
            if t.relay_for < 0 {
                if let Some(first) = self.tenders.iter_mut().find(|f| f.id as i64 == -t.relay_for) {
                    first.open += t.open;
                }
            }
        }
        let out: Vec<Tender> = lapsed.iter().map(|id| self.get(*id).unwrap().clone()).collect();
        // (A tender taken or carried on is cleared with the rest, unless a tender carrying it on is still posted: the mod
        // clears it at once, leaving the give-back nothing to give to when its carrier's lapses later.)
        let carried: Vec<i64> = self.tenders.iter().filter(|t| t.relay_for < 0 && t.open > 0 && !lapses(t)).map(|t| -t.relay_for).collect();
        self.tenders.retain(|t| (t.open > 0 || carried.contains(&(t.id as i64))) && !lapses(t));
        for t in &out {
            if t.relay_for == 0 && t.reposts && t.open > 0 {
                let work = Training { bonus: t.work.bonus.mul_round(REPOST_RAISE), ..t.work };
                let id = self.post(t.board, t.buyer, work, t.open, now + t.span, now, 0);
                self.tenders.iter_mut().find(|x| x.id == id).unwrap().reposts = true;
            }
        }
        out
    }

    /// A wagon between two towns: `reader`'s Merchant reads `board`'s board and takes what's worth it (`Tenders.read`).
    /// Returns the contracts drafted or tenders carried on.
    pub fn read<'a, D: Fn(usize) -> Option<Desk<'a>>>(&mut self, book: &mut Book, env: &Env<'a, D>, reader: usize,
            board: usize) -> u32 {
        let mut taken = 0;
        let ids: Vec<usize> = self.on(board).iter().filter(|t| t.buyer != reader).map(|t| t.id).collect();
        for id in ids {
            while self.get(id).is_some_and(|t| t.open > 0) && self.consider(book, env, reader, id) {
                taken += 1;
            }
        }
        taken
    }

    /// The desk's Merchant considers one of a tender: takes it for its own town, or carries it on (`Tenders.consider`).
    fn consider<'a, D: Fn(usize) -> Option<Desk<'a>>>(&mut self, book: &mut Book, env: &Env<'a, D>, reader: usize,
            id: usize) -> bool {
        let tender = self.get(id).unwrap().clone();
        let own = (env.desk)(reader).and_then(|desk| worth(env.content, env.now, &desk, &tender));
        let already = book.training_for(reader).any(|(_, t)| t.trade == tender.work.trade)
            || book.open.iter().map(|&i| &book.contracts[i])
                .any(|c| c.seller == reader && c.state == State::Drafted && c.training.is_some());
        if own.is_some_and(|w| w > 0.0) && !already {
            self.take(book, env, reader, id);
            return true;
        }
        // Can't (or it isn't worth it) here: carried on, to the towns this market's wagons go to that don't know the board
        // it's on (the atlas's `known`: here, a road joins them).
        let knows = |a: usize, b: usize| a == b || env.routes.iter().any(|r| (r.from_id == a && r.to_id == b) || (r.from_id == b && r.to_id == a));
        let roads = env.routes.iter().any(|r| r.from_id == reader && (env.desk)(r.to_id).is_some_and(|d| d.roster.is_some())
            && r.to_id != tender.buyer && !knows(r.to_id, tender.board));
        let relaying = self.tenders.iter().any(|t| t.board == reader && t.relay_for == -(id as i64));
        if roads && !relaying && tender.relay_for == 0 {
            self.tenders.iter_mut().find(|t| t.id == id).unwrap().open -= 1;
            // Less the relayer's margin, rounded down to the thousandth (what it offers on).
            let price = tender.work.bonus.mul_floor(1.0 - env.content.dials.relay_margin);
            let work = Training { bonus: price, already: 0, ..tender.work };
            let sooner = (env.content.dials.supply_days * env.content.day_ticks as f64) as u64;
            self.post(reader, reader, work, 1, tender.due.saturating_sub(sooner), env.now, -(id as i64));
            return true;
        }
        false
    }

    /// Takes one of a tender: a training contract with its buyer (and, carried on, the Merchant's own contract too)
    /// (`Tenders.take`). Returns the seller's contract.
    fn take<'a, D: Fn(usize) -> Option<Desk<'a>>>(&mut self, book: &mut Book, env: &Env<'a, D>, seller: usize, id: usize) -> usize {
        let (content, now) = (env.content, env.now);
        let day = content.day_ticks;
        let tender = self.tenders.iter_mut().find(|t| t.id == id).unwrap();
        tender.open -= 1;
        let tender = tender.clone();
        let roster = (env.desk)(seller).and_then(|d| d.roster);
        let work = tender.work.for_seller(mastered(roster, tender.work.trade, tender.work.level));
        let share = |book: &Book, seller: usize, buyer: usize| {
            if book.trust(seller, buyer, now, day) >= MIN_TRUST { content.dials.advance_share } else { 0.0 }
        };
        let advance = share(book, seller, tender.buyer);
        let contract = book.draft_training(seller, tender.buyer, work, tender.due, now, day, advance, false);
        if tender.relay_for < 0 {
            // The tender this one carried on: the carrying Merchant now contracts with the first buyer, done when this is.
            if let Some(first) = self.get((-tender.relay_for) as usize).cloned() {
                let advance = share(book, tender.buyer, first.buyer);
                let upstream = book.draft_training(tender.buyer, first.buyer, first.work.for_seller(0), first.due, now, day,
                    advance, false);
                book.contracts[upstream].training.as_mut().unwrap().via = Some(contract);
            }
        }
        contract
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The host's calls

use crate::society::Society;

impl Society {
    /// A training contract drafted at the seller's (`both`: both hold copies, in force at once): the seller to bring a
    /// hand of `trade` up to `level` by `due` for `bonus`; with an `offtake`, the buyer takes the pupil's work at that
    /// share of its worth; with `advance`, a third of the bonus up front. The seller had `already` of the trade at the
    /// level when it was drafted (`Training.forSeller`).
    #[allow(clippy::too_many_arguments)]
    pub fn draft_training(&mut self, seller: usize, buyer: usize, trade: &str, level: u8, bonus: f64, offtake: f64, due: u64,
            advance: bool, both: bool) -> usize {
        let t = self.content().trade(trade).unwrap_or_else(|| panic!("no trade called {trade}"));
        let already = self.mastered(seller, trade, level);
        let share = if advance { self.content().dials.advance_share } else { 0.0 };
        let (now, day) = (self.now(), self.content().day_ticks);
        let work = Training::new(t, level, bonus, offtake).for_seller(already);
        self.world.resource_mut::<Book>().draft_training(seller, buyer, work, due, now, day, share, both)
    }

    /// The town's hands of a trade at a level or above, adults, by its roster (`ContractBook.at`).
    pub fn mastered(&self, town: usize, trade: &str, level: u8) -> u32 {
        let t = self.content().trade(trade).unwrap_or_else(|| panic!("no trade called {trade}"));
        mastered(self.world.get::<Roster>(self.towns[town]), t, level)
    }

    /// The training contracts in force are judged now, as each look does.
    pub fn review_training(&mut self) {
        let (now, day) = (self.now(), self.content().day_ticks);
        let rosters: Vec<Option<Roster>> = self.towns.iter().map(|&e| self.world.get::<Roster>(e).cloned()).collect();
        self.world.resource_mut::<Book>().review_training(now, day, |c, t| mastered(rosters[c.seller].as_ref(), t.trade, t.level));
    }

    /// Posts a tender on `town`'s board: a hand of `trade` at `level` for `bonus`, due by `due`, one wanted, reposted for a
    /// quarter more if it lapses (`Sovereign.offerTraining`). Returns its id.
    pub fn tender(&mut self, town: usize, trade: &str, level: u8, bonus: f64, due: u64) -> usize {
        self.tender_for(town, trade, level, bonus, 0.0, 1, due)
    }

    /// As [`Society::tender`], with an offtake and a count.
    #[allow(clippy::too_many_arguments)]
    pub fn tender_for(&mut self, town: usize, trade: &str, level: u8, bonus: f64, offtake: f64, count: u32, due: u64) -> usize {
        let t = self.content().trade(trade).unwrap_or_else(|| panic!("no trade called {trade}"));
        let now = self.now();
        let mut tenders = self.world.resource_mut::<Tenders>();
        let id = tenders.post(town, town, Training::new(t, level, bonus, offtake), count, due, now, 0);
        tenders.tenders.iter_mut().find(|x| x.id == id).unwrap().reposts = true;
        id
    }

    /// The tenders open on a town's board.
    pub fn board(&self, town: usize) -> Vec<Tender> {
        self.world.resource::<Tenders>().on(town).into_iter().cloned().collect()
    }

    /// Every tender posted, open or not, wherever it is.
    pub fn tenders(&self) -> Vec<Tender> {
        self.world.resource::<Tenders>().tenders.clone()
    }

    /// Open tenders first wanted by `origin` for a trade, wherever they are now.
    pub fn tenders_open_for(&self, origin: usize, trade: &str) -> u32 {
        let t = self.content().trade(trade).unwrap_or_else(|| panic!("no trade called {trade}"));
        self.world.resource::<Tenders>().open_for(origin, t)
    }

    /// What a tender is worth to a town's own Merchant now (`Tenders.worth`): `None` if the town can't do it.
    pub fn tender_worth(&self, town: usize, tender: usize) -> Option<f64> {
        let e = self.world.entity(self.towns[town]);
        let tender = self.world.resource::<Tenders>().get(tender)?.clone();
        let desk = Desk { roster: e.get::<Roster>(), money: e.get::<Money>()?.0, stock: e.get::<Stock>()?,
            trades: e.get::<Trades>()?, farm: e.get::<Farm>(), crates: crate::cargo::crates_held(self.pallets(), town) };
        worth(self.content(), self.now(), &desk, &tender)
    }

    /// `reader`'s Merchant reads `board`'s board and takes what's worth it, as a wagon between them lets it. Returns the
    /// tenders taken or carried on.
    pub fn read_board(&mut self, reader: usize, board: usize) -> u32 {
        let mut tenders = std::mem::take(&mut *self.world.resource_mut::<Tenders>());
        let mut book = std::mem::take(&mut *self.world.resource_mut::<Book>());
        let routes: Vec<Route> = self.routes.iter().filter_map(|&r| self.world.get::<Route>(r).copied()).collect();
        let taken = {
            let world = &self.world;
            let desk = |t: usize| {
                let e = world.entity(self.towns[t]);
                Some(Desk { roster: e.get::<Roster>(), money: e.get::<Money>()?.0, stock: e.get::<Stock>()?,
                    trades: e.get::<Trades>()?, farm: e.get::<Farm>(),
                    crates: crate::cargo::crates_held(world.resource::<Pallets>(), t) })
            };
            let env = Env { content: world.resource::<Content>(), now: self.now(), routes: &routes, desk };
            tenders.read(&mut book, &env, reader, board)
        };
        *self.world.resource_mut::<Tenders>() = tenders;
        *self.world.resource_mut::<Book>() = book;
        taken
    }

    /// Tenders past their day lapse now, as each day's review does. Returns those that lapsed.
    pub fn lapse_tenders(&mut self) -> Vec<Tender> {
        let now = self.now();
        let mut tenders = std::mem::take(&mut *self.world.resource_mut::<Tenders>());
        let lapsed = tenders.lapse(self.content(), now);
        *self.world.resource_mut::<Tenders>() = tenders;
        lapsed
    }

    /// Tells a town's lockers what its training contracts ask (as the look does before the lockers restock).
    pub(crate) fn plan_lessons(&mut self, town: usize) {
        let e = self.towns[town];
        let Some(mut lockers) = self.world.get::<Lockers>(e).cloned() else { return };
        let mut apprenticeships = std::mem::take(&mut *self.world.resource_mut::<Apprenticeships>());
        {
            let ent = self.world.entity(e);
            let (Some(stock), Some(trades)) = (ent.get::<Stock>(), ent.get::<Trades>()) else { return };
            plan(self.content(), self.world.resource::<Book>(), &mut apprenticeships, town, &mut lockers, stock, trades,
                ent.get::<Farm>(), crate::cargo::crates_held(self.world.resource::<Pallets>(), town));
        }
        *self.world.resource_mut::<Apprenticeships>() = apprenticeships;
        self.world.entity_mut(e).insert(lockers);
    }

    /// The pupil's paid work crated for the buyer and billed (as the look does after the lockers restock).
    pub(crate) fn finish_lessons(&mut self, town: usize) {
        let e = self.towns[town];
        let Some(mut lockers) = self.world.get::<Lockers>(e).cloned() else { return };
        if lockers.lessons.made.is_empty() {
            return;
        }
        let ent = self.world.entity(e);
        let (mut stock, mut money, trades) = (ent.get::<Stock>().unwrap().clone(), *ent.get::<Money>().unwrap(),
            ent.get::<Trades>().unwrap().clone());
        let farm = ent.get::<Farm>().cloned();
        let (mut store, mut paperwork) = (ent.get::<Shipments>().unwrap().clone(), ent.get::<Paperwork>().unwrap().clone());
        let net = self.network();
        let mut bills = std::mem::take(&mut *self.world.resource_mut::<Bills>());
        let mut ledger = std::mem::take(&mut *self.world.resource_mut::<Ledger>());
        let mut cash = std::mem::take(&mut *self.world.resource_mut::<CashBook>());
        let mut pallets = std::mem::take(&mut *self.world.resource_mut::<Pallets>());
        finish(self.content(), self.world.resource::<Book>(), &mut bills, &mut ledger, &mut cash, &mut pallets, &net,
            self.now(), town, &mut lockers, &mut stock, &mut money, &trades, farm.as_ref(), &mut store, &mut paperwork);
        *self.world.resource_mut::<Pallets>() = pallets;
        *self.world.resource_mut::<Bills>() = bills;
        *self.world.resource_mut::<Ledger>() = ledger;
        *self.world.resource_mut::<CashBook>() = cash;
        self.world.entity_mut(e).insert((lockers, stock, money, store, paperwork));
    }
}
