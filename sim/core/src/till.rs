//! A town's money where the host keeps it (`docs/binding-plan.md` 1.5, task `money-adapter`): emeralds and Numismatics
//! coins in a desk's chests, and the settlement's Blaze Banker's account. The mod's `money/Treasury.java` and
//! `money/MoneyChanger.java` are the rules this ports; the core's treasury is one number, a [`Fixed`] of emerald-worth,
//! and the till is how the host holds it.
//!
//! **Counting.** Chests and the Banker's account stay physical, so a player, a hopper or Create adds to them and takes
//! from them. At each look the host counts a desk's till ([`Society::count_cash`], `Input::Cashcount`): whole emeralds
//! in its chests, spurs of coin in them, and the Banker's balance if one touches the desk. What's more than the core took
//! the till to hold is money paid in from outside the society (`Reason::PaidIn`), what's less is money taken out
//! (`Reason::Withdrawn`, never more than the treasury holds): both through `cash_book::post`, so the books balance.
//!
//! **Spurs convert at the treasury.** A spur is 1/16 of an emerald at the mint's base rate, and the rate floats
//! ([`Society::mint_rate`]), so a spur of coin has no worth of its own: it has the worth it came in at. Each pile of
//! coin (the chests', the Banker's) is a [`Hoard`], its spurs and the worth they stand for. Spurs a player pays in come
//! in at the rate then (`spurs / rate`); spurs the mint pays for emeralds stand for those emeralds; spurs taken out
//! take their share of the pile's worth with them. So the rate moving makes no money and loses none, and the treasury
//! needs no posting for a conversion.
//!
//! **Showing.** The treasury is the core's to say; the till shows it. After a look's counts ([`Society::show_cash`],
//! `Output::Cash`), for each desk counted where the host writes:
//!
//! - emeralds above the working reserve ([`EMERALD_RESERVE`], kept while a switched-on trade of the town's lockers
//!   costs emeralds) are sold to the mint for coin: into the Banker's account if there is one, else coins in the
//!   chests, if they have room (`MoneyChanger.settle`);
//! - the treasury above what the till holds is put in as whole emeralds, as far as the chests have room; fractions stay
//!   on the books (as a town's fractions of a good do);
//! - the treasury below what the till holds is taken out: whole emeralds first, then coin, then the Banker, and if those
//!   run short an emerald is changed at the mint and the change goes back as coin (`Treasury.withdraw`).
//!
//! What the host couldn't do comes back as `Input::CashUnmoved`. A host that doesn't write (Java mode, where the Java
//! rules own the chests) only counts: the core sells nothing to the mint and moves nothing.
//!
//! **Not here.** The mod's reserve also holds a wage float (`Territory.wageFloat`: land's); the core has no wage float
//! yet, so its reserve is the emeralds alone. The Merchant changing coin for emeralds to pay its villagers
//! (`MoneyChanger.buyEmeralds`) is the till topping its emeralds up to the reserve from its coin, at the mint's price: the lockers' adapter decides when villagers need more. A sovereign
//! minting coin with nothing behind it is the crown's (`Mint::mint`).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cash_book::{Account, Reason};
use crate::fixed::Fixed;
use crate::messages::TownId;
use crate::points::{Kind, Spot};
use crate::society::Society;

/// Emeralds a Merchant keeps when any of its lockers has a trade switched on that costs emeralds
/// (`MoneyChanger.EMERALD_RESERVE`).
pub const EMERALD_RESERVE: u64 = 64;

/// Emeralds in a stack.
pub const EMERALD_STACK: u64 = 64;

/// A pile of coin: how many spurs, and the emerald-worth they stand for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hoard {
    pub spurs: u64,
    pub worth: Fixed,
}

impl Hoard {
    /// `spurs` more, standing for `worth`.
    pub fn add(&mut self, spurs: u64, worth: Fixed) {
        self.spurs += spurs;
        self.worth += worth;
    }

    /// Takes out `spurs` (as many as it holds, at most), and the worth that goes with them, in proportion (down to the
    /// thousandth). Returns both.
    pub fn take_spurs(&mut self, spurs: u64) -> (u64, Fixed) {
        let spurs = spurs.min(self.spurs);
        if spurs == 0 {
            return (0, Fixed::ZERO);
        }
        if spurs == self.spurs {
            let worth = self.worth;
            *self = Hoard::default();
            return (spurs, worth);
        }
        let worth = Fixed::from_millis((self.worth.millis() as i128 * spurs as i128 / self.spurs as i128) as i64);
        self.spurs -= spurs;
        self.worth -= worth;
        (spurs, worth)
    }

    /// Pays out `spurs` (as many as it holds, at most) and `worth` with them (as much as it holds, at most), whatever
    /// the spurs' own share was: a purchase at the mint settles what its spurs stand for by the worth bought. A pile
    /// left with no spurs stands for nothing. Returns the spurs.
    pub fn pay(&mut self, spurs: u64, worth: Fixed) -> u64 {
        let spurs = spurs.min(self.spurs);
        self.spurs -= spurs;
        self.worth -= worth.min(self.worth).at_least_zero();
        if self.spurs == 0 {
            self.worth = Fixed::ZERO;
        }
        spurs
    }

    /// Pays out `worth` exactly (all of it, if it's worth less) in as few spurs as stand for at least that much; the
    /// spurs left stand for what's left. Returns the spurs and the worth paid.
    pub fn take_worth(&mut self, worth: Fixed) -> (u64, Fixed) {
        if worth <= Fixed::ZERO || self.spurs == 0 {
            return (0, Fixed::ZERO);
        }
        if worth >= self.worth {
            return self.take_spurs(self.spurs);
        }
        let each = self.worth.millis() as i128;
        let spurs = ((self.spurs as i128 * worth.millis() as i128 + each - 1) / each) as u64;
        if spurs >= self.spurs {
            return self.take_spurs(self.spurs); // (the last spur stands for what's left, and no more)
        }
        self.spurs -= spurs;
        self.worth -= worth;
        (spurs, worth)
    }
}

/// What the core takes a desk's chests, and the Banker touching it, to hold.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Till {
    /// Whole emeralds in the chests.
    pub emeralds: u64,
    /// Coin in the chests.
    pub coins: Hoard,
    /// The Blaze Banker's account, if one touches the desk.
    pub bank: Option<Hoard>,
    /// At the last count: whether the chests had room for more coin.
    pub coin_room: bool,
    /// At the last count: whether the host writes the core's moves to them.
    pub writes: bool,
}

impl Till {
    /// What it all stands for.
    pub fn worth(&self) -> Fixed {
        Fixed::emeralds(self.emeralds as i64) + self.coins.worth + self.bank.map_or(Fixed::ZERO, |b| b.worth)
    }
}

/// Every desk's till, by where the desk stands.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tills(pub BTreeMap<Spot, Till>);

/// The core's money moves at a desk: emeralds put in (above nothing) or taken out of its chests, spurs of coin put in or
/// taken out of them, and spurs paid into or taken out of the Banker's account.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CashMove {
    pub at: Spot,
    pub emeralds: i64,
    pub coins: i64,
    pub bank: i64,
}

impl CashMove {
    fn is_none(&self) -> bool {
        self.emeralds == 0 && self.coins == 0 && self.bank == 0
    }
}

/// A desk's till as the core takes it to be, with its town's treasury.
#[derive(Clone, Debug, PartialEq)]
pub struct CashView {
    pub town: TownId,
    pub treasury: Fixed,
    pub till: Till,
    pub reserve: u64,
}

impl Society {
    /// A town's working reserve of emeralds: [`EMERALD_RESERVE`] if any of its lockers has a trade switched on that
    /// costs emeralds (one that sells a good for them), else none (`MoneyChanger.reserve`).
    pub fn emerald_reserve(&self, town: TownId) -> u64 {
        let spends = self.lockers(town).is_some_and(|l| l.hands.iter().any(|h| h.offers.iter().any(|o| o.on && o.sells && o.emeralds > 0)));
        if spends { EMERALD_RESERVE } else { 0 }
    }

    /// The treasury brought up to now (the town makes and uses for the time since), as a withdrawal is.
    fn touch_treasury(&mut self, town: TownId) {
        let e = self.towns[town];
        let now = self.now();
        self.world.resource_scope(|world, content: Mut<crate::content::Content>| {
            world.resource_scope(|world, mut ledger: Mut<crate::state::Ledger>| {
                world.resource_scope(|world, mut book: Mut<crate::cash_book::CashBook>| {
                    let mut t = world.entity_mut(e);
                    let id = t.get::<crate::state::Town>().expect("a town").id;
                    let trades = t.get::<crate::state::Trades>().expect("a town").clone();
                    let mut touched = *t.get::<crate::state::Touched>().unwrap();
                    let mut money = *t.get::<crate::state::Money>().unwrap();
                    let mut stock = t.get::<crate::state::Stock>().unwrap().clone();
                    crate::rules::touch::catch_up_town(now, &content, &mut ledger, &mut book, id, &mut stock, &mut money,
                        &trades, &mut touched);
                    t.insert((stock, money, touched));
                })
            })
        });
    }

    /// What a desk's chests and Banker hold now, as the host counted them: `emeralds` whole emeralds, `coins` spurs of
    /// coin, and the Banker's `bank` balance (`None`: no Banker touches the desk). What's more than the core took them to
    /// hold is paid into the town's treasury from outside the society, what's less taken out of it (never more than it
    /// holds): spurs at the mint's rate now. `coin_room` is whether the chests could take more coin, `writes` whether the
    /// host writes the core's moves to them. This is what they hold from now on. Returns the worth booked (+ in, - out).
    /// Errors if no desk stands at `at`.
    pub fn count_cash(&mut self, at: &Spot, emeralds: u64, coins: u64, bank: Option<u64>, coin_room: bool, writes: bool)
        -> Result<Fixed, String> {
        let town = match self.point(at) {
            Some(p) if p.kind == Kind::Desk => p.town,
            Some(p) => return Err(format!("a {:?} at {at:?}, not a desk", p.kind)),
            None => return Err(format!("no point at {at:?}")),
        };
        self.touch_treasury(town);
        let rate = self.mint_rate();
        let mut till = self.world.resource::<Tills>().0.get(at).cloned().unwrap_or_default();
        let before = till.worth();
        till.emeralds = emeralds;
        till.coins = recount(till.coins, coins, rate);
        till.bank = match (till.bank, bank) {
            (_, None) => None,
            (Some(held), Some(now)) => Some(recount(held, now, rate)),
            (None, Some(now)) => Some(recount(Hoard::default(), now, rate)),
        };
        till.coin_room = coin_room;
        till.writes = writes;
        let booked = till.worth() - before;
        self.world.resource_mut::<Tills>().0.insert(at.clone(), till);
        if booked.is_positive() {
            self.post(Account::Outside, Account::Treasury(town), booked, Reason::PaidIn);
        } else if booked.is_negative() {
            let held = self.balance(Account::Treasury(town)).expect("a town's treasury");
            self.post(Account::Treasury(town), Account::Outside, (-booked).min(held).at_least_zero(), Reason::Withdrawn);
        }
        Ok(booked)
    }

    /// A desk's till gone with the desk: what the core took its chests and Banker to hold leaves the treasury (never
    /// more than it holds), since they are the host's, and a desk put there again counts them in afresh. Called by
    /// [`Society::remove_point`].
    pub(crate) fn forget_till(&mut self, at: &Spot, town: TownId) {
        let Some(till) = self.world.resource_mut::<Tills>().0.remove(at) else { return };
        let gone = till.worth();
        if gone.is_positive() {
            self.touch_treasury(town);
            let held = self.balance(Account::Treasury(town)).expect("a town's treasury");
            self.post(Account::Treasury(town), Account::Outside, gone.min(held).at_least_zero(), Reason::Withdrawn);
        }
    }

    /// The core's money moves at the desks among `counted` (points the host counted in this look, where the host writes):
    /// the surplus sold to the mint, the treasury put in or taken out of the till (see the module). Each is in the till
    /// from now on. In towns' order, one desk to a town (its first counted).
    pub fn show_cash(&mut self, counted: &[Spot]) -> Vec<CashMove> {
        self.show_cash_after(counted, &[])
    }

    /// [`Society::show_cash`] after the same look's stock moves (`chests.rs`'s `show_stock`): the empty slots they fill
    /// are no room for emeralds.
    pub fn show_cash_after(&mut self, counted: &[Spot], stock: &[crate::chests::Move]) -> Vec<CashMove> {
        let taken = self.slots_taken(stock);
        let mut by_town: BTreeMap<TownId, Spot> = BTreeMap::new();
        for at in counted {
            let Some(p) = self.point(at).filter(|p| p.kind == Kind::Desk) else { continue };
            if self.world.resource::<Tills>().0.get(at).is_some_and(|t| t.writes) {
                by_town.entry(p.town).or_insert_with(|| at.clone());
            }
        }
        let mut moves = Vec::new();
        for (town, at) in by_town {
            let mv = self.show_town_cash(town, &at, taken.get(&at).copied().unwrap_or(0));
            if !mv.is_none() {
                moves.push(mv);
            }
        }
        moves
    }

    /// The empty slots stock moves into a point fill (each move's new stacks, against what the point holds after it).
    fn slots_taken(&self, stock: &[crate::chests::Move]) -> BTreeMap<Spot, u64> {
        let mut taken: BTreeMap<Spot, u64> = BTreeMap::new();
        for m in stock.iter().filter(|m| m.count > 0) {
            let Some(p) = self.point(&m.at) else { continue };
            let stack = self.content().goods.get(m.good).map_or(1, |g| g.stack.max(1)) as f64;
            let after = p.counted.get(m.good).copied().unwrap_or(0.0);
            let before = (after - m.count as f64).max(0.0);
            let stacks = ((after - 1e-9) / stack).ceil().max(0.0) - ((before - 1e-9) / stack).ceil().max(0.0);
            *taken.entry(m.at.clone()).or_default() += stacks.max(0.0) as u64;
        }
        taken
    }

    fn show_town_cash(&mut self, town: TownId, at: &Spot, taken: u64) -> CashMove {
        self.touch_treasury(town);
        let treasury = self.balance(Account::Treasury(town)).expect("a town's treasury");
        let reserve = self.emerald_reserve(town);
        let mut free = (self.point(at).map_or(0, |p| p.free) as u64).saturating_sub(taken);
        let others: Fixed = self.world.resource::<Tills>().0.iter()
            .filter(|(s, _)| *s != at && self.point(s).is_some_and(|p| p.town == town))
            .map(|(_, t)| t.worth()).sum();
        let mut till = self.world.resource::<Tills>().0.get(at).cloned().expect("a counted desk");
        let mut mv = CashMove { at: at.clone(), ..Default::default() };

        // The treasury against what the till holds.
        let books = treasury - till.worth() - others;
        if books >= Fixed::ONE {
            // Whole emeralds in, as far as the chests have room.
            let n = ((books.millis() / 1000) as u64).min(emerald_room(&till, free));
            let stacks = till.emeralds.div_ceil(EMERALD_STACK);
            till.emeralds += n;
            free = free.saturating_sub(till.emeralds.div_ceil(EMERALD_STACK) - stacks);
            mv.emeralds += n as i64;
        } else if books.is_negative() {
            self.take_out(&mut till, &mut mv, -books);
        }

        // Short of the working reserve, the Merchant changes coin for emeralds at the mint (`MoneyChanger.buyEmeralds`).
        if till.emeralds < reserve {
            let want = (reserve - till.emeralds).min(emerald_room(&till, free));
            self.buy_emeralds(&mut till, &mut mv, want);
        }

        // Emeralds above the reserve are sold to the mint for coin: the Banker's account, else the chests (if they have
        // room) (`MoneyChanger.settle`).
        if till.emeralds > reserve && (till.bank.is_some() || till.coin_room) && self.mint_pays(till.emeralds - reserve) {
            let n = till.emeralds - reserve;
            let spurs = self.sell_emeralds(n);
            till.emeralds -= n;
            mv.emeralds -= n as i64;
            let sold = Hoard { spurs, worth: Fixed::emeralds(n as i64) };
            pay_in(&mut till, &mut mv, sold);
        }
        self.world.resource_mut::<Tills>().0.insert(at.clone(), till);
        mv
    }

    /// Whether the mint would pay any spurs for `emeralds` now (the spur is worth so little after heavy selling that it
    /// may not).
    fn mint_pays(&self, emeralds: u64) -> bool {
        self.mint().quote(emeralds, self.now(), self.content().day_ticks) > 0
    }

    /// `need` of worth taken out of the till: whole emeralds first, then coin, then the Banker, and if those run short
    /// an emerald is changed at the mint, the part-emerald paid and the change back as coin (`Treasury.withdraw`).
    fn take_out(&mut self, till: &mut Till, mv: &mut CashMove, mut need: Fixed) {
        let whole = ((need.millis() / 1000) as u64).min(till.emeralds);
        till.emeralds -= whole;
        mv.emeralds -= whole as i64;
        need -= Fixed::emeralds(whole as i64);
        if need.is_positive() {
            let (spurs, worth) = till.coins.take_worth(need);
            need -= worth;
            mv.coins -= spurs as i64;
        }
        if need.is_positive() {
            if let Some(bank) = till.bank.as_mut() {
                let (spurs, worth) = bank.take_worth(need);
                need -= worth;
                mv.bank -= spurs as i64;
            }
        }
        let change = till.emeralds.min((need.millis() as u64).div_ceil(1000).max(1));
        if need.is_positive() && till.emeralds > 0 && (till.bank.is_some() || till.coin_room) && self.mint_pays(change) {
            let n = change;
            let spurs = self.sell_emeralds(n);
            till.emeralds -= n;
            mv.emeralds -= n as i64;
            let mut changed = Hoard { spurs, worth: Fixed::emeralds(n as i64) };
            changed.take_worth(need);
            pay_in(till, mv, changed);
        }
    }

    /// Up to `emeralds` bought from the mint with the till's coin, then the Banker's: the spurs it costs (the rate
    /// over, by `mint::BUY_SPREAD`) leave the piles with `emeralds`' worth between them, so the spread shows in what a
    /// spur stands for, not in the treasury; the emeralds come into the chests, and the pressure the mint had is taken
    /// back. As many as the coin pays for.
    fn buy_emeralds(&mut self, till: &mut Till, mv: &mut CashMove, emeralds: u64) {
        let now = self.now();
        let day = self.content().day_ticks;
        let funds = till.coins.spurs + till.bank.map_or(0, |b| b.spurs);
        let mut n = emeralds;
        while n > 0 && self.mint().buy_cost(n, now, day) > funds {
            n -= 1;
        }
        if n == 0 {
            return;
        }
        let cost = self.world.resource_mut::<crate::mint::Mint>().buy(n, now, day);
        let worth = Fixed::emeralds(n as i64);
        // The coin pays first, with its share of the worth the emeralds stand for; the Banker's account the rest.
        let from_coins = cost.min(till.coins.spurs);
        let share = Fixed::from_millis((worth.millis() as i128 * from_coins as i128 / cost as i128) as i64);
        mv.coins -= till.coins.pay(from_coins, share) as i64;
        if cost > from_coins {
            if let Some(bank) = till.bank.as_mut() {
                mv.bank -= bank.pay(cost - from_coins, worth - share) as i64;
            }
        }
        till.emeralds += n;
        mv.emeralds += n as i64;
    }

    /// What the host couldn't do of a [`CashMove`] at the desk at `at`: `emeralds` didn't fit (above nothing) or weren't
    /// there to take (below), and the same for `coins` and the Banker's `bank` spurs. The till holds that much less (or
    /// more), never below nothing; the treasury is the same, so the next count's difference is shown again. False if no
    /// desk stands at `at` (a stale view, ignored).
    pub fn cash_unmoved(&mut self, at: &Spot, emeralds: i64, coins: i64, bank: i64) -> bool {
        if !self.point(at).is_some_and(|p| p.kind == Kind::Desk) {
            return false;
        }
        let rate = self.mint_rate();
        let Some(mut till) = self.world.resource::<Tills>().0.get(at).cloned() else { return false };
        till.emeralds = (till.emeralds as i64 - emeralds).max(0) as u64;
        unmove(&mut till.coins, coins, rate);
        if let Some(b) = till.bank.as_mut() {
            unmove(b, bank, rate);
        }
        self.world.resource_mut::<Tills>().0.insert(at.clone(), till);
        true
    }

    /// Every desk's till.
    pub fn tills(&self) -> &Tills {
        self.world.resource::<Tills>()
    }

    /// A desk's till as the core takes it to be, if a desk stands at `at`.
    pub fn cash(&self, at: &Spot) -> Option<CashView> {
        let p = self.point(at).filter(|p| p.kind == Kind::Desk)?;
        let till = self.world.resource::<Tills>().0.get(at).cloned().unwrap_or_default();
        let treasury = self.balance(Account::Treasury(p.town)).expect("a town's treasury");
        Some(CashView { town: p.town, treasury, till, reserve: self.emerald_reserve(p.town) })
    }
}

/// Room in a desk's chests for emeralds: what's left of a stack of them, and `free` empty slots.
fn emerald_room(till: &Till, free: u64) -> u64 {
    let partial = till.emeralds % EMERALD_STACK;
    (if partial > 0 { EMERALD_STACK - partial } else { 0 }) + free * EMERALD_STACK
}

/// Coin paid into the till: into the Banker's account if there is one, else into the chests.
fn pay_in(till: &mut Till, mv: &mut CashMove, coin: Hoard) {
    if let Some(bank) = till.bank.as_mut() {
        bank.add(coin.spurs, coin.worth);
        mv.bank += coin.spurs as i64;
    } else {
        till.coins.add(coin.spurs, coin.worth);
        mv.coins += coin.spurs as i64;
    }
}

/// A pile counted again at `spurs`: more come in at the rate now, fewer go out with their share of the worth.
fn recount(mut held: Hoard, spurs: u64, rate: f64) -> Hoard {
    if spurs >= held.spurs {
        let more = spurs - held.spurs;
        held.add(more, Fixed::round(more as f64 / rate));
    } else {
        held.take_spurs(held.spurs - spurs);
    }
    held
}

/// A pile the host couldn't move: `spurs` above nothing didn't fit (so the pile holds that many fewer), below nothing
/// weren't there to take (that many more, at the pile's own worth a spur, or the rate's if it was empty).
fn unmove(pile: &mut Hoard, spurs: i64, rate: f64) {
    if spurs > 0 {
        pile.take_spurs(spurs as u64);
    } else if spurs < 0 {
        let more = spurs.unsigned_abs();
        let worth = if pile.spurs > 0 {
            Fixed::from_millis((pile.worth.millis() as i128 * more as i128 / pile.spurs as i128) as i64)
        } else {
            Fixed::round(more as f64 / rate)
        };
        pile.add(more, worth);
    }
}
