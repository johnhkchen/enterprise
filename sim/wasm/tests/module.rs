//! The built module: the imports it asks of a host are only the allowed set, its exports are the four (and its
//! memory), and the proof's requests run in it under Node's WASI answer with the bytes the native core gives.
//!
//! The module is built here (`cargo build --profile wasm --target wasm32-wasip1`). Without that target or Node a test
//! says so and passes, unless `ENT_WASM_REQUIRED` is set (`just wasm-test` sets it).

mod proof;

use std::path::{Path, PathBuf};
use std::process::Command;

use enterprise_core::wire::handle_bytes;

/// What a module may ask its host for: Rust's std on `wasm32-wasip1`, for an abort's message and its environment.
/// Never `random_get` or a clock: either would break determinism.
const ALLOWED_IMPORTS: &[(&str, &str)] = &[
    ("wasi_snapshot_preview1", "fd_write"),
    ("wasi_snapshot_preview1", "proc_exit"),
    ("wasi_snapshot_preview1", "environ_get"),
    ("wasi_snapshot_preview1", "environ_sizes_get"),
];

const EXPORTS: &[&str] = &["ent_abi", "ent_alloc", "ent_call", "ent_free", "memory"];

fn sim() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// The built module's path, or None (said aloud) when this machine can't build or run it.
fn module() -> Option<PathBuf> {
    let required = std::env::var_os("ENT_WASM_REQUIRED").is_some();
    let skip = |why: &str| {
        assert!(!required, "ENT_WASM_REQUIRED is set and {why}");
        eprintln!("skipped: {why}");
        None
    };
    if Command::new("node").arg("--version").output().is_err() {
        return skip("there is no node");
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let built = Command::new(cargo)
        .args(["build", "--profile", "wasm", "-p", "enterprise-wasm", "--target", "wasm32-wasip1", "--quiet"])
        .current_dir(sim())
        .status()
        .expect("cargo runs");
    if !built.success() {
        return skip("the module did not build (is the wasm32-wasip1 target installed?)");
    }
    Some(sim().join("target/wasm32-wasip1/wasm/enterprise_wasm.wasm"))
}

fn leb(bytes: &[u8], at: &mut usize) -> usize {
    let (mut value, mut shift) = (0usize, 0);
    loop {
        let b = bytes[*at];
        *at += 1;
        value |= ((b & 0x7f) as usize) << shift;
        if b & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn name(bytes: &[u8], at: &mut usize) -> String {
    let n = leb(bytes, at);
    let s = String::from_utf8(bytes[*at..*at + n].to_vec()).unwrap();
    *at += n;
    s
}

/// The module's import section as (module, name) pairs, and its export section as names, read from the binary.
fn imports_and_exports(wasm: &[u8]) -> (Vec<(String, String)>, Vec<String>) {
    assert_eq!(&wasm[..8], b"\0asm\x01\0\0\0");
    let (mut imports, mut exports) = (Vec::new(), Vec::new());
    let mut at = 8;
    while at < wasm.len() {
        let id = wasm[at];
        at += 1;
        let size = leb(wasm, &mut at);
        let end = at + size;
        match id {
            2 => {
                for _ in 0..leb(wasm, &mut at) {
                    let module = name(wasm, &mut at);
                    let field = name(wasm, &mut at);
                    // The description: a kind byte, then a type index, limits, or a type.
                    let kind = wasm[at];
                    at += 1;
                    match kind {
                        0 => {
                            leb(wasm, &mut at);
                        }
                        1 => {
                            at += 1;
                            let flags = leb(wasm, &mut at);
                            leb(wasm, &mut at);
                            if flags & 1 != 0 {
                                leb(wasm, &mut at);
                            }
                        }
                        2 => {
                            let flags = leb(wasm, &mut at);
                            leb(wasm, &mut at);
                            if flags & 1 != 0 {
                                leb(wasm, &mut at);
                            }
                        }
                        3 => at += 2,
                        k => panic!("an import of kind {k}"),
                    }
                    imports.push((module, field));
                }
            }
            7 => {
                for _ in 0..leb(wasm, &mut at) {
                    exports.push(name(wasm, &mut at));
                    at += 1;
                    leb(wasm, &mut at);
                }
            }
            _ => {}
        }
        at = end;
    }
    (imports, exports)
}

#[test]
fn the_module_imports_only_the_allowed_set_and_exports_only_the_four() {
    let Some(path) = module() else { return };
    let wasm = std::fs::read(&path).unwrap();
    let (imports, mut exports) = imports_and_exports(&wasm);
    for (m, f) in &imports {
        assert!(ALLOWED_IMPORTS.contains(&(m.as_str(), f.as_str())), "the module imports {m}.{f}: not allowed");
    }
    exports.sort();
    let mut want: Vec<&str> = EXPORTS.to_vec();
    want.sort();
    assert_eq!(exports, want, "the module's exports");
    eprintln!("module: {} bytes; imports {imports:?}", wasm.len());
}

#[test]
fn the_proofs_requests_in_webassembly_give_the_native_answers() {
    let Some(path) = module() else { return };
    let requests = proof::requests();
    let dir = std::env::temp_dir().join(format!("enterprise-wasm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let frames = |of: &[Vec<u8>]| -> Vec<u8> { of.iter().flat_map(|f| (f.len() as u32).to_le_bytes().into_iter().chain(f.iter().copied())).collect() };
    let (input, output) = (dir.join("in.bin"), dir.join("out.bin"));
    std::fs::write(&input, frames(&requests)).unwrap();
    let ran = Command::new("node")
        .arg(sim().join("wasi-run.mjs"))
        .arg("--reactor")
        .arg(&path)
        .arg(&input)
        .arg(&output)
        .output()
        .expect("node runs");
    assert!(ran.status.success(), "node: {}", String::from_utf8_lossy(&ran.stderr));
    let mut society = None;
    let native: Vec<Vec<u8>> = requests.iter().map(|r| handle_bytes(&mut society, r)).collect();
    assert_eq!(std::fs::read(&output).unwrap(), frames(&native), "the responses, byte for byte");
    std::fs::remove_dir_all(&dir).ok();
}
