//! RV32 Zbkc extension (subset of Zbc extension)

use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbc::Rv32ZbcInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zbkc instruction
#[instruction(
    reorder = [Clmul, Clmulh],
    ignore = [Rv32ZbcInstruction],
    inherit = [Rv32ZbcInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZbkcInstruction<Reg> {}

#[instruction]
const impl<Reg> Instruction for Rv32ZbkcInstruction<Reg>
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
impl<Reg> fmt::Display for Rv32ZbkcInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
