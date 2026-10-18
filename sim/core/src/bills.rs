//! Bills of exchange, as the mod keeps them (`contract/Bills.java`): a buyer short of coin pays the rest with a bill
//! drawn on itself, due in four days with 2% for the wait. It rides home with the seller's wagon; the seller's next
//! wagon to the drawee presents it once due, and the drawee pays whole bills as far as its money goes (what it owes
//! isn't its money manager's to hold back). Its grace runs from the day it's first put to it: unpaid past that, the
//! bill is dishonoured and trust falls a whole miss; honoured, it counts half a kept installment. A bank sells it on
//! (`endorse`: the payee changes) or its drawee buys it back early (`redeem`); its grace grows with what the drawee
//! owes the same holder (`grace`); one no one can carry to its drawee lapses (`lapse`). Java: `Bills.endorse`, `redeem`,
//! `grace`, `lapse`, `LAPSE_TICKS`.

use serde::{Deserialize, Serialize};
use bevy_ecs::prelude::Resource;

use crate::fixed::Fixed;
use crate::messages::TownId;

/// Days a bill runs, and what it carries over the debt for the wait.
pub const TERM_DAYS: f64 = 4.0;
pub const INTEREST: f64 = 0.02;
/// A drawee's grace once a bill is put to it: so many days, and a day more for every so many emeralds it's for.
pub const GRACE_DAYS: f64 = 8.0;
pub const GRACE_EMERALDS_PER_DAY: f64 = 100.0;
/// A bill paid when presented counts this much of a kept installment toward trust; a dishonoured one, a whole miss.
pub const HONOURED_WEIGHT: f64 = 0.5;
/// Never paid this many days past its day, a bill no one can carry to its drawee lapses (`LAPSE_TICKS`: three times
/// `DISHONOUR_TICKS`, 4 days); the dial `lapse_days`.
pub const LAPSE_DAYS: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillState {
    Outstanding,
    Paid,
    Dishonoured,
}

/// Where a bill's paper is: with a town, or riding with a wagon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Held {
    Town(TownId),
    Wagon,
}

/// A bill: who pays it, to whom, the sum, when drawn and due, when first put to the drawee, and where its paper is.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bill {
    pub id: usize,
    pub drawee: TownId,
    pub payee: TownId,
    pub amount: Fixed,
    pub drawn: u64,
    pub due: u64,
    pub presented: Option<u64>,
    pub state: BillState,
    pub held: Held,
    /// The contract it pays for, if any (goods crated for the drawee: their title; `Bill.contract`).
    pub contract: Option<usize>,
}

/// Every bill, and the ones still to be paid.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bills {
    pub bills: Vec<Bill>,
    pub open: Vec<usize>,
}

impl Bills {
    /// A bill on `drawee` for a debt, payable to `payee`, due in a term: its paper rides with the wagon.
    pub fn draw(&mut self, drawee: TownId, payee: TownId, debt: impl Into<Fixed>, now: u64, day: u64) -> usize {
        self.draw_for(drawee, payee, debt, now, day, None)
    }

    /// As [`Bills::draw`], paying for goods under `contract`.
    pub fn draw_for(&mut self, drawee: TownId, payee: TownId, debt: impl Into<Fixed>, now: u64, day: u64, contract: Option<usize>)
            -> usize {
        self.draw_term(drawee, payee, debt, now, day, (TERM_DAYS * day as f64) as u64, contract)
    }

    /// As [`Bills::draw_for`], due in `term` ticks rather than a bill term (`Bills.draw`'s `term`: a hub's bill for stranded
    /// paper in installments, a town's bill for freight on credit), its interest a bill term's for every term it runs
    /// (never less than one): `debt * (1 + INTEREST * max(1, term / TERM))`, rounded up to the thousandth (the drawee's
    /// debt, as the mod's withdrawal for it rounds up to the spur).
    pub fn draw_term(&mut self, drawee: TownId, payee: TownId, debt: impl Into<Fixed>, now: u64, day: u64, term: u64,
            contract: Option<usize>) -> usize {
        let debt = debt.into();
        let terms = (term as f64 / (TERM_DAYS * day as f64)).max(1.0);
        let id = self.bills.len();
        self.bills.push(Bill {
            id,
            drawee,
            payee,
            amount: debt.mul_ceil(1.0 + INTEREST * terms),
            drawn: now,
            due: now + term,
            presented: None,
            state: BillState::Outstanding,
            held: Held::Wagon,
            contract,
        });
        self.open.push(id);
        id
    }

    /// The grace a drawee has to pay a bill put to it, in ticks: `grace_days` (the dial `bill_grace_days`), and a day
    /// more for every `per` emeralds (`bill_grace_emeralds_per_day`; 0: none) it owes the same holder on bills
    /// outstanding (a capital owing a town a fortune needs longer to find it): `Bills.grace`.
    pub fn grace(&self, bill: &Bill, day: u64, grace_days: f64, per: f64) -> u64 {
        let owed = self.owed_to(bill.drawee, bill.payee);
        let more = if per > 0.0 { (owed.to_f64() / per * day as f64).round() as u64 } else { 0 };
        (grace_days * day as f64) as u64 + more
    }

    /// Emeralds outstanding on bills `drawee` owes `payee`.
    pub fn owed_to(&self, drawee: TownId, payee: TownId) -> Fixed {
        self.open.iter().map(|&id| &self.bills[id])
            .filter(|b| b.state == BillState::Outstanding && b.drawee == drawee && b.payee == payee)
            .map(|b| b.amount).sum()
    }

    /// The bill is sold on (to a bank): it's paid to `holder` now (`Bills.endorse`).
    pub fn endorse(&mut self, id: usize, holder: TownId) {
        self.bills[id].payee = holder;
    }

    /// The drawee buys back its own bill early (a bank at the drawee's market discounted it): it's settled
    /// (`Bills.redeem`).
    pub fn redeem(&mut self, id: usize) {
        self.bills[id].state = BillState::Paid;
    }

    /// Bills outstanding more than `lapse_days` past their day that no one can carry to their drawee (`carried(payee,
    /// drawee)` false: no road between the two either way, and no freight hub the holder reaches that runs there to buy
    /// them) lapse: dishonoured, the drawee's miss. A bill someone could still carry stands (`Bills.lapse`). Returns the
    /// bills lapsed.
    pub fn lapse(&mut self, book: &mut crate::contracts::Book, now: u64, day: u64, lapse_days: f64,
            carried: impl Fn(TownId, TownId) -> bool) -> Vec<usize> {
        let after = (lapse_days * day as f64) as u64;
        let mut lapsed = Vec::new();
        for &id in &self.open {
            let b = &mut self.bills[id];
            if b.state == BillState::Outstanding && now > b.due + after && !carried(b.payee, b.drawee) {
                b.state = BillState::Dishonoured;
                book.missed(b.drawee, b.payee, 1.0, now, day);
                lapsed.push(id);
            }
        }
        lapsed
    }

    /// Outstanding bills on `drawee` falling due by `by`: money its money manager can't count as free.
    pub fn due_by(&self, drawee: TownId, by: u64) -> Fixed {
        self.open.iter().map(|&id| &self.bills[id])
            .filter(|b| b.drawee == drawee && b.state != BillState::Paid && b.due <= by)
            .map(|b| b.amount).sum()
    }

    /// Bills a town holds on `drawee`, due by `by`, to carry there: their paper goes with the wagon.
    pub fn take_to_present(&mut self, holder: TownId, drawee: TownId, by: u64) -> Vec<usize> {
        let mut taken = Vec::new();
        for &id in &self.open {
            let b = &mut self.bills[id];
            if b.held == Held::Town(holder) && b.payee == holder && b.drawee == drawee && b.due <= by {
                b.held = Held::Wagon;
                taken.push(id);
            }
        }
        taken
    }

    /// The papers a wagon brings home: with the town now.
    pub fn home(&mut self, ids: &[usize], town: TownId) {
        for &id in ids {
            self.bills[id].held = Held::Town(town);
        }
    }

    pub(crate) fn tidy(&mut self) {
        let bills = &self.bills;
        self.open.retain(|&id| bills[id].state != BillState::Paid);
    }
}
