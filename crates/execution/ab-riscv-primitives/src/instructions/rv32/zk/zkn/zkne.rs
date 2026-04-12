//! RV32 Zkne extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::rv32::zk::zkn::zknd::Rv32AesBs;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zkne instructions (AES encryption)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZkneInstruction<Reg> {
    /// AES final round encryption step: SubBytes on one byte of rs2, rotated to the byte lane
    /// selected by bs, XOR'd into rs1.
    ///
    /// `rd = rs1 ^ rol32(SBOX[(rs2 >> (bs*8)) & 0xff] as u32, bs*8)`
    Aes32Esi {
        rd: Reg,
        rs1: Reg,
        rs2: Reg,
        bs: Rv32AesBs,
    },
    /// AES middle round encryption step: SubBytes + partial MixColumns on one byte of rs2, rotated
    /// to the byte lane selected by bs, XOR'd into rs1.
    ///
    /// `rd = rs1 ^ rol32(MixColByte(SBOX[(rs2 >> (bs*8)) & 0xff]), bs*8)`
    Aes32Esmi {
        rd: Reg,
        rs1: Reg,
        rs2: Reg,
        bs: Rv32AesBs,
    },
}

/// Encoding layout (R-type, opcode 0x33, funct3 0x0):
///
/// ```text
/// [31:30] bs       — 2-bit byte select
/// [29:25] funct5   — 0b10001 (aes32esi) / 0b10011 (aes32esmi)
/// [24:20] rs2
/// [19:15] rs1
/// [14:12] funct3   — 0b000
/// [11:7]  rd
/// [6:0]   opcode   — 0b0110011 (OP)
/// ```
///
/// Ratified match/mask values (from riscv-opcodes):
///   MATCH_AES32ESI  = 0x22000033, MASK_AES32ESI  = 0x3e00707f
///   MATCH_AES32ESMI = 0x26000033, MASK_AES32ESMI = 0x3e00707f
///
/// `rd` and `rs1` are independent fields. The assembler convention places
/// the accumulator in both rd and rs1 (the `rt` pattern), but the hardware
/// does not require rd == rs1 and the decoder must not enforce it.
#[instruction]
impl<Reg> const Instruction for Rv32ZkneInstruction<Reg>
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
        let funct5 = ((instruction >> 25) & 0b1_1111) as u8;
        let bs_bits = ((instruction >> 30) & 0b11) as u8;

        // R-type OP opcode only
        if opcode != 0b0110011 {
            None?;
        }
        if funct3 != 0b000 {
            None?;
        }

        let rd = Reg::from_bits(rd_bits)?;
        let rs1 = Reg::from_bits(rs1_bits)?;
        let rs2 = Reg::from_bits(rs2_bits)?;
        let bs = Rv32AesBs::from_bits(bs_bits)?;

        match funct5 {
            // aes32esi:  bs[31:30] | 0b10001[29:25]
            0b10001 => Some(Self::Aes32Esi { rd, rs1, rs2, bs }),
            // aes32esmi: bs[31:30] | 0b10011[29:25]
            0b10011 => Some(Self::Aes32Esmi { rd, rs1, rs2, bs }),
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

impl<Reg> fmt::Display for Rv32ZkneInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aes32Esi { rd, rs1, rs2, bs } => {
                write!(f, "aes32esi {rd}, {rs1}, {rs2}, {bs}")
            }
            Self::Aes32Esmi { rd, rs1, rs2, bs } => {
                write!(f, "aes32esmi {rd}, {rs1}, {rs2}, {bs}")
            }
        }
    }
}
