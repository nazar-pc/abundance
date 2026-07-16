//! RV32 M extension

#[cfg(test)]
mod tests;
pub mod zmmul;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 M instruction
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32MInstruction<Reg> {
    Mul { rd: Reg, rs1: Reg, rs2: Reg },
    Mulh { rd: Reg, rs1: Reg, rs2: Reg },
    Mulhsu { rd: Reg, rs1: Reg, rs2: Reg },
    Mulhu { rd: Reg, rs1: Reg, rs2: Reg },
    Div { rd: Reg, rs1: Reg, rs2: Reg },
    Divu { rd: Reg, rs1: Reg, rs2: Reg },
    Rem { rd: Reg, rs1: Reg, rs2: Reg },
    Remu { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
const impl<Reg> Instruction for Rv32MInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
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
            // R-type
            0b011_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b000, 0b000_0001) => Some(Self::Mul { rd, rs1, rs2 }),
                    (0b001, 0b000_0001) => Some(Self::Mulh { rd, rs1, rs2 }),
                    (0b010, 0b000_0001) => Some(Self::Mulhsu { rd, rs1, rs2 }),
                    (0b011, 0b000_0001) => Some(Self::Mulhu { rd, rs1, rs2 }),
                    (0b100, 0b000_0001) => Some(Self::Div { rd, rs1, rs2 }),
                    (0b101, 0b000_0001) => Some(Self::Divu { rd, rs1, rs2 }),
                    (0b110, 0b000_0001) => Some(Self::Rem { rd, rs1, rs2 }),
                    (0b111, 0b000_0001) => Some(Self::Remu { rd, rs1, rs2 }),
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

#[instruction]
impl<Reg> fmt::Display for Rv32MInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mul { rd, rs1, rs2 } => write!(f, "mul {rd}, {rs1}, {rs2}"),
            Self::Mulh { rd, rs1, rs2 } => write!(f, "mulh {rd}, {rs1}, {rs2}"),
            Self::Mulhsu { rd, rs1, rs2 } => write!(f, "mulhsu {rd}, {rs1}, {rs2}"),
            Self::Mulhu { rd, rs1, rs2 } => write!(f, "mulhu {rd}, {rs1}, {rs2}"),
            Self::Div { rd, rs1, rs2 } => write!(f, "div {rd}, {rs1}, {rs2}"),
            Self::Divu { rd, rs1, rs2 } => write!(f, "divu {rd}, {rs1}, {rs2}"),
            Self::Rem { rd, rs1, rs2 } => write!(f, "rem {rd}, {rs1}, {rs2}"),
            Self::Remu { rd, rs1, rs2 } => write!(f, "remu {rd}, {rs1}, {rs2}"),
        }
    }
}
