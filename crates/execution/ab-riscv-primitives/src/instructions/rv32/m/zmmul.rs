//! RV32 Zmmul extension (multiplication subset of M extension)

use crate::instructions::Instruction;
use crate::instructions::rv32::m::Rv32MInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zmmul instruction
#[instruction(
    reorder = [Mul, Mulh, Mulhsu, Mulhu],
    ignore = [Rv32MInstruction],
    inherit = [Rv32MInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZmmulInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv32ZmmulInstruction<Reg>
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
        size_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv32ZmmulInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
