//! `ZcmpUrlist` is tied to the register type it was validated for by `ZcmpUrlist::try_from_raw()`.
//! `reg_list()` relies on this with `unwrap_unchecked()`, so a list valid for RVI (like
//! `{ra, s0-s11}`) must never reach an RVE hart.

use ab_riscv_primitives::prelude::*;

fn push(urlist: ZcmpUrlist<Reg<u32>>, stack_adj: u8) -> Rv32ZcmpInstruction<BasicHart<EReg<u32>>> {
    Rv32ZcmpInstruction::CmPush {
        // `#[instruction]` adds these to all instructions
        rs1: EReg::ZERO,
        rs2: EReg::ZERO,
        urlist,
        stack_adj,
    }
}

fn main() {}
