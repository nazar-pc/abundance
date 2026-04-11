//! RV64 Zbkx extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zbkx instruction (Crossbar permutations)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZbkxInstruction<Reg> {
    Xperm4 { rd: Reg, rs1: Reg, rs2: Reg },
    Xperm8 { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZbkxInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        match opcode {
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b010, 0b0010100) => Some(Self::Xperm4 { rd, rs1, rs2 }),
                    (0b100, 0b0010100) => Some(Self::Xperm8 { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            _ => None,
        }
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

impl<Reg> fmt::Display for Rv64ZbkxInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xperm4 { rd, rs1, rs2 } => write!(f, "xperm4 {}, {}, {}", rd, rs1, rs2),
            Self::Xperm8 { rd, rs1, rs2 } => write!(f, "xperm8 {}, {}, {}", rd, rs1, rs2),
        }
    }
}
