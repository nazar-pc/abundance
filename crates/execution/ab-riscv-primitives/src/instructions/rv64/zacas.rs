//! RV64 Zacas extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::rv64::a::zaamo::Rv64ZaamoInstruction;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zacas instruction (Atomic Compare-and-Swap)
#[instruction(inherit = [Rv64ZaamoInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZacasInstruction<Hart>
where
    Hart: HartConfig,
{
    /// Compare-and-swap word
    AmocasW { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    /// Compare-and-swap doubleword
    AmocasD { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    // TODO: Register pairs are not enforced by types. Decoding only produces even `rd` and `rs2`
    //  with `rd_hi` and `rs2_hi` being the registers right after them, and execution relies on that
    //  instead of checking it again, like with other invariants checked by the decoder, so manually
    //  constructed instructions can use arbitrary registers (not a memory safety issue). A type for
    //  register pairs isn't an option yet: every instruction stores `rs1` and `rs2` with the same
    //  type at the same position, so that reading operands compiles to reads at fixed offsets.
    //  Revisit once `rs1` and `rs2` are no longer added to every instruction unconditionally, which
    //  requires https://github.com/rust-lang/rfcs/pull/4001.
    /// Compare-and-swap quadword, using register pairs `(rd, rd_hi)` and `(rs2, rs2_hi)` since
    /// RV64 registers are only 64 bits wide.
    AmocasQ { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, rd_hi: Hart::Reg, rs2_hi: Hart::Reg, aq: bool, rl: bool },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZacasInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zacas", 1, 0)];

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

        match (opcode, funct3, funct7 >> 2) {
            // AMOCAS.W
            (0b010_1111, 0b010, 0b00101) => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                let aq = (funct7 & 0b10) != 0;
                let rl = (funct7 & 0b01) != 0;

                Some(Self::AmocasW {
                    rd,
                    rs1,
                    rs2,
                    aq,
                    rl,
                })
            }
            // AMOCAS.D
            (0b010_1111, 0b011, 0b00101) => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                let aq = (funct7 & 0b10) != 0;
                let rl = (funct7 & 0b01) != 0;

                Some(Self::AmocasD {
                    rd,
                    rs1,
                    rs2,
                    aq,
                    rl,
                })
            }
            // AMOCAS.Q, register-pair mode (RV64)
            (0b010_1111, 0b100, 0b00101) => {
                match (rd_bits & 1, rs2_bits & 1) {
                    // Both halves of the register pairs must be even numbered, otherwise the
                    // encoding is reserved
                    (0, 0) => {
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        let rd = Reg::from_bits(rd_bits)?;
                        let rd_hi = Reg::from_bits(rd_bits + 1)?;
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        let rs2_hi = Reg::from_bits(rs2_bits + 1)?;
                        let aq = (funct7 & 0b10) != 0;
                        let rl = (funct7 & 0b01) != 0;

                        Some(Self::AmocasQ {
                            rd,
                            rs1,
                            rs2,
                            rd_hi,
                            rs2_hi,
                            aq,
                            rl,
                        })
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

/// Format `aq`/`rl` suffix for display
#[inline(always)]
fn aq_rl_suffix(aq: &bool, rl: &bool) -> &'static str {
    match (*aq, *rl) {
        (false, false) => "",
        (true, false) => ".aq",
        (false, true) => ".rl",
        (true, true) => ".aqrl",
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for Rv64ZacasInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::AmocasW { rd, rs1, rs2, aq, rl } => write!(f, "amocas.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmocasD { rd, rs1, rs2, aq, rl } => write!(f, "amocas.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmocasQ { rd, rs1, rs2, rd_hi, rs2_hi, aq, rl } => write!(f, "amocas.q{} {rd}, {rd_hi}, {rs2}, {rs2_hi}, ({rs1})", aq_rl_suffix(aq, rl)),
        }
    }
}
