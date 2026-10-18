//! Old saves, brought forward (`docs/binding-plan.md` 1.4). A blob of an older [`FORMAT`] is read by running it through
//! the [`CHAIN`]: one step a format, each taking the payload of format N and giving the payload of format N+1, until
//! it is today's. A blob of a newer format than this core's is refused (nothing is guessed), and so is an older one the
//! chain no longer reaches back to.
//!
//! **Until the first release the chain is empty and [`FORMAT`] stays 1**: a dev world whose core changed shape is
//! rebuilt (John's decision, 2026-10-18). From the first release, a change to the state's shape after it lands as a
//! format bump with a migration, in this order:
//!
//! 1. **Before** the change, write the golden save of the format about to be left: `SAVES=blob cargo test --release
//!    --test saves` writes `sim/core/tests/saves/vN.blob` (N = the current [`FORMAT`]) from a fixed world. Commit it.
//! 2. Freeze the old shapes: `save/migrate/vN.rs` holds a copy of only the types that changed at N, as they were
//!    (`#[derive(Deserialize)]` is enough: they are only read), and `pub fn run(payload: &[u8]) -> Result<Vec<u8>, String>`
//!    that reads the payload as the frozen shape, builds today's, and writes it with postcard. Postcard has no field
//!    names, so a frozen type must keep the old fields in the old order. The payload is the blob without its header
//!    (magic and format): [`save`](super) writes the header, the chain never sees it.
//! 3. Add `Step { from: N, run: vN::run }` to the end of [`CHAIN`] and bump [`FORMAT`] to N+1. A test fails until the
//!    chain's steps are consecutive and end at [`FORMAT`].
//! 4. Write the digests: `SAVES=digest cargo test --release --test saves` runs every golden save forward some days and
//!    commits what it came to (`vN.digest`). `tests/saves.rs` then fails if a golden doesn't load or doesn't reach its
//!    digest; a change of rules rewrites the digests the way it rewrites `replay_digest.txt`.
//!
//! `tests/saves.rs` also holds the template: a hand-made format 0 (a field renamed and moved) brought through a chain
//! of one step, as a migration `vN.rs` does, to be copied.

use super::FORMAT;

/// One step of the chain: the payload of format `from`, read as it was (a frozen shape), written as format `from + 1`.
#[derive(Clone, Copy)]
pub struct Step {
    pub from: u32,
    pub run: fn(&[u8]) -> Result<Vec<u8>, String>,
}

/// The oldest format this core still brings forward. Raised only by deleting the oldest migrations (and saying so in
/// the release notes): a world older than that is refused.
pub const OLDEST: u32 = 1;

/// The steps from [`OLDEST`] up to [`FORMAT`] (`CHAIN[i].from == OLDEST + i`): empty while [`FORMAT`] is [`OLDEST`].
pub const CHAIN: &[Step] = &[];

/// Why a payload couldn't be brought to today's format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unreadable {
    /// A format this core has no way to read: newer than its own, or older than the chain goes.
    Format { oldest: u32, core: u32, save: u32 },
    /// A step failed on the payload (a broken save).
    Broken { from: u32, why: String },
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unreadable::Format { oldest, core, save } if save > core => {
                write!(f, "a save of format {save}, from a newer core; this core reads formats {oldest} to {core}")
            }
            Unreadable::Format { oldest, core, save } => {
                write!(f, "a save of format {save}, older than this core brings forward; it reads formats {oldest} to {core}")
            }
            Unreadable::Broken { from, why } => write!(f, "a broken save of format {from}: {why}"),
        }
    }
}

/// Whether a save of `format` can be read: it is today's, or the chain reaches it from there.
pub fn reads(format: u32) -> bool {
    reads_with(CHAIN, OLDEST, FORMAT, format)
}

/// The payload of a save of `format` as today's.
pub fn upgrade(format: u32, payload: &[u8]) -> Result<Vec<u8>, Unreadable> {
    upgrade_with(CHAIN, OLDEST, FORMAT, format, payload)
}

/// [`reads`] over any chain (a test's), whose first step is from `oldest` and which ends at `current`.
pub fn reads_with(chain: &[Step], oldest: u32, current: u32, format: u32) -> bool {
    format == current || (format >= oldest && format < current && chain.iter().any(|s| s.from == format))
}

/// [`upgrade`] over any chain (a test's): the steps from `format` on, each at the place its `from` says.
pub fn upgrade_with(chain: &[Step], oldest: u32, current: u32, format: u32, payload: &[u8]) -> Result<Vec<u8>, Unreadable> {
    if !reads_with(chain, oldest, current, format) {
        return Err(Unreadable::Format { oldest, core: current, save: format });
    }
    let mut payload = payload.to_vec();
    for from in format..current {
        let step = chain.iter().find(|s| s.from == from).ok_or(Unreadable::Format { oldest, core: current, save: format })?;
        payload = (step.run)(&payload).map_err(|why| Unreadable::Broken { from, why })?;
    }
    Ok(payload)
}

/// Whether `chain` is whole: its steps are `oldest`, `oldest + 1`... and end just before `current`.
pub fn is_whole(chain: &[Step], oldest: u32, current: u32) -> bool {
    oldest <= current
        && chain.len() as u32 == current - oldest
        && chain.iter().enumerate().all(|(i, s)| s.from == oldest + i as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chain_is_whole_and_reaches_the_current_format() {
        assert!(is_whole(CHAIN, OLDEST, FORMAT), "CHAIN must hold one step for each format from OLDEST up to FORMAT");
    }

    fn plus_one(payload: &[u8]) -> Result<Vec<u8>, String> {
        Ok(payload.iter().map(|b| b + 1).collect())
    }

    fn fails(_: &[u8]) -> Result<Vec<u8>, String> {
        Err("no".into())
    }

    #[test]
    fn a_payload_goes_through_every_step_from_its_format_and_not_the_ones_before() {
        let chain = [Step { from: 1, run: plus_one }, Step { from: 2, run: plus_one }, Step { from: 3, run: plus_one }];
        assert!(is_whole(&chain, 1, 4));
        assert_eq!(upgrade_with(&chain, 1, 4, 1, &[0]), Ok(vec![3]));
        assert_eq!(upgrade_with(&chain, 1, 4, 3, &[0]), Ok(vec![1]));
        assert_eq!(upgrade_with(&chain, 1, 4, 4, &[0]), Ok(vec![0]), "today's is left as it is");
        assert!(!is_whole(&chain, 1, 5) && !is_whole(&chain[1..], 1, 4) && !is_whole(&chain, 1, 3));
    }

    #[test]
    fn a_newer_format_or_one_older_than_the_chain_is_refused_and_a_failing_step_is_a_broken_save() {
        let chain = [Step { from: 2, run: plus_one }, Step { from: 3, run: fails }];
        assert_eq!(upgrade_with(&chain, 2, 4, 5, &[0]), Err(Unreadable::Format { oldest: 2, core: 4, save: 5 }));
        assert_eq!(upgrade_with(&chain, 2, 4, 1, &[0]), Err(Unreadable::Format { oldest: 2, core: 4, save: 1 }));
        assert_eq!(upgrade_with(&chain, 2, 4, 0, &[0]), Err(Unreadable::Format { oldest: 2, core: 4, save: 0 }));
        assert_eq!(upgrade_with(&chain, 2, 4, 2, &[0]), Err(Unreadable::Broken { from: 3, why: "no".into() }));
        assert!(!reads_with(&chain, 2, 4, 5) && !reads_with(&chain, 2, 4, 1) && reads_with(&chain, 2, 4, 3));
        let said = Unreadable::Format { oldest: 2, core: 4, save: 5 }.to_string();
        assert!(said.contains("format 5") && said.contains("newer") && said.contains("2 to 4"), "{said}");
    }
}
