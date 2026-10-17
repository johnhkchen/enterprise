//! Bills of exchange, as the mod keeps them (`contract/Bills.java`): a buyer short of coin pays the rest with a bill
//! drawn on itself, due in four days with 2% for the wait. It rides home with the seller's wagon; the seller's next
//! wagon to the drawee presents it once due, and the drawee pays whole bills as far as its money goes (what it owes
//! isn't its money manager's to hold back). Its grace runs from the day it's first put to it: unpaid past that, the
//! bill is dishonoured and trust falls a whole miss; honoured, it counts half a kept installment.

use serde::{Deserialize, Serialize};
use bevy_ecs::prelude::Resource;

use crate::messages::TownId;

/// Days a bill runs, and what it carries over the debt for the wait.
pub const TERM_DAYS: f64 = 4.0;
pub const INTEREST: f64 = 0.02;
/// A drawee's grace once a bill is put to it: so many days, and a day more for every so many emeralds it's for.
pub const GRACE_DAYS: f64 = 8.0;
pub const GRACE_EMERALDS_PER_DAY: f64 = 100.0;
/// A bill paid when presented counts this much of a kept installment toward trust; a dishonoured one, a whole miss.
pub const HONOURED_WEIGHT: f64 = 0.5;

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
    pub amount: f64,
    pub drawn: u64,
    pub due: u64,
    pub presented: Option<u64>,
    pub state: BillState,
    pub held: Held,
}

/// Every bill, and the ones still to be paid.
#[derive(Resource, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bills {
    pub bills: Vec<Bill>,
    pub open: Vec<usize>,
}

impl Bills {
    /// A bill on `drawee` for a debt, payable to `payee`, due in a term: its paper rides with the wagon.
    pub fn draw(&mut self, drawee: TownId, payee: TownId, debt: f64, now: u64, day: u64) -> usize {
        let id = self.bills.len();
        self.bills.push(Bill {
            id,
            drawee,
            payee,
            amount: debt * (1.0 + INTEREST),
            drawn: now,
            due: now + (TERM_DAYS * day as f64) as u64,
            presented: None,
            state: BillState::Outstanding,
            held: Held::Wagon,
        });
        self.open.push(id);
        id
    }

    /// The grace a drawee has to pay a bill, in ticks.
    pub fn grace(bill: &Bill, day: u64) -> u64 {
        ((GRACE_DAYS + bill.amount / GRACE_EMERALDS_PER_DAY) * day as f64) as u64
    }

    /// Outstanding bills on `drawee` falling due by `by`: money its money manager can't count as free.
    pub fn due_by(&self, drawee: TownId, by: u64) -> f64 {
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
