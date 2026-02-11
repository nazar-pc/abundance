//! RV64 Zve64x configuration instructions

#[cfg(test)]
mod tests;

use crate::instruction::Instruction;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x configuration instruction.
///
/// These instructions set the vector type (`vtype`) and vector length (`vl`) registers. They use
/// the OP-V major opcode (0b1010111) with funct3=0b111 (OPCFG).
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xConfigInstruction<Reg> {
    /// Set vector length and type from GPR
    ///
    /// `vsetvli rd, rs1, vtypei`
    /// rd = new vl, rs1 = AVL, vtypei = new vtype setting (11-bit immediate)
    Vsetvli { rd: Reg, rs1: Reg, vtypei: u16 },
    /// Set vector length and type from immediate AVL
    ///
    /// `vsetivli rd, uimm, vtypei`
    /// rd = new vl, uimm\[4:0] = AVL, vtypei = new vtype setting (10-bit immediate)
    Vsetivli { rd: Reg, uimm: u8, vtypei: u16 },
    /// Set vector length and type from GPRs
    ///
    /// `vsetvl rd, rs1, rs2`
    /// rd = new vl, rs1 = AVL, rs2 = new vtype value
    Vsetvl { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xConfigInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;

        // OP-V major opcode
        if opcode != 0b1010111 {
            None?;
        }

        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let bit31 = (instruction >> 31) & 1;
        let bit30 = (instruction >> 30) & 1;

        // OPCFG: funct3 = 0b111
        if funct3 != 0b111 {
            None?;
        }

        let rd = Reg::from_bits(rd_bits)?;

        match bit31 {
            // vsetvli: bit31=0
            // [0|zimm[10:0]|rs1|111|rd|1010111]
            0 => {
                let rs1 = Reg::from_bits(rs1_bits)?;
                let vtypei = ((instruction >> 20) & 0x7ff) as u16;
                Some(Self::Vsetvli { rd, rs1, vtypei })
            }
            // bit31=1: vsetivli or vsetvl
            _ => match bit30 {
                // vsetivli: bits[31:30]=11
                // [11|zimm[9:0]|uimm[4:0]|111|rd|1010111]
                1 => {
                    let uimm = rs1_bits;
                    let vtypei = ((instruction >> 20) & 0x3ff) as u16;
                    Some(Self::Vsetivli { rd, uimm, vtypei })
                }
                // vsetvl: bit31=1, bit30=0
                // [1000000|rs2|rs1|111|rd|1010111]
                _ => {
                    // bits[29:25] must be 0b00000
                    let bits_29_25 = ((instruction >> 25) & 0b1_1111) as u8;
                    if bits_29_25 != 0 {
                        None?;
                    }
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
                    let rs2 = Reg::from_bits(rs2_bits)?;
                    Some(Self::Vsetvl { rd, rs1, rs2 })
                }
            },
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        size_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

impl<Reg> fmt::Display for Rv64Zve64xConfigInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::Vsetvli { rd, rs1, vtypei } => write!(f, "vsetvli {rd}, {rs1}, {vtypei}"),
            Self::Vsetivli { rd, uimm, vtypei } => write!(f, "vsetivli {rd}, {uimm}, {vtypei}"),
            Self::Vsetvl { rd, rs1, rs2 } => write!(f, "vsetvl {rd}, {rs1}, {rs2}"),
        }
    }
}
