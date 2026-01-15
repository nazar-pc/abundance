//! RV64 Zbc extension

#[cfg(test)]
mod tests;

use crate::instruction::Instruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::registers::Register;
use core::fmt;

/// RISC-V RV64 Zbc instruction (Carryless multiplication)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZbcInstruction<Reg> {
    Clmul { rd: Reg, rs1: Reg, rs2: Reg },
    Clmulh { rd: Reg, rs1: Reg, rs2: Reg },
    Clmulr { rd: Reg, rs1: Reg, rs2: Reg },
}

impl<Reg> const Instruction for Rv64ZbcInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Base = Rv64Instruction<Reg>;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        match opcode {
            // R-type
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b001, 0b0000101) => Some(Self::Clmul { rd, rs1, rs2 }),
                    (0b011, 0b0000101) => Some(Self::Clmulh { rd, rs1, rs2 }),
                    (0b010, 0b0000101) => Some(Self::Clmulr { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            _ => None,
        }
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

impl<Reg> fmt::Display for Rv64ZbcInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clmul { rd, rs1, rs2 } => write!(f, "clmul {}, {}, {}", rd, rs1, rs2),
            Self::Clmulh { rd, rs1, rs2 } => write!(f, "clmulh {}, {}, {}", rd, rs1, rs2),
            Self::Clmulr { rd, rs1, rs2 } => write!(f, "clmulr {}, {}, {}", rd, rs1, rs2),
        }
    }
}
