//! The count of the bytes that the runtime allocates on this thread.
//!
//! The default allocator of vendor/tree-sitter/src/alloc.c counts the net bytes that each thread
//! holds, and the peak of that count since the last reset. A parse resets the count before it
//! starts, and its progress callback reads the peak. A parse whose peak passes the ceiling of
//! `corpus::CEILING` then stops, and the file gets an error in the place of the machine.

use std::ffi::{CString, c_char};
use std::ptr;

unsafe extern "C" {
    fn ts_allocation_reset();
    fn ts_allocation_peak() -> u64;
    fn ts_allocation_label(label: *const c_char);
}

/// Set the count of this thread to zero.
pub fn reset() {
    // SAFETY: the function reads and writes the counters of the calling thread only.
    unsafe { ts_allocation_reset() }
}

/// The highest net number of bytes that this thread held since the last reset.
pub fn peak() -> u64 {
    // SAFETY: the function reads the counters of the calling thread only.
    unsafe { ts_allocation_peak() }
}

/// The name of the input that this thread parses, for the message of the cap of the runtime.
///
/// The runtime keeps a pointer to the text while the label lives. The drop of the label removes
/// the pointer before the text goes.
pub struct Label {
    /// The text that the runtime points to. Only its lifetime matters.
    _text: CString,
}

impl Label {
    /// Give the runtime the name of the input of this thread.
    pub fn new(text: &str) -> Self {
        let text = CString::new(text.replace('\0', "")).expect("the text holds no NUL byte");
        // SAFETY: the pointer stays valid while `self` holds the text, and `drop` removes it.
        unsafe { ts_allocation_label(text.as_ptr()) };
        Self { _text: text }
    }
}

impl Drop for Label {
    fn drop(&mut self) {
        // SAFETY: a null pointer removes the label, and the runtime reads no text after it.
        unsafe { ts_allocation_label(ptr::null()) }
    }
}
