//! The crown, its realms and its metal exchange as the host sees them (`docs/binding-plan.md` 2, concern 10; task
//! `crown-views`): what `/enterprise realms` and `metals` show. The rules are the core's own (`crown.rs`, `army.rs`,
//! `realms.rs`, `sourcing.rs`); `crown_host.rs` builds these from the state it keeps. Nothing here is saved.

use serde::{Deserialize, Serialize};

use super::{GoodNo, TownNo};

/// A realm beyond the border as `/enterprise realms` tells of it (`Realms.report`): its trade with the markets it
/// sends wagons to, in spurs at the mint's rate now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RealmLine {
    pub town: TownNo,
    pub name: String,
    /// Trips its wagons made, the markets it trades with, spurs sold and spurs bought on them, the customs it paid the
    /// crown, the goods its people made and the spurs it holds.
    pub trips: u32,
    pub partners: u32,
    pub sold_spurs: f64,
    pub bought_spurs: f64,
    pub customs_spurs: f64,
    pub made: f64,
    pub in_hand_spurs: f64,
}

/// A metal the exchange keeps, as `/enterprise metals` tells of it (`MetalExchange`): what it holds of its target, what
/// is coming under contract, and how much over its worth it pays now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetalLine {
    pub good: GoodNo,
    pub name: String,
    pub held: f64,
    pub target: f64,
    pub coming: f64,
    pub premium: f64,
}
