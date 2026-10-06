//! RV64 M extension

#[cfg(test)]
mod tests;
pub mod zmmul;

use crate::hart::HartConfig;
use crate::instructions::Instruction;
use crate::instructions::rv64::m::zmmul::Rv64ZmmulInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 M instruction
#[instruction(inherit = [Rv64ZmmulInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64MInstruction<Hart>
where
    Hart: HartConfig,
{
    Div { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Divu { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Rem { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Remu { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },

    // RV64M instructions
    Divw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Divuw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Remw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Remuw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64MInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
                    (0b100, 0b000_0001) => Some(Self::Div { rd, rs1, rs2 }),
                    (0b101, 0b000_0001) => Some(Self::Divu { rd, rs1, rs2 }),
                    (0b110, 0b000_0001) => Some(Self::Rem { rd, rs1, rs2 }),
                    (0b111, 0b000_0001) => Some(Self::Remu { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // R-type W
            0b011_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b100, 0b000_0001) => Some(Self::Divw { rd, rs1, rs2 }),
                    (0b101, 0b000_0001) => Some(Self::Divuw { rd, rs1, rs2 }),
                    (0b110, 0b000_0001) => Some(Self::Remw { rd, rs1, rs2 }),
                    (0b111, 0b000_0001) => Some(Self::Remuw { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for Rv64MInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Div { rd, rs1, rs2 } => write!(f, "div {rd}, {rs1}, {rs2}"),
            Self::Divu { rd, rs1, rs2 } => write!(f, "divu {rd}, {rs1}, {rs2}"),
            Self::Rem { rd, rs1, rs2 } => write!(f, "rem {rd}, {rs1}, {rs2}"),
            Self::Remu { rd, rs1, rs2 } => write!(f, "remu {rd}, {rs1}, {rs2}"),

            Self::Divw { rd, rs1, rs2 } => write!(f, "divw {rd}, {rs1}, {rs2}"),
            Self::Divuw { rd, rs1, rs2 } => write!(f, "divuw {rd}, {rs1}, {rs2}"),
            Self::Remw { rd, rs1, rs2 } => write!(f, "remw {rd}, {rs1}, {rs2}"),
            Self::Remuw { rd, rs1, rs2 } => write!(f, "remuw {rd}, {rs1}, {rs2}"),
        }
    }
}
