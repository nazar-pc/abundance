//! `VectorLengths` can only be created with `VectorLengths::new()`, which rejects `ELEN > VLEN`,
//! and its fields can't be modified afterwards. Vector code relies on `ELEN <= VLEN` to guarantee
//! that at least one element fits into a vector register.

use ab_riscv_primitives::prelude::*;

fn create(elen: Elen, vlen: Vlen) -> VectorLengths {
    VectorLengths { elen, vlen }
}

fn modify(lengths: &mut VectorLengths, elen: Elen, vlen: Vlen) {
    lengths.elen = elen;
    lengths.vlen = vlen;
}

fn main() {}
