//! RV64 Zmmul extension (multiplication subset of M extension)

use crate::instructions::Instruction;
use crate::instructions::rv64::m::Rv64MInstruction;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zmmul instruction
#[instruction(
    reorder = [Mul, Mulh, Mulhsu, Mulhu, Mulw],
    ignore = [Rv64MInstruction],
    inherit = [Rv64MInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZmmulInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv64ZmmulInstruction<Reg>
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
impl<Reg> fmt::Display for Rv64ZmmulInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
