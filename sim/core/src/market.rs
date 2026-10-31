//! The market: each town's shop keeps a book of what it bids for (`docs/markets-plan.md`; John, 2026-10-31: "Let's use
//! supply and demand as the core mechanism ... letting the Create Numismatics shops mediate trade between
//! settlements").
//!
//! - **A bid** is what a town will pay for a good and how many it wants: the room its shelf has (a comfort, its par
//!   less what it holds of the kind; anything else, its shelf), at no more than the good's use is worth to it (its
//!   **ceiling**): a comfort what its people would pay ([`crate::desires::will_pay`]); a good its people or lockers use,
//!   its worth; an input its trades turn into something it bids for, what that would fetch less the other inputs at
//!   their marks, a batch's share (paper for books, cane for that paper). A town bids only where the use is worth more
//!   than the good's worth: a good worth just its worth to it is bought at a bargain, as before, the Merchants' split
//!   its margin (`trade_loop.rs`).
//! - **The bid creeps.** It starts at the least a seller takes (half the good's worth, [`crate::bargain::SELLER_FLOOR`]),
//!   or its ceiling if that's less; each day it's not filled it rises by `bid_creep` toward the ceiling. A town short of
//!   paper bids more for it day by day until a wagon brings some; one well supplied bids no more than a bargain.
//! - **A fill marks the price.** A wagon selling to a bid sells at it (`rules/arrive.rs`); the price it sold at is the
//!   good's **mark** in the town, its price to its Farmers ([`crate::farms::Farm::prices`]).
//! - **News travels with carriers.** A wagon at the far end reads its book ([`Market::sighted`]); what wagons last saw
//!   is what every Merchant thinks a market bids ([`Market::seen`]), as with the shops' listings.
//!
//! Asks (what a town sells its surplus at) and the shops' vendors showing the book are the plan's next steps.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::messages::TownId;

/// One good in a town's book: its bid (each, and how many it wants), the day it was last filled, and its mark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub bid: f64,
    pub want: f64,
    pub ceiling: f64,
    pub filled: Option<u64>,
    pub mark: Option<f64>,
}

/// Every town's book, by good, and what wagons last saw of each (the day, and its bids).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Market {
    pub books: BTreeMap<TownId, BTreeMap<GoodId, Quote>>,
    pub seen: BTreeMap<TownId, (u64, BTreeMap<GoodId, (f64, f64)>)>,
    pub reviewed: BTreeMap<TownId, u64>,
}

impl Market {
    /// What the town bids for a good, each and how many: none while it wants none.
    pub fn bid(&self, town: TownId, good: GoodId) -> Option<(f64, f64)> {
        self.books.get(&town).and_then(|b| b.get(&good)).filter(|q| q.want > 0.0 && q.bid > 0.0).map(|q| (q.bid, q.want))
    }

    /// What wagons last saw the town bid for a good, each and how many.
    pub fn seen_bid(&self, town: TownId, good: GoodId) -> Option<(f64, f64)> {
        self.seen.get(&town).and_then(|(_, b)| b.get(&good)).copied().filter(|&(each, want)| each > 0.0 && want > 0.0)
    }

    /// The good's price in the town: what it last traded at there, else its worth.
    pub fn mark(&self, content: &Content, town: TownId, good: GoodId) -> f64 {
        self.books.get(&town).and_then(|b| b.get(&good)).and_then(|q| q.mark).unwrap_or(content.goods[good].value)
    }

    /// `n` of a good sold to the town at `each`: its bid is filled today for as many, and its mark is the price.
    pub fn fill(&mut self, town: TownId, good: GoodId, n: f64, each: f64, today: u64) {
        let q = self.books.entry(town).or_default().entry(good).or_default();
        q.filled = Some(today);
        q.want = (q.want - n).max(0.0);
        q.mark = Some(each);
    }

    /// A wagon at the town reads its book: what it bids for is news.
    pub fn sighted(&mut self, town: TownId, today: u64) {
        let bids = self.books.get(&town).map(|b| b.iter().filter(|(_, q)| q.want > 0.0 && q.bid > 0.0)
            .map(|(&g, q)| (g, (q.bid, q.want))).collect()).unwrap_or_default();
        self.seen.insert(town, (today, bids));
    }
}

/// A good's use to a town: how many it wants, at what most.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Use {
    pub want: f64,
    pub ceiling: f64,
}

impl Use {
    /// The better of two uses: the dearer, and the more wanted on a tie.
    fn or(self, other: Use) -> Use {
        if other.want <= 0.0 || other.ceiling <= 0.0 {
            return self;
        }
        if self.want <= 0.0 || other.ceiling > self.ceiling || other.ceiling == self.ceiling && other.want > self.want {
            other
        } else {
            self
        }
    }
}

/// Each good's use to a town, from what it wants for itself (`direct`: its comforts, its shelf) and what its trades
/// make of what it wants (`conversions` it can do: input, how many a batch, output, how many it makes), two steps back
/// (paper for books, then cane for that paper); `mark` prices the other inputs of a batch.
pub fn uses(direct: Vec<Use>, conversions: &[(Vec<(GoodId, f64)>, GoodId, f64)], stock: &[f64],
        mark: impl Fn(GoodId) -> f64) -> Vec<Use> {
    let mut out = direct;
    for _ in 0..2 {
        let before = out.clone();
        for (inputs, output, made) in conversions {
            let o = before[*output];
            if o.want <= 0.0 || o.ceiling <= 0.0 {
                continue;
            }
            let batches = (o.want / made - 1e-9).ceil();
            for &(g, n) in inputs {
                let others: f64 = inputs.iter().filter(|&&(h, _)| h != g).map(|&(h, m)| m * mark(h)).sum();
                let ceiling = (o.ceiling * made - others) / n;
                let want = (batches * n - stock[g]).max(0.0);
                out[g] = out[g].or(Use { want, ceiling });
            }
        }
    }
    out
}

/// The day's bid on a good: `use_` of it, last `bid` and whether it was filled since the last review. Unwanted, or
/// worth no more than its worth to the town, none; new, the seller's floor of its worth; unfilled, `creep` higher;
/// never over the ceiling.
pub fn rebid(use_: Use, bid: f64, filled: bool, worth: f64, creep: f64) -> f64 {
    if use_.want <= 0.0 || use_.ceiling <= worth {
        return 0.0;
    }
    let bid = if bid <= 0.0 { crate::bargain::seller_floor(worth) } else if filled { bid } else { bid * (1.0 + creep) };
    bid.min(use_.ceiling)
}

impl crate::society::Society {
    /// What the town bids for a good, each and how many (`Market::bid`).
    pub fn bid(&self, town: TownId, good: GoodId) -> Option<(f64, f64)> {
        self.world.resource::<Market>().bid(town, good)
    }

    /// The good's price in the town (`Market::mark`).
    pub fn mark(&self, town: TownId, good: GoodId) -> f64 {
        self.world.resource::<Market>().mark(self.content(), town, good)
    }

    /// What wagons last saw the town bid for a good (`Market::seen_bid`).
    pub fn seen_bid(&self, town: TownId, good: GoodId) -> Option<(f64, f64)> {
        self.world.resource::<Market>().seen_bid(town, good)
    }

    /// Every town's book as it stands.
    pub fn market(&self) -> &Market {
        self.world.resource::<Market>()
    }
}
