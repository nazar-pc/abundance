//! Computes an FNV-1a checksum of a buffer provided by the host, accumulates it atomically and
//! reports the result with a syscall.
//!
//! Compiled for RV64IMAC, see `../../README.md`.

#![no_main]
#![no_std]

// Linked in for its panic handler
use ab_riscv_interpreter_guests as _;
use core::arch::asm;
use core::slice;
use core::sync::atomic::{AtomicU64, Ordering};

/// FNV-1a offset basis
const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a prime
const PRIME: u64 = 0x0000_0100_0000_01b3;
/// The syscall that asks the host to print the `u64` in `a0`.
///
/// Nothing about `ecall` says where its operands are, this is just the convention the host of this
/// program implements.
const HOST_CALL_REPORT: u64 = 1;

/// Accumulator in guest memory, updated with an `amoadd.d` of the A extension
static ACCUMULATOR: AtomicU64 = AtomicU64::new(0);

/// FNV-1a hash of `data`
fn checksum(data: &[u8]) -> u64 {
    let mut hash = OFFSET_BASIS;

    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }

    hash
}

/// Ask the host to print `value`
fn report(value: u64) {
    // SAFETY: `ecall` reads the registers listed below and the host's handler writes nothing back
    unsafe {
        asm!(
            "ecall",
            in("a7") HOST_CALL_REPORT,
            in("a0") value,
            options(nostack),
        );
    }
}

/// Checksum `len` bytes at `data`, add the result to [`ACCUMULATOR`], report the new total to the
/// host and return it.
///
/// # Safety
/// `data` must point to at least `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start(data: *const u8, len: usize) -> u64 {
    // SAFETY: Guaranteed by function contract
    let data = unsafe { slice::from_raw_parts(data, len) };

    let checksum = checksum(data);
    let total = ACCUMULATOR
        .fetch_add(checksum, Ordering::Relaxed)
        .wrapping_add(checksum);

    report(total);

    total
}
