//! RV32 Zbkb extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbb::Rv32ZbbInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zbkb instruction (Bit-manipulation for Cryptography)
#[instruction(
    reorder = [Andn, Orn, Xnor, Rol, Ror, Rori, Rev8, Pack, Packh, Brev8],
    ignore = [Rv32ZbbInstruction],
    inherit = [Rv32ZbbInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZbkbInstruction<Reg> {
    /// Pack low 16 bits of `rs1` into `rd[15:0]`, low 16 bits of `rs2` into `rd[31:16]`
    Pack { rd: Reg, rs1: Reg, rs2: Reg },
    /// Pack low 8 bits of `rs1` into `rd[7:0]`, low 8 bits of `rs2` into `rd[15:8]`
    Packh { rd: Reg, rs1: Reg, rs2: Reg },
    /// Reverse bits in each byte of `rs1`
    Brev8 { rd: Reg, rs1: Reg },
    /// Bit-interleave: scatter lower-half bits of `rs1` to even positions, upper-half bits to odd
    /// positions
    Zip { rd: Reg, rs1: Reg },
    /// Inverse of zip: gather even-position bits of `rs1` to lower half, odd-position bits to upper
    /// half
    Unzip { rd: Reg, rs1: Reg },
}

#[instruction]
const impl<Reg> Instruction for Rv32ZbkbInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;
        let funct12 = ((instruction >> 20) & 0xfff) as u16;

        match opcode {
            // OP-IMM (I-type): brev8, zip, unzip
            0b001_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    // brev8: funct12 = 0b011010000111 = 0x687
                    0b101 if funct12 == 0b0110_1000_0111 => Some(Self::Brev8 { rd, rs1 }),
                    // zip:   funct7=0000100, rs2=01111, funct3=001 -> funct12=0x08F
                    0b001 if funct12 == 0b0000_1000_1111 => Some(Self::Zip { rd, rs1 }),
                    // unzip: funct7=0000100, rs2=01111, funct3=101 -> funct12=0x08F
                    0b101 if funct12 == 0b0000_1000_1111 => Some(Self::Unzip { rd, rs1 }),
                    _ => None,
                }
            }
            // OP (R-type): pack, packh
            0b011_0011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7, rs2_bits) {
                    // pack: funct3=100, funct7=0000100, rs2!=0
                    // rs2=0 collides with inherited RV32 Zbb zext.h and must fall through.
                    (0b100, 0b000_0100, rs2_bits) if rs2_bits != 0 => {
                        Some(Self::Pack { rd, rs1, rs2 })
                    }
                    // packh: funct3=111, funct7=0000100
                    (0b111, 0b000_0100, _) => Some(Self::Packh { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv32ZbkbInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pack { rd, rs1, rs2 } => write!(f, "pack {rd}, {rs1}, {rs2}"),
            Self::Packh { rd, rs1, rs2 } => write!(f, "packh {rd}, {rs1}, {rs2}"),
            Self::Brev8 { rd, rs1 } => write!(f, "brev8 {rd}, {rs1}"),
            Self::Zip { rd, rs1 } => write!(f, "zip {rd}, {rs1}"),
            Self::Unzip { rd, rs1 } => write!(f, "unzip {rd}, {rs1}"),
        }
    }
}
