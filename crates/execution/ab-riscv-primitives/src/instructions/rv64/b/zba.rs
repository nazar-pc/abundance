//! RV64 Zba extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::utils::Shamt;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zba instruction (Address generation)
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZbaInstruction<Hart>
where
    Hart: HartConfig,
{
    AddUw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Sh1add { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Sh1addUw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Sh2add { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Sh2addUw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Sh3add { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Sh3addUw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    SlliUw { rd: Hart::Reg, rs1: Hart::Reg, shamt: Shamt<u64> },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZbaInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zba", 1, 0)];

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
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        match opcode {
            // R-type
            0b011_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b010, 0b001_0000) => Some(Self::Sh1add { rd, rs1, rs2 }),
                    (0b100, 0b001_0000) => Some(Self::Sh2add { rd, rs1, rs2 }),
                    (0b110, 0b001_0000) => Some(Self::Sh3add { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // I-type W (OP-IMM-32)
            0b001_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                // shamt is 6 bits: [25:20]
                let shamt = Shamt::<u64>::new(((instruction >> 20) & 0x3f) as u8)?;
                match (funct3, funct6) {
                    (0b001, 0b00_0010) => Some(Self::SlliUw { rd, rs1, shamt }),
                    _ => None,
                }
            }
            // R-type W (OP-32)
            0b011_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    0b000 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b000_0100 => Some(Self::AddUw { rd, rs1, rs2 }),
                            _ => None,
                        }
                    }
                    0b010 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b001_0000 => Some(Self::Sh1addUw { rd, rs1, rs2 }),
                            _ => None,
                        }
                    }
                    0b100 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b001_0000 => Some(Self::Sh2addUw { rd, rs1, rs2 }),
                            _ => None,
                        }
                    }
                    0b110 => {
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        match funct7 {
                            0b001_0000 => Some(Self::Sh3addUw { rd, rs1, rs2 }),
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
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for Rv64ZbaInstruction<Hart>
where
    Reg: fmt::Display,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddUw { rd, rs1, rs2 } => write!(f, "add.uw {rd}, {rs1}, {rs2}"),
            Self::Sh1add { rd, rs1, rs2 } => write!(f, "sh1add {rd}, {rs1}, {rs2}"),
            Self::Sh1addUw { rd, rs1, rs2 } => write!(f, "sh1add.uw {rd}, {rs1}, {rs2}"),
            Self::Sh2add { rd, rs1, rs2 } => write!(f, "sh2add {rd}, {rs1}, {rs2}"),
            Self::Sh2addUw { rd, rs1, rs2 } => write!(f, "sh2add.uw {rd}, {rs1}, {rs2}"),
            Self::Sh3add { rd, rs1, rs2 } => write!(f, "sh3add {rd}, {rs1}, {rs2}"),
            Self::Sh3addUw { rd, rs1, rs2 } => write!(f, "sh3add.uw {rd}, {rs1}, {rs2}"),
            Self::SlliUw { rd, rs1, shamt } => write!(f, "slli.uw {rd}, {rs1}, {shamt}"),
        }
    }
}
