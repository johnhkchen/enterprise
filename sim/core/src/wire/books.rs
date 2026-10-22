//! The books, the cash book, the reasons, trust and contracts as the host's commands show them (`docs/binding-plan.md`
//! 2, concern 11): numbers and words for `/enterprise cashbook`, `books`, `whys`, `trust` and `contracts`, and for the
//! studies that read the same numbers (`dev/experiments`). `commands_host.rs` builds them.

use serde::{Deserialize, Serialize};

use super::TownNo;
use crate::fixed::Fixed;

/// One line of a town's cash book: the day, what the money was for (in words), and the money in (+) or out (-) of its
/// treasury.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BookLine {
    pub day: u64,
    pub what: String,
    pub amount: Fixed,
}

/// A line of a market's books summed: the kind of dealing (`bought`, `sold`, `sent`, `home`, `shop`), with whom (a town's
/// name; `the town` for itself), which good, how many in all and the money.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryLine {
    pub kind: String,
    pub with: String,
    pub good: String,
    pub count: f64,
    pub money: Fixed,
}

/// A reason a market's wagons gave: its words (`with | good | why`), how many times, how many items it was about, and
/// the last day.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WhyView {
    pub line: String,
    pub times: u64,
    pub items: f64,
    pub last: u64,
}

/// Two towns' record: installments kept and missed (as of now), and the trust it gives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrustLine {
    pub a: TownNo,
    pub b: TownNo,
    pub parties: String,
    pub trust: f64,
    pub kept: f64,
    pub missed: f64,
}

/// A contract as `/enterprise contracts` lists it: its number, its parties, state and terms in words, what it is for
/// (`what`: the goods and how many, or a levy of soldiers), how many installments were delivered and are on the road,
/// the money paid and the advance, the installments each side missed, and the trust between the parties.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContractLine {
    pub id: u32,
    pub seller: TownNo,
    pub buyer: TownNo,
    pub seller_name: String,
    pub buyer_name: String,
    pub state: String,
    pub incoterm: String,
    pub copies: u8,
    pub what: String,
    pub delivered: f64,
    pub shipped: f64,
    pub paid: Fixed,
    pub advanced: Fixed,
    pub missed: u32,
    pub buyer_missed: u32,
    pub trust: f64,
    /// What the contract has done, newest last, as lines of words (`day N: ...`): worked out from its state, since the
    /// core keeps no history of events.
    pub history: Vec<String>,
}

/// A rent as `/enterprise contracts` lists it after the contracts: its number, the farm that owes it and the holder
/// (names), what is owed a season (`quantity` of `crop`), what it was bought for, its state in words, the coupons issued,
/// the crop delivered in all and in arrears, and the lines of its history (worked out from its state).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RentLine {
    pub id: u32,
    pub farm: TownNo,
    pub holder: TownNo,
    pub farm_name: String,
    pub holder_name: String,
    pub quantity: f64,
    pub crop: String,
    pub capital: Fixed,
    pub state: String,
    pub coupons: u32,
    pub delivered: f64,
    pub arrears: f64,
    pub history: Vec<String>,
}

/// What a cash book's lines come to for one cause: the sum of the money in (+) or out (-) of the treasury, by what for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForTotal {
    pub what: String,
    pub amount: Fixed,
}
