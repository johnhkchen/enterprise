//! The wire's Rust source (`sim/core/src/wire/`), read for two things the registry doesn't have: where each type is
//! defined (a type on the wire defined anywhere else is a state type, and a state change would become a wire change)
//! and the doc comments, so the Java says what each type, variant and field is.

use std::collections::BTreeMap;
use std::path::Path;

use crate::registry::Registry;

/// A type `wire/` defines: its file, its doc, and its members' docs (a struct's fields, an enum's variants).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TypeDoc {
    pub file: String,
    pub doc: Vec<String>,
    pub members: BTreeMap<String, Vec<String>>,
}

/// Every `pub struct` and `pub enum` in `wire/`, by name.
pub type Docs = BTreeMap<String, TypeDoc>;

/// Reads every `.rs` file in `dir`.
pub fn scan(dir: &Path) -> Result<Docs, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("reading {}: {e}", dir.display()))?;
    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs")).collect();
    files.sort();
    let mut docs = Docs::new();
    for path in files {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let file = path.file_name().expect("a file").to_string_lossy().to_string();
        for (name, doc) in scan_text(&file, &text) {
            if let Some(other) = docs.get(&name) {
                return Err(format!("{name} is defined twice in wire/: {} and {file}", other.file));
            }
            docs.insert(name, doc);
        }
    }
    Ok(docs)
}

/// The types one file defines, with their docs.
pub fn scan_text(file: &str, text: &str) -> Vec<(String, TypeDoc)> {
    let mut out = Vec::new();
    let mut doc: Vec<String> = Vec::new();
    // The type whose body we're in, its depth in braces, and its member's doc so far.
    let mut body: Option<(String, TypeDoc, i32)> = None;
    let mut member_doc: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let code = code_of(trimmed);
        if let Some((name, mut td, depth)) = body.take() {
            if depth == 1 {
                if let Some(d) = doc_line(trimmed) {
                    member_doc.push(d);
                } else if let Some(member) = member_name(code) {
                    td.members.insert(member, std::mem::take(&mut member_doc));
                } else if !trimmed.starts_with("#[") {
                    member_doc.clear();
                }
            }
            let depth = depth + braces(code);
            if depth <= 0 {
                out.push((name, td));
            } else {
                body = Some((name, td, depth));
            }
            continue;
        }
        if let Some(d) = doc_line(trimmed) {
            doc.push(d);
            continue;
        }
        if trimmed.starts_with("#[") {
            continue;
        }
        let kind = ["pub struct ", "pub enum "].into_iter().find(|k| code.starts_with(k));
        if let Some(kind) = kind {
            let name: String = code[kind.len()..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let td = TypeDoc { file: file.to_string(), doc: std::mem::take(&mut doc), members: BTreeMap::new() };
            let depth = braces(code);
            member_doc.clear();
            if depth > 0 {
                body = Some((name, td, depth));
            } else {
                out.push((name, td));
            }
        } else {
            doc.clear();
        }
    }
    out
}

/// A `///` line's text.
fn doc_line(trimmed: &str) -> Option<String> {
    trimmed.strip_prefix("///").map(|d| d.strip_prefix(' ').unwrap_or(d).to_string())
}

/// The line without its comment (a `//` outside a string, which wire types don't hold).
fn code_of(trimmed: &str) -> &str {
    match trimmed.find("//") {
        Some(i) => trimmed[..i].trim_end(),
        None => trimmed,
    }
}

fn braces(code: &str) -> i32 {
    code.chars().map(|c| match c {
        '{' => 1,
        '}' => -1,
        _ => 0,
    }).sum()
}

/// The field or variant a line in a body starts: `pub name: T,`, `name: T,`, `Name { .. },`, `Name,`, `Name(T),`.
fn member_name(code: &str) -> Option<String> {
    let code = code.strip_prefix("pub ").unwrap_or(code);
    let name: String = code.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    if name.is_empty() {
        return None;
    }
    let rest = code[name.len()..].trim_start();
    (rest.is_empty() || rest.starts_with([':', '{', ',', '('])).then_some(name)
}

/// What's wrong between the registry and the types `wire/` defines: a type on the wire defined elsewhere, or one
/// defined in `wire/` that crosses nowhere.
pub fn defined_in_wire(registry: &Registry, docs: &Docs) -> Vec<String> {
    let mut wrong = Vec::new();
    for name in registry.keys() {
        if !docs.contains_key(name) {
            wrong.push(format!(
                "{name} crosses the wire but isn't defined in sim/core/src/wire/: a state type on the wire would make \
                 every change to the state a change to the wire; give the wire a type of its own"
            ));
        }
    }
    for (name, doc) in docs {
        if !registry.contains_key(name) {
            wrong.push(format!(
                "{name} ({}) is defined in wire/ but no Request or Response holds it: wire/ holds only what crosses",
                doc.file
            ));
        }
    }
    wrong
}
