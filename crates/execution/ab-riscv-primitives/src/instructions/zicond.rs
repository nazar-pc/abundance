//! Zicond extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V Zicond instruction (Integer Conditional Operations)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZicondInstruction<Reg> {
    /// `czero.eqz rd, rs1, rs2` — move zero to `rd` if `rs2 == 0`, else move `rs1`
    CzeroEqz { rd: Reg, rs1: Reg, rs2: Reg },
    /// `czero.nez rd, rs1, rs2` — move zero to `rd` if `rs2 != 0`, else move `rs1`
    CzeroNez { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
impl<Reg> const Instruction for ZicondInstruction<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0x7f) as u8;

        // Both Zicond instructions share opcode=0x33 (OP) and funct7=0x07
        match (opcode, funct7) {
            (0b011_0011, 0b000_0111) => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match funct3 {
                    0b101 => Some(Self::CzeroEqz { rd, rs1, rs2 }),
                    0b111 => Some(Self::CzeroNez { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            _ => None,
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

impl<Reg> fmt::Display for ZicondInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CzeroEqz { rd, rs1, rs2 } => write!(f, "czero.eqz {rd}, {rs1}, {rs2}"),
            Self::CzeroNez { rd, rs1, rs2 } => write!(f, "czero.nez {rd}, {rs1}, {rs2}"),
        }
    }
}
