//! RV64 Zknh extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zknh instruction (SHA-256 and SHA-512 sigma functions)
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZknhInstruction<Reg> {
    Sha256Sig0 { rd: Reg, rs1: Reg },
    Sha256Sig1 { rd: Reg, rs1: Reg },
    Sha256Sum0 { rd: Reg, rs1: Reg },
    Sha256Sum1 { rd: Reg, rs1: Reg },
    Sha512Sig0 { rd: Reg, rs1: Reg },
    Sha512Sig1 { rd: Reg, rs1: Reg },
    Sha512Sum0 { rd: Reg, rs1: Reg },
    Sha512Sum1 { rd: Reg, rs1: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZknhInstruction<Reg>
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
        let funct5 = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        match opcode {
            // I-type format (OP-IMM encoding)
            0b0010011 => {
                if funct3 != 0b001 || funct7 != 0b0001000 {
                    None
                } else {
                    let rd = Reg::from_bits(rd_bits)?;
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    match funct5 {
                        // SHA-256 instructions
                        0b00010 => Some(Self::Sha256Sig0 { rd, rs1 }),
                        0b00011 => Some(Self::Sha256Sig1 { rd, rs1 }),
                        0b00000 => Some(Self::Sha256Sum0 { rd, rs1 }),
                        0b00001 => Some(Self::Sha256Sum1 { rd, rs1 }),
                        // SHA-512 instructions
                        0b00110 => Some(Self::Sha512Sig0 { rd, rs1 }),
                        0b00111 => Some(Self::Sha512Sig1 { rd, rs1 }),
                        0b00100 => Some(Self::Sha512Sum0 { rd, rs1 }),
                        0b00101 => Some(Self::Sha512Sum1 { rd, rs1 }),
                        _ => None,
                    }
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

impl<Reg> fmt::Display for Rv64ZknhInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha256Sig0 { rd, rs1 } => write!(f, "sha256sig0 {}, {}", rd, rs1),
            Self::Sha256Sig1 { rd, rs1 } => write!(f, "sha256sig1 {}, {}", rd, rs1),
            Self::Sha256Sum0 { rd, rs1 } => write!(f, "sha256sum0 {}, {}", rd, rs1),
            Self::Sha256Sum1 { rd, rs1 } => write!(f, "sha256sum1 {}, {}", rd, rs1),
            Self::Sha512Sig0 { rd, rs1 } => write!(f, "sha512sig0 {}, {}", rd, rs1),
            Self::Sha512Sig1 { rd, rs1 } => write!(f, "sha512sig1 {}, {}", rd, rs1),
            Self::Sha512Sum0 { rd, rs1 } => write!(f, "sha512sum0 {}, {}", rd, rs1),
            Self::Sha512Sum1 { rd, rs1 } => write!(f, "sha512sum1 {}, {}", rd, rs1),
        }
    }
}
