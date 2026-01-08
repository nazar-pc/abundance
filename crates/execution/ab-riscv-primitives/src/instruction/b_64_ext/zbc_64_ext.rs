//! RV64 Zbc extension

#[cfg(test)]
mod tests;

use crate::instruction::GenericInstruction;
use crate::registers::GenericRegister;
use core::fmt;

/// RISC-V Zbc instruction (Carryless multiplication)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zbc64ExtInstruction<Reg> {
    Clmul { rd: Reg, rs1: Reg, rs2: Reg },
    Clmulh { rd: Reg, rs1: Reg, rs2: Reg },
    Clmulr { rd: Reg, rs1: Reg, rs2: Reg },
}

impl<Reg> const GenericInstruction for Zbc64ExtInstruction<Reg>
where
    Reg: [const] GenericRegister<Type = u64>,
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

        Some(match opcode {
            // R-type
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b001, 0b0000101) => Self::Clmul { rd, rs1, rs2 },
                    (0b011, 0b0000101) => Self::Clmulh { rd, rs1, rs2 },
                    (0b010, 0b0000101) => Self::Clmulr { rd, rs1, rs2 },
                    _ => {
                        return None;
                    }
                }
            }
            _ => {
                return None;
            }
        })
    }

    #[inline(always)]
    fn size(&self) -> usize {
        size_of::<u32>()
    }
}

impl<Reg> fmt::Display for Zbc64ExtInstruction<Reg>
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
