//! The Enterprise core: a society for a factory game. Towns make and use goods, keep money, send wagons to sell
//! to each other within what the buyer can pay. No game is in here: a host passes time and happenings in
//! ([`Input`]) and shows what comes out ([`Output`]). Nothing reads a clock, a file or the network, so the same core
//! runs inside a game, beside it, or in a test, and the same inputs give the same results.
//!
//! Built on Bevy's ECS: towns and routes are entities, the rules are systems run in a fixed order each look
//! ([`rules::look_schedule`]). [`Society`] wraps a world of them for hosts that aren't Bevy apps.
//!
//! The first slice is the trade loop: see `docs/architecture.md` for the whole plan.

pub mod advances;
pub mod bargain;
pub mod bills;
pub mod cash_book;
pub mod chokepoints;
pub mod cargo;
pub mod check;
pub mod content;
pub mod dials;
pub mod farms;
pub mod contracts;
pub mod herds;
pub mod harvest;
pub mod hex;
pub mod jobs;
pub mod lockers;
pub mod map;
pub mod messages;
pub mod mint;
pub mod people;
pub mod prices;
pub mod road;
pub mod roster;
pub mod rules;
pub mod save;
pub mod settlers;
pub mod slices;
pub mod squad;
pub mod society;
pub mod state;
pub mod territory;
pub mod works;
pub mod training;
pub mod workshop;

pub use content::{Content, Dials, Good, GoodId};
pub use contracts::{Contract, Incoterm, State};
pub use hex::Hex;
pub use map::{Ground, Map, Path, Post, Posts};
pub use messages::{Input, Output, RouteId, TownId};
pub use society::Society;
pub use roster::{Hand, Roster, Trade, TradeId};
pub use lockers::{Lockers, Offer};
