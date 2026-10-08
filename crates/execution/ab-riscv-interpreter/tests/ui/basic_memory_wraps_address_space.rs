//! `BasicMemory` whose region wraps around the end of the address space must not compile. Its
//! bounds checks rely on an address below the base address wrapping to an offset past the end of
//! the region.

use ab_riscv_interpreter::basic::BasicMemory;
use ab_riscv_interpreter::prelude::*;

fn main() {
    // Region `u64::MAX - 7..u64::MAX + 9`, the last 8 bytes would wrap to addresses `0..8`
    let memory = BasicMemory::<{ u64::MAX - 7 }, 16>::default();
    let _ = memory.read::<u32>(0);
}
