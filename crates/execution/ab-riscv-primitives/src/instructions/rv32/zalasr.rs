//! RV32 Zalasr extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zalasr instruction (Load-Acquire/Store-Release).
///
/// Load-acquire instructions always carry an acquire annotation (not stored as a field since it
/// is always `true`), plus an optional release annotation stored in `rl`. Store-release
/// instructions always carry a release annotation (not stored as a field since it is always
/// `true`), plus an optional acquire annotation stored in `aq`.
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv32ZalasrInstruction<Hart>
where
    Hart: HartConfig,
{
    /// Load-acquire byte
    LbAq { rd: Hart::Reg, rs1: Hart::Reg, rl: bool },
    /// Load-acquire halfword
    LhAq { rd: Hart::Reg, rs1: Hart::Reg, rl: bool },
    /// Load-acquire word
    LwAq { rd: Hart::Reg, rs1: Hart::Reg, rl: bool },
    /// Store-release byte
    SbRl { rs1: Hart::Reg, rs2: Hart::Reg, aq: bool },
    /// Store-release halfword
    ShRl { rs1: Hart::Reg, rs2: Hart::Reg, aq: bool },
    /// Store-release word
    SwRl { rs1: Hart::Reg, rs2: Hart::Reg, aq: bool },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv32ZalasrInstruction<Hart>
where
    Reg: [const] Register<Type = u32>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zalasr", 1, 0)];

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
        let funct5 = funct7 >> 2;
        let aq = (funct7 & 0b10) != 0;
        let rl = (funct7 & 0b01) != 0;

        match (opcode, funct5) {
            // Load-acquire, `aq` bit is mandatory and `rs2` must be zero
            (0b010_1111, 0b00110) => match (funct3, rs2_bits, aq) {
                (0b000, 0, true) => {
                    let rd = Reg::from_bits(rd_bits)?;
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::LbAq { rd, rs1, rl })
                }
                (0b001, 0, true) => {
                    let rd = Reg::from_bits(rd_bits)?;
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::LhAq { rd, rs1, rl })
                }
                (0b010, 0, true) => {
                    let rd = Reg::from_bits(rd_bits)?;
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::LwAq { rd, rs1, rl })
                }
                _ => None,
            },
            // Store-release, `rl` bit is mandatory and `rd` must be zero
            (0b010_1111, 0b00111) => match (funct3, rd_bits, rl) {
                (0b000, 0, true) => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    let rs2 = Reg::from_bits(rs2_bits)?;
                    Some(Self::SbRl { rs1, rs2, aq })
                }
                (0b001, 0, true) => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    let rs2 = Reg::from_bits(rs2_bits)?;
                    Some(Self::ShRl { rs1, rs2, aq })
                }
                (0b010, 0, true) => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    let rs2 = Reg::from_bits(rs2_bits)?;
                    Some(Self::SwRl { rs1, rs2, aq })
                }
                _ => None,
            },
            _ => None,
        }
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

/// Format optional `rl` suffix for load-acquire display
#[inline(always)]
fn rl_suffix(rl: &bool) -> &'static str {
    if *rl { ".aqrl" } else { ".aq" }
}

/// Format optional `aq` suffix for store-release display
#[inline(always)]
fn aq_suffix(aq: &bool) -> &'static str {
    if *aq { ".aqrl" } else { ".rl" }
}

#[instruction]
impl<Reg, Hart> fmt::Display for Rv32ZalasrInstruction<Hart>
where
    Reg: fmt::Display,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LbAq { rd, rs1, rl } => write!(f, "lb{} {rd}, ({rs1})", rl_suffix(rl)),
            Self::LhAq { rd, rs1, rl } => write!(f, "lh{} {rd}, ({rs1})", rl_suffix(rl)),
            Self::LwAq { rd, rs1, rl } => write!(f, "lw{} {rd}, ({rs1})", rl_suffix(rl)),
            Self::SbRl { rs1, rs2, aq } => write!(f, "sb{} {rs2}, ({rs1})", aq_suffix(aq)),
            Self::ShRl { rs1, rs2, aq } => write!(f, "sh{} {rs2}, ({rs1})", aq_suffix(aq)),
            Self::SwRl { rs1, rs2, aq } => write!(f, "sw{} {rs2}, ({rs1})", aq_suffix(aq)),
        }
    }
}
