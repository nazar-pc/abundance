//! Zve* extensions only support `ELEN` of 32 or 64 bits, so instruction sets with ZveXx
//! instructions for a hart with any other `ELEN` must not compile

use ab_riscv_primitives::prelude::*;

// `ELEN = 128` is above the maximum of Zve64x
type HartElen128 = BasicVectorHart<Reg<u64>, { Elen::L128 }, { Vlen::L128 }>;
// `ELEN = 16` is below the minimum of Zve32x
type HartElen16 = BasicVectorHart<Reg<u64>, { Elen::L16 }, { Vlen::L128 }>;

fn main() {
    let _ = <ZveXxInstruction<HartElen128> as Instruction>::ISA_STRING;
    let _ = <ZveXxInstruction<HartElen16> as Instruction>::ISA_STRING;
}
