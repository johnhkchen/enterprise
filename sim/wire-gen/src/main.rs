//! `wire-gen`: writes the wire's registry and Java from the core's wire types (`just wire-gen`), or with `--check`
//! says what differs from the committed files and fails (`just wire-check`).
//!
//! `--renumber` lets an enum's variants move (old logs and fixtures of it stop reading); `--root DIR` works on
//! another checkout than the one this was built in.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut check = false;
    let mut renumber = false;
    let mut root = enterprise_wire_gen::repo_root();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => check = true,
            "--renumber" => renumber = true,
            "--root" => match args.next() {
                Some(dir) => root = PathBuf::from(dir),
                None => return usage("--root needs a folder"),
            },
            other => return usage(&format!("what's {other:?}?")),
        }
    }
    let generated = match enterprise_wire_gen::generate(&root, renumber) {
        Ok(g) => g,
        Err(why) => {
            eprintln!("wire-gen: {why}");
            return ExitCode::FAILURE;
        }
    };
    for w in &generated.warnings {
        eprintln!("wire-gen: {w}");
    }
    if check {
        let wrong = enterprise_wire_gen::differences(&root, &generated);
        if wrong.is_empty() {
            println!("wire-check: the registry and the Java are the wire's (schema {:016x}, {} files)",
                generated.schema, generated.files.len());
            return ExitCode::SUCCESS;
        }
        eprintln!("wire-check: the committed wire isn't what sim/core/src/wire generates; run `just wire-gen`:");
        for w in wrong {
            eprintln!("  {w}");
        }
        return ExitCode::FAILURE;
    }
    match enterprise_wire_gen::write(&root, &generated) {
        Ok(changed) => {
            for path in &changed {
                println!("wrote {path}");
            }
            println!("wire-gen: schema {:016x}, {} files, {} changed", generated.schema, generated.files.len(),
                changed.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("wire-gen: writing: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage(why: &str) -> ExitCode {
    eprintln!("wire-gen: {why}\nusage: wire-gen [--check] [--renumber] [--root DIR]");
    ExitCode::from(2)
}
