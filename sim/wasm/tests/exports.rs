//! The exports' framing, natively: a buffer allocated, filled, called and freed as the Java side does, and the answers
//! the same bytes `handle_bytes` gives over a society of its own.

mod proof;

use enterprise_core::wire::{handle_bytes, Refusal, Response, ABI};
use enterprise_wasm::{alloc, call, ent_abi, free};

#[test]
fn a_call_through_the_framing_gives_the_bytes_handle_gives() {
    assert_eq!(ent_abi(), ABI);
    let mut society = None;
    for (at, request) in proof::requests().iter().enumerate() {
        let expected = handle_bytes(&mut society, request);
        unsafe {
            let ptr = alloc(request.len());
            std::slice::from_raw_parts_mut(ptr, request.len()).copy_from_slice(request);
            let (out, len) = call(ptr, request.len());
            let got = std::slice::from_raw_parts(out, len).to_vec();
            free(ptr, request.len());
            free(out, len);
            assert_eq!(got, expected, "request {at}");
        }
    }
    // The first asked before any society: refused; the last three are refused too.
    let first: Response = postcard::from_bytes(&handle_bytes(&mut None, &proof::requests()[0])).unwrap();
    assert_eq!(first, Response::Refused { why: Refusal::NoSociety });
}

#[test]
fn an_empty_buffer_is_made_and_freed() {
    unsafe {
        let ptr = alloc(0);
        free(ptr, 0);
    }
}
