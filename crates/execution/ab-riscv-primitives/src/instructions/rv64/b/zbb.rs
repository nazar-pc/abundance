//! RV64 Zbb extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::utils::Shamt;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 instructions shared between Zbb and Zbkb extensions.
///
/// Not an extension on its own, only exists such that both Zbb and Zbkb can inherit it.
#[instruction]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZbbZbkbSharedInstruction<Hart>
where
    Hart: HartConfig,
{
    Andn { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Orn { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Xnor { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Rol { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Rolw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Ror { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Rori { rd: Hart::Reg, rs1: Hart::Reg, shamt: Shamt<u64> },
    Roriw { rd: Hart::Reg, rs1: Hart::Reg, shamt: Shamt<u32> },
    Rorw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Rev8 { rd: Hart::Reg, rs1: Hart::Reg },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZbbZbkbSharedInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[];

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
        // bits 25:20 for I-type distinctions
        let low6 = ((instruction >> 20) & 0x3f) as u8;
        let funct12 = ((instruction >> 20) & 0xfff) as u16;

        match opcode {
            // OP-IMM
            0b001_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    0b101 => {
                        if funct12 == 0b0110_1011_1000 {
                            Some(Self::Rev8 { rd, rs1 })
                        } else if funct6 == 0b01_1000 {
                            Some(Self::Rori {
                                rd,
                                rs1,
                                shamt: Shamt::<u64>::new(low6)?,
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            // OP / R-type
            0b011_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b001, 0b011_0000) => Some(Self::Rol { rd, rs1, rs2 }),
                    (0b100, 0b010_0000) => Some(Self::Xnor { rd, rs1, rs2 }),
                    (0b101, 0b011_0000) => Some(Self::Ror { rd, rs1, rs2 }),
                    (0b110, 0b010_0000) => Some(Self::Orn { rd, rs1, rs2 }),
                    (0b111, 0b010_0000) => Some(Self::Andn { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // OP-IMM-32
            0b001_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match (funct3, funct7) {
                    (0b101, 0b011_0000) => {
                        let shamt = Shamt::<u32>::new(rs2_bits)?;
                        Some(Self::Roriw { rd, rs1, shamt })
                    }
                    _ => None,
                }
            }
            // OP-32
            0b011_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b001, 0b011_0000) => Some(Self::Rolw { rd, rs1, rs2 }),
                    (0b101, 0b011_0000) => Some(Self::Rorw { rd, rs1, rs2 }),
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
impl<Reg, Hart> fmt::Display for Rv64ZbbZbkbSharedInstruction<Hart>
where
    Reg: fmt::Display,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Andn { rd, rs1, rs2 } => write!(f, "andn {rd}, {rs1}, {rs2}"),
            Self::Orn { rd, rs1, rs2 } => write!(f, "orn {rd}, {rs1}, {rs2}"),
            Self::Xnor { rd, rs1, rs2 } => write!(f, "xnor {rd}, {rs1}, {rs2}"),
            Self::Rol { rd, rs1, rs2 } => write!(f, "rol {rd}, {rs1}, {rs2}"),
            Self::Rolw { rd, rs1, rs2 } => write!(f, "rolw {rd}, {rs1}, {rs2}"),
            Self::Ror { rd, rs1, rs2 } => write!(f, "ror {rd}, {rs1}, {rs2}"),
            Self::Rori { rd, rs1, shamt } => write!(f, "rori {rd}, {rs1}, {shamt}"),
            Self::Roriw { rd, rs1, shamt } => write!(f, "roriw {rd}, {rs1}, {shamt}"),
            Self::Rorw { rd, rs1, rs2 } => write!(f, "rorw {rd}, {rs1}, {rs2}"),
            Self::Rev8 { rd, rs1 } => write!(f, "rev8 {rd}, {rs1}"),
        }
    }
}

/// RISC-V RV64 Zbb instruction (Basic bit manipulation)
#[instruction(
    reorder = [
        Andn, Orn, Xnor, Clz, Clzw, Ctz, Ctzw, Cpop, Cpopw, Max, Maxu, Min, Minu, Sextb, Sexth,
        Zexth, Rol, Rolw, Ror, Rori, Roriw, Rorw, Orcb, Rev8,
    ],
    inherit = [Rv64ZbbZbkbSharedInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZbbInstruction<Hart>
where
    Hart: HartConfig,
{
    // RV64 Zbb instructions
    Clz { rd: Hart::Reg, rs1: Hart::Reg },
    Clzw { rd: Hart::Reg, rs1: Hart::Reg },
    Ctz { rd: Hart::Reg, rs1: Hart::Reg },
    Ctzw { rd: Hart::Reg, rs1: Hart::Reg },
    Cpop { rd: Hart::Reg, rs1: Hart::Reg },
    Cpopw { rd: Hart::Reg, rs1: Hart::Reg },
    Max { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Maxu { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Min { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Minu { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    Sextb { rd: Hart::Reg, rs1: Hart::Reg },
    Sexth { rd: Hart::Reg, rs1: Hart::Reg },
    Zexth { rd: Hart::Reg, rs1: Hart::Reg },
    Orcb { rd: Hart::Reg, rs1: Hart::Reg },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZbbInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zbb", 1, 0)];

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
        // bits 25:20 for I-type distinctions
        let low6 = ((instruction >> 20) & 0x3f) as u8;
        let funct12 = ((instruction >> 20) & 0xfff) as u16;

        match opcode {
            // OP-IMM
            0b001_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    0b001 => {
                        if funct6 == 0b01_1000 {
                            match low6 {
                                0 => Some(Self::Clz { rd, rs1 }),
                                1 => Some(Self::Ctz { rd, rs1 }),
                                2 => Some(Self::Cpop { rd, rs1 }),
                                4 => Some(Self::Sextb { rd, rs1 }),
                                5 => Some(Self::Sexth { rd, rs1 }),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    0b101 => {
                        if funct12 == 0b0010_1000_0111 {
                            Some(Self::Orcb { rd, rs1 })
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            // OP / R-type
            0b011_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b100, 0b000_0101) => Some(Self::Min { rd, rs1, rs2 }),
                    (0b101, 0b000_0101) => Some(Self::Minu { rd, rs1, rs2 }),
                    (0b110, 0b000_0101) => Some(Self::Max { rd, rs1, rs2 }),
                    (0b111, 0b000_0101) => Some(Self::Maxu { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // OP-IMM-32
            0b001_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match (funct3, funct7) {
                    (0b001, 0b011_0000) => match rs2_bits {
                        0 => Some(Self::Clzw { rd, rs1 }),
                        1 => Some(Self::Ctzw { rd, rs1 }),
                        2 => Some(Self::Cpopw { rd, rs1 }),
                        _ => None,
                    },
                    _ => None,
                }
            }
            // OP-32
            0b011_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match (funct3, funct7) {
                    (0b100, 0b000_0100) if rs2_bits == 0 => Some(Self::Zexth { rd, rs1 }),
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
impl<Reg, Hart> fmt::Display for Rv64ZbbInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clz { rd, rs1 } => write!(f, "clz {rd}, {rs1}"),
            Self::Clzw { rd, rs1 } => write!(f, "clzw {rd}, {rs1}"),
            Self::Ctz { rd, rs1 } => write!(f, "ctz {rd}, {rs1}"),
            Self::Ctzw { rd, rs1 } => write!(f, "ctzw {rd}, {rs1}"),
            Self::Cpop { rd, rs1 } => write!(f, "cpop {rd}, {rs1}"),
            Self::Cpopw { rd, rs1 } => write!(f, "cpopw {rd}, {rs1}"),
            Self::Max { rd, rs1, rs2 } => write!(f, "max {rd}, {rs1}, {rs2}"),
            Self::Maxu { rd, rs1, rs2 } => write!(f, "maxu {rd}, {rs1}, {rs2}"),
            Self::Min { rd, rs1, rs2 } => write!(f, "min {rd}, {rs1}, {rs2}"),
            Self::Minu { rd, rs1, rs2 } => write!(f, "minu {rd}, {rs1}, {rs2}"),
            Self::Sextb { rd, rs1 } => write!(f, "sext.b {rd}, {rs1}"),
            Self::Sexth { rd, rs1 } => write!(f, "sext.h {rd}, {rs1}"),
            Self::Zexth { rd, rs1 } => write!(f, "zext.h {rd}, {rs1}"),
            Self::Orcb { rd, rs1 } => write!(f, "orc.b {rd}, {rs1}"),
        }
    }
}
