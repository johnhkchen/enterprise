//! The core as a WebAssembly module (`docs/binding-plan.md` 1.1): four exports, never more.
//!
//! | Export | Does |
//! |---|---|
//! | `ent_abi() -> u32` | The ABI's number ([`enterprise_core::wire::ABI`]) |
//! | `ent_alloc(len) -> ptr` | A buffer in the module's memory for a request |
//! | `ent_call(ptr, len) -> u64` | A `Request` (postcard) in; a `Response` out at `ptr << 32 \| len` |
//! | `ent_free(ptr, len)` | Frees a buffer either side made |
//!
//! One instance holds one society (one world), and every call is on one thread. The exports are thin shells over
//! [`alloc`], [`call`] and [`free`], which work in native pointers so a native test can run the same framing; on
//! `wasm32` a pointer is a `u32`. The module imports only what Rust's std wants for an abort (`fd_write`,
//! `proc_exit`, `environ_*`): a test lists them.

use std::cell::UnsafeCell;

use enterprise_core::wire::{handle_bytes, ABI};
use enterprise_core::Society;

/// The society this instance holds.
struct Held(UnsafeCell<Option<Society>>);

// One thread calls into a module instance (the binding's rule); the cell is never shared across threads.
unsafe impl Sync for Held {}

static SOCIETY: Held = Held(UnsafeCell::new(None));

/// A buffer of `len` bytes (zeroed) for a caller to fill; free it with [`free`] (or hand it to [`call`]).
pub fn alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// Frees a buffer from [`alloc`] or [`call`], of the length it was made with.
///
/// # Safety
/// `ptr` and `len` are exactly a buffer this module made and hasn't freed.
pub unsafe fn free(ptr: *mut u8, len: usize) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
}

/// Runs the request at `ptr`, `len` (postcard) over the society this instance holds. The response is a new buffer:
/// its pointer and length. The request's buffer stays the caller's to [`free`].
///
/// # Safety
/// `ptr` and `len` are a live buffer.
pub unsafe fn call(ptr: *const u8, len: usize) -> (*mut u8, usize) {
    let request = std::slice::from_raw_parts(ptr, len);
    let response = handle_bytes(&mut *SOCIETY.0.get(), request).into_boxed_slice();
    let len = response.len();
    (Box::into_raw(response) as *mut u8, len)
}

#[no_mangle]
pub extern "C" fn ent_abi() -> u32 {
    ABI
}

#[no_mangle]
pub extern "C" fn ent_alloc(len: u32) -> u32 {
    alloc(len as usize) as usize as u32
}

#[no_mangle]
pub unsafe extern "C" fn ent_call(ptr: u32, len: u32) -> u64 {
    let (out, n) = call(ptr as usize as *const u8, len as usize);
    ((out as usize as u64) << 32) | n as u64
}

#[no_mangle]
pub unsafe extern "C" fn ent_free(ptr: u32, len: u32) {
    free(ptr as usize as *mut u8, len as usize)
}
