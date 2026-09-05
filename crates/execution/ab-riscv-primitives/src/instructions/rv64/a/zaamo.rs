//! RV64 Zaamo extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zaamo instruction (Atomic memory operations)
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZaamoInstruction<Reg> {
    Amoswap { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amoadd { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amoxor { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amoand { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amoor { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amomin { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amomax { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amominu { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    Amomaxu { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },

    AmoswapD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmoaddD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmoxorD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmoandD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmoorD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmominD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmomaxD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmominuD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
    AmomaxuD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
}

#[instruction]
const impl<Reg> Instruction for Rv64ZaamoInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Reg = Reg;

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
            // AMO, word- or doubleword-sized (the only sizes that exist in RV64)
            (0b010_1111, 0b010 | 0b011) => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                let funct5 = funct7 >> 2;
                let aq = (funct7 & 0b10) != 0;
                let rl = (funct7 & 0b01) != 0;

                if funct3 == 0b010 {
                    match funct5 {
                        0b00001 => Some(Self::Amoswap {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00000 => Some(Self::Amoadd {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00100 => Some(Self::Amoxor {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01100 => Some(Self::Amoand {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01000 => Some(Self::Amoor {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10000 => Some(Self::Amomin {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10100 => Some(Self::Amomax {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11000 => Some(Self::Amominu {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11100 => Some(Self::Amomaxu {
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
                        0b00001 => Some(Self::AmoswapD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00000 => Some(Self::AmoaddD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b00100 => Some(Self::AmoxorD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01100 => Some(Self::AmoandD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b01000 => Some(Self::AmoorD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10000 => Some(Self::AmominD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b10100 => Some(Self::AmomaxD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11000 => Some(Self::AmominuD {
                            rd,
                            rs1,
                            rs2,
                            aq,
                            rl,
                        }),
                        0b11100 => Some(Self::AmomaxuD {
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
impl<Reg> fmt::Display for Rv64ZaamoInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::Amoswap { rd, rs1, rs2, aq, rl } => write!(f, "amoswap.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amoadd { rd, rs1, rs2, aq, rl } => write!(f, "amoadd.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amoxor { rd, rs1, rs2, aq, rl } => write!(f, "amoxor.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amoand { rd, rs1, rs2, aq, rl } => write!(f, "amoand.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amoor { rd, rs1, rs2, aq, rl } => write!(f, "amoor.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amomin { rd, rs1, rs2, aq, rl } => write!(f, "amomin.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amomax { rd, rs1, rs2, aq, rl } => write!(f, "amomax.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amominu { rd, rs1, rs2, aq, rl } => write!(f, "amominu.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Amomaxu { rd, rs1, rs2, aq, rl } => write!(f, "amomaxu.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),

            Self::AmoswapD { rd, rs1, rs2, aq, rl } => write!(f, "amoswap.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoaddD { rd, rs1, rs2, aq, rl } => write!(f, "amoadd.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoxorD { rd, rs1, rs2, aq, rl } => write!(f, "amoxor.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoandD { rd, rs1, rs2, aq, rl } => write!(f, "amoand.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmoorD { rd, rs1, rs2, aq, rl } => write!(f, "amoor.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmominD { rd, rs1, rs2, aq, rl } => write!(f, "amomin.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmomaxD { rd, rs1, rs2, aq, rl } => write!(f, "amomax.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmominuD { rd, rs1, rs2, aq, rl } => write!(f, "amominu.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::AmomaxuD { rd, rs1, rs2, aq, rl } => write!(f, "amomaxu.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
        }
    }
}
