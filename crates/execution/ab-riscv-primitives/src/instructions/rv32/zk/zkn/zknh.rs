//! RV32 Zknh extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zknh instruction (SHA-256 and SHA-512 sigma/sum functions).
///
/// SHA-256 instructions take a single source register.
/// SHA-512 instructions take two source registers because 64-bit operands must be split across two
/// 32-bit registers on RV32. The register conventions differ by instruction and follow the RISC-V
/// scalar crypto Sail model exactly:
///
/// - `sha512sig0l`, `sha512sig1l`: rs1 = LOW word, rs2 = HIGH word
/// - `sha512sig0h`, `sha512sig1h`: rs1 = HIGH word, rs2 = LOW word
/// - `sha512sum0r`, `sha512sum1r`: rs1 = LOW word, rs2 = HIGH word
///
/// For `sha512sum0r` and `sha512sum1r` the Sail pseudocode builds the 64-bit operand as
/// `x[63:32] = X(rs2), x[31:0] = X(rs1)` (rs2 is the HIGH half) and writes the low 32 bits of the
/// result to `rd`.
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZknhInstruction<Reg> {
    // SHA-256 (single-register, identical encoding to RV64)
    Sha256Sig0 { rd: Reg, rs1: Reg },
    Sha256Sig1 { rd: Reg, rs1: Reg },
    Sha256Sum0 { rd: Reg, rs1: Reg },
    Sha256Sum1 { rd: Reg, rs1: Reg },
    // SHA-512 (two-register, RV32-only R-type)
    Sha512Sig0h { rd: Reg, rs1: Reg, rs2: Reg },
    Sha512Sig0l { rd: Reg, rs1: Reg, rs2: Reg },
    Sha512Sig1h { rd: Reg, rs1: Reg, rs2: Reg },
    Sha512Sig1l { rd: Reg, rs1: Reg, rs2: Reg },
    Sha512Sum0r { rd: Reg, rs1: Reg, rs2: Reg },
    Sha512Sum1r { rd: Reg, rs1: Reg, rs2: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv32ZknhInstruction<Reg>
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
        // Same field as rs2 for I-type
        let funct5 = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        match opcode {
            // SHA-256: I-type format (OP-IMM)
            0b001_0011 => {
                if funct3 != 0b001 || funct7 != 0b000_1000 {
                    None
                } else {
                    let rd = Reg::from_bits(rd_bits)?;
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    match funct5 {
                        0b0_0010 => Some(Self::Sha256Sig0 { rd, rs1 }),
                        0b0_0011 => Some(Self::Sha256Sig1 { rd, rs1 }),
                        0b0_0000 => Some(Self::Sha256Sum0 { rd, rs1 }),
                        0b0_0001 => Some(Self::Sha256Sum1 { rd, rs1 }),
                        _ => None,
                    }
                }
            }
            // SHA-512: R-type format (OP)
            // RV32-only two-register instructions.
            0b011_0011 => {
                if funct3 == 0b000 {
                    let rd = Reg::from_bits(rd_bits)?;
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    let rs2 = Reg::from_bits(rs2_bits)?;
                    match funct7 {
                        // 0b010_1000 = 40
                        0b010_1000 => Some(Self::Sha512Sum0r { rd, rs1, rs2 }),
                        // 0b010_1001 = 41
                        0b010_1001 => Some(Self::Sha512Sum1r { rd, rs1, rs2 }),
                        // 0b010_1010 = 42
                        0b010_1010 => Some(Self::Sha512Sig0l { rd, rs1, rs2 }),
                        // 0b010_1011 = 43
                        0b010_1011 => Some(Self::Sha512Sig1l { rd, rs1, rs2 }),
                        // 0b010_1110 = 46
                        0b010_1110 => Some(Self::Sha512Sig0h { rd, rs1, rs2 }),
                        // 0b010_1111 = 47
                        0b010_1111 => Some(Self::Sha512Sig1h { rd, rs1, rs2 }),
                        _ => None,
                    }
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
impl<Reg> fmt::Display for Rv32ZknhInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha256Sig0 { rd, rs1 } => write!(f, "sha256sig0 {rd}, {rs1}"),
            Self::Sha256Sig1 { rd, rs1 } => write!(f, "sha256sig1 {rd}, {rs1}"),
            Self::Sha256Sum0 { rd, rs1 } => write!(f, "sha256sum0 {rd}, {rs1}"),
            Self::Sha256Sum1 { rd, rs1 } => write!(f, "sha256sum1 {rd}, {rs1}"),
            Self::Sha512Sig0h { rd, rs1, rs2 } => write!(f, "sha512sig0h {rd}, {rs1}, {rs2}"),
            Self::Sha512Sig0l { rd, rs1, rs2 } => write!(f, "sha512sig0l {rd}, {rs1}, {rs2}"),
            Self::Sha512Sig1h { rd, rs1, rs2 } => write!(f, "sha512sig1h {rd}, {rs1}, {rs2}"),
            Self::Sha512Sig1l { rd, rs1, rs2 } => write!(f, "sha512sig1l {rd}, {rs1}, {rs2}"),
            Self::Sha512Sum0r { rd, rs1, rs2 } => write!(f, "sha512sum0r {rd}, {rs1}, {rs2}"),
            Self::Sha512Sum1r { rd, rs1, rs2 } => write!(f, "sha512sum1r {rd}, {rs1}, {rs2}"),
        }
    }
}
