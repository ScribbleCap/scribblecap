//! ScribbleCap core for the editor UI, compiled to `wasm32-unknown-unknown`
//! (decision 0011).
//!
//! The boundary is deliberately tiny: the UI copies a UTF-8 JSON request into memory
//! from [`sc_alloc`], calls [`sc_call`], reads the JSON response it points to, and
//! releases both buffers with [`sc_free`]. All editing logic lives in the safe
//! [`editor`] module; this file only moves bytes.

pub mod editor;

use std::cell::RefCell;

use editor::Editor;

thread_local! {
    // WebAssembly here is single-threaded: one editor session per module instance.
    static EDITOR: RefCell<Editor> = RefCell::new(Editor::default());
}

/// Allocate `len` bytes for the caller to write a request into.
#[allow(unsafe_code)] // `no_mangle` exports the symbol; the body is safe.
#[unsafe(no_mangle)]
pub extern "C" fn sc_alloc(len: usize) -> *mut u8 {
    leak(vec![0; len].into_boxed_slice())
}

/// Hand a buffer to the caller. Every buffer crossing the boundary is a boxed
/// slice, whose allocation is exactly its length, so [`sc_free`] can rebuild it.
fn leak(buffer: Box<[u8]>) -> *mut u8 {
    Box::into_raw(buffer).cast::<u8>()
}

/// Release a buffer from [`sc_alloc`] or a response from [`sc_call`].
///
/// # Safety
/// `ptr` and `len` must describe a buffer this module returned and that has not been
/// freed yet.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_free(ptr: *mut u8, len: usize) {
    // SAFETY: per the contract, `ptr`/`len` came from `leak` (a `Box<[u8]>` of
    // exactly `len` bytes) and are freed once.
    unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len))) };
}

/// Handle the JSON request in `ptr[..len]` (which stays owned by the caller).
/// Returns the response location packed as `(ptr << 32) | len`; free it with
/// [`sc_free`].
///
/// # Safety
/// `ptr[..len]` must be initialised memory inside this module's linear memory.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_call(ptr: *const u8, len: usize) -> u64 {
    // SAFETY: the caller wrote `len` bytes at `ptr`, obtained from `sc_alloc(len)`.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let response = match std::str::from_utf8(bytes) {
        Ok(request) => EDITOR.with(|e| e.borrow_mut().handle(request)),
        Err(_) => r#"{"error":"request is not UTF-8"}"#.to_owned(),
    };
    let response = response.into_bytes().into_boxed_slice();
    let out_len = response.len();
    ((leak(response) as usize as u64) << 32) | out_len as u64
}
