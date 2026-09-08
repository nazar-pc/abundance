//! Copies a greeting into a buffer provided by the host.
//!
//! Compiled for RV32I, see `../../README.md`.

#![no_main]
#![no_std]

// Linked in for its panic handler
use ab_riscv_interpreter_guests as _;
use core::ptr;

/// What the host expects to find in the output buffer afterward
const GREETING: &str = "Hello from a RISC-V guest!";

/// Copy [`GREETING`] into the buffer at `out` and return its length in bytes.
///
/// # Safety
/// `out` must point to a writable buffer of at least `GREETING.len()` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start(out: *mut u8) -> usize {
    let greeting = GREETING.as_bytes();

    // SAFETY: Guaranteed by function contract
    unsafe {
        ptr::copy_nonoverlapping(greeting.as_ptr(), out, greeting.len());
    }

    greeting.len()
}
