//! The pallet registry in the random worlds: nothing in the rules uses it yet, so a world keeps it empty.

use super::super::*;

pub fn extend(_s: &mut Society, _r: &mut Rng) {}

pub fn daily(_s: &mut Society, _r: &mut Rng, _day: u64) {}

pub fn check(s: &Society, _day: u64) -> Result<(), String> {
    let n = s.world.resource::<enterprise_core::pallet::Pallets>().pallets.len();
    if n == 0 { Ok(()) } else { Err(format!("{n} pallets in a world whose rules don't pack any")) }
}
