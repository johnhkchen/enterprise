//! The wire: everything that crosses between the core and a host, and nothing else (`docs/binding-plan.md` 1.2).
//!
//! A host sends a [`Request`] and gets a [`Response`]: one call a look, the same natively, in WebAssembly under Java
//! (Chicory) or from a companion program. The types here hold numbers, strings, ids and vectors of each, and other
//! wire types, never a state type: the state can change shape every day and a host never knows, and only a change to
//! these regenerates the Java (`wire-gen`), which refuses a wire type that holds anything else (it walks the registry).
//!
//! - [`Request`]: the ABI and schema it was built against, and a [`Command`]: create a society from a host's
//!   [`ContentPack`], load or save one, apply a look's happenings ([`Input`]), or ask it questions ([`Query`]).
//! - [`Response`]: what came of the happenings ([`Output`]) and the answers ([`Answer`]), a [`SaveFile`], or a
//!   [`Refusal`]: a request the core won't run is a message, never a panic, and nothing of it is applied.
//! - [`handle`]: a request in, a response out, over the society a host keeps (`None` until it creates or loads one);
//!   [`handle_bytes`] the same as postcard bytes, which is what the WebAssembly module's `ent_call` runs.
//!
//! **Adding to the wire.** A new variant goes at the end of its enum, a new field at the end of its struct: postcard
//! names a variant by its place and has no field names, so a reordering misreads every message after it (`wire-gen`
//! refuses a moved variant). Then `just wire-gen` regenerates `sim/wire/registry.yaml` and the Java (a doc comment
//! here is the Java's too: regenerate after editing one), and a golden fixture and a scenario show it. A new enum
//! gets a line in `sim/wire-gen/src/registry.rs::trace_wire`, so every variant is traced.

use serde::{Deserialize, Serialize};

mod handle;
mod input;
mod output;
mod pack;
mod pallet;
mod point;
mod query;
mod save_file;

pub use crate::content::GoodId;
pub use handle::{handle, handle_bytes};
pub use input::{Count, Input};
pub use output::Output;
pub use pack::{ContentPack, GoodFlag, PackGood};
pub use pallet::{PackageLine, Place};
pub use point::{Point, PointKind};
pub use query::{Answer, Query, TownPoint};
pub use save_file::SaveFile;

/// A town, by its place in the order towns were added: stable across saves and hosts, unlike an ECS entity.
pub type TownId = usize;
/// A route, by its place in the order routes were added.
pub type RouteId = usize;

/// The ABI's number: the four exports of the WebAssembly module (`ent_abi`, `ent_alloc`, `ent_call`, `ent_free`).
/// Bumped only if those change.
pub const ABI: u32 = 1;

/// The wire's schema: a request built against another is refused ([`Refusal::Schema`]), so a jar built from two
/// commits fails at its first call rather than misreading. The FNV-1a 64 of the committed `sim/wire/registry.yaml`
/// (what `wire-gen` traces from these types), computed when the core compiles: the generated Java's `Wire.SCHEMA` is
/// the same number, and `just wire-check` fails while the registry isn't what these types make.
pub const SCHEMA: u64 = fnv1a(include_bytes!("../../../wire/registry.yaml"));

/// FNV-1a, 64 bits, at compile time.
const fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        h ^= bytes[i] as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
        i += 1;
    }
    h
}

/// The core that wrote a save: the build's revision when it's given one (`ENTERPRISE_CORE_REV` at compile time),
/// otherwise the crate's version.
pub const CORE: &str = match option_env!("ENTERPRISE_CORE_REV") {
    Some(rev) => rev,
    None => concat!("enterprise-core ", env!("CARGO_PKG_VERSION")),
};

/// A call to the core: built against [`ABI`] and [`SCHEMA`], with what to do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub abi: u32,
    pub schema: u64,
    pub command: Command,
}

impl Request {
    /// A request for `command` as this core builds them (its own ABI and schema).
    pub fn new(command: Command) -> Self {
        Request { abi: ABI, schema: SCHEMA, command }
    }
}

/// What a request asks the core to do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// A new society for the host's content, its dice seeded with `seed`, in place of any it had.
    Create { pack: ContentPack, seed: u64 },
    /// A society from a save, in place of any it had (kept if the save is refused).
    Load { save: SaveFile },
    /// The society as a save, for the host to keep with its world.
    Save,
    /// A look's happenings, in order: its stocktakes, deliveries and other doings, then its `Advance`. All of them are
    /// checked before any is applied.
    Apply { inputs: Vec<Input> },
    /// Questions, answered in order. Asking changes nothing.
    Ask { queries: Vec<Query> },
}

/// The core's reply to a request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Response {
    /// Done: what came of the happenings, in order, and the answers to the questions, in the order asked.
    Done { outputs: Vec<Output>, answers: Vec<Answer> },
    /// The society saved.
    Saved { save: SaveFile },
    /// Not done, and nothing of it applied.
    Refused { why: Refusal },
}

/// Why the core wouldn't run a request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Refusal {
    /// The bytes aren't a request this core can read.
    Unreadable { why: String },
    /// Built against another ABI.
    Abi { core: u32, request: u32 },
    /// Built against another wire schema: the host's generated code and the core are from different commits.
    Schema { core: u64, request: u64 },
    /// There's no society yet: create or load one first.
    NoSociety,
    /// A content pack the core won't run, and everything wrong with it.
    Pack { why: String },
    /// A save of a format this core doesn't read.
    SaveFormat { core: u32, save: u32 },
    /// A save the core can't load.
    Save { why: String },
    /// The happening at `at` (its place in `Apply`'s list) names something that isn't there or makes no sense.
    Input { at: u32, why: String },
    /// The question at `at` names something that isn't there.
    Query { at: u32, why: String },
}
