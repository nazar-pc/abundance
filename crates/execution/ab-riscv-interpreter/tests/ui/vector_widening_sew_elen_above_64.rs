//! Widening with `SEW = 64` would be legal with `ELEN = 128`, but the interpreter processes
//! elements as `u64`, so `WideningSew` must not be created for a hart with `ELEN` above 64

use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;

type Hart = BasicVectorHart<Reg<u64>, { Elen::L128 }, { Vlen::L128 }>;

fn main() {
    let _sew = zvexx_helpers::WideningSew::<Hart>::new(Vsew::E8);
}
