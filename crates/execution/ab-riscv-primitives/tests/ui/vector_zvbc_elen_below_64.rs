//! Zvbc instructions require `SEW = 64`, so instruction sets with them for a hart with `ELEN < 64`
//! must not compile rather than contain instructions that can never execute legally

use ab_riscv_primitives::prelude::*;

// `ELEN = 32` doesn't support 64-bit elements
type Hart = BasicVectorHart<Reg<u64>, { Elen::L32 }, { Vlen::L128 }>;

fn main() {
    let _ = <ZvbcInstruction<Hart> as Instruction>::ISA_STRING;
}
