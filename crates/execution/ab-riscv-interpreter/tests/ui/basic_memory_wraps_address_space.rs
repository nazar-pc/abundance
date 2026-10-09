//! `BasicMemory` whose region wraps around the end of the address space must not be constructible.
//! Its bounds checks rely on an address below the base address wrapping to an offset past the end
//! of the region.

use ab_riscv_interpreter::basic::BasicMemory;

fn main() {
    // Region `u64::MAX - 7..u64::MAX + 9`, the last 8 bytes would wrap to addresses `0..8`
    let _memory = BasicMemory::<{ u64::MAX - 7 }, 16>::default();
}
