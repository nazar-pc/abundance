//! RV32 Zknd extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// 2-bit byte-select immediate for RV32 AES instructions.
///
/// Selects which byte of `rs2` is fed into the S-box: `bs ∈ {0,1,2,3}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Rv32AesBs {
    B0 = 0,
    B1 = 1,
    B2 = 2,
    B3 = 3,
}

impl From<Rv32AesBs> for u8 {
    #[inline(always)]
    fn from(bs: Rv32AesBs) -> Self {
        bs as u8
    }
}

impl From<Rv32AesBs> for usize {
    #[inline(always)]
    fn from(bs: Rv32AesBs) -> Self {
        usize::from(bs as u8)
    }
}

impl fmt::Display for Rv32AesBs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&(*self as u8), f)
    }
}

impl Rv32AesBs {
    /// Create from raw 2-bit value. Returns `None` if `bits > 3`.
    #[inline(always)]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::B0),
            1 => Some(Self::B1),
            2 => Some(Self::B2),
            3 => Some(Self::B3),
            _ => None,
        }
    }
}

/// RISC-V RV32 Zknd instructions (AES decryption)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZkndInstruction<Reg> {
    /// AES final round decryption step: InvSubBytes on one byte of rs2,
    /// rotated to the byte lane selected by bs, XOR'd into rs1.
    ///
    /// `rd = rs1 ^ rol32(INV_SBOX[(rs2 >> (bs*8)) & 0xff] as u32, bs*8)`
    Aes32Dsi {
        rd: Reg,
        rs1: Reg,
        rs2: Reg,
        bs: Rv32AesBs,
    },
    /// AES middle round decryption step: InvSubBytes + partial InvMixColumns
    /// on one byte of rs2, rotated to the byte lane selected by bs, XOR'd into rs1.
    ///
    /// `rd = rs1 ^ rol32(InvMixColByte(INV_SBOX[(rs2 >> (bs*8)) & 0xff]), bs*8)`
    Aes32Dsmi {
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
/// [29:25] funct5   — 0b10101 (aes32dsi) / 0b10111 (aes32dsmi)
/// [24:20] rs2
/// [19:15] rs1
/// [14:12] funct3   — 0b000
/// [11:7]  rd
/// [6:0]   opcode   — 0b0110011 (OP)
/// ```
///
/// Ratified match/mask values (from riscv-opcodes):
///   MATCH_AES32DSI  = 0x2a000033, MASK_AES32DSI  = 0x3e00707f
///   MATCH_AES32DSMI = 0x2e000033, MASK_AES32DSMI = 0x3e00707f
///
/// `rd` and `rs1` are independent fields. The assembler convention places
/// the accumulator in both rd and rs1 (the `rt` pattern), but the hardware
/// does not require rd == rs1 and the decoder must not enforce it.
#[instruction]
impl<Reg> const Instruction for Rv32ZkndInstruction<Reg>
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
            // aes32dsi:  bs[31:30] | 0b10101[29:25]
            0b10101 => Some(Self::Aes32Dsi { rd, rs1, rs2, bs }),
            // aes32dsmi: bs[31:30] | 0b10111[29:25]
            0b10111 => Some(Self::Aes32Dsmi { rd, rs1, rs2, bs }),
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

impl<Reg> fmt::Display for Rv32ZkndInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aes32Dsi { rd, rs1, rs2, bs } => {
                write!(f, "aes32dsi {rd}, {rs1}, {rs2}, {bs}")
            }
            Self::Aes32Dsmi { rd, rs1, rs2, bs } => {
                write!(f, "aes32dsmi {rd}, {rs1}, {rs2}, {bs}")
            }
        }
    }
}
