//! RV64 B extension

pub mod zba_64_ext;
pub mod zbb_64_ext;
pub mod zbc_64_ext;
pub mod zbs_64_ext;

use crate::b_64_ext::zba_64_ext::execute_zba_64_ext;
use crate::b_64_ext::zbb_64_ext::execute_zbb_64_ext;
use crate::b_64_ext::zbc_64_ext::execute_zbc_64_ext;
use crate::b_64_ext::zbs_64_ext::execute_zbs_64_ext;
use ab_riscv_primitives::instruction::b_64_ext::BZbc64ExtInstruction;
use ab_riscv_primitives::registers::{GenericRegister64, GenericRegisters64};

/// Execute instructions from B (Zba + Zbb + Zbs) + Zbc extensions
#[inline(always)]
pub fn execute_b_zbc_64_ext<Reg, Registers>(
    regs: &mut Registers,
    instruction: BZbc64ExtInstruction<Reg>,
) where
    Reg: GenericRegister64,
    Registers: GenericRegisters64<Reg>,
{
    match instruction {
        BZbc64ExtInstruction::Zba(instruction) => execute_zba_64_ext(regs, instruction),
        BZbc64ExtInstruction::Zbb(instruction) => execute_zbb_64_ext(regs, instruction),
        BZbc64ExtInstruction::Zbc(instruction) => execute_zbc_64_ext(regs, instruction),
        BZbc64ExtInstruction::Zbs(instruction) => execute_zbs_64_ext(regs, instruction),
    }
}
