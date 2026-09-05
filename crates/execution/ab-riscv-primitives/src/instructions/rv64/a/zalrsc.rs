//! RV64 Zalrsc extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zalrsc instruction (Load-Reserved/Store-Conditional)
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZalrscInstruction<Reg> {
    Lr { rd: Reg, rs1: Reg, aq: bool, rl: bool },
    Sc { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },

    LrD { rd: Reg, rs1: Reg, aq: bool, rl: bool },
    ScD { rd: Reg, rs1: Reg, rs2: Reg, aq: bool, rl: bool },
}

#[instruction]
const impl<Reg> Instruction for Rv64ZalrscInstruction<Reg>
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
                let funct5 = funct7 >> 2;
                let aq = (funct7 & 0b10) != 0;
                let rl = (funct7 & 0b01) != 0;

                if funct3 == 0b010 {
                    match (funct5, rs2_bits) {
                        (0b00010, 0) => Some(Self::Lr { rd, rs1, aq, rl }),
                        (0b00011, _) => {
                            let rs2 = Reg::from_bits(rs2_bits)?;
                            Some(Self::Sc {
                                rd,
                                rs1,
                                rs2,
                                aq,
                                rl,
                            })
                        }
                        _ => None,
                    }
                } else {
                    match (funct5, rs2_bits) {
                        (0b00010, 0) => Some(Self::LrD { rd, rs1, aq, rl }),
                        (0b00011, _) => {
                            let rs2 = Reg::from_bits(rs2_bits)?;
                            Some(Self::ScD {
                                rd,
                                rs1,
                                rs2,
                                aq,
                                rl,
                            })
                        }
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
impl<Reg> fmt::Display for Rv64ZalrscInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::Lr { rd, rs1, aq, rl } => write!(f, "lr.w{} {rd}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::Sc { rd, rs1, rs2, aq, rl } => write!(f, "sc.w{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::LrD { rd, rs1, aq, rl } => write!(f, "lr.d{} {rd}, ({rs1})", aq_rl_suffix(aq, rl)),
            Self::ScD { rd, rs1, rs2, aq, rl } => write!(f, "sc.d{} {rd}, {rs2}, ({rs1})", aq_rl_suffix(aq, rl)),
        }
    }
}
