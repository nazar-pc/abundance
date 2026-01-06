//! M extension instructions for RISC-V RV64 base ISA

#[cfg(test)]
mod tests;

use crate::instruction::GenericInstruction;
use crate::registers::GenericRegister64;
use core::fmt;

/// RISC-V M instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum M64ExtInstruction<Reg> {
    Mul { rd: Reg, rs1: Reg, rs2: Reg },
    Mulh { rd: Reg, rs1: Reg, rs2: Reg },
    Mulhsu { rd: Reg, rs1: Reg, rs2: Reg },
    Mulhu { rd: Reg, rs1: Reg, rs2: Reg },
    Div { rd: Reg, rs1: Reg, rs2: Reg },
    Divu { rd: Reg, rs1: Reg, rs2: Reg },
    Rem { rd: Reg, rs1: Reg, rs2: Reg },
    Remu { rd: Reg, rs1: Reg, rs2: Reg },

    // RV64M instructions
    Mulw { rd: Reg, rs1: Reg, rs2: Reg },
    Divw { rd: Reg, rs1: Reg, rs2: Reg },
    Divuw { rd: Reg, rs1: Reg, rs2: Reg },
    Remw { rd: Reg, rs1: Reg, rs2: Reg },
    Remuw { rd: Reg, rs1: Reg, rs2: Reg },
}

impl<Reg> const GenericInstruction for M64ExtInstruction<Reg>
where
    Reg: [const] GenericRegister64,
{
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
                    // M extension
                    (0b000, 0b0000001) => Self::Mul { rd, rs1, rs2 },
                    (0b001, 0b0000001) => Self::Mulh { rd, rs1, rs2 },
                    (0b010, 0b0000001) => Self::Mulhsu { rd, rs1, rs2 },
                    (0b011, 0b0000001) => Self::Mulhu { rd, rs1, rs2 },
                    (0b100, 0b0000001) => Self::Div { rd, rs1, rs2 },
                    (0b101, 0b0000001) => Self::Divu { rd, rs1, rs2 },
                    (0b110, 0b0000001) => Self::Rem { rd, rs1, rs2 },
                    (0b111, 0b0000001) => Self::Remu { rd, rs1, rs2 },
                    _ => {
                        return None;
                    }
                }
            }
            // R-type W
            0b0111011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b000, 0b0000001) => Self::Mulw { rd, rs1, rs2 },
                    (0b100, 0b0000001) => Self::Divw { rd, rs1, rs2 },
                    (0b101, 0b0000001) => Self::Divuw { rd, rs1, rs2 },
                    (0b110, 0b0000001) => Self::Remw { rd, rs1, rs2 },
                    (0b111, 0b0000001) => Self::Remuw { rd, rs1, rs2 },
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

impl<Reg> fmt::Display for M64ExtInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mul { rd, rs1, rs2 } => write!(f, "mul {}, {}, {}", rd, rs1, rs2),
            Self::Mulh { rd, rs1, rs2 } => write!(f, "mulh {}, {}, {}", rd, rs1, rs2),
            Self::Mulhsu { rd, rs1, rs2 } => write!(f, "mulhsu {}, {}, {}", rd, rs1, rs2),
            Self::Mulhu { rd, rs1, rs2 } => write!(f, "mulhu {}, {}, {}", rd, rs1, rs2),
            Self::Div { rd, rs1, rs2 } => write!(f, "div {}, {}, {}", rd, rs1, rs2),
            Self::Divu { rd, rs1, rs2 } => write!(f, "divu {}, {}, {}", rd, rs1, rs2),
            Self::Rem { rd, rs1, rs2 } => write!(f, "rem {}, {}, {}", rd, rs1, rs2),
            Self::Remu { rd, rs1, rs2 } => write!(f, "remu {}, {}, {}", rd, rs1, rs2),

            Self::Mulw { rd, rs1, rs2 } => write!(f, "mulw {}, {}, {}", rd, rs1, rs2),
            Self::Divw { rd, rs1, rs2 } => write!(f, "divw {}, {}, {}", rd, rs1, rs2),
            Self::Divuw { rd, rs1, rs2 } => write!(f, "divuw {}, {}, {}", rd, rs1, rs2),
            Self::Remw { rd, rs1, rs2 } => write!(f, "remw {}, {}, {}", rd, rs1, rs2),
            Self::Remuw { rd, rs1, rs2 } => write!(f, "remuw {}, {}, {}", rd, rs1, rs2),
        }
    }
}
