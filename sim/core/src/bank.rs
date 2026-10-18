//! Banks and the discount desk, as the mod keeps them (`market/Bank.java`: `deal`, `discount`, `lend`, `collect`,
//! `repayFrom`, `owedOn`, `FLOAT`, `LOAN_SHARE`, `LOAN_INTEREST`; `market/Discounts.java`: `quote`, `limit`, `book`,
//! `onDrawee`, `foreign`, `isForeign`, `backed`; `caravan/Caravans.java`: `dispatch`'s `bankFloat`, `collectExWorks` and
//! `settleOwing`'s repay first; `docs/decisions.md` "Pacing and the crown's bank").
//!
//! A bank is a market whose Merchant lends and discounts from its own treasury, through its own wagons: money goes only
//! where a wagon carries it. Its wagon takes up to `bank_float` more in its purse, as far as the market can spare
//! (`PlanBuy`). Where it calls (`Papers`, after the advances and before the bills are presented, as `Caravans.arrive`):
//!
//! - **collect:** a loan left on a contract that's done or broken is taken back from what the town can spare beyond
//!   its reserve;
//! - **discount:** the town's bills not yet due are bought for coin now, at face × (1 − `discount_fee` −
//!   `discount_per_day` × days to their due day). A bill drawn on the bank's own market is redeemed (settled now); any
//!   other is endorsed to the bank and carried home, to be presented in its turn;
//! - **lend:** a town with a contract in force as seller, whose money beyond its reserve is short of `loan_share` of
//!   what the goods still to come are worth, is lent up to that share less what it was paid up front and lent on it
//!   already, owing the principal and `loan_interest` more, secured on the contract. When the bank's wagon pays for
//!   those goods (collected Ex Works, or owed for goods a player brought), the loan comes out of the payment first.
//!
//! The discount desk ([`Discounts`]) is a hub's: what it would pay for a bill, as a share of its face, or why not. Its
//! callers (the freight hubs, the stranded paper) are later slices'; whether the hub has a counting house is its
//! caller's to say (`Trades.counting_house`, works.rs); foreign realms aren't in the core yet (a hook: none foreign).
//!
//! The core counts money in emerald-worth, unfloored; the mod floors a bill's price and a loan in spurs.

use std::collections::{BTreeMap, BTreeSet};

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::bills::{Bill, BillState, Bills};
use crate::cash_book::Reason;
use crate::content::Dials;
use crate::contracts::{Book, Contract, State};
use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::society::Society;

/// A hub desk's fee on a bill (`Freight.DISCOUNT_FEE`: a hub takes twice a bank's), and so much a day to its due day
/// (`Freight.DISCOUNT_PER_DAY`).
pub const HUB_DISCOUNT_FEE: f64 = 0.02;
pub const HUB_DISCOUNT_PER_DAY: f64 = 0.01;
/// Loans owed under this are paid off (`Bank.repayFrom`, `collect`); a collection under this isn't worth taking.
pub const LOAN_EPS: Fixed = Fixed::from_millis(1);
pub const COLLECT_EPS: Fixed = Fixed::from_millis(10);
/// A loan smaller than an emerald isn't made (`Bank.lend`: `spurs < rate`), nor one on less than an emerald's cap.
pub const MIN_LOAN: Fixed = Fixed::ONE;

pub(crate) fn discount_reason() -> Reason {
    Reason::Other("discount".into())
}

pub(crate) fn loan_reason() -> Reason {
    Reason::Other("loan".into())
}

pub(crate) fn repaid_reason() -> Reason {
    Reason::Other("loan repaid".into())
}

/// A loan: the bank, the borrower, the contract it's secured on, what was lent, what's still owed, and when.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loan {
    pub id: usize,
    pub bank: TownId,
    pub borrower: TownId,
    pub contract: usize,
    pub principal: Fixed,
    pub owed: Fixed,
    pub made: u64,
}

/// What a bank has done, in all (`Bank.Book`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BankBook {
    pub bills_bought: u32,
    /// Emeralds paid for them, and what they were worth when due.
    pub bills_paid: Fixed,
    pub bills_face: Fixed,
    pub loans_made: u32,
    pub lent: Fixed,
    pub repaid: Fixed,
}

/// Every bank, its loans and its book (`Bank`, the mod's saved data: one for the world, a resource here).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Banks {
    pub banks: BTreeSet<TownId>,
    pub loans: Vec<Loan>,
    pub books: BTreeMap<TownId, BankBook>,
    pub next_id: usize,
    /// The float each bank's route planned this look (by route), spent off the bank's day's budget once its wagon
    /// leaves; empty between looks.
    pub floats: BTreeMap<usize, Fixed>,
}

impl Banks {
    pub fn open(&mut self, town: TownId) {
        self.banks.insert(town);
    }

    pub fn close(&mut self, town: TownId) {
        self.banks.remove(&town);
    }

    pub fn is(&self, town: TownId) -> bool {
        self.banks.contains(&town)
    }

    pub fn book(&mut self, bank: TownId) -> &mut BankBook {
        self.books.entry(bank).or_default()
    }

    /// Loans still owed on a contract, whichever bank made them (`Bank.owedOn`).
    pub fn owed_on(&self, contract: usize) -> Fixed {
        self.loans.iter().filter(|l| l.contract == contract).map(|l| l.owed).sum()
    }

    /// Loans `bank` made on a contract, still owed: what its wagon takes back out of the payment for its goods.
    pub fn owed_to(&self, bank: TownId, contract: usize) -> Fixed {
        self.loans.iter().filter(|l| l.bank == bank && l.contract == contract).map(|l| l.owed).sum()
    }

    /// A bank's wagon pays a borrower `payment` for goods under `contract`: the loan on it comes out first. Returns what
    /// the bank took back (`Bank.repayFrom`).
    pub fn repay_from(&mut self, bank: TownId, contract: usize, payment: Fixed) -> Fixed {
        let mut taken = Fixed::ZERO;
        for loan in &mut self.loans {
            if loan.bank == bank && loan.contract == contract && loan.owed > LOAN_EPS && taken < payment {
                let back = loan.owed.min(payment - taken);
                loan.owed -= back;
                taken += back;
            }
        }
        if taken.is_positive() {
            self.book(bank).repaid += taken;
            self.loans.retain(|l| l.owed > LOAN_EPS);
        }
        taken
    }

    /// A loan made: the principal and the interest owed (rounded up to the thousandth: the borrower's debt), the book's
    /// tally.
    pub fn lend(&mut self, bank: TownId, borrower: TownId, contract: usize, principal: Fixed, interest: f64, now: u64) {
        let id = self.next_id;
        self.next_id += 1;
        self.loans.push(Loan { id, bank, borrower, contract, principal, owed: principal.mul_ceil(1.0 + interest), made: now });
        let book = self.book(bank);
        book.loans_made += 1;
        book.lent += principal;
    }
}

/// What a bank pays for a bill of `face` due at `due`, now: its face less the fee and so much a day to its due day
/// (`Bank.discount`), rounded down to the thousandth (`Bank.deal`'s `Math.floor(price * rate)`).
pub fn discount_price(face: impl Into<Fixed>, due: u64, now: u64, day: u64, dials: &Dials) -> Fixed {
    let face = face.into();
    let days = due.saturating_sub(now) as f64 / day as f64;
    face.mul_floor(1.0 - dials.discount_fee - dials.discount_per_day * days)
}

/// What the goods still to come under a contract are worth at its price; a training contract's, its bonus
/// (`Bank.lend`'s `toCome`).
pub fn to_come(c: &Contract) -> f64 {
    if c.training.is_some() {
        return c.worth();
    }
    (c.quantity * c.installments as f64 - c.delivered).max(0.0) * c.price
}

/// The most a bank would lend on a contract: `loan_share` of the goods to come, less what was paid up front and not
/// worked off, less what's lent on it already (`Bank.lend`'s `cap`), rounded down to the thousandth.
pub fn loan_cap(c: &Contract, owed_on: impl Into<Fixed>, dials: &Dials) -> Fixed {
    let owed_on = owed_on.into();
    Fixed::floor(dials.loan_share * to_come(c)) - (c.advanced - c.recouped).at_least_zero() - owed_on
}

/// A price for a bill at the discount desk: the share of its face paid, or why not (`Discounts.Quote`).
#[derive(Clone, Debug, PartialEq)]
pub struct Quote {
    pub share: f64,
    pub refused: Option<String>,
}

impl Quote {
    fn no(why: String) -> Quote {
        Quote { share: 0.0, refused: Some(why) }
    }

    pub fn taken(&self) -> bool {
        self.refused.is_none()
    }
}

/// A hub's discount desk (`Discounts`): the bills it buys, at what price, and how many. The hub earns from the spread
/// and the throughput, so it wants steady, well-secured paper, not the most of it.
pub struct Discounts;

impl Discounts {
    /// Is the market a foreign realm's, or off the map (`Discounts.isForeign`)? Not in the core yet (realms): none is.
    pub fn is_foreign(_market: TownId) -> bool {
        false
    }

    /// Emeralds of face the hub holds in other markets' bills, outstanding: its book (`Discounts.book`).
    pub fn book(bills: &Bills, hub: TownId) -> Fixed {
        Self::held(bills, hub, |b| b.drawee != hub)
    }

    /// As [`Discounts::book`], of bills on `drawee` (`Discounts.onDrawee`).
    pub fn on_drawee(bills: &Bills, hub: TownId, drawee: TownId) -> Fixed {
        Self::held(bills, hub, |b| b.drawee == drawee)
    }

    /// As [`Discounts::book`], of bills on foreign markets (`Discounts.foreign`).
    pub fn foreign(bills: &Bills, hub: TownId) -> Fixed {
        Self::held(bills, hub, |b| Self::is_foreign(b.drawee))
    }

    fn held(bills: &Bills, hub: TownId, which: impl Fn(&Bill) -> bool) -> Fixed {
        bills.open.iter().map(|&id| &bills.bills[id])
            .filter(|b| b.state == BillState::Outstanding && b.payee == hub && which(b))
            .map(|b| b.amount).sum()
    }

    /// The most paper the hub will hold, in emeralds of face: `discount_book_share` of its means, twice with a counting
    /// house (`counting`: `Discounts.counting`, `Buildings.COUNTING_HOUSE`) (`Discounts.limit`).
    pub fn limit(counting: bool, means: impl Into<Fixed>, dials: &Dials) -> Fixed {
        means.into().at_least_zero().mul_floor(dials.discount_book_share * if counting { 2.0 } else { 1.0 })
    }

    /// Is a bill backed by goods the hub can see: delivered, or crated and on their way, under its contract
    /// (`Discounts.backed`)? Not one whose pallets are all lost (`Bill::goods_lost`): nothing is behind it.
    pub fn backed(book: &Book, bill: &Bill) -> bool {
        !bill.goods_lost() && bill.contract.and_then(|id| book.contracts.get(id))
            .is_some_and(|c| c.delivered + c.shipped > 0.0 || c.state == State::Completed)
    }

    /// What the hub would pay for `bill` now, as a share of its face (`Discounts.quote`). `backed` if its goods are
    /// known to be in hand (crates bought with it, say), else read from its contract; `counting` if the hub has a counting
    /// house (its clerks keep the book at half the fee, and it carries twice the paper); `taking` is face already taken on
    /// in this sitting (not yet in the ledger), `fill` the warehouse's fill (0 empty, 1 full), `means` the hub's spare
    /// money. Refused past its limits: the book full, a drawee's share of it, foreign paper's. Else its face less the
    /// fee (half with a counting house), less so much a day to its due day (or the stale discount past it), less more
    /// for goods it can't see, more again past its season, and the throttle: dearer as the warehouse fills and the
    /// book nears its limit.
    #[allow(clippy::too_many_arguments)]
    pub fn quote(bills: &Bills, book: &Book, hub: TownId, counting: bool, bill: &Bill, backed: bool, taking: impl Into<Fixed>,
            fill: f64, means: impl Into<Fixed>, now: u64, day: u64, dials: &Dials) -> Quote {
        let (taking, means) = (taking.into(), means.into());
        let limit = Self::limit(counting, means, dials);
        let held = Self::book(bills, hub) + taking;
        if held + bill.amount > limit {
            return Quote::no(format!("its book is full: {} of {} emeralds", held.to_f64().round(), limit.to_f64().round()));
        }
        if Self::on_drawee(bills, hub, bill.drawee) + bill.amount > limit.mul_floor(dials.discount_drawee_share) {
            return Quote::no("it holds its fill of that market's paper".into());
        }
        if Self::is_foreign(bill.drawee) && Self::foreign(bills, hub) + bill.amount > limit.mul_floor(dials.discount_foreign_share) {
            return Quote::no("it holds its fill of foreign paper".into());
        }
        let days = (bill.due as f64 - now as f64).max(0.0) / day as f64;
        let mut off = HUB_DISCOUNT_FEE * if counting { 0.5 } else { 1.0 }
            + if bill.due < now { dials.stale_bill_discount } else { HUB_DISCOUNT_PER_DAY * days };
        if bill.goods_lost() || !(backed || Self::backed(book, bill)) {
            off += dials.discount_unbacked; // crop not yet in anyone's hands
        }
        if days > dials.discount_long_days {
            off += dials.discount_long; // due past the season
        }
        off += dials.discount_throttle * (fill.clamp(0.0, 1.0) + held.ratio(limit.max(Fixed::ONE)).min(1.0));
        Quote { share: (1.0 - off).max(0.0), refused: None }
    }
}

impl Society {
    /// The town's market opens a bank (`Bank.open`; the mod's `/enterprise bank open <desk>`).
    pub fn open_bank(&mut self, town: TownId) {
        self.world.resource_mut::<Banks>().open(town);
    }

    pub fn close_bank(&mut self, town: TownId) {
        self.world.resource_mut::<Banks>().close(town);
    }

    pub fn is_bank(&self, town: TownId) -> bool {
        self.world.resource::<Banks>().is(town)
    }

    /// What a bank has done, in all (an empty book for a town that never dealt).
    pub fn bank(&self, town: TownId) -> BankBook {
        self.world.resource::<Banks>().books.get(&town).cloned().unwrap_or_default()
    }

    /// Every loan still owed.
    pub fn loans(&self) -> Vec<Loan> {
        self.world.resource::<Banks>().loans.clone()
    }

    /// Loans still owed on a contract (`Bank.owedOn`).
    pub fn owed_on(&self, contract: usize) -> Fixed {
        self.world.resource::<Banks>().owed_on(contract)
    }

    /// `drawee` draws a bill on itself to pay `payee` `debt` (and the interest for the wait), filed with `payee` (`Bills
    /// .draw`, then `Bills.file` in the payee's warehouse): a host's bill, and tests'.
    pub fn draw_bill(&mut self, drawee: TownId, payee: TownId, debt: impl Into<Fixed>) -> usize {
        let debt = debt.into();
        let (now, day) = (self.now(), self.content().day_ticks);
        let mut bills = self.world.resource_mut::<Bills>();
        let id = bills.draw(drawee, payee, debt, now, day);
        bills.bills[id].held = crate::bills::Held::Town(payee);
        id
    }

    /// What hub `hub`'s discount desk would pay for bill `bill` now (`Discounts.quote`; see [`Discounts::quote`]), with
    /// a counting house if its market has one.
    pub fn quote(&self, hub: TownId, bill: usize, backed: bool, taking: impl Into<Fixed>, fill: f64, means: impl Into<Fixed>)
            -> Quote {
        let (taking, means) = (taking.into(), means.into());
        let bills = self.world.resource::<Bills>();
        let counting = self.world.get::<crate::state::Trades>(self.towns[hub]).is_some_and(|t| t.counting_house);
        Discounts::quote(bills, self.world.resource::<Book>(), hub, counting, &bills.bills[bill], backed, taking, fill,
            means, self.now(), self.content().day_ticks, &self.content().dials)
    }
}
