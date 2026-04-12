//! RV64 Zkne extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::rv64::zk::zkn::zknd::{Rv64ZkndInstruction, Rv64ZkndKsRnum};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zkne instructions
#[instruction(
    reorder = [Aes64Es, Aes64Esm, Aes64Ks1i, Aes64Ks2],
    ignore = [Rv64ZkndInstruction],
    inherit = [Rv64ZkndInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZkneInstruction<Reg> {
    /// AES final round encryption: ShiftRows + SubBytes, no MixColumns
    Aes64Es { rd: Reg, rs1: Reg, rs2: Reg },
    /// AES middle round encryption: ShiftRows + SubBytes + MixColumns
    Aes64Esm { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZkneInstruction<Reg>
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

        // R-type: OP opcode (0x33)
        //   aes64es:  funct7=0b0011001, funct3=0 → MATCH=0x32000033
        //   aes64esm: funct7=0b0011011, funct3=0 → MATCH=0x36000033
        match opcode {
            0b0110011 => {
                if funct3 != 0b000 {
                    None?;
                }
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match funct7 {
                    0b0011001 => Some(Self::Aes64Es { rd, rs1, rs2 }),
                    0b0011011 => Some(Self::Aes64Esm { rd, rs1, rs2 }),
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
impl<Reg> fmt::Display for Rv64ZkneInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aes64Es { rd, rs1, rs2 } => write!(f, "aes64es {rd}, {rs1}, {rs2}"),
            Self::Aes64Esm { rd, rs1, rs2 } => write!(f, "aes64esm {rd}, {rs1}, {rs2}"),
        }
    }
}
