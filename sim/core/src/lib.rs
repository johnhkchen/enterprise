//! The Enterprise core: a society for a factory game. Towns make and use goods, keep money, send wagons to sell
//! to each other within what the buyer can pay. No game is in here: a host passes time and happenings in
//! ([`Input`]) and shows what comes out ([`Output`]). Nothing reads a clock, a file or the network, so the same core
//! runs inside a game, beside it, or in a test, and the same inputs give the same results.
//!
//! Built on Bevy's ECS: towns and routes are entities, the rules are systems run in a fixed order each look
//! ([`rules::look_schedule`]). [`Society`] wraps a world of them for hosts that aren't Bevy apps.
//!
//! The first slice is the trade loop: see `docs/architecture.md` for the whole plan.

pub mod bills;
pub mod check;
pub mod content;
pub mod contracts;
pub mod messages;
pub mod mint;
pub mod people;
pub mod rules;
pub mod society;
pub mod state;

pub use content::{Content, Dials, Good, GoodId};
pub use contracts::{Contract, Incoterm, State};
pub use messages::{Input, Output, RouteId, TownId};
pub use society::Society;
