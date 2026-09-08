//! Support code linked into every guest program.
//!
//! A `no_std` binary has to come with a panic handler, and there is nothing to gain from writing
//! the same one in each of them.

#![no_std]

use core::arch::asm;
use core::panic::PanicInfo;

/// The guest programs are written such that they can't panic, and there is nowhere to report a
/// panic to anyway. Trapping with an illegal instruction at least makes the interpreter stop with
/// an error and the address of this handler instead of spinning forever.
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // SAFETY: `unimp` has no operands and never returns
    unsafe {
        asm!("unimp", options(noreturn, nostack));
    }
}
