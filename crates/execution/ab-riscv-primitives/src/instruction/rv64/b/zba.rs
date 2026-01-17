//! RV64 Zba extension

#[cfg(test)]
mod tests;

use crate::instruction::Instruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zba instruction (Address generation)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZbaInstruction<Reg> {
    AddUw { rd: Reg, rs1: Reg, rs2: Reg },
    Sh1add { rd: Reg, rs1: Reg, rs2: Reg },
    Sh1addUw { rd: Reg, rs1: Reg, rs2: Reg },
    Sh2add { rd: Reg, rs1: Reg, rs2: Reg },
    Sh2addUw { rd: Reg, rs1: Reg, rs2: Reg },
    Sh3add { rd: Reg, rs1: Reg, rs2: Reg },
    Sh3addUw { rd: Reg, rs1: Reg, rs2: Reg },
    SlliUw { rd: Reg, rs1: Reg, shamt: u8 },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZbaInstruction<Reg>
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
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        match opcode {
            // R-type
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b010, 0b0010000) => Some(Self::Sh1add { rd, rs1, rs2 }),
                    (0b100, 0b0010000) => Some(Self::Sh2add { rd, rs1, rs2 }),
                    (0b110, 0b0010000) => Some(Self::Sh3add { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // R-type W
            0b0111011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    0b000 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b0000100 => Some(Self::AddUw { rd, rs1, rs2 }),
                            _ => None,
                        }
                    }
                    0b001 => {
                        let shamt = rs2_bits;
                        match funct6 {
                            0b000010 => Some(Self::SlliUw { rd, rs1, shamt }),
                            _ => None,
                        }
                    }
                    0b010 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b0010000 => Some(Self::Sh1addUw { rd, rs1, rs2 }),
                            _ => None,
                        }
                    }
                    0b100 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b0010000 => Some(Self::Sh2addUw { rd, rs1, rs2 }),
                            _ => None,
                        }
                    }
                    0b110 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b0010000 => Some(Self::Sh3addUw { rd, rs1, rs2 }),
                            _ => None,
                        }
                    }
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

impl<Reg> fmt::Display for Rv64ZbaInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddUw { rd, rs1, rs2 } => write!(f, "add.uw {}, {}, {}", rd, rs1, rs2),
            Self::Sh1add { rd, rs1, rs2 } => write!(f, "sh1add {}, {}, {}", rd, rs1, rs2),
            Self::Sh1addUw { rd, rs1, rs2 } => write!(f, "sh1add.uw {}, {}, {}", rd, rs1, rs2),
            Self::Sh2add { rd, rs1, rs2 } => write!(f, "sh2add {}, {}, {}", rd, rs1, rs2),
            Self::Sh2addUw { rd, rs1, rs2 } => write!(f, "sh2add.uw {}, {}, {}", rd, rs1, rs2),
            Self::Sh3add { rd, rs1, rs2 } => write!(f, "sh3add {}, {}, {}", rd, rs1, rs2),
            Self::Sh3addUw { rd, rs1, rs2 } => write!(f, "sh3add.uw {}, {}, {}", rd, rs1, rs2),
            Self::SlliUw { rd, rs1, shamt } => write!(f, "slli.uw {}, {}, {}", rd, rs1, shamt),
        }
    }
}
