//! RV64 Zknd extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::{fmt, mem};

/// AES key schedule round constant number
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Rv64ZkndKsRnum {
    R0 = 0x0,
    R1 = 0x1,
    R2 = 0x2,
    R3 = 0x3,
    R4 = 0x4,
    R5 = 0x5,
    R6 = 0x6,
    R7 = 0x7,
    R8 = 0x8,
    R9 = 0x9,
    Final = 0xA,
}

impl const From<Rv64ZkndKsRnum> for u8 {
    #[inline(always)]
    fn from(rnum: Rv64ZkndKsRnum) -> Self {
        rnum as u8
    }
}

impl const From<Rv64ZkndKsRnum> for usize {
    #[inline(always)]
    fn from(rnum: Rv64ZkndKsRnum) -> Self {
        usize::from(rnum as u8)
    }
}

impl fmt::Display for Rv64ZkndKsRnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&(*self as u8), f)
    }
}

impl Rv64ZkndKsRnum {
    /// Round constants `RC[0..=9]`, indexed by rnum (0-based).
    /// `RC[rnum]` corresponds to FIPS 197 `Rcon[rnum+1]`.
    const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

    /// Create from raw bits
    #[inline(always)]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits <= Rv64ZkndKsRnum::Final as u8 {
            // SAFETY: The transmute is safe because `Rv64ZkndKsRnum` is `#[repr(u8)]` enum with
            // known valid values
            Some(unsafe { mem::transmute::<u8, Self>(bits) })
        } else {
            None
        }
    }

    /// Round constant (unless final)
    #[inline(always)]
    pub const fn constant(self) -> Option<u8> {
        if matches!(self, Rv64ZkndKsRnum::Final) {
            None
        } else {
            Some(Self::RCON[usize::from(self)])
        }
    }
}

/// RISC-V RV64 Zknd instructions (AES decryption and key schedule)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZkndInstruction<Reg> {
    /// AES final round decryption: InvShiftRows + InvSubBytes, no MixColumns
    Aes64Ds { rd: Reg, rs1: Reg, rs2: Reg },
    /// AES middle round decryption: InvShiftRows + InvSubBytes + InvMixColumns
    Aes64Dsm { rd: Reg, rs1: Reg, rs2: Reg },
    /// AES inverse MixColumns on each 32-bit word of rs1
    Aes64Im { rd: Reg, rs1: Reg },
    /// AES key schedule step 1 (rnum in 0..=10)
    Aes64Ks1i {
        rd: Reg,
        rs1: Reg,
        rnum: Rv64ZkndKsRnum,
    },
    /// AES key schedule step 2
    Aes64Ks2 { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZkndInstruction<Reg>
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

        match opcode {
            // R-type: OP opcode (0x33)
            //   aes64ds:  funct7=0b0011101, funct3=0 -> MATCH=0x3a000033
            //   aes64dsm: funct7=0b0011111, funct3=0 -> MATCH=0x3e000033
            //   aes64ks2: funct7=0b0111111, funct3=0 -> MATCH=0x7e000033
            0b0110011 => {
                if funct3 != 0b000 {
                    None?;
                }
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match funct7 {
                    0b0011101 => Some(Self::Aes64Ds { rd, rs1, rs2 }),
                    0b0011111 => Some(Self::Aes64Dsm { rd, rs1, rs2 }),
                    0b0111111 => Some(Self::Aes64Ks2 { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // I-type: OP-IMM opcode (0x13), funct3=0b001
            //   aes64im:   imm[11:0]=0x300  (funct7=0b0011000, rs2=0b00000) -> MATCH=0x30001013
            //   aes64ks1i: imm[11:5]=0b0011000, imm[4]=1, imm[3:0]=rnum     -> MATCH=0x31001013+
            0b0010011 => {
                if funct3 != 0b001 {
                    None?;
                }
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let imm12 = instruction >> 20;
                if imm12 == 0x300 {
                    Some(Self::Aes64Im { rd, rs1 })
                } else if (imm12 >> 5) == 0b0011000 && (imm12 & 0b1_0000) != 0 {
                    // bits[11:5]=0b0011000, bit[4]=1, bits[3:0]=rnum
                    let rnum = Rv64ZkndKsRnum::from_bits((imm12 & 0xf) as u8)?;
                    Some(Self::Aes64Ks1i { rd, rs1, rnum })
                } else {
                    None
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
impl<Reg> fmt::Display for Rv64ZkndInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aes64Ds { rd, rs1, rs2 } => write!(f, "aes64ds {rd}, {rs1}, {rs2}"),
            Self::Aes64Dsm { rd, rs1, rs2 } => write!(f, "aes64dsm {rd}, {rs1}, {rs2}"),
            Self::Aes64Im { rd, rs1 } => write!(f, "aes64im {rd}, {rs1}"),
            Self::Aes64Ks1i { rd, rs1, rnum } => write!(f, "aes64ks1i {rd}, {rs1}, {rnum}"),
            Self::Aes64Ks2 { rd, rs1, rs2 } => write!(f, "aes64ks2 {rd}, {rs1}, {rs2}"),
        }
    }
}
