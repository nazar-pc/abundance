//! RV64 Zabha extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::rv64::a::zaamo::Rv64ZaamoInstruction;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zabha instruction (Byte and Halfword Atomic Memory Operations)
#[instruction(inherit = [Rv64ZaamoInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZabhaInstruction<Hart>
where
    Hart: HartConfig,
{
    AmoswapB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoswapH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoaddB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoaddH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoxorB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoxorH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoandB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoandH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoorB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmoorH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmominB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmominH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmomaxB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmomaxH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmominuB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmominuH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmomaxuB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    AmomaxuH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    /// Compare-and-swap byte. Only present when `Zacas` is also implemented.
    #[instruction(if = [Rv64ZacasInstruction])]
    AmocasB { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
    /// Compare-and-swap halfword. Only present when `Zacas` is also implemented.
    #[instruction(if = [Rv64ZacasInstruction])]
    AmocasH { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg, aq: bool, rl: bool },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZabhaInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zabha", 1, 0)];

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

        match (opcode, funct3) {
            // AMO, byte- or halfword-sized
            (0b010_1111, 0b000 | 0b001) => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                let funct5 = funct7 >> 2;
                let aq = (funct7 & 0b10) != 0;
                let rl = (funct7 & 0b01) != 0;

                if funct3 == 0b000 {
                    match funct5 {
                        0b00001 => Some(Self::AmoswapB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00000 => Some(Self::AmoaddB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00100 => Some(Self::AmoxorB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01100 => Some(Self::AmoandB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01000 => Some(Self::AmoorB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10000 => Some(Self::AmominB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10100 => Some(Self::AmomaxB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11000 => Some(Self::AmominuB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11100 => Some(Self::AmomaxuB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00101 => Some(Self::AmocasB {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        _ => None,
                    }
                } else {
                    match funct5 {
                        0b00001 => Some(Self::AmoswapH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00000 => Some(Self::AmoaddH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00100 => Some(Self::AmoxorH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01100 => Some(Self::AmoandH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01000 => Some(Self::AmoorH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10000 => Some(Self::AmominH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10100 => Some(Self::AmomaxH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11000 => Some(Self::AmominuH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11100 => Some(Self::AmomaxuH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00101 => Some(Self::AmocasH {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        _ => None,
                    }
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
impl<Reg, Hart> fmt::Display for Rv64ZabhaInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::AmoswapB { rd, rs1, rs2, aq, rl } => write!(f, "amoswap.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoswapH { rd, rs1, rs2, aq, rl } => write!(f, "amoswap.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoaddB { rd, rs1, rs2, aq, rl } => write!(f, "amoadd.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoaddH { rd, rs1, rs2, aq, rl } => write!(f, "amoadd.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoxorB { rd, rs1, rs2, aq, rl } => write!(f, "amoxor.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoxorH { rd, rs1, rs2, aq, rl } => write!(f, "amoxor.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoandB { rd, rs1, rs2, aq, rl } => write!(f, "amoand.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoandH { rd, rs1, rs2, aq, rl } => write!(f, "amoand.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoorB { rd, rs1, rs2, aq, rl } => write!(f, "amoor.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoorH { rd, rs1, rs2, aq, rl } => write!(f, "amoor.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmominB { rd, rs1, rs2, aq, rl } => write!(f, "amomin.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmominH { rd, rs1, rs2, aq, rl } => write!(f, "amomin.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmomaxB { rd, rs1, rs2, aq, rl } => write!(f, "amomax.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmomaxH { rd, rs1, rs2, aq, rl } => write!(f, "amomax.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmominuB { rd, rs1, rs2, aq, rl } => write!(f, "amominu.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmominuH { rd, rs1, rs2, aq, rl } => write!(f, "amominu.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmomaxuB { rd, rs1, rs2, aq, rl } => write!(f, "amomaxu.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmomaxuH { rd, rs1, rs2, aq, rl } => write!(f, "amomaxu.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmocasB { rd, rs1, rs2, aq, rl } => write!(f, "amocas.b{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmocasH { rd, rs1, rs2, aq, rl } => write!(f, "amocas.h{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
        }
    }
}
