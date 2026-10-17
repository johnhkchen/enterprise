//! Contracts, as the mod keeps them (`contract/ContractBook.java`): a seller owes a buyer so many of a good each
//! installment, on a schedule, at a price locked when drafted, shipped on its incoterm. Each installment may be
//! settled until its grace runs out; then it's judged: delivered in full is kept, short by half or more is a miss,
//! and three misses break the contract. Trust between two towns is their track record: installments kept and missed,
//! the misses fading faster.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;

use crate::content::GoodId;
use crate::messages::TownId;

/// Installments may be settled this long after they fall due before they're judged (two days, in the host's ticks
/// at 24,000 a day: scaled by the day's length).
pub const GRACE_DAYS: f64 = 2.0;
/// Misses that break a contract.
pub const MISSES_TO_BREAK: u32 = 3;
/// An installment short by at least this share counts toward breaking the contract.
pub const BREAKING_SHORTFALL: f64 = 0.5;
/// Strangers' trust; below the least, towns won't draft.
pub const START_TRUST: f64 = 0.5;
pub const MIN_TRUST: f64 = 0.3;
/// The record every pair starts with: as if two installments kept and two missed.
pub const PRIOR: f64 = 2.0;
/// A miss's weight halves every 20 days; a kept installment's every 120.
pub const MISS_HALF_LIFE_DAYS: f64 = 20.0;
pub const KEPT_HALF_LIFE_DAYS: f64 = 120.0;
/// Until a pair has kept this many installments, a miss counts a quarter: they're new to each other.
pub const TEETHING: f64 = 3.0;
pub const TEETHING_WEIGHT: f64 = 0.25;
/// A good sold well enough for a standing order: this many in one visit, in lots of this many.
pub const CONTRACT_MIN_ITEMS: f64 = 16.0;
pub const CONTRACT_INSTALLMENTS: u32 = 10;
pub const CONTRACT_MIN_INSTALLMENTS: u32 = 3;

/// Who carries a delivery and who bears the road: the shipping terms, after the trade's own Incoterms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Incoterm {
    /// Ex Works: the buyer collects at the seller's, with its own wagon, and bears everything from there.
    Exw,
    /// Carriage Paid To: the seller's wagon carries it, but it's the buyer's once on the road.
    Cpt,
    /// Delivered At Place: the seller carries it and bears the road until it's there.
    Dap,
    /// Delivered Duty Paid: the seller carries it, bears the road and pays the tolls.
    Ddp,
}

impl Incoterm {
    /// The seller's wagons carry it (otherwise the buyer's collect it).
    pub fn seller_carries(self) -> bool {
        self != Incoterm::Exw
    }

    /// The seller bears the road: delivered only once it's there (otherwise once on the road).
    pub fn seller_bears_road(self) -> bool {
        matches!(self, Incoterm::Dap | Incoterm::Ddp)
    }

    /// The seller pays the tolls on the way (otherwise the buyer pays them on top of the price).
    pub fn seller_pays_tolls(self) -> bool {
        self == Incoterm::Ddp
    }

    /// The price per item under these terms, from the price at the seller's (`ex_works`): plus the carriage where
    /// the seller carries, and the tolls where it pays them. Java: `Incoterm.price` (`contract/Incoterm.java`).
    pub fn price(self, ex_works: f64, carriage: f64, tolls: f64) -> f64 {
        ex_works + if self.seller_carries() { carriage } else { 0.0 } + if self.seller_pays_tolls() { tolls } else { 0.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Only one party holds a copy yet.
    Drafted,
    Active,
    Completed,
    Broken,
}

/// A contract for so many of a good each installment.
#[derive(Clone, Debug)]
pub struct Contract {
    pub id: usize,
    pub seller: TownId,
    pub buyer: TownId,
    pub good: GoodId,
    pub quantity: f64,
    pub price: f64,
    pub start: u64,
    pub every: u64,
    pub installments: u32,
    pub incoterm: Incoterm,
    pub state: State,
    pub known_by_seller: bool,
    pub known_by_buyer: bool,
    /// Items delivered so far, and on the road (counted as delivered where the buyer bears the road).
    pub delivered: f64,
    pub shipped: f64,
    /// The seller's misses (not delivered), the buyer's (not collected Ex Works), and installments judged.
    pub missed: u32,
    pub buyer_missed: u32,
    pub judged: u32,
    /// Money paid, and money the buyer owes for goods delivered it couldn't pay for at the time.
    pub paid: f64,
    pub owing: f64,
}

impl Contract {
    pub fn in_force(&self) -> bool {
        self.state == State::Active
    }

    /// Installments due by `at`.
    pub fn due_by(&self, at: u64) -> u32 {
        if at < self.start {
            0
        } else {
            (((at - self.start) / self.every.max(1)) as u32 + 1).min(self.installments)
        }
    }

    /// Items owed by `at`, not yet delivered.
    pub fn owed(&self, at: u64) -> f64 {
        (self.due_by(at) as f64 * self.quantity - self.delivered).max(0.0)
    }

    /// What counts toward the installments: delivered, and on the road where the buyer bears it.
    pub fn counted(&self) -> f64 {
        self.delivered + if self.incoterm.seller_bears_road() { 0.0 } else { self.shipped }
    }

    /// The last moment the next installment not yet counted can come before it's judged; `None` if none is left.
    pub fn next_deadline(&self, grace: u64) -> Option<u64> {
        if self.quantity <= 0.0 {
            return None;
        }
        let n = self.judged.max((self.counted() / self.quantity).floor() as u32);
        (n < self.installments).then(|| self.start + n as u64 * self.every + grace)
    }

    /// When the installment before the next one fell due (before the first: when it was drafted).
    pub fn previous_due(&self, drafted: u64) -> u64 {
        let n = self.judged.max((self.counted() / self.quantity.max(1e-9)).floor() as u32) as i64;
        drafted.max((self.start as i64 + (n - 1) * self.every as i64).max(0) as u64)
    }
}

/// Two towns' record: installments kept and missed (weighted, fading), as of `touched`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Record {
    pub kept: f64,
    pub missed: f64,
    pub touched: u64,
}

impl Record {
    pub fn trust(&self) -> f64 {
        (self.kept + PRIOR) / (self.kept + self.missed + 2.0 * PRIOR)
    }

    /// The record as it stands at `now`: misses forgiven, kept installments a little forgotten.
    fn at(&self, now: u64, day: u64) -> Record {
        let days = now.saturating_sub(self.touched) as f64 / day as f64;
        Record {
            kept: self.kept * 0.5f64.powf(days / KEPT_HALF_LIFE_DAYS),
            missed: self.missed * 0.5f64.powf(days / MISS_HALF_LIFE_DAYS),
            touched: now.max(self.touched),
        }
    }
}

/// Every contract, the ones still open (drafted or in force: what each look reads), and every pair's record.
#[derive(Resource, Clone, Debug, Default)]
pub struct Book {
    pub contracts: Vec<Contract>,
    pub open: Vec<usize>,
    pub records: BTreeMap<(TownId, TownId), Record>,
    /// The day judging last ran.
    pub reviewed: Option<u64>,
}

fn pair(a: TownId, b: TownId) -> (TownId, TownId) {
    (a.min(b), a.max(b))
}

impl Book {
    /// A contract drafted at one party's (it holds the first copy); in force once both do.
    #[allow(clippy::too_many_arguments)]
    pub fn draft(&mut self, seller: TownId, buyer: TownId, good: GoodId, quantity: f64, price: f64, start: u64,
            every: u64, installments: u32, incoterm: Incoterm, at_seller: bool, both: bool) -> usize {
        let id = self.contracts.len();
        let known_by_seller = at_seller || both;
        let known_by_buyer = !at_seller || both;
        self.contracts.push(Contract {
            id,
            seller,
            buyer,
            good,
            quantity,
            price,
            start,
            every,
            installments,
            incoterm,
            state: if known_by_seller && known_by_buyer { State::Active } else { State::Drafted },
            known_by_seller,
            known_by_buyer,
            delivered: 0.0,
            shipped: 0.0,
            missed: 0,
            buyer_missed: 0,
            judged: 0,
            paid: 0.0,
            owing: 0.0,
        });
        self.open.push(id);
        id
    }

    /// Drops contracts no longer open from the open list (finished ones that still owe money stay till paid).
    fn tidy(&mut self) {
        let contracts = &self.contracts;
        self.open.retain(|&id| {
            let c = &contracts[id];
            matches!(c.state, State::Drafted | State::Active) || c.owing > 0.0
        });
    }

    /// A party gets its copy (a wagon brought it, or the post): in force once both hold one.
    pub fn deliver_copy(&mut self, id: usize, town: TownId) {
        let c = &mut self.contracts[id];
        if c.seller == town {
            c.known_by_seller = true;
        }
        if c.buyer == town {
            c.known_by_buyer = true;
        }
        if c.state == State::Drafted && c.known_by_seller && c.known_by_buyer {
            c.state = State::Active;
        }
    }

    /// Contracts in force between a seller and a buyer.
    pub fn in_force(&self, seller: TownId, buyer: TownId) -> impl Iterator<Item = &Contract> {
        self.open.iter().map(|&id| &self.contracts[id]).filter(move |c| c.in_force() && c.seller == seller && c.buyer == buyer)
    }

    /// Open contracts (any state) between a seller and a buyer.
    pub fn open_between(&self, seller: TownId, buyer: TownId) -> Vec<usize> {
        self.open.iter().copied().filter(|&id| self.contracts[id].seller == seller && self.contracts[id].buyer == buyer).collect()
    }

    /// Is there a contract drafted or in force between them for this good?
    pub fn covers(&self, seller: TownId, buyer: TownId, good: GoodId) -> bool {
        self.open.iter().map(|&id| &self.contracts[id]).any(|c| matches!(c.state, State::Drafted | State::Active)
            && c.seller == seller && c.buyer == buyer && c.good == good)
    }

    pub fn trust(&self, a: TownId, b: TownId, now: u64, day: u64) -> f64 {
        self.records.get(&pair(a, b)).map_or(START_TRUST, |r| r.at(now, day).trust())
    }

    pub fn record(&self, a: TownId, b: TownId, now: u64, day: u64) -> Record {
        self.records.get(&pair(a, b)).map_or(Record { touched: now, ..Default::default() }, |r| r.at(now, day))
    }

    /// `weight` of an installment kept between the two.
    pub fn kept(&mut self, a: TownId, b: TownId, weight: f64, now: u64, day: u64) {
        let mut r = self.record(a, b, now, day);
        r.kept += weight;
        self.records.insert(pair(a, b), r);
    }

    /// `weight` of an installment missed between the two: a quarter while they're new to each other.
    pub fn missed(&mut self, a: TownId, b: TownId, weight: f64, now: u64, day: u64) {
        let mut r = self.record(a, b, now, day);
        r.missed += weight * if r.kept < TEETHING { TEETHING_WEIGHT } else { 1.0 };
        self.records.insert(pair(a, b), r);
    }

    /// Goods put on the road under a contract (counted as delivered where the buyer bears the road).
    pub fn ship(&mut self, id: usize, items: f64) {
        self.contracts[id].shipped += items;
    }

    /// Goods delivered and paid for (or owed for): no longer on the road; done once all are in.
    pub fn settle(&mut self, id: usize, carried: f64, delivered: f64, paid: f64, owing: f64) {
        let c = &mut self.contracts[id];
        c.shipped = (c.shipped - carried).max(0.0);
        c.delivered += delivered;
        c.paid += paid;
        c.owing += owing;
        if c.delivered >= c.quantity * c.installments as f64 - 1e-9 {
            c.state = State::Completed;
        }
        self.tidy();
    }

    /// Judges installments whose grace has run out: short by nothing is kept; short by half or more counts toward
    /// breaking. Ex Works, an installment not collected while the seller held the goods is the buyer's miss.
    /// `seller_holds(contract)`: does the seller have the goods on hand?
    pub fn review(&mut self, now: u64, day: u64, seller_holds: impl Fn(&Contract) -> bool) {
        let grace = (GRACE_DAYS * day as f64) as u64;
        for i in self.open.clone() {
            if !self.contracts[i].in_force() {
                continue;
            }
            loop {
                let c = &self.contracts[i];
                let judged = c.due_by(now.saturating_sub(grace));
                if c.judged >= judged {
                    break;
                }
                let needed = (c.judged + 1) as f64 * c.quantity;
                let short = ((needed - c.counted()) / c.quantity).clamp(0.0, 1.0);
                let (seller, buyer) = (c.seller, c.buyer);
                if short <= 0.0 {
                    self.kept(seller, buyer, 1.0, now, day);
                } else if c.incoterm == Incoterm::Exw && seller_holds(c) {
                    self.contracts[i].buyer_missed += 1;
                    self.missed(seller, buyer, 1.0, now, day);
                } else {
                    self.kept(seller, buyer, 1.0 - short, now, day);
                    self.missed(seller, buyer, short, now, day);
                    if short >= BREAKING_SHORTFALL {
                        self.contracts[i].missed += 1;
                    }
                }
                self.contracts[i].judged += 1;
            }
            let c = &mut self.contracts[i];
            if c.missed >= MISSES_TO_BREAK || c.buyer_missed >= MISSES_TO_BREAK {
                c.state = State::Broken;
            }
        }
        self.tidy();
        self.reviewed = Some(now / day);
    }
}
