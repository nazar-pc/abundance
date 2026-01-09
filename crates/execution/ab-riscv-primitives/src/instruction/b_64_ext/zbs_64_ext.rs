//! RV64 Zbs extension

#[cfg(test)]
mod tests;

use crate::instruction::GenericInstruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::registers::GenericRegister;
use core::fmt;

/// RISC-V Zbs instruction (Single-bit instructions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zbs64ExtInstruction<Reg> {
    // Single-Bit Set
    Bset { rd: Reg, rs1: Reg, rs2: Reg },
    Bseti { rd: Reg, rs1: Reg, shamt: u8 },

    // Single-Bit Clear
    Bclr { rd: Reg, rs1: Reg, rs2: Reg },
    Bclri { rd: Reg, rs1: Reg, shamt: u8 },

    // Single-Bit Invert
    Binv { rd: Reg, rs1: Reg, rs2: Reg },
    Binvi { rd: Reg, rs1: Reg, shamt: u8 },

    // Single-Bit Extract
    Bext { rd: Reg, rs1: Reg, rs2: Reg },
    Bexti { rd: Reg, rs1: Reg, shamt: u8 },
}

impl<Reg> const GenericInstruction for Zbs64ExtInstruction<Reg>
where
    Reg: [const] GenericRegister<Type = u64>,
{
    type Base = Rv64Instruction<Reg>;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let shamt = ((instruction >> 20) & 0x3f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        Some(match opcode {
            // R-type instructions
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b001, 0b0010100) => Self::Bset { rd, rs1, rs2 },
                    (0b001, 0b0100100) => Self::Bclr { rd, rs1, rs2 },
                    (0b001, 0b0110100) => Self::Binv { rd, rs1, rs2 },
                    (0b101, 0b0100100) => Self::Bext { rd, rs1, rs2 },
                    _ => {
                        return None;
                    }
                }
            }
            // I-type instructions
            0b0010011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match (funct3, funct6) {
                    (0b001, 0b001010) => Self::Bseti { rd, rs1, shamt },
                    (0b001, 0b010010) => Self::Bclri { rd, rs1, shamt },
                    (0b001, 0b011010) => Self::Binvi { rd, rs1, shamt },
                    (0b101, 0b010010) => Self::Bexti { rd, rs1, shamt },
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
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

impl<Reg> fmt::Display for Zbs64ExtInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bset { rd, rs1, rs2 } => write!(f, "bset {}, {}, {}", rd, rs1, rs2),
            Self::Bseti { rd, rs1, shamt } => write!(f, "bseti {}, {}, {}", rd, rs1, shamt),
            Self::Bclr { rd, rs1, rs2 } => write!(f, "bclr {}, {}, {}", rd, rs1, rs2),
            Self::Bclri { rd, rs1, shamt } => write!(f, "bclri {}, {}, {}", rd, rs1, shamt),
            Self::Binv { rd, rs1, rs2 } => write!(f, "binv {}, {}, {}", rd, rs1, rs2),
            Self::Binvi { rd, rs1, shamt } => write!(f, "binvi {}, {}, {}", rd, rs1, shamt),
            Self::Bext { rd, rs1, rs2 } => write!(f, "bext {}, {}, {}", rd, rs1, rs2),
            Self::Bexti { rd, rs1, shamt } => write!(f, "bexti {}, {}, {}", rd, rs1, shamt),
        }
    }
}
