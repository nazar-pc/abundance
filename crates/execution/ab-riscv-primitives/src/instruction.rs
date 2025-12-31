//! This module defines the RISC-V instruction set for the RV64 architecture

#[cfg(test)]
mod tests;

use crate::registers::GenericRegister;
use core::fmt;

/// Generic instruction
pub const trait GenericInstruction: fmt::Display + fmt::Debug + Copy + Sized {
    /// Decode a single instruction
    fn decode(instruction: u32) -> Self;

    /// Instruction size in bytes
    fn size(&self) -> usize;
}

// TODO: Composable instruction via nested extensions?
/// RISC-V RV64 instruction.
///
/// Usage of RV64I or RV64E variant is defined by the register generic used.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rv64Instruction<Reg> {
    // R-type
    Add { rd: Reg, rs1: Reg, rs2: Reg },
    Sub { rd: Reg, rs1: Reg, rs2: Reg },
    Sll { rd: Reg, rs1: Reg, rs2: Reg },
    Slt { rd: Reg, rs1: Reg, rs2: Reg },
    Sltu { rd: Reg, rs1: Reg, rs2: Reg },
    Xor { rd: Reg, rs1: Reg, rs2: Reg },
    Srl { rd: Reg, rs1: Reg, rs2: Reg },
    Sra { rd: Reg, rs1: Reg, rs2: Reg },
    Or { rd: Reg, rs1: Reg, rs2: Reg },
    And { rd: Reg, rs1: Reg, rs2: Reg },

    // M extension
    Mul { rd: Reg, rs1: Reg, rs2: Reg },
    Mulh { rd: Reg, rs1: Reg, rs2: Reg },
    Mulhsu { rd: Reg, rs1: Reg, rs2: Reg },
    Mulhu { rd: Reg, rs1: Reg, rs2: Reg },
    Div { rd: Reg, rs1: Reg, rs2: Reg },
    Divu { rd: Reg, rs1: Reg, rs2: Reg },
    Rem { rd: Reg, rs1: Reg, rs2: Reg },
    Remu { rd: Reg, rs1: Reg, rs2: Reg },

    // RV64 R-type W
    Addw { rd: Reg, rs1: Reg, rs2: Reg },
    Subw { rd: Reg, rs1: Reg, rs2: Reg },
    Sllw { rd: Reg, rs1: Reg, rs2: Reg },
    Srlw { rd: Reg, rs1: Reg, rs2: Reg },
    Sraw { rd: Reg, rs1: Reg, rs2: Reg },
    Mulw { rd: Reg, rs1: Reg, rs2: Reg },
    Divw { rd: Reg, rs1: Reg, rs2: Reg },
    Divuw { rd: Reg, rs1: Reg, rs2: Reg },
    Remw { rd: Reg, rs1: Reg, rs2: Reg },
    Remuw { rd: Reg, rs1: Reg, rs2: Reg },

    // I-type
    Addi { rd: Reg, rs1: Reg, imm: i32 },
    Slti { rd: Reg, rs1: Reg, imm: i32 },
    Sltiu { rd: Reg, rs1: Reg, imm: i32 },
    Xori { rd: Reg, rs1: Reg, imm: i32 },
    Ori { rd: Reg, rs1: Reg, imm: i32 },
    Andi { rd: Reg, rs1: Reg, imm: i32 },
    Slli { rd: Reg, rs1: Reg, shamt: u32 },
    Srli { rd: Reg, rs1: Reg, shamt: u32 },
    Srai { rd: Reg, rs1: Reg, shamt: u32 },

    // RV64 I-type W
    Addiw { rd: Reg, rs1: Reg, imm: i32 },
    Slliw { rd: Reg, rs1: Reg, shamt: u32 },
    Srliw { rd: Reg, rs1: Reg, shamt: u32 },
    Sraiw { rd: Reg, rs1: Reg, shamt: u32 },

    // Loads (I-type)
    Lb { rd: Reg, rs1: Reg, imm: i32 },
    Lh { rd: Reg, rs1: Reg, imm: i32 },
    Lw { rd: Reg, rs1: Reg, imm: i32 },
    Ld { rd: Reg, rs1: Reg, imm: i32 },
    Lbu { rd: Reg, rs1: Reg, imm: i32 },
    Lhu { rd: Reg, rs1: Reg, imm: i32 },
    Lwu { rd: Reg, rs1: Reg, imm: i32 },

    // Jalr (I-type)
    Jalr { rd: Reg, rs1: Reg, imm: i32 },

    // S-type
    Sb { rs1: Reg, rs2: Reg, imm: i32 },
    Sh { rs1: Reg, rs2: Reg, imm: i32 },
    Sw { rs1: Reg, rs2: Reg, imm: i32 },
    Sd { rs1: Reg, rs2: Reg, imm: i32 },

    // B-type
    Beq { rs1: Reg, rs2: Reg, imm: i32 },
    Bne { rs1: Reg, rs2: Reg, imm: i32 },
    Blt { rs1: Reg, rs2: Reg, imm: i32 },
    Bge { rs1: Reg, rs2: Reg, imm: i32 },
    Bltu { rs1: Reg, rs2: Reg, imm: i32 },
    Bgeu { rs1: Reg, rs2: Reg, imm: i32 },

    // Lui (U-type)
    Lui { rd: Reg, imm: i32 },

    // Auipc (U-type)
    Auipc { rd: Reg, imm: i32 },

    // Jal (J-type)
    Jal { rd: Reg, imm: i32 },

    // Fence (I-type like, simplified for EM)
    Fence { pred: u8, succ: u8, fm: u8 },

    // System instructions
    Ecall,
    Ebreak,

    // Unimplemented/illegal
    Unimp,

    // Invalid instruction
    Invalid(u32),
}

impl<Reg> const GenericInstruction for Rv64Instruction<Reg>
where
    Reg: [const] GenericRegister,
{
    fn decode(instruction: u32) -> Self {
        Self::decode_internal(instruction).unwrap_or(Self::Invalid(instruction))
    }

    fn size(&self) -> usize {
        size_of::<u32>()
    }
}

impl<Reg> Rv64Instruction<Reg> {
    const fn decode_internal(instruction: u32) -> Option<Self>
    where
        Reg: [const] GenericRegister,
    {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        Some(match opcode {
            // R-type
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b000, 0b0000000) => Self::Add { rd, rs1, rs2 },
                    (0b000, 0b0100000) => Self::Sub { rd, rs1, rs2 },
                    (0b001, 0b0000000) => Self::Sll { rd, rs1, rs2 },
                    (0b010, 0b0000000) => Self::Slt { rd, rs1, rs2 },
                    (0b011, 0b0000000) => Self::Sltu { rd, rs1, rs2 },
                    (0b100, 0b0000000) => Self::Xor { rd, rs1, rs2 },
                    (0b101, 0b0000000) => Self::Srl { rd, rs1, rs2 },
                    (0b101, 0b0100000) => Self::Sra { rd, rs1, rs2 },
                    (0b110, 0b0000000) => Self::Or { rd, rs1, rs2 },
                    (0b111, 0b0000000) => Self::And { rd, rs1, rs2 },
                    // M extension
                    (0b000, 0b0000001) => Self::Mul { rd, rs1, rs2 },
                    (0b001, 0b0000001) => Self::Mulh { rd, rs1, rs2 },
                    (0b010, 0b0000001) => Self::Mulhsu { rd, rs1, rs2 },
                    (0b011, 0b0000001) => Self::Mulhu { rd, rs1, rs2 },
                    (0b100, 0b0000001) => Self::Div { rd, rs1, rs2 },
                    (0b101, 0b0000001) => Self::Divu { rd, rs1, rs2 },
                    (0b110, 0b0000001) => Self::Rem { rd, rs1, rs2 },
                    (0b111, 0b0000001) => Self::Remu { rd, rs1, rs2 },
                    _ => Self::Invalid(instruction),
                }
            }
            // RV64 R-type W
            0b0111011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b000, 0b0000000) => Self::Addw { rd, rs1, rs2 },
                    (0b000, 0b0100000) => Self::Subw { rd, rs1, rs2 },
                    (0b001, 0b0000000) => Self::Sllw { rd, rs1, rs2 },
                    (0b101, 0b0000000) => Self::Srlw { rd, rs1, rs2 },
                    (0b101, 0b0100000) => Self::Sraw { rd, rs1, rs2 },
                    // M extension W
                    (0b000, 0b0000001) => Self::Mulw { rd, rs1, rs2 },
                    (0b100, 0b0000001) => Self::Divw { rd, rs1, rs2 },
                    (0b101, 0b0000001) => Self::Divuw { rd, rs1, rs2 },
                    (0b110, 0b0000001) => Self::Remw { rd, rs1, rs2 },
                    (0b111, 0b0000001) => Self::Remuw { rd, rs1, rs2 },
                    _ => Self::Invalid(instruction),
                }
            }
            // I-type
            0b0010011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let imm = instruction.cast_signed() >> 20;
                match funct3 {
                    0b000 => Self::Addi { rd, rs1, imm },
                    0b010 => Self::Slti { rd, rs1, imm },
                    0b011 => Self::Sltiu { rd, rs1, imm },
                    0b100 => Self::Xori { rd, rs1, imm },
                    0b110 => Self::Ori { rd, rs1, imm },
                    0b111 => Self::Andi { rd, rs1, imm },
                    0b001 => {
                        let shamt = (instruction >> 20) & 0b11_1111;
                        let funct6 = (instruction >> 26) & 0b11_1111;
                        if funct6 == 0b000000 {
                            Self::Slli { rd, rs1, shamt }
                        } else {
                            Self::Invalid(instruction)
                        }
                    }
                    0b101 => {
                        let shamt = (instruction >> 20) & 0b11_1111;
                        let funct6 = (instruction >> 26) & 0b11_1111;
                        match funct6 {
                            0b000000 => Self::Srli { rd, rs1, shamt },
                            0b010000 => Self::Srai { rd, rs1, shamt },
                            _ => Self::Invalid(instruction),
                        }
                    }
                    _ => Self::Invalid(instruction),
                }
            }
            // RV64 I-type W
            0b0011011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let imm = instruction.cast_signed() >> 20;
                let shamt = (instruction >> 20) & 0b1_1111; // 5-bit for W shifts
                match funct3 {
                    0b000 => Self::Addiw { rd, rs1, imm },
                    0b001 => {
                        if funct7 == 0b0000000 {
                            Self::Slliw { rd, rs1, shamt }
                        } else {
                            Self::Invalid(instruction)
                        }
                    }
                    0b101 => match funct7 {
                        0b0000000 => Self::Srliw { rd, rs1, shamt },
                        0b0100000 => Self::Sraiw { rd, rs1, shamt },
                        _ => Self::Invalid(instruction),
                    },
                    _ => Self::Invalid(instruction),
                }
            }
            // Loads (I-type)
            0b0000011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let imm = instruction.cast_signed() >> 20;
                match funct3 {
                    0b000 => Self::Lb { rd, rs1, imm },
                    0b001 => Self::Lh { rd, rs1, imm },
                    0b010 => Self::Lw { rd, rs1, imm },
                    0b011 => Self::Ld { rd, rs1, imm },
                    0b100 => Self::Lbu { rd, rs1, imm },
                    0b101 => Self::Lhu { rd, rs1, imm },
                    0b110 => Self::Lwu { rd, rs1, imm },
                    _ => Self::Invalid(instruction),
                }
            }
            // Jalr (I-type)
            0b1100111 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                if funct3 == 0b000 {
                    let imm = instruction.cast_signed() >> 20;
                    Self::Jalr { rd, rs1, imm }
                } else {
                    Self::Invalid(instruction)
                }
            }
            // S-type
            0b0100011 => {
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                let imm11_5 = ((instruction >> 25) & 0b111_1111).cast_signed();
                let imm4_0 = ((instruction >> 7) & 0b1_1111).cast_signed();
                let imm = (imm11_5 << 5) | imm4_0;
                // Sign extend
                let imm = (imm << 20) >> 20;
                match funct3 {
                    0b000 => Self::Sb { rs1, rs2, imm },
                    0b001 => Self::Sh { rs1, rs2, imm },
                    0b010 => Self::Sw { rs1, rs2, imm },
                    0b011 => Self::Sd { rs1, rs2, imm },
                    _ => Self::Invalid(instruction),
                }
            }
            // B-type
            0b1100011 => {
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                let imm12 = ((instruction >> 31) & 1).cast_signed();
                let imm10_5 = ((instruction >> 25) & 0b11_1111).cast_signed();
                let imm4_1 = ((instruction >> 8) & 0b1111).cast_signed();
                let imm11 = ((instruction >> 7) & 1).cast_signed();
                let imm = (imm12 << 12) | (imm11 << 11) | (imm10_5 << 5) | (imm4_1 << 1);
                // Sign extend
                let imm = (imm << 19) >> 19;
                match funct3 {
                    0b000 => Self::Beq { rs1, rs2, imm },
                    0b001 => Self::Bne { rs1, rs2, imm },
                    0b100 => Self::Blt { rs1, rs2, imm },
                    0b101 => Self::Bge { rs1, rs2, imm },
                    0b110 => Self::Bltu { rs1, rs2, imm },
                    0b111 => Self::Bgeu { rs1, rs2, imm },
                    _ => Self::Invalid(instruction),
                }
            }
            // Lui (U-type)
            0b0110111 => {
                let rd = Reg::from_bits(rd_bits)?;
                let imm = (instruction & 0xffff_f000).cast_signed();
                Self::Lui { rd, imm }
            }
            // Auipc (U-type)
            0b0010111 => {
                let rd = Reg::from_bits(rd_bits)?;
                let imm = (instruction & 0xffff_f000).cast_signed();
                Self::Auipc { rd, imm }
            }
            // Jal (J-type)
            0b1101111 => {
                let rd = Reg::from_bits(rd_bits)?;
                let imm20 = ((instruction >> 31) & 1).cast_signed();
                let imm10_1 = ((instruction >> 21) & 0b11_1111_1111).cast_signed();
                let imm11 = ((instruction >> 20) & 1).cast_signed();
                let imm19_12 = ((instruction >> 12) & 0b1111_1111).cast_signed();
                let imm = (imm20 << 20) | (imm19_12 << 12) | (imm11 << 11) | (imm10_1 << 1);
                // Sign extend
                let imm = (imm << 11) >> 11;
                Self::Jal { rd, imm }
            }
            // Fence (I-type like, simplified for EM)
            0b0001111 => {
                if funct3 == 0b000 {
                    if rd_bits != 0 || rs1_bits != 0 {
                        return None;
                    }
                    let pred = ((instruction >> 24) & 0xf) as u8;
                    let succ = ((instruction >> 20) & 0xf) as u8;
                    let fm = ((instruction >> 28) & 0xf) as u8;
                    Self::Fence { pred, succ, fm }
                } else {
                    Self::Invalid(instruction)
                }
            }
            // System instructions
            0b1110011 => {
                let imm = (instruction >> 20) & 0xfff;
                if funct3 == 0 && rd_bits == 0 && rs1_bits == 0 {
                    match imm {
                        0 => Self::Ecall,
                        1 => Self::Ebreak,
                        _ => Self::Invalid(instruction),
                    }
                } else if funct3 == 0b001 && rd_bits == 0 && rs1_bits == 0 && imm == 0xc00 {
                    // `0xc0001073` is emitted as `unimp`/illegal instruction by various compilers,
                    // including Rust when it hits a panic
                    Self::Unimp
                } else {
                    Self::Invalid(instruction)
                }
            }
            _ => Self::Invalid(instruction),
        })
    }
}

impl<Reg> fmt::Display for Rv64Instruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add { rd, rs1, rs2 } => write!(f, "add {}, {}, {}", rd, rs1, rs2),
            Self::Sub { rd, rs1, rs2 } => write!(f, "sub {}, {}, {}", rd, rs1, rs2),
            Self::Sll { rd, rs1, rs2 } => write!(f, "sll {}, {}, {}", rd, rs1, rs2),
            Self::Slt { rd, rs1, rs2 } => write!(f, "slt {}, {}, {}", rd, rs1, rs2),
            Self::Sltu { rd, rs1, rs2 } => write!(f, "sltu {}, {}, {}", rd, rs1, rs2),
            Self::Xor { rd, rs1, rs2 } => write!(f, "xor {}, {}, {}", rd, rs1, rs2),
            Self::Srl { rd, rs1, rs2 } => write!(f, "srl {}, {}, {}", rd, rs1, rs2),
            Self::Sra { rd, rs1, rs2 } => write!(f, "sra {}, {}, {}", rd, rs1, rs2),
            Self::Or { rd, rs1, rs2 } => write!(f, "or {}, {}, {}", rd, rs1, rs2),
            Self::And { rd, rs1, rs2 } => write!(f, "and {}, {}, {}", rd, rs1, rs2),

            Self::Mul { rd, rs1, rs2 } => write!(f, "mul {}, {}, {}", rd, rs1, rs2),
            Self::Mulh { rd, rs1, rs2 } => write!(f, "mulh {}, {}, {}", rd, rs1, rs2),
            Self::Mulhsu { rd, rs1, rs2 } => write!(f, "mulhsu {}, {}, {}", rd, rs1, rs2),
            Self::Mulhu { rd, rs1, rs2 } => write!(f, "mulhu {}, {}, {}", rd, rs1, rs2),
            Self::Div { rd, rs1, rs2 } => write!(f, "div {}, {}, {}", rd, rs1, rs2),
            Self::Divu { rd, rs1, rs2 } => write!(f, "divu {}, {}, {}", rd, rs1, rs2),
            Self::Rem { rd, rs1, rs2 } => write!(f, "rem {}, {}, {}", rd, rs1, rs2),
            Self::Remu { rd, rs1, rs2 } => write!(f, "remu {}, {}, {}", rd, rs1, rs2),

            Self::Addw { rd, rs1, rs2 } => write!(f, "addw {}, {}, {}", rd, rs1, rs2),
            Self::Subw { rd, rs1, rs2 } => write!(f, "subw {}, {}, {}", rd, rs1, rs2),
            Self::Sllw { rd, rs1, rs2 } => write!(f, "sllw {}, {}, {}", rd, rs1, rs2),
            Self::Srlw { rd, rs1, rs2 } => write!(f, "srlw {}, {}, {}", rd, rs1, rs2),
            Self::Sraw { rd, rs1, rs2 } => write!(f, "sraw {}, {}, {}", rd, rs1, rs2),
            Self::Mulw { rd, rs1, rs2 } => write!(f, "mulw {}, {}, {}", rd, rs1, rs2),
            Self::Divw { rd, rs1, rs2 } => write!(f, "divw {}, {}, {}", rd, rs1, rs2),
            Self::Divuw { rd, rs1, rs2 } => write!(f, "divuw {}, {}, {}", rd, rs1, rs2),
            Self::Remw { rd, rs1, rs2 } => write!(f, "remw {}, {}, {}", rd, rs1, rs2),
            Self::Remuw { rd, rs1, rs2 } => write!(f, "remuw {}, {}, {}", rd, rs1, rs2),

            Self::Addi { rd, rs1, imm } => write!(f, "addi {}, {}, {}", rd, rs1, imm),
            Self::Slti { rd, rs1, imm } => write!(f, "slti {}, {}, {}", rd, rs1, imm),
            Self::Sltiu { rd, rs1, imm } => write!(f, "sltiu {}, {}, {}", rd, rs1, imm),
            Self::Xori { rd, rs1, imm } => write!(f, "xori {}, {}, {}", rd, rs1, imm),
            Self::Ori { rd, rs1, imm } => write!(f, "ori {}, {}, {}", rd, rs1, imm),
            Self::Andi { rd, rs1, imm } => write!(f, "andi {}, {}, {}", rd, rs1, imm),
            Self::Slli { rd, rs1, shamt } => write!(f, "slli {}, {}, {}", rd, rs1, shamt),
            Self::Srli { rd, rs1, shamt } => write!(f, "srli {}, {}, {}", rd, rs1, shamt),
            Self::Srai { rd, rs1, shamt } => write!(f, "srai {}, {}, {}", rd, rs1, shamt),

            Self::Addiw { rd, rs1, imm } => write!(f, "addiw {}, {}, {}", rd, rs1, imm),
            Self::Slliw { rd, rs1, shamt } => write!(f, "slliw {}, {}, {}", rd, rs1, shamt),
            Self::Srliw { rd, rs1, shamt } => write!(f, "srliw {}, {}, {}", rd, rs1, shamt),
            Self::Sraiw { rd, rs1, shamt } => write!(f, "sraiw {}, {}, {}", rd, rs1, shamt),

            Self::Lb { rd, rs1, imm } => write!(f, "lb {}, {}({})", rd, imm, rs1),
            Self::Lh { rd, rs1, imm } => write!(f, "lh {}, {}({})", rd, imm, rs1),
            Self::Lw { rd, rs1, imm } => write!(f, "lw {}, {}({})", rd, imm, rs1),
            Self::Ld { rd, rs1, imm } => write!(f, "ld {}, {}({})", rd, imm, rs1),
            Self::Lbu { rd, rs1, imm } => write!(f, "lbu {}, {}({})", rd, imm, rs1),
            Self::Lhu { rd, rs1, imm } => write!(f, "lhu {}, {}({})", rd, imm, rs1),
            Self::Lwu { rd, rs1, imm } => write!(f, "lwu {}, {}({})", rd, imm, rs1),

            Self::Jalr { rd, rs1, imm } => write!(f, "jalr {}, {}({})", rd, imm, rs1),

            Self::Sb { rs1, rs2, imm } => write!(f, "sb {}, {}({})", rs2, imm, rs1),
            Self::Sh { rs1, rs2, imm } => write!(f, "sh {}, {}({})", rs2, imm, rs1),
            Self::Sw { rs1, rs2, imm } => write!(f, "sw {}, {}({})", rs2, imm, rs1),
            Self::Sd { rs1, rs2, imm } => write!(f, "sd {}, {}({})", rs2, imm, rs1),

            Self::Beq { rs1, rs2, imm } => write!(f, "beq {}, {}, {}", rs1, rs2, imm),
            Self::Bne { rs1, rs2, imm } => write!(f, "bne {}, {}, {}", rs1, rs2, imm),
            Self::Blt { rs1, rs2, imm } => write!(f, "blt {}, {}, {}", rs1, rs2, imm),
            Self::Bge { rs1, rs2, imm } => write!(f, "bge {}, {}, {}", rs1, rs2, imm),
            Self::Bltu { rs1, rs2, imm } => write!(f, "bltu {}, {}, {}", rs1, rs2, imm),
            Self::Bgeu { rs1, rs2, imm } => write!(f, "bgeu {}, {}, {}", rs1, rs2, imm),

            Self::Lui { rd, imm } => write!(f, "lui {}, 0x{:x}", rd, imm >> 12),

            Self::Auipc { rd, imm } => write!(f, "auipc {}, 0x{:x}", rd, imm >> 12),

            Self::Jal { rd, imm } => write!(f, "jal {}, {}", rd, imm),

            Self::Fence { pred, succ, fm } => write!(f, "fence {}, {}, {}", pred, succ, fm),

            Self::Ecall => write!(f, "ecall"),
            Self::Ebreak => write!(f, "ebreak"),

            Self::Unimp => write!(f, "unimp"),

            Self::Invalid(instruction) => write!(f, "invalid {instruction:#010x}"),
        }
    }
}

impl<Reg> fmt::Debug for Rv64Instruction<Reg>
where
    Self: fmt::Display,
    Reg: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
