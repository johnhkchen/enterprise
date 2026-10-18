//! The core's whole interface: happenings in, results out. The same messages whether the core runs inside a game,
//! beside it or in a test, so a run's messages can be logged and replayed. Towns and routes go by their number, in
//! the order they were added: stable across saves and hosts, unlike the ECS's own entity ids.
//!
//! They live on the wire now (`wire/`, with the envelope a host calls through); here for the hosts and slices that
//! name them from here.

pub use crate::wire::{Input, Output, RouteId, TownId};
