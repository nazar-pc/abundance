//! RV64 Zicsr extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zicsr instruction (Control and Status Register)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZicsrInstruction<Reg> {
    Csrrw { rd: Reg, rs1: Reg, csr: u16 },
    Csrrs { rd: Reg, rs1: Reg, csr: u16 },
    Csrrc { rd: Reg, rs1: Reg, csr: u16 },
    Csrrwi { rd: Reg, zimm: u8, csr: u16 },
    Csrrsi { rd: Reg, zimm: u8, csr: u16 },
    Csrrci { rd: Reg, zimm: u8, csr: u16 },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZicsrInstruction<Reg>
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
        let csr_bits = ((instruction >> 20) & 0x0fff) as u16;

        match opcode {
            0b1110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                match funct3 {
                    0b001 => {
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        Some(Self::Csrrw {
                            rd,
                            rs1,
                            csr: csr_bits,
                        })
                    }
                    0b010 => {
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        Some(Self::Csrrs {
                            rd,
                            rs1,
                            csr: csr_bits,
                        })
                    }
                    0b011 => {
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        Some(Self::Csrrc {
                            rd,
                            rs1,
                            csr: csr_bits,
                        })
                    }
                    0b101 => Some(Self::Csrrwi {
                        rd,
                        zimm: rs1_bits,
                        csr: csr_bits,
                    }),
                    0b110 => Some(Self::Csrrsi {
                        rd,
                        zimm: rs1_bits,
                        csr: csr_bits,
                    }),
                    0b111 => Some(Self::Csrrci {
                        rd,
                        zimm: rs1_bits,
                        csr: csr_bits,
                    }),
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

impl<Reg> fmt::Display for Rv64ZicsrInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csrrw { rd, rs1, csr } => write!(f, "csrrw {rd}, {csr}, {rs1}"),
            Self::Csrrs { rd, rs1, csr } => write!(f, "csrrs {rd}, {csr}, {rs1}"),
            Self::Csrrc { rd, rs1, csr } => write!(f, "csrrc {rd}, {csr}, {rs1}"),
            Self::Csrrwi { rd, zimm, csr } => write!(f, "csrrwi {rd}, {csr}, {zimm}"),
            Self::Csrrsi { rd, zimm, csr } => write!(f, "csrrsi {rd}, {csr}, {zimm}"),
            Self::Csrrci { rd, zimm, csr } => write!(f, "csrrci {rd}, {csr}, {zimm}"),
        }
    }
}
