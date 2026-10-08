//! Base instruction sets are tied to the register width of the hart: RV32 instructions require
//! 32-bit registers and RV64 instructions require 64-bit registers. Mixing them would make
//! XLEN-dependent decoding (shift amounts, `*W` instructions, `LD`/`SD`) disagree with execution.

use ab_riscv_primitives::prelude::*;

fn instruction_set<I>()
where
    I: Instruction,
{
}

fn main() {
    // RV32 instructions with 64-bit registers
    instruction_set::<Rv32Instruction<BasicHart<Reg<u64>>>>();

    // RV64 instructions with 32-bit registers
    instruction_set::<Rv64Instruction<BasicHart<Reg<u32>>>>();
}
