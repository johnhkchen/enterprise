//! Money before the goods, as the mod keeps it (`contract/Advance.java`, `Contract.java` `worth`, `advanceDue`,
//! `recoupFrom`, `contract/ContractBook.java` `advanced`, `recouped`, `refunds`, `broken`): a buyer pays a share of a
//! contract's worth up front, carried by its wagon with the copy (coin as far as the purse goes, the rest a bill on the
//! buyer where trusted); the seller works it off at the same share of each delivery's price, the last delivery taking
//! what's left; if the contract breaks, what isn't worked off is owed back, and the next wagon between the two
//! collects it from what the seller can spare beyond its trading capital (`Caravans.payAdvances`, `settleOwing`).
//!
//! The data lives on [`crate::contracts::Contract`] and [`crate::contracts::Book`]; the rules are in
//! `rules/advances.rs`; this module is the host's calls.

use crate::content::GoodId;
use crate::contracts::{Book, Contract, Incoterm};
use crate::messages::TownId;
use crate::society::Society;

impl Society {
    /// A contract drafted as [`Society::draft`] is, at the buyer's (it holds the copy; its wagon carries it to the
    /// seller), with `advance` the buyer pays the dial `advance_share` of its worth up front.
    #[allow(clippy::too_many_arguments)]
    pub fn draft_advanced(&mut self, seller: TownId, buyer: TownId, good: GoodId, quantity: f64, price: f64, start: u64,
            every: u64, installments: u32, incoterm: Incoterm, at_seller: bool, both: bool, advance: bool) -> usize {
        let share = if advance { self.content().dials.advance_share } else { 0.0 };
        self.world.resource_mut::<Book>().draft_with_advance(seller, buyer, good, quantity, price, start, every,
            installments, incoterm, at_seller, both, share)
    }

    /// Contracts where this seller owes this buyer back an advance, for the buyer's wagon to collect.
    pub fn refunds(&self, seller: TownId, buyer: TownId) -> Vec<Contract> {
        let book = self.world.resource::<Book>();
        book.refunds(seller, buyer).into_iter().map(|id| book.contracts[id].clone()).collect()
    }

    /// Contracts where the buyer owes the seller for goods already delivered.
    pub fn owing(&self, seller: TownId, buyer: TownId) -> Vec<Contract> {
        let book = self.world.resource::<Book>();
        book.owing(seller, buyer).into_iter().map(|id| book.contracts[id].clone()).collect()
    }

    /// What a seller holds paid up front and not yet worked off.
    pub fn advances_held(&self, seller: TownId) -> f64 {
        self.world.resource::<Book>().advances_held(seller)
    }
}
