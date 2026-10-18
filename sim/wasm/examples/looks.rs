//! A look script played natively through the module's own framing (`alloc`, `call`, `free`: what `ent_call` runs), with
//! each request timed: the native column of the binding's spike (`just binding-spike`, docs/binding-plan.md section 4).
//!
//!     cargo run --release -p enterprise-wasm --example looks -- SCRIPT RESPONSES TIMES
//!
//! SCRIPT is requests in the module's framing (a u32 length, little-endian, then the request), as the study host's
//! `--wire-looks` writes them; RESPONSES gets the responses in the same framing (the JVM and Node must give the same
//! bytes); TIMES a line for each request: its nanoseconds, wall and the thread's CPU (the machine is shared, and a
//! busy one stretches the wall time far more than the CPU time).

use std::time::Instant;

/// The calling thread's CPU time, in nanoseconds (libSystem's or libc's `clock_gettime`, linked anyway: no dependency).
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn thread_cpu() -> u128 {
    #[repr(C)]
    struct Timespec {
        sec: i64,
        nsec: std::os::raw::c_long,
    }
    extern "C" {
        fn clock_gettime(clock: std::os::raw::c_int, tp: *mut Timespec) -> std::os::raw::c_int;
    }
    const THREAD_CPU: std::os::raw::c_int = if cfg!(target_os = "macos") { 16 } else { 3 };
    let mut t = Timespec { sec: 0, nsec: 0 };
    // SAFETY: `t` is a valid timespec for the call to fill.
    unsafe { clock_gettime(THREAD_CPU, &mut t) };
    t.sec as u128 * 1_000_000_000 + t.nsec as u128
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn thread_cpu() -> u128 {
    0
}

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
        let cpu = thread_cpu();
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
        let cpu = thread_cpu() - cpu;
        at += len;
        out.extend((response.len() as u32).to_le_bytes());
        out.extend(response);
        lines.push_str(&format!("{took} {cpu}\n"));
    }
    std::fs::write(responses, out).unwrap_or_else(|e| panic!("{responses}: {e}"));
    std::fs::write(times, lines).unwrap_or_else(|e| panic!("{times}: {e}"));
}
