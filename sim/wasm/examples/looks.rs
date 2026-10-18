//! A look script played natively through the module's own framing (`alloc`, `call`, `free`: what `ent_call` runs), with
//! each request timed: the native column of the binding's spike (`just binding-spike`, docs/binding-plan.md section 4).
//!
//!     cargo run --release -p enterprise-wasm --example looks -- SCRIPT RESPONSES TIMES
//!
//! SCRIPT is requests in the module's framing (a u32 length, little-endian, then the request), as the study host's
//! `--wire-looks` writes them; RESPONSES gets the responses in the same framing (the JVM and Node must give the same
//! bytes); TIMES a line of nanoseconds for each request.

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [script, responses, times] = &args[..] else {
        eprintln!("looks SCRIPT RESPONSES TIMES");
        std::process::exit(2);
    };
    let input = std::fs::read(script).unwrap_or_else(|e| panic!("{script}: {e}"));
    let mut out = Vec::new();
    let mut lines = String::new();
    let mut at = 0;
    while at < input.len() {
        let len = u32::from_le_bytes(input[at..at + 4].try_into().unwrap()) as usize;
        at += 4;
        let started = Instant::now();
        let ptr = enterprise_wasm::alloc(len);
        // SAFETY: `ptr` is a fresh buffer of `len` bytes from `alloc`; the response is the module's own buffer, read and
        // freed once, and this process calls `call` from one thread.
        let response = unsafe {
            std::ptr::copy_nonoverlapping(input[at..].as_ptr(), ptr, len);
            let (rp, rn) = enterprise_wasm::call(ptr, len);
            let bytes = std::slice::from_raw_parts(rp, rn).to_vec();
            enterprise_wasm::free(rp, rn);
            enterprise_wasm::free(ptr, len);
            bytes
        };
        let took = started.elapsed().as_nanos();
        at += len;
        out.extend((response.len() as u32).to_le_bytes());
        out.extend(response);
        lines.push_str(&format!("{took}\n"));
    }
    std::fs::write(responses, out).unwrap_or_else(|e| panic!("{responses}: {e}"));
    std::fs::write(times, lines).unwrap_or_else(|e| panic!("{times}: {e}"));
}
