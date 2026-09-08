//! Computes the dot product of two arrays of `u64` provided by the host.
//!
//! Like `vector-sum.rs` it is a plain loop that LLVM vectorizes.
//!
//! Compiled for RV64IM with Zve64x and Zvl128b, see `../../README.md`.

#![no_main]
#![no_std]

// Linked in for its panic handler
use ab_riscv_interpreter_guests as _;
use core::slice;

/// Dot product of `len` `u64` values at `a` and at `b`.
///
/// # Safety
/// `a` and `b` must each point to at least `len` readable and correctly aligned `u64` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start(a: *const u64, b: *const u64, len: usize) -> u64 {
    // SAFETY: Guaranteed by function contract
    let (a, b) = unsafe {
        (
            slice::from_raw_parts(a, len),
            slice::from_raw_parts(b, len),
        )
    };

    let mut sum = 0u64;
    for (&a, &b) in a.iter().zip(b) {
        sum = sum.wrapping_add(a.wrapping_mul(b));
    }

    sum
}
