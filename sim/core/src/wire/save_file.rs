//! The society as a host keeps it with its world: the state's blob (`save.rs`), and what wrote it.

use serde::{Deserialize, Serialize};

use crate::society::Society;

/// A saved society: the blob's [`crate::save::FORMAT`] (a core reads only its own: `docs/binding-plan.md` 1.4), the
/// wire's [`super::SCHEMA`] and the [`super::CORE`] that wrote it (for a bug report: neither is checked on a load),
/// and the blob.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    pub format: u32,
    pub schema: u64,
    pub core: String,
    pub blob: Vec<u8>,
}

impl SaveFile {
    /// The society, saved.
    pub fn of(society: &mut Society) -> SaveFile {
        SaveFile { format: crate::save::FORMAT, schema: super::SCHEMA, core: super::CORE.to_string(), blob: society.save() }
    }
}
