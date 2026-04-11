//! RV64 Zbkb extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::rv64::b::zbb::Rv64ZbbInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zbkb instruction (Bit-manipulation for Cryptography)
#[instruction(
    reorder = [Andn, Orn, Xnor, Rol, Rolw, Ror, Rori, Roriw, Rorw, Rev8, Pack, Packh, Packw, Brev8],
    ignore = [Rv64ZbbInstruction],
    inherit = [Rv64ZbbInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZbkbInstruction<Reg> {
    /// Pack low 32 bits of `rs1` and `rs2` into `rd`
    Pack { rd: Reg, rs1: Reg, rs2: Reg },
    /// Pack low 8 bits of `rs1` and `rs2` into `rd` bytes `0` and `1`
    Packh { rd: Reg, rs1: Reg, rs2: Reg },
    /// Pack low 16 bits of `rs1` and `rs2` into lower 32 bits of `rd`, sign-extend
    Packw { rd: Reg, rs1: Reg, rs2: Reg },
    /// Reverse bits in each byte of `rs1`
    Brev8 { rd: Reg, rs1: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZbkbInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
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
            // OP-IMM (I-type): brev8
            0b0010011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    // brev8: funct12 = 0b011010000111 = 0x687
                    0b101 if funct12 == 0b0110_1000_0111 => Some(Self::Brev8 { rd, rs1 }),
                    _ => None,
                }
            }
            // OP (R-type): pack, packh
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    // pack: funct3=100, funct7=0000100
                    (0b100, 0b0000100) => Some(Self::Pack { rd, rs1, rs2 }),
                    // packh: funct3=111, funct7=0000100
                    (0b111, 0b0000100) => Some(Self::Packh { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // OP-32 (R-type): packw
            0b0111011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    // packw: funct3=100, funct7=0000100, rs2 != 0
                    // rs2=0 collides with inherited RV64 Zbb zext.h and must fall through.
                    (0b100, 0b0000100) if rs2_bits != 0 => Some(Self::Packw { rd, rs1, rs2 }),
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
impl<Reg> fmt::Display for Rv64ZbkbInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pack { rd, rs1, rs2 } => write!(f, "pack {}, {}, {}", rd, rs1, rs2),
            Self::Packh { rd, rs1, rs2 } => write!(f, "packh {}, {}, {}", rd, rs1, rs2),
            Self::Packw { rd, rs1, rs2 } => write!(f, "packw {}, {}, {}", rd, rs1, rs2),
            Self::Brev8 { rd, rs1 } => write!(f, "brev8 {}, {}", rd, rs1),
        }
    }
}
