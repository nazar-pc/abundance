//! `I24WithZeroedBits` only supports `LOW_ZEROED_BITS` in `8..32` range. With fewer zeroed bits the
//! remaining bits of a 32-bit number don't fit into 24 bits and would be truncated silently, with
//! 32 or more the shifts in conversions overflow.

use ab_riscv_primitives::prelude::*;

fn main() {
    let _ = I24WithZeroedBits::<7>::default();
    let _ = I24WithZeroedBits::<32>::default();
}
