//! The generator: the committed registry and Java are what the core's wire types make (`just wire-check`); the
//! schema hash is one number on both sides; the registry holds only types `wire/` defines; shapes postcard and Java
//! can't carry plainly are refused; enums are append-only against the committed registry.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_reflection::{ContainerFormat, Format, Named, VariantFormat};

use enterprise_wire_gen::registry::{self, Registry};
use enterprise_wire_gen::{fnv1a, generate, generate_from, repo_root, sources, JAVA_DIR, REGISTRY_PATH, WIRE_SOURCES};

fn wire() -> (Registry, sources::Docs) {
    (registry::trace_wire().unwrap(), sources::scan(&repo_root().join(WIRE_SOURCES)).unwrap())
}

#[test]
fn the_committed_registry_and_java_are_what_the_wire_generates() {
    let generated = generate(&repo_root(), false).unwrap();
    let wrong = enterprise_wire_gen::differences(&repo_root(), &generated);
    assert!(wrong.is_empty(), "run `just wire-gen`:\n  {}", wrong.join("\n  "));
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);
}

#[test]
fn the_schema_is_the_committed_registrys_hash_on_both_sides() {
    let committed = std::fs::read(repo_root().join(REGISTRY_PATH)).unwrap();
    let generated = generate(&repo_root(), false).unwrap();
    assert_eq!(generated.schema, fnv1a(&committed));
    assert_eq!(enterprise_core::wire::SCHEMA, generated.schema, "the core compiled against another registry");
    let wire_java = &generated.files[&format!("{JAVA_DIR}/Wire.java")];
    assert!(wire_java.contains(&format!("SCHEMA = 0x{:016x}L;", generated.schema)), "{wire_java}");
    assert!(wire_java.contains(&format!("ABI = {}L;", enterprise_core::wire::ABI)), "{wire_java}");
}

#[test]
fn fnv1a_is_the_reference_one() {
    // The published FNV-1a 64 test vectors.
    assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn every_wire_type_is_traced_and_carried() {
    let (registry, docs) = wire();
    assert!(registry::check(&registry).is_empty(), "{:?}", registry::check(&registry));
    assert!(sources::defined_in_wire(&registry, &docs).is_empty(), "{:?}", sources::defined_in_wire(&registry, &docs));
    let names: Vec<&str> = registry.keys().map(String::as_str).collect();
    for name in ["Request", "Command", "Response", "Refusal", "Input", "Output", "Query", "Answer", "Point", "SaveFile"] {
        assert!(names.contains(&name), "{name} not traced: {names:?}");
    }
    // Every enum traced to its last variant.
    let variants = registry::variants(&registry);
    assert_eq!(variants["Command"], ["Create", "Load", "Save", "Apply", "Ask"]);
    assert_eq!(variants["PointKind"], ["Desk", "Locker", "Shop", "Banker", "Depot"]);
}

#[test]
fn a_state_type_on_the_wire_is_refused() {
    let (mut registry, docs) = wire();
    // A wire type holding a state type: the state type is traced into the registry, and isn't wire/'s.
    registry.insert("People".into(), ContainerFormat::Struct(vec![Named { name: "count".into(), value: Format::U32 }]));
    let wrong = sources::defined_in_wire(&registry, &docs);
    assert_eq!(wrong.len(), 1, "{wrong:?}");
    assert!(wrong[0].starts_with("People crosses the wire but isn't defined in sim/core/src/wire/"), "{wrong:?}");
    assert!(generate_from(&registry, &docs, None, false).is_err());
    // And a type wire/ defines that crosses nowhere.
    let (registry, mut docs) = wire();
    docs.insert("Loose".into(), sources::TypeDoc { file: "loose.rs".into(), ..Default::default() });
    let wrong = sources::defined_in_wire(&registry, &docs);
    assert!(wrong.len() == 1 && wrong[0].contains("Loose (loose.rs) is defined in wire/ but no Request"), "{wrong:?}");
}

// ---- Shapes refused -----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Inner {
    a: u32,
}

#[derive(Serialize, Deserialize)]
struct Flat {
    #[serde(flatten)]
    inner: Inner,
    b: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Loose {
    A(u32),
    B(String),
}

#[derive(Serialize, Deserialize)]
struct Wide {
    big: u128,
    signed: i128,
}

#[derive(Serialize, Deserialize)]
struct Odd {
    letter: char,
    pair: (u32, f64),
    fixed: [u8; 4],
    map: BTreeMap<u32, f64>,
    nothing: (),
}

#[derive(Serialize, Deserialize)]
struct Pair(u32, f64);

#[derive(Serialize, Deserialize)]
enum Tupled {
    Two(u32, f64),
}

/// What tracing `T` and checking it says is wrong, all of it.
fn refusals<T: for<'de> Deserialize<'de>>() -> String {
    match registry::trace_with(|t| t.trace_simple_type::<T>().map(|_| ())) {
        Err(why) => why,
        Ok(r) => registry::check(&r).join("\n"),
    }
}

#[test]
fn flatten_is_refused() {
    let why = refusals::<Flat>();
    assert!(!why.is_empty(), "a flattened struct traced and passed");
    assert!(why.contains("flatten"), "{why}");
}

#[test]
fn an_untagged_enum_is_refused() {
    let why = refusals::<Loose>();
    assert!(why.contains("Not supported") && why.contains("untagged"), "{why}");
}

#[test]
fn a_128_bit_number_is_refused() {
    let why = refusals::<Wide>();
    assert!(why.contains("Wide: field big: a 128-bit number"), "{why}");
    assert!(why.contains("Wide: field signed: a 128-bit number"), "{why}");
}

#[test]
fn chars_tuples_arrays_maps_and_units_are_refused() {
    let why = refusals::<Odd>();
    for what in ["field letter: a char", "field pair: a tuple", "field fixed: a fixed-length array",
        "field map: a map", "field nothing: a () carries nothing"] {
        assert!(why.contains(what), "{what} not refused:\n{why}");
    }
    assert!(refusals::<Pair>().contains("Pair: a tuple struct"), "{}", refusals::<Pair>());
    assert!(refusals::<Tupled>().contains("variant Two: a tuple variant"), "{}", refusals::<Tupled>());
}

#[test]
fn names_java_would_misread_are_refused() {
    let mut r = Registry::new();
    r.insert("List".into(), ContainerFormat::UnitStruct);
    r.insert("Thing".into(), ContainerFormat::Enum(BTreeMap::from([(0, Named { name: "Thing".into(), value: VariantFormat::Unit })])));
    r.insert("Two".into(), ContainerFormat::Struct(vec![
        Named { name: "spurs_each".into(), value: Format::F64 },
        Named { name: "spurs__each".into(), value: Format::F64 },
    ]));
    let why = registry::check(&r).join("\n");
    assert!(why.contains("List: a name the generated Java already uses"), "{why}");
    assert!(why.contains("Thing: variant Thing: named as its enum"), "{why}");
    assert!(why.contains("fields spurs_each and spurs__each are one name in Java"), "{why}");
}

// ---- Enums append-only --------------------------------------------------------------------------------------

/// The wire's registry with `Input`'s variants changed by `change`.
fn with_input(change: impl FnOnce(&mut Vec<Named<VariantFormat>>)) -> Registry {
    let (mut registry, _) = wire();
    let Some(ContainerFormat::Enum(variants)) = registry.get_mut("Input") else { panic!("Input is an enum") };
    let mut list: Vec<_> = std::mem::take(variants).into_values().collect();
    change(&mut list);
    *variants = list.into_iter().enumerate().map(|(i, v)| (i as u32, v)).collect();
    registry
}

#[test]
fn the_registrys_variants_read_back_from_its_yaml() {
    let (registry, _) = wire();
    let yaml = registry::to_yaml(&registry);
    assert_eq!(registry::parse_variants(&yaml).unwrap(), registry::variants(&registry));
    let committed = std::fs::read_to_string(repo_root().join(REGISTRY_PATH)).unwrap();
    assert_eq!(registry::parse_variants(&committed).unwrap(), registry::variants(&registry));
}

#[test]
fn a_moved_variant_is_refused_and_renumbering_says_so() {
    let (registry, docs) = wire();
    let committed = registry::to_yaml(&registry);
    let swapped = with_input(|v| v.swap(1, 2));
    let why = generate_from(&swapped, &docs, Some(&committed), false).unwrap_err();
    assert!(why.contains("an enum's variants moved"), "{why}");
    assert!(why.contains("Input: variant 1 was Deliver, now Pay"), "{why}");
    assert!(why.contains("Input: variant 2 was Pay, now Deliver"), "{why}");
    let renumbered = generate_from(&swapped, &docs, Some(&committed), true).unwrap();
    assert_eq!(renumbered.warnings.len(), 2, "{:?}", renumbered.warnings);
    assert!(renumbered.warnings[0].starts_with("RENUMBERED"), "{:?}", renumbered.warnings);
    assert_ne!(renumbered.schema, generate_from(&registry, &docs, Some(&committed), false).unwrap().schema);
}

#[test]
fn an_inserted_or_removed_variant_is_refused() {
    let (registry, docs) = wire();
    let committed = registry::to_yaml(&registry);
    let inserted = with_input(|v| v.insert(0, Named { name: "Hello".into(), value: VariantFormat::Unit }));
    let why = generate_from(&inserted, &docs, Some(&committed), false).unwrap_err();
    assert!(why.contains("Input: variant 0 was Advance, now Hello"), "{why}");
    let removed = with_input(|v| {
        v.pop();
    });
    let why = generate_from(&removed, &docs, Some(&committed), false).unwrap_err();
    let last = input_variants(&registry).pop().expect("an Input variant");
    assert!(why.contains(&format!("({last}) is gone")), "{why}");
}

/// The names of `Input`'s variants, in their order (so these tests don't move each time the wire grows one).
fn input_variants(registry: &serde_reflection::Registry) -> Vec<String> {
    match registry.get("Input") {
        Some(ContainerFormat::Enum(vs)) => vs.values().map(|v| v.name.clone()).collect(),
        _ => panic!("no Input enum"),
    }
}

#[test]
fn a_variant_added_at_the_end_is_welcome() {
    let (registry, docs) = wire();
    let committed = registry::to_yaml(&registry);
    let appended = with_input(|v| v.push(Named { name: "Hello".into(), value: VariantFormat::Unit }));
    let generated = generate_from(&appended, &docs, Some(&committed), false).unwrap();
    assert!(generated.warnings.is_empty());
    let input = &generated.files[&format!("{JAVA_DIR}/Input.java")];
    let at = input_variants(&registry).len();
    assert!(input.contains(&format!("case {at} -> new Hello();")), "{input}");
    assert!(input.contains("record Hello() implements Input"), "{input}");
}

// ---- The Java's shape ---------------------------------------------------------------------------------------

#[test]
fn the_java_names_every_wire_type_and_routes_every_variant() {
    let (registry, _) = wire();
    let generated = generate(&repo_root(), false).unwrap();
    for (name, container) in &registry {
        let java = &generated.files[&format!("{JAVA_DIR}/{name}.java")];
        match container {
            ContainerFormat::Struct(_) => assert!(java.contains(&format!("public record {name}(")), "{java}"),
            ContainerFormat::Enum(vs) if vs.values().all(|v| v.value == VariantFormat::Unit) => {
                assert!(java.contains(&format!("public enum {name} {{")), "{java}")
            }
            ContainerFormat::Enum(vs) => {
                assert!(java.contains(&format!("public sealed interface {name}")), "{java}");
                for (i, v) in vs {
                    assert!(java.contains(&format!("{name}.{}", v.name)), "{name}.{} not permitted", v.name);
                    assert!(java.contains(&format!("case {i} -> new {}(", v.name)), "{name}.{}", v.name);
                    assert!(java.contains(&format!("out.variant({i});")), "{name}.{}", v.name);
                }
            }
            _ => {}
        }
    }
    // u64 ids and ticks as long, u32 as long, money (Fixed, an i64) as long, a save's blob as Bytes.
    let input = &generated.files[&format!("{JAVA_DIR}/Input.java")];
    assert!(input.contains("record Advance(long to) implements Input"), "{input}");
    assert!(input.contains("record Pay(long town, long money) implements Input"), "{input}");
    let save = &generated.files[&format!("{JAVA_DIR}/SaveFile.java")];
    assert!(save.contains("public record SaveFile(long format, long schema, String core, Bytes blob)"), "{save}");
    let pack = &generated.files[&format!("{JAVA_DIR}/PackGood.java")];
    assert!(pack.contains("Optional<String> comfort, List<GoodFlag> flags, double weight"), "{pack}");
    assert!(pack.contains("in.option(PostcardIn::string)"), "{pack}");
}

#[test]
fn a_type_a_variant_shadows_is_named_in_full() {
    // Refusal has variants named Input and Query; were one to hold an Input, the record's own name would hide it.
    let (mut registry, docs) = wire();
    let Some(ContainerFormat::Enum(vs)) = registry.get_mut("Refusal") else { panic!() };
    let next = vs.len() as u32;
    vs.insert(next, Named { name: "Holds".into(), value: VariantFormat::Struct(vec![Named {
        name: "input".into(), value: Format::TypeName("Input".into()) }]) });
    let generated = generate_from(&registry, &docs, None, false).unwrap();
    let refusal = &generated.files[&format!("{JAVA_DIR}/Refusal.java")];
    assert!(refusal.contains("record Holds(dev.b28.enterprise.wire.Input input) implements Refusal"), "{refusal}");
    assert!(refusal.contains("new Holds(dev.b28.enterprise.wire.Input.decode(in))"), "{refusal}");
}

#[test]
fn docs_follow_their_types_variants_and_fields() {
    let text = "/// A thing.\n/// Two lines.\n#[derive(Debug)]\npub struct Thing {\n    /// Its count.\n    pub count: u32,\n    \
        pub plain: u32,\n}\n\n/// Kinds.\npub enum Kind {\n    /// The first.\n    A { x: u32 },\n    B,\n    /// The third,\n    /// in two.\n    C,\n}\n";
    let docs: BTreeMap<_, _> = sources::scan_text("t.rs", text).into_iter().collect();
    assert_eq!(docs["Thing"].doc, ["A thing.", "Two lines."]);
    assert_eq!(docs["Thing"].members["count"], ["Its count."]);
    assert!(docs["Thing"].members["plain"].is_empty());
    assert_eq!(docs["Kind"].members["A"], ["The first."]);
    assert!(docs["Kind"].members["B"].is_empty());
    assert_eq!(docs["Kind"].members["C"], ["The third,", "in two."]);
}

#[test]
fn a_serde_attribute_or_a_path_in_a_wire_type_is_refused() {
    // serde-reflection traces only Deserialize: `skip_serializing_if` and `serialize_with` would trace as plain
    // shapes while postcard writes something else, so the source is read for them (the old source scan's rule).
    let text = "/// A thing.\n#[derive(Serialize, Deserialize)]\n#[serde(rename_all = \"lowercase\")]\npub struct Outer {\n    \
        pub x: u32,\n}\n\npub struct Thing {\n    pub x: u32,\n    #[serde(skip_serializing_if = \"Option::is_none\", default)]\n    \
        pub y: Option<u32>,\n}\n\npub enum Kind {\n    A { held: Vec<crate::state::Stock> },\n    B,\n}\n\n\
        #[serde(transparent)]\nstruct Private(u32);\n\npub struct Plain {\n    pub n: u32,\n}\n";
    let docs: BTreeMap<_, _> = sources::scan_text("t.rs", text).into_iter().collect();
    assert_eq!(docs["Outer"].refused.len(), 1, "{:?}", docs["Outer"].refused);
    assert!(docs["Outer"].refused[0].starts_with("Outer (t.rs:3: "), "{:?}", docs["Outer"].refused);
    assert_eq!(docs["Thing"].refused.len(), 2, "{:?}", docs["Thing"].refused); // the attribute and its `Option::is_none`
    assert!(docs["Thing"].refused[0].starts_with("Thing (t.rs:10): a serde attribute inside"), "{:?}", docs["Thing"].refused);
    assert_eq!(docs["Kind"].refused.len(), 1, "{:?}", docs["Kind"].refused);
    assert!(docs["Kind"].refused[0].contains("a field named by a path"), "{:?}", docs["Kind"].refused);
    // A private item's attribute isn't carried to the next wire type.
    assert!(docs["Plain"].refused.is_empty(), "{:?}", docs["Plain"].refused);
    // And through the generator: the real wire with one such type in it is refused.
    let (registry, mut docs) = wire();
    docs.get_mut("Request").unwrap().refused.push("Request (request.rs:1): a serde attribute".into());
    let wrong = sources::defined_in_wire(&registry, &docs);
    assert_eq!(wrong, ["Request (request.rs:1): a serde attribute"]);
    assert!(generate_from(&registry, &docs, None, false).is_err());
    // The real wire has none.
    let (registry, docs) = wire();
    assert!(docs.values().all(|d| d.refused.is_empty()));
    assert!(sources::defined_in_wire(&registry, &docs).is_empty());
}

#[test]
fn a_backslash_in_a_doc_cant_become_a_java_unicode_escape() {
    // javac reads a backslash-u as a unicode escape even inside a comment: `C:\users` would stop the folder compiling.
    let (registry, mut docs) = wire();
    docs.get_mut("Request").unwrap().doc = vec![r"Kept at C:\users\me, written `\u{41}` in Rust.".into()];
    let generated = generate_from(&registry, &docs, None, false).unwrap();
    let java = &generated.files[&format!("{JAVA_DIR}/Request.java")];
    assert!(java.contains("C:&#92;users&#92;me"), "{java}");
    assert!(java.contains("<code>&#92;u{41}</code>"), "{java}");
    assert!(!java.contains('\\'), "{java}");
}
