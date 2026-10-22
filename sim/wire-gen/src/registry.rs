//! The registry: the wire's types by name, each with its serde shape, traced with serde-reflection; what the
//! generator refuses in it; its YAML; and the enums' variant places, held append-only against the committed file.

use std::collections::BTreeMap;

use serde_reflection::{ContainerFormat, Format, Named, Tracer, TracerConfig, VariantFormat};

use enterprise_core::wire::{Answer, Command, GoodFlag, Input, LandKind, Output, Place, PointKind, Query, Refusal, Request, Response, ShopRole, VendorMode, Work};

pub use serde_reflection::Registry;

/// The wire's registry: every type reachable from [`Request`] and [`Response`], the two that cross. Tracing finds
/// every variant only of an enum it's given by name (one met inside another is left at its first variant): a new
/// enum on the wire gets a line here, and until it does tracing fails naming it.
pub fn trace_wire() -> Result<Registry, String> {
    trace_with(|tracer| {
        tracer.trace_simple_type::<Request>()?;
        tracer.trace_simple_type::<Response>()?;
        tracer.trace_simple_type::<Command>()?;
        tracer.trace_simple_type::<Refusal>()?;
        tracer.trace_simple_type::<Input>()?;
        tracer.trace_simple_type::<Output>()?;
        tracer.trace_simple_type::<Query>()?;
        tracer.trace_simple_type::<Answer>()?;
        tracer.trace_simple_type::<PointKind>()?;
        tracer.trace_simple_type::<GoodFlag>()?;
        tracer.trace_simple_type::<Place>()?;
        tracer.trace_simple_type::<LandKind>()?;
        tracer.trace_simple_type::<Work>()?;
        tracer.trace_simple_type::<VendorMode>()?;
        tracer.trace_simple_type::<ShopRole>()?;
        Ok(())
    })
}

/// A registry of whatever `trace` traces (`tracer.trace_simple_type::<T>()` for each root), or why it can't be had.
/// Tracing reads each type's `Deserialize`, so a shape postcard can't carry plainly fails here: an untagged enum or a
/// `#[serde(flatten)]` struct asks for a self-describing format (`deserialize_any`), which postcard isn't.
pub fn trace_with(trace: impl FnOnce(&mut Tracer) -> serde_reflection::Result<()>) -> Result<Registry, String> {
    let mut tracer = Tracer::new(TracerConfig::default());
    let explain = |e: serde_reflection::Error| {
        let hint = match &e {
            serde_reflection::Error::MissingVariants(_) => {
                " (an enum on the wire that isn't traced by name: a line for it in wire-gen's registry::trace_wire)"
            }
            serde_reflection::Error::NotSupported(_) => {
                " (an untagged enum, `#[serde(flatten)]` or another self-describing shape: postcard can't carry it; \
                 name every field and tag every enum)"
            }
            _ => "",
        };
        format!("tracing the wire: {e}{hint}")
    };
    trace(&mut tracer).map_err(explain)?;
    tracer.registry().map_err(explain)
}

/// Everything in the registry the generator won't carry, one line each (none: it can be generated).
pub fn check(registry: &Registry) -> Vec<String> {
    let mut wrong = Vec::new();
    for (name, container) in registry {
        let mut bad = |what: String| wrong.push(format!("{name}: {what}"));
        if !is_type_name(name) {
            bad("not a plain type name (letters and digits, a capital first)".into());
        }
        if RESERVED_TYPE_NAMES.contains(&name.as_str()) {
            bad("a name the generated Java already uses for something else: rename the type".into());
        }
        match container {
            ContainerFormat::UnitStruct => {}
            ContainerFormat::NewTypeStruct(f) => check_format(f, "its value", &mut bad),
            ContainerFormat::TupleStruct(_) => bad("a tuple struct: name its fields".into()),
            ContainerFormat::Struct(fields) => check_fields(fields, &mut bad),
            ContainerFormat::Enum(variants) => {
                if variants.is_empty() {
                    bad("an enum of no variants".into());
                }
                for (i, (index, v)) in variants.iter().enumerate() {
                    if *index as usize != i {
                        bad(format!("variant {} at index {index}, not {i}", v.name));
                    }
                    let mut bad_variant = |what: String| bad(format!("variant {}: {what}", v.name));
                    if !is_type_name(&v.name) {
                        bad_variant("not a plain variant name".into());
                    }
                    if v.name == *name {
                        bad_variant("named as its enum (Java can't nest a type in one of its own name)".into());
                    }
                    if RESERVED_TYPE_NAMES.contains(&v.name.as_str()) {
                        bad_variant("a name the generated Java already uses for something else".into());
                    }
                    match &v.value {
                        VariantFormat::Unit => {}
                        VariantFormat::NewType(f) => check_format(f, "its value", &mut bad_variant),
                        VariantFormat::Tuple(_) => bad_variant("a tuple variant: name its fields".into()),
                        VariantFormat::Struct(fields) => check_fields(fields, &mut bad_variant),
                        VariantFormat::Variable(_) => bad_variant("not traced".into()),
                    }
                }
            }
        }
    }
    wrong
}

fn check_fields(fields: &[Named<Format>], bad: &mut dyn FnMut(String)) {
    let mut java_names = BTreeMap::new();
    for field in fields {
        if !is_field_name(&field.name) {
            bad(format!("field {:?}: not a plain field name (lower case, digits and underscores)", field.name));
        } else if let Some(other) = java_names.insert(crate::java::field_name(&field.name), field.name.clone()) {
            bad(format!("fields {other} and {} are one name in Java", field.name));
        }
        check_format(&field.value, &format!("field {}", field.name), bad);
    }
}

fn check_format(format: &Format, at: &str, bad: &mut dyn FnMut(String)) {
    match format {
        Format::Variable(_) => bad(format!("{at}: not traced")),
        Format::TypeName(_) | Format::Bool | Format::I8 | Format::I16 | Format::I32 | Format::I64 | Format::U8
        | Format::U16 | Format::U32 | Format::U64 | Format::F32 | Format::F64 | Format::Str | Format::Bytes => {}
        Format::Unit => bad(format!("{at}: a () carries nothing: leave it out")),
        Format::I128 | Format::U128 => {
            bad(format!("{at}: a 128-bit number, which Java has no number for: a u64, or two"))
        }
        Format::Char => bad(format!("{at}: a char: a String")),
        Format::Option(f) => check_format(f, at, bad),
        Format::Seq(f) => check_format(f, at, bad),
        Format::Map { .. } => {
            bad(format!("{at}: a map (or a #[serde(flatten)] field): a Vec of a struct naming its key and value"))
        }
        Format::Tuple(_) => bad(format!("{at}: a tuple: a struct naming its fields")),
        Format::TupleArray { .. } => bad(format!("{at}: a fixed-length array: a Vec")),
    }
}

/// Type names the generated Java uses unqualified, which a wire type or variant of the same name would shadow.
pub const RESERVED_TYPE_NAMES: &[&str] = &[
    "Wire", "PostcardIn", "PostcardOut", "Bytes", "WireException", "String", "Object", "Boolean", "Byte", "Short",
    "Integer", "Long", "Float", "Double", "Character", "Number", "Void", "Class", "Record", "Enum", "Override",
    "Math", "System", "Iterable", "Comparable", "Exception", "RuntimeException", "Error", "List", "Optional",
    "Objects", "Function", "BiConsumer", "Consumer", "ArrayList", "Arrays",
];

fn is_type_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_uppercase()) && chars.all(|c| c.is_ascii_alphanumeric())
}

fn is_field_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

// ---- The YAML -------------------------------------------------------------------------------------------------

const HEADER: &str = "\
# The wire's schema: every type that crosses between the core and a host, traced from sim/core/src/wire by
# sim/wire-gen. Written by `just wire-gen`: don't edit. Its FNV-1a 64 is the schema hash (wire::SCHEMA, Wire.SCHEMA).
# An enum's variants are named by their place: new ones go at the end.
";

/// The registry as YAML: serde-reflection's own shape (the names upper-cased, as `serde_yaml` would write a
/// [`Registry`]), types in name order, under a header saying what it is.
pub fn to_yaml(registry: &Registry) -> String {
    let mut out = String::from(HEADER);
    for (name, container) in registry {
        write_entry(&mut out, 0, name, &container_yaml(container));
    }
    out
}

/// A YAML node: a word, a map in order, or a list.
enum Node {
    Word(String),
    Map(Vec<(String, Node)>),
    List(Vec<Node>),
}

fn word(s: &str) -> Node {
    Node::Word(s.to_string())
}

fn tagged(tag: &str, node: Node) -> Node {
    Node::Map(vec![(tag.to_string(), node)])
}

fn named_list(fields: &[Named<Format>]) -> Node {
    Node::List(fields.iter().map(|f| Node::Map(vec![(f.name.clone(), format_yaml(&f.value))])).collect())
}

fn container_yaml(c: &ContainerFormat) -> Node {
    match c {
        ContainerFormat::UnitStruct => word("UNITSTRUCT"),
        ContainerFormat::NewTypeStruct(f) => tagged("NEWTYPESTRUCT", format_yaml(f)),
        ContainerFormat::TupleStruct(fs) => tagged("TUPLESTRUCT", Node::List(fs.iter().map(format_yaml).collect())),
        ContainerFormat::Struct(fields) => tagged("STRUCT", named_list(fields)),
        ContainerFormat::Enum(variants) => tagged(
            "ENUM",
            Node::Map(
                variants
                    .iter()
                    .map(|(i, v)| (i.to_string(), Node::Map(vec![(v.name.clone(), variant_yaml(&v.value))])))
                    .collect(),
            ),
        ),
    }
}

fn variant_yaml(v: &VariantFormat) -> Node {
    match v {
        VariantFormat::Variable(_) => word("UNKNOWN"),
        VariantFormat::Unit => word("UNIT"),
        VariantFormat::NewType(f) => tagged("NEWTYPE", format_yaml(f)),
        VariantFormat::Tuple(fs) => tagged("TUPLE", Node::List(fs.iter().map(format_yaml).collect())),
        VariantFormat::Struct(fields) => tagged("STRUCT", named_list(fields)),
    }
}

fn format_yaml(f: &Format) -> Node {
    match f {
        Format::Variable(_) => word("UNKNOWN"),
        Format::TypeName(n) => tagged("TYPENAME", word(n)),
        Format::Unit => word("UNIT"),
        Format::Bool => word("BOOL"),
        Format::I8 => word("I8"),
        Format::I16 => word("I16"),
        Format::I32 => word("I32"),
        Format::I64 => word("I64"),
        Format::I128 => word("I128"),
        Format::U8 => word("U8"),
        Format::U16 => word("U16"),
        Format::U32 => word("U32"),
        Format::U64 => word("U64"),
        Format::U128 => word("U128"),
        Format::F32 => word("F32"),
        Format::F64 => word("F64"),
        Format::Char => word("CHAR"),
        Format::Str => word("STR"),
        Format::Bytes => word("BYTES"),
        Format::Option(f) => tagged("OPTION", format_yaml(f)),
        Format::Seq(f) => tagged("SEQ", format_yaml(f)),
        Format::Map { key, value } => {
            tagged("MAP", Node::Map(vec![("KEY".into(), format_yaml(key)), ("VALUE".into(), format_yaml(value))]))
        }
        Format::Tuple(fs) => tagged("TUPLE", Node::List(fs.iter().map(format_yaml).collect())),
        Format::TupleArray { content, size } => tagged(
            "TUPLEARRAY",
            Node::Map(vec![("CONTENT".into(), format_yaml(content)), ("SIZE".into(), Node::Word(size.to_string()))]),
        ),
    }
}

/// `key: value` at `indent`, the value on the line if it's a word or an empty list, else below.
fn write_entry(out: &mut String, indent: usize, key: &str, value: &Node) {
    let pad = " ".repeat(indent);
    match value {
        Node::Word(w) => out.push_str(&format!("{pad}{key}: {w}\n")),
        Node::List(items) if items.is_empty() => out.push_str(&format!("{pad}{key}: []\n")),
        Node::Map(entries) if entries.is_empty() => out.push_str(&format!("{pad}{key}: {{}}\n")),
        _ => {
            out.push_str(&format!("{pad}{key}:\n"));
            write_node(out, indent + 2, value);
        }
    }
}

fn write_node(out: &mut String, indent: usize, node: &Node) {
    match node {
        Node::Word(w) => out.push_str(&format!("{}{w}\n", " ".repeat(indent))),
        Node::Map(entries) => {
            for (k, v) in entries {
                write_entry(out, indent, k, v);
            }
        }
        Node::List(items) => {
            for item in items {
                // The item's first line follows its dash; the rest line up under it.
                let mut text = String::new();
                write_node(&mut text, indent + 2, item);
                let text = text.strip_prefix(&" ".repeat(indent + 2)).expect("indented").to_string();
                out.push_str(&format!("{}- {text}", " ".repeat(indent)));
            }
        }
    }
}

// ---- Variant places -------------------------------------------------------------------------------------------

/// Each enum's variants, in their places.
pub type Variants = BTreeMap<String, Vec<String>>;

/// The registry's enums and their variants in order.
pub fn variants(registry: &Registry) -> Variants {
    registry
        .iter()
        .filter_map(|(name, c)| match c {
            ContainerFormat::Enum(vs) => Some((name.clone(), vs.values().map(|v| v.name.clone()).collect())),
            _ => None,
        })
        .collect()
}

/// The enums and their variants in a registry [`to_yaml`] wrote: a type at the margin, `  ENUM:` under it, each
/// variant's place at four spaces and its name at six.
pub fn parse_variants(yaml: &str) -> Result<Variants, String> {
    let mut out = Variants::new();
    let mut current: Option<String> = None;
    let mut in_enum = false;
    let mut expecting: Option<usize> = None;
    for (n, line) in yaml.lines().enumerate() {
        let wrong = |what: &str| format!("{}: line {}: {what}: {line:?}", crate::REGISTRY_PATH, n + 1);
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if let Some(index) = expecting.take() {
            let name = line.trim().split(':').next().unwrap_or("");
            if indent != 6 || name.is_empty() {
                return Err(wrong("a variant's name expected"));
            }
            let enum_name = current.clone().expect("in a type");
            let list = out.entry(enum_name).or_default();
            if list.len() != index {
                return Err(wrong(&format!("variant {index} out of order")));
            }
            list.push(name.to_string());
            continue;
        }
        match indent {
            0 => {
                current = Some(line.trim_end_matches(':').to_string());
                in_enum = false;
            }
            2 => in_enum = line.trim() == "ENUM:",
            4 if in_enum => {
                let index = line.trim().trim_end_matches(':');
                expecting = Some(index.parse().map_err(|_| wrong("a variant's place expected"))?);
            }
            _ => {}
        }
    }
    Ok(out)
}

/// What moved from the committed variant places to the new: every committed variant must keep its place and name (a
/// new one goes after the last). An enum new or gone moves nothing.
pub fn append_only(committed: &Variants, new: &Variants) -> Vec<String> {
    let mut moved = Vec::new();
    for (name, old) in committed {
        let Some(now) = new.get(name) else { continue };
        for (i, was) in old.iter().enumerate() {
            match now.get(i) {
                Some(is) if is == was => {}
                Some(is) => moved.push(format!("{name}: variant {i} was {was}, now {is}")),
                None => moved.push(format!("{name}: variant {i} ({was}) is gone")),
            }
        }
    }
    moved
}
