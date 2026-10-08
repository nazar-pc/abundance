//! RV64 Zbkb extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::rv64::b::zbb::Rv64ZbbZbkbSharedInstruction;
use crate::instructions::utils::Shamt;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zbkb instruction (Bit-manipulation for Cryptography)
#[instruction(inherit = [Rv64ZbbZbkbSharedInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZbkbInstruction<Hart>
where
    Hart: HartConfig,
{
    /// Pack low 32 bits of `rs1` and `rs2` into `rd`
    Pack { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    /// Pack low 8 bits of `rs1` and `rs2` into `rd` bytes `0` and `1`
    Packh { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    /// Pack low 16 bits of `rs1` and `rs2` into lower 32 bits of `rd`, sign-extend
    Packw { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    /// Reverse bits in each byte of `rs1`
    Brev8 { rd: Hart::Reg, rs1: Hart::Reg },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZbkbInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zbkb", 1, 0)];

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
        let funct12 = ((instruction >> 20) & 0xfff) as u16;

        match opcode {
            // OP-IMM (I-type): brev8
            0b001_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    // brev8: funct12 = 0b0110_1000_0111 = 0x687
                    0b101 if funct12 == 0b0110_1000_0111 => Some(Self::Brev8 { rd, rs1 }),
                    _ => None,
                }
            }
            // OP (R-type): pack, packh
            0b011_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    // pack: funct3=100, funct7=000_0100
                    (0b100, 0b000_0100) => Some(Self::Pack { rd, rs1, rs2 }),
                    // packh: funct3=111, funct7=000_0100
                    (0b111, 0b000_0100) => Some(Self::Packh { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // OP-32 (R-type): packw
            0b011_1011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    // packw: funct3=100, funct7=000_0100
                    (0b100, 0b000_0100) => {
                        // NOTE: `packw rd, rs1, x0` has exactly the same encoding and semantics
                        // as Zbb's `zext.h`, which is cheaper to execute, so it is decoded as
                        // `zext.h` when it is present in the instruction set, regardless of the
                        // order in which extensions are inherited. The macro replaces `Self::Zexth`
                        // with `None?` in instruction sets without it, which makes this a regular
                        // `packw`.
                        let zexth = if rs2_bits == 0 {
                            try { Self::Zexth { rd, rs1 } }
                        } else {
                            None
                        };
                        if zexth.is_some() {
                            zexth
                        } else {
                            Some(Self::Packw { rd, rs1, rs2 })
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
impl<Reg, Hart> fmt::Display for Rv64ZbkbInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pack { rd, rs1, rs2 } => write!(f, "pack {rd}, {rs1}, {rs2}"),
            Self::Packh { rd, rs1, rs2 } => write!(f, "packh {rd}, {rs1}, {rs2}"),
            Self::Packw { rd, rs1, rs2 } => write!(f, "packw {rd}, {rs1}, {rs2}"),
            Self::Brev8 { rd, rs1 } => write!(f, "brev8 {rd}, {rs1}"),
        }
    }
}
