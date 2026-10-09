//! Vector register file, which every environment executing vector instructions stores, can only be
//! created for harts with `ELEN` supported by Zve* extensions (32 or 64 bits). So instructions of
//! individual ZveXx enums can't be executed for other harts either, even when they are constructed
//! from fields rather than decoded.

use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;

// Below the minimum of Zve32x
type HartElen16 = BasicVectorHart<Reg<u64>, { Elen::L16 }, { Vlen::L128 }>;
// Above the maximum of Zve64x
type HartElen128 = BasicVectorHart<Reg<u64>, { Elen::L128 }, { Vlen::L128 }>;

fn main() {
    let _vregs = VectorRegisterFile::<HartElen16>::default();
    let _vregs = VectorRegisterFile::<HartElen128>::default();
}
