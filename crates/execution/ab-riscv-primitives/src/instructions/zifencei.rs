//! Zifencei extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V Zifencei instruction
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum ZifenceiInstruction<Reg> {
    /// Instruction-fetch fence
    FenceI,
}

#[instruction]
const impl<Reg> Instruction for ZifenceiInstruction<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;

        // MISC-MEM major opcode, funct3=001
        if opcode != 0b000_1111 || funct3 != 0b001 {
            None?;
        }

        // `rd`, `rs1` and the immediate are all reserved and must be ignored by implementations
        // rather than checked, so any encoding with funct3=001 in this opcode is a valid `fence.i`
        Some(Self::FenceI)
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
impl<Reg> fmt::Display for ZifenceiInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FenceI => write!(f, "fence.i"),
        }
    }
}
