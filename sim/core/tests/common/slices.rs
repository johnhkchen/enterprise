//! Each slice's part of the random worlds: a file of its own in `slices/` with three functions, and a line below.
//!
//! - `extend(s: &mut Society, r: &mut Rng)`: what it adds to a new world (towns' state, contracts, roads...).
//! - `daily(s: &mut Society, r: &mut Rng, day: u64)`: a host's doings each day, after the day's run (payments,
//!   deliveries, posts...).
//! - `check(s: &Society, day: u64) -> Result<(), String>`: its invariants at each day's end.
//!
//! Each gets a random stream of its own, so adding one leaves the others' worlds as they were. Bodies may be empty.
//!
//! **Add lines only.** Git merges this file by union (`.gitattributes`): two slices adding a line never conflict.

use super::*;

slices! {
    roster,
    cash_book,
    territory_growth,
    advances_and_refunds,
    bargain_and_par,
    workshop,
    settlers_rule,
    farms_and_wages,
    herds,
    lockers,
    road_food,
    harvest_and_rents,
    jobs,
    chokepoints,
    crates_and_papers,
    buildings_and_works,
}
