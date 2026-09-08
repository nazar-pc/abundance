//! Sums an array of `u64` provided by the host.
//!
//! Written as a plain loop on purpose: compiled for a target with a vector extension, LLVM turns
//! this into a strip-mined vector loop, so the guest stays ordinary safe Rust.
//!
//! Compiled for RV64IM with Zve64x and Zvl128b, see `../../README.md`.

#![no_main]
#![no_std]

// Linked in for its panic handler
use ab_riscv_interpreter_guests as _;
use core::slice;

/// Sum `len` `u64` values at `data`.
///
/// # Safety
/// `data` must point to at least `len` readable and correctly aligned `u64` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start(data: *const u64, len: usize) -> u64 {
    // SAFETY: Guaranteed by function contract
    let data = unsafe { slice::from_raw_parts(data, len) };

    let mut sum = 0u64;
    for &value in data {
        sum = sum.wrapping_add(value);
    }

    sum
}
