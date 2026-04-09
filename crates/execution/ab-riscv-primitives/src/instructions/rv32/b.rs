//! RV32 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::instructions::Instruction;
use crate::instructions::rv32::b::zba::Rv32ZbaInstruction;
use crate::instructions::rv32::b::zbb::Rv32ZbbInstruction;
use crate::instructions::rv32::b::zbs::Rv32ZbsInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 B (Zba + Zbb + Zbs) instruction
#[instruction(
    inherit = [Rv32ZbaInstruction, Rv32ZbbInstruction, Rv32ZbsInstruction]
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32BInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv32BInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv32BInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
