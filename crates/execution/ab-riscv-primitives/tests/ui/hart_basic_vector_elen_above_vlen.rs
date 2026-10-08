//! `BasicVectorHart` with `ELEN > VLEN` must not compile once its vector lengths are used. Vector
//! code relies on `ELEN <= VLEN` to guarantee that at least one element fits into a vector
//! register.

use ab_riscv_primitives::prelude::*;

// `ELEN = 128` is wider than `VLEN = 64`
type Hart = BasicVectorHart<Reg<u64>, { Elen::L128 }, { Vlen::L64 }>;

fn main() {
    let _ = Hart::VECTOR_LENGTHS;
}
