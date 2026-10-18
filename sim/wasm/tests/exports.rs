//! The exports' framing, natively: a buffer allocated, filled, called and freed as the Java side does, and the answers
//! the same bytes `handle_bytes` gives over a society of its own.
//!
//! The crate's society is one static for the process: only one test here may call [`call`] (tests run on parallel
//! threads, and two would share and race on it).

mod proof;

use enterprise_core::wire::{handle_bytes, Answer, Refusal, Response, ABI};
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
    // What the conversation says: refused before any society; the loaded world, not the one made before it, answers
    // after the load (Bly holds the 32 wheat delivered after it); the last three refused for their ABI, their schema
    // and their bytes.
    let mut society = None;
    let said: Vec<Response> =
        proof::requests().iter().map(|r| postcard::from_bytes(&handle_bytes(&mut society, r)).unwrap()).collect();
    assert_eq!(said[0], Response::Refused { why: Refusal::NoSociety });
    assert!(matches!(said[9], Response::Done { .. }), "the load: {:?}", said[9]);
    let Response::Done { answers, .. } = &said[11] else { panic!("after the load: {:?}", said[11]) };
    assert_eq!(answers[1], Answer::Stock { town: 1, stock: vec![32.0, 0.0] });
    assert!(matches!(said[12], Response::Saved { .. }));
    let n = said.len();
    assert!(matches!(said[n - 3], Response::Refused { why: Refusal::Abi { .. } }), "{:?}", said[n - 3]);
    assert!(matches!(said[n - 2], Response::Refused { why: Refusal::Schema { .. } }), "{:?}", said[n - 2]);
    assert!(matches!(said[n - 1], Response::Refused { why: Refusal::Unreadable { .. } }), "{:?}", said[n - 1]);
}

#[test]
fn an_empty_buffer_is_made_and_freed() {
    unsafe {
        let ptr = alloc(0);
        free(ptr, 0);
    }
}
