//! Double-entry books, as the mod keeps its cash book (`money/CashBook.java`: `note`, `totals`, `KEPT_DAYS`) and its
//! Accountant (`money/Accountant.java`: `free`, `today`, `left`, `leftFor`, `approve`, `allow`), with the money moved
//! as `money/Treasury.java` moves it (`withdraw`, `deposit`, `available`: the book noted on every move).
//!
//! Every emerald that moves goes from one account to another ([`post`]): a town's treasury, its households' purse, a
//! wagon's purse, or the outside (money paid into the society, and money spent out of it: road food, homes,
//! children). Each posting is recorded on both sides, by day and by what it was for, so a town that goes broke can be
//! read off its book, and the books balance by construction: every account's balance moves by exactly what's posted
//! to it, and the society's money by what the outside posted in less what it was paid. `check.rs` reconciles both.
//!
//! The mod's book is a treasury's only, labelled by its caller; here every account has one, labelled by a
//! [`Reason`]. The Accountant sits on the town's day's budget ([`Budget`], set at the day's first look from its free
//! money, `rules::free`: money beyond its trading capital, less bills due within `budget_days`; the mod's `reserve`,
//! `territory/Territory.java`, also holds back a wage float and guards' pay, which the core doesn't have yet): every
//! spender at the town's own choosing draws on the one budget, so no two spend the same spare money.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::content::{Content, GoodId};
use crate::fixed::Fixed;
use crate::messages::{RouteId, TownId};
use crate::state::{Budget, Ledger};

/// Days of entries a book keeps (the running totals are kept for ever).
pub const KEPT_DAYS: u64 = 60;
/// Postings of nothing aren't noted, as the mod's `note` skips the smallest (money is in thousandths: `fixed.rs`).
pub const TOO_SMALL: Fixed = Fixed::ZERO;

/// Where money is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum Account {
    /// A town's money.
    Treasury(TownId),
    /// A town's households' purse: their wages, which pay for their comforts.
    Households(TownId),
    /// The purse of the wagon on a route (by its number).
    Wagon(RouteId),
    /// Outside the society: money paid in, and money spent out of it.
    #[default]
    Outside,
}

/// What money moved for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum Reason {
    /// The town's people buying a good they use from its warehouse ("lockers: sold wheat").
    Sold(GoodId),
    /// A share of what the town makes, paid its households.
    Wages,
    /// The households paying for the comforts they use.
    Comforts,
    /// A wagon buying a round trip's road food.
    RoadFood,
    /// The money a wagon takes for its Ex Works collections.
    WagonPurse,
    /// A buyer paying a wagon for goods sold it off the cart.
    Sale,
    /// Goods under contract paid for, and debts under contracts paid off.
    Contract,
    /// A bill of exchange paid when presented.
    Bill,
    /// A wagon's takings into its town's money, home again.
    Takings,
    /// A home built.
    Home,
    /// A child raised.
    Child,
    /// Money paid a town from outside the society (a player buying at its shops).
    PaidIn,
    /// Anything else, in words.
    #[default]
    Unlabelled,
    Other(String),
}

impl Reason {
    /// The reason in words, as a book shows it.
    pub fn label(&self, content: &Content) -> String {
        match self {
            Reason::Sold(g) => format!("lockers: sold {}", content.goods.get(*g).map_or("?", |g| g.name.as_str())),
            Reason::Wages => "wages".into(),
            Reason::Comforts => "comforts".into(),
            Reason::RoadFood => "road food".into(),
            Reason::WagonPurse => "wagon purse".into(),
            Reason::Sale => "sale".into(),
            Reason::Contract => "contract".into(),
            Reason::Bill => "bill".into(),
            Reason::Takings => "takings".into(),
            Reason::Home => "home".into(),
            Reason::Child => "child".into(),
            Reason::PaidIn => "paid in".into(),
            Reason::Unlabelled => "?".into(),
            Reason::Other(s) => s.clone(),
        }
    }

    /// The reason for a label: a good sold by its name, one of the words above, or the words themselves.
    pub fn parse(content: &Content, label: &str) -> Reason {
        if let Some(g) = label.strip_prefix("lockers: sold ").and_then(|name| content.good(name)) {
            return Reason::Sold(g);
        }
        let known = [Reason::Wages, Reason::Comforts, Reason::RoadFood, Reason::WagonPurse, Reason::Sale, Reason::Contract,
            Reason::Bill, Reason::Takings, Reason::Home, Reason::Child, Reason::PaidIn];
        known.into_iter().find(|r| r.label(content) == label).unwrap_or_else(|| Reason::Other(label.to_string()))
    }
}

/// One side of a posting: the account, and its balance to move (none for the outside).
pub struct Side<'a> {
    pub account: Account,
    pub balance: Option<&'a mut Fixed>,
}

impl<'a> Side<'a> {
    pub fn treasury(town: TownId, money: &'a mut Fixed) -> Self {
        Side { account: Account::Treasury(town), balance: Some(money) }
    }
    pub fn households(town: TownId, purse: &'a mut Fixed) -> Self {
        Side { account: Account::Households(town), balance: Some(purse) }
    }
    pub fn wagon(route: RouteId, purse: &'a mut Fixed) -> Self {
        Side { account: Account::Wagon(route), balance: Some(purse) }
    }
    pub fn outside() -> Self {
        Side { account: Account::Outside, balance: None }
    }
}

/// Every account's book: what moved in (+) and out (-) by day and reason, the last [`KEPT_DAYS`]; every account's
/// net postings since the world began; and what the Accountant held back (noted, not moved).
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct CashBook {
    pub books: BTreeMap<Account, BTreeMap<(u64, Reason), Fixed>>,
    pub net: BTreeMap<Account, Fixed>,
    pub held: BTreeMap<Account, BTreeMap<(u64, Reason), Fixed>>,
}

impl CashBook {
    /// `amount` in (+) or out (-) of `account` for `reason`, on the day of `now`; days past keeping dropped.
    pub fn note(&mut self, account: Account, reason: Reason, amount: Fixed, now: u64, day_ticks: u64) {
        *self.net.entry(account).or_default() += amount;
        if amount.abs() <= TOO_SMALL {
            return;
        }
        let day = now / day_ticks.max(1);
        let book = self.books.entry(account).or_default();
        *book.entry((day, reason)).or_default() += amount;
        while book.first_key_value().is_some_and(|((d, _), _)| *d + KEPT_DAYS < day) {
            book.pop_first();
        }
    }

    /// The Accountant held back `amount` the town would have spent on `reason`.
    pub fn hold(&mut self, account: Account, reason: Reason, amount: Fixed, now: u64, day_ticks: u64) {
        if amount <= TOO_SMALL {
            return;
        }
        let day = now / day_ticks.max(1);
        let held = self.held.entry(account).or_default();
        *held.entry((day, reason)).or_default() += amount;
        while held.first_key_value().is_some_and(|((d, _), _)| *d + KEPT_DAYS < day) {
            held.pop_first();
        }
    }

    /// An account's entries on days `from` to `to` (inclusive), by day then reason.
    pub fn entries(&self, account: Account, from: u64, to: u64) -> Vec<(u64, Reason, Fixed)> {
        self.books.get(&account).map_or_else(Vec::new, |book| {
            book.range((from, Reason::Sold(0))..).take_while(|((d, _), _)| *d <= to)
                .map(|((d, r), v)| (*d, r.clone(), *v)).collect()
        })
    }

    /// An account's totals by reason on days `from` to `to` (inclusive), largest first by size (the mod's `totals`).
    pub fn totals(&self, account: Account, from: u64, to: u64) -> Vec<(Reason, Fixed)> {
        let mut out: BTreeMap<Reason, Fixed> = BTreeMap::new();
        for (_, r, v) in self.entries(account, from, to) {
            *out.entry(r).or_default() += v;
        }
        let mut out: Vec<(Reason, Fixed)> = out.into_iter().collect();
        out.sort_by(|a, b| b.1.abs().cmp(&a.1.abs()));
        out
    }

    /// An account's net postings since the world began.
    pub fn net(&self, account: Account) -> Fixed {
        self.net.get(&account).copied().unwrap_or_default()
    }
}

/// Moves `amount` from one account to another for `reason`, noting both sides; money crossing into or out of the
/// society also keeps the ledger's running totals (`income` in; `road` for road food, `spent` for the rest, out).
#[allow(clippy::too_many_arguments)]
pub fn post(ledger: &mut Ledger, book: &mut CashBook, from: Side, to: Side, amount: Fixed, reason: Reason, now: u64,
        day_ticks: u64) {
    if amount.is_zero() {
        return;
    }
    post_legs(ledger, book, from, vec![(to, amount, reason)], now, day_ticks);
}

/// One payment out of `from` in several legs, each to its own account for its own reason: taken off `from` as one sum,
/// noted leg by leg as [`post`] notes one.
pub fn post_legs(ledger: &mut Ledger, book: &mut CashBook, from: Side, legs: Vec<(Side, Fixed, Reason)>, now: u64,
        day_ticks: u64) {
    let total: Fixed = legs.iter().map(|(_, a, _)| *a).sum();
    if let Some(b) = from.balance {
        *b -= total;
    }
    for (to, amount, reason) in legs {
        if amount.is_zero() {
            continue;
        }
        if let Some(b) = to.balance {
            *b += amount;
        }
        if from.account == Account::Outside {
            ledger.income += amount;
        }
        if to.account == Account::Outside {
            if reason == Reason::RoadFood {
                ledger.road += amount;
            } else {
                ledger.spent += amount;
            }
        }
        book.note(from.account, reason.clone(), -amount, now, day_ticks);
        book.note(to.account, reason, amount, now, day_ticks);
    }
}

/// A town's Accountant: what it may spend at its own choosing today, on its day's budget.
pub struct Accountant;

impl Accountant {
    /// What's left of today's budget (set at the day's first look, from `free` then), less what's spent of it.
    pub fn left(content: &Content, now: u64, free: Fixed, budget: &mut Budget) -> Fixed {
        crate::rules::left_for(content, now, free, budget, 1.0)
    }

    /// May the town spend `amount` on `what` now? If so it's taken off today's budget; if not, the book notes it held
    /// back.
    #[allow(clippy::too_many_arguments)]
    pub fn approve(content: &Content, book: &mut CashBook, town: TownId, now: u64, free: Fixed, budget: &mut Budget,
            what: Reason, amount: Fixed) -> bool {
        if amount <= Fixed::ZERO {
            return true;
        }
        if amount > Self::left(content, now, free, budget) {
            book.hold(Account::Treasury(town), what, amount, now, content.day_ticks);
            return false;
        }
        budget.spent += amount;
        true
    }

    /// As much of `amount` for `what` as today's budget has left, taken off it; the rest noted as held back.
    #[allow(clippy::too_many_arguments)]
    pub fn allow(content: &Content, book: &mut CashBook, town: TownId, now: u64, free: Fixed, budget: &mut Budget,
            what: Reason, amount: Fixed) -> Fixed {
        if amount <= Fixed::ZERO {
            return Fixed::ZERO;
        }
        let allowed = amount.min(Self::left(content, now, free, budget));
        if allowed < amount {
            book.hold(Account::Treasury(town), what, amount - allowed, now, content.day_ticks);
        }
        budget.spent += allowed;
        allowed
    }
}
