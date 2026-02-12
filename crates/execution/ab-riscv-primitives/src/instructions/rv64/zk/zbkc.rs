//! Zbkc extension (subset of Zbc extension)

use crate::instructions::Instruction;
use crate::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zbkc instruction
#[instruction(
    reorder = [Clmul, Clmulh],
    ignore = [Rv64ZbcInstruction],
    inherit = [Rv64ZbcInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZbkcInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv64ZbkcInstruction<Reg>
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
impl<Reg> fmt::Display for Rv64ZbkcInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
