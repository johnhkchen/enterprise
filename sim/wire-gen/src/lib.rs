//! The wire's generator (`docs/binding-plan.md` 1.2): the core's wire types (`sim/core/src/wire/`) are the source, and
//! everything a host in another language needs is generated from them.
//!
//! 1. [`registry::trace_wire`] traces the types with serde-reflection, from [`Request`] and [`Response`] down: the
//!    registry, every type's serde shape by name. [`registry::check`] refuses a shape postcard and Java can't carry
//!    plainly (`flatten`, untagged enums, maps, tuples, `u128`, `char`), and [`sources`] a type on the wire that isn't
//!    defined in `wire/` (a state type would make every state change a wire change).
//! 2. [`registry::to_yaml`] writes it as `sim/wire/registry.yaml`, committed and reviewed like code. Its FNV-1a 64 is
//!    the schema hash: the core's `wire::SCHEMA` (computed from the committed file when the core compiles) and the
//!    Java's `Wire.SCHEMA`. A request built against another is refused.
//! 3. [`java::emit`] writes the Java 21: a record per struct, a sealed interface with a record per variant per enum
//!    (a Java `enum` where every variant is bare), a postcard codec on each (`encode(PostcardOut)`, `decode(PostcardIn)`,
//!    `toBytes`, `fromBytes`), the runtime they share (`PostcardIn`, `PostcardOut`, `Bytes`, `WireException`) and
//!    `Wire` (the ABI and the schema), into `binding/src/generated/java/dev/b28/enterprise/wire/`.
//!
//! **Enums are append-only.** Postcard names a variant by its place, so a variant inserted, moved or taken out
//! misreads every message, input log and fixture after it. [`registry::append_only`] holds each enum's committed
//! variants to their places: new ones go at the end, or `--renumber` says so out loud.
//!
//! `just wire-gen` writes the files; `just wire-check` (and this crate's tests) fail when the committed ones differ
//! from what the wire types generate.
//!
//! [`Request`]: enterprise_core::wire::Request
//! [`Response`]: enterprise_core::wire::Response

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod java;
pub mod registry;
pub mod sources;

pub use registry::Registry;

/// Where the registry goes, from the repository's root.
pub const REGISTRY_PATH: &str = "sim/wire/registry.yaml";
/// Where the generated Java goes, from the repository's root.
pub const JAVA_DIR: &str = "binding/src/generated/java/dev/b28/enterprise/wire";
/// The core's wire types, from the repository's root.
pub const WIRE_SOURCES: &str = "sim/core/src/wire";

/// The repository this generator was built in (`sim/wire-gen/../..`).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// FNV-1a, 64 bits: the schema hash of the registry's bytes (the same as the core's `wire::SCHEMA`).
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// What the wire generates: every file by its path from the repository's root, the schema hash, and what was said
/// on the way (an enum renumbered with `--renumber`).
#[derive(Debug)]
pub struct Generated {
    pub files: BTreeMap<String, String>,
    pub schema: u64,
    pub warnings: Vec<String>,
}

/// Everything the core's wire generates, checked against the committed registry under `root` (enums append-only
/// unless `renumber`) and the types defined in its `sim/core/src/wire/`.
pub fn generate(root: &Path, renumber: bool) -> Result<Generated, String> {
    let registry = registry::trace_wire()?;
    let committed = std::fs::read_to_string(root.join(REGISTRY_PATH)).ok();
    let docs = sources::scan(&root.join(WIRE_SOURCES))?;
    generate_from(&registry, &docs, committed.as_deref(), renumber)
}

/// [`generate`] for a traced registry, the types its source defines and the committed registry if there is one.
pub fn generate_from(registry: &Registry, docs: &sources::Docs, committed: Option<&str>, renumber: bool)
    -> Result<Generated, String> {
    let mut wrong = registry::check(registry);
    wrong.extend(sources::defined_in_wire(registry, docs));
    if !wrong.is_empty() {
        return Err(format!("the wire can't be generated:\n  {}", wrong.join("\n  ")));
    }
    let yaml = registry::to_yaml(registry);
    let mut warnings = Vec::new();
    if let Some(committed) = committed {
        let moved = registry::append_only(&registry::parse_variants(committed)?, &registry::variants(registry));
        if !moved.is_empty() {
            if !renumber {
                return Err(format!(
                    "an enum's variants moved: postcard names a variant by its place, so every message, input log and \
                     fixture after it would misread. Put new variants at the end of their enum; to renumber anyway \
                     (old logs and fixtures unreadable), or to take out one never committed, \
                     `just wire-gen --renumber`.\n  {}",
                    moved.join("\n  ")
                ));
            }
            for m in moved {
                warnings.push(format!("RENUMBERED (old logs and fixtures of it no longer read): {m}"));
            }
        }
    }
    let schema = fnv1a(yaml.as_bytes());
    let mut files = BTreeMap::new();
    for (name, text) in java::emit(registry, docs, schema, enterprise_core::wire::ABI)? {
        files.insert(format!("{JAVA_DIR}/{name}"), text);
    }
    files.insert(REGISTRY_PATH.to_string(), yaml);
    Ok(Generated { files, schema, warnings })
}

/// Writes what was generated under `root`, and takes out generated Java no type makes any more. Returns the paths
/// written or removed.
pub fn write(root: &Path, generated: &Generated) -> std::io::Result<Vec<String>> {
    let mut changed = Vec::new();
    for (path, text) in &generated.files {
        let full = root.join(path);
        if std::fs::read_to_string(&full).ok().as_deref() != Some(text.as_str()) {
            std::fs::create_dir_all(full.parent().expect("a file in a folder"))?;
            std::fs::write(&full, text)?;
            changed.push(path.clone());
        }
    }
    for stale in stale_java(root, generated) {
        std::fs::remove_file(root.join(&stale))?;
        changed.push(format!("{stale} (removed)"));
    }
    Ok(changed)
}

/// What differs between what was generated and the files under `root`: each file missing, different or left over.
pub fn differences(root: &Path, generated: &Generated) -> Vec<String> {
    let mut wrong = Vec::new();
    for (path, text) in &generated.files {
        match std::fs::read_to_string(root.join(path)) {
            Err(_) => wrong.push(format!("{path}: missing")),
            Ok(on_disk) if on_disk != *text => wrong.push(format!("{path}: differs{}", first_difference(&on_disk, text))),
            Ok(_) => {}
        }
    }
    for stale in stale_java(root, generated) {
        wrong.push(format!("{stale}: generated by no wire type"));
    }
    wrong
}

/// The first line where the committed text and the generated differ, to say where.
fn first_difference(committed: &str, generated: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (committed.lines().collect(), generated.lines().collect());
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or("(the end)"), b.get(i).copied().unwrap_or("(the end)"));
        if x != y {
            return format!(" at line {}: committed {x:?}, generated {y:?}", i + 1);
        }
    }
    String::new()
}

/// Java files in the generated folder that nothing generates any more.
fn stale_java(root: &Path, generated: &Generated) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join(JAVA_DIR)) else {
        return Vec::new();
    };
    let mut stale: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".java"))
        .map(|name| format!("{JAVA_DIR}/{name}"))
        .filter(|path| !generated.files.contains_key(path))
        .collect();
    stale.sort();
    stale
}
