//! RV64 Zkne extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::rv64::zk::zkn::zknd::{Rv64ZkndKsRnum, Rv64ZkndZkneSharedInstruction};
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zkne instructions
#[instruction(
    reorder = [Aes64Es, Aes64Esm],
    inherit = [Rv64ZkndZkneSharedInstruction],
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv64ZkneInstruction<Hart>
where
    Hart: HartConfig,
{
    /// AES final round encryption: ShiftRows + SubBytes, no MixColumns
    Aes64Es { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
    /// AES middle round encryption: ShiftRows + SubBytes + MixColumns
    Aes64Esm { rd: Hart::Reg, rs1: Hart::Reg, rs2: Hart::Reg },
}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZkneInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zkne", 1, 0)];

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

        // R-type: OP opcode (0x33)
        //   aes64es:  funct7=0b001_1001, funct3=0 -> MATCH=0x3200_0033
        //   aes64esm: funct7=0b001_1011, funct3=0 -> MATCH=0x3600_0033
        match opcode {
            0b011_0011 => {
                if funct3 != 0b000 {
                    None?;
                }
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match funct7 {
                    0b001_1001 => Some(Self::Aes64Es { rd, rs1, rs2 }),
                    0b001_1011 => Some(Self::Aes64Esm { rd, rs1, rs2 }),
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
impl<Reg, Hart> fmt::Display for Rv64ZkneInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aes64Es { rd: rd1, rs1, rs2 } => write!(f, "aes64es {rd1}, {rs1}, {rs2}"),
            Self::Aes64Esm { rd, rs1, rs2 } => write!(f, "aes64esm {rd}, {rs1}, {rs2}"),
        }
    }
}
