//! RV64 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::instructions::Instruction;
use crate::instructions::rv64::b::zba::Rv64ZbaInstruction;
use crate::instructions::rv64::b::zbb::Rv64ZbbInstruction;
use crate::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 B (Zba + Zbb + Zbs) instruction
#[instruction(
    inherit = [Rv64ZbaInstruction, Rv64ZbbInstruction, Rv64ZbsInstruction]
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64BInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv64BInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn alignment() -> u8 {
        size_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv64BInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
