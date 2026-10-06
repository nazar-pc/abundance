//! RV64 Zbkc extension (subset of Zbc extension)

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zbkc instruction
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZbkcInstruction<Hart>
where
    Hart: HartConfig,
{
    Clmul { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Clmulh { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZbkcInstruction<Hart>
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
                    (0b001, 0b000_0101) => Some(Self::Clmul { rd, rs1, rs2 }),
                    (0b011, 0b000_0101) => Some(Self::Clmulh { rd, rs1, rs2 }),
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
impl<Reg, Hart> fmt::Display for Rv64ZbkcInstruction<Hart>
where
    Reg: fmt::Display,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clmul { rd, rs1, rs2 } => write!(f, "clmul {rd}, {rs1}, {rs2}"),
            Self::Clmulh { rd, rs1, rs2 } => write!(f, "clmulh {rd}, {rs1}, {rs2}"),
        }
    }
}
