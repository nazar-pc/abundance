//! Base RISC-V RV64 instruction set

use crate::instruction::{BaseInstruction, Instruction};
use crate::registers::Register;
use core::fmt;

pub mod b;
pub mod m;
#[cfg(test)]
mod tests;

/// RISC-V RV64 instruction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    // RV64 R-type W
    Addw { rd: Reg, rs1: Reg, rs2: Reg },
    Subw { rd: Reg, rs1: Reg, rs2: Reg },
    Sllw { rd: Reg, rs1: Reg, rs2: Reg },
    Srlw { rd: Reg, rs1: Reg, rs2: Reg },
    Sraw { rd: Reg, rs1: Reg, rs2: Reg },

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
    Sb { rs2: Reg, rs1: Reg, imm: i32 },
    Sh { rs2: Reg, rs1: Reg, imm: i32 },
    Sw { rs2: Reg, rs1: Reg, imm: i32 },
    Sd { rs2: Reg, rs1: Reg, imm: i32 },

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

impl<Reg> const Instruction for Rv64Instruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Base = Rv64Instruction<Reg>;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        match opcode {
            // R-type
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b000, 0b0000000) => Some(Self::Add { rd, rs1, rs2 }),
                    (0b000, 0b0100000) => Some(Self::Sub { rd, rs1, rs2 }),
                    (0b001, 0b0000000) => Some(Self::Sll { rd, rs1, rs2 }),
                    (0b010, 0b0000000) => Some(Self::Slt { rd, rs1, rs2 }),
                    (0b011, 0b0000000) => Some(Self::Sltu { rd, rs1, rs2 }),
                    (0b100, 0b0000000) => Some(Self::Xor { rd, rs1, rs2 }),
                    (0b101, 0b0000000) => Some(Self::Srl { rd, rs1, rs2 }),
                    (0b101, 0b0100000) => Some(Self::Sra { rd, rs1, rs2 }),
                    (0b110, 0b0000000) => Some(Self::Or { rd, rs1, rs2 }),
                    (0b111, 0b0000000) => Some(Self::And { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // RV64 R-type W
            0b0111011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match (funct3, funct7) {
                    (0b000, 0b0000000) => Some(Self::Addw { rd, rs1, rs2 }),
                    (0b000, 0b0100000) => Some(Self::Subw { rd, rs1, rs2 }),
                    (0b001, 0b0000000) => Some(Self::Sllw { rd, rs1, rs2 }),
                    (0b101, 0b0000000) => Some(Self::Srlw { rd, rs1, rs2 }),
                    (0b101, 0b0100000) => Some(Self::Sraw { rd, rs1, rs2 }),
                    _ => None,
                }
            }
            // I-type
            0b0010011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let imm = instruction.cast_signed() >> 20;
                match funct3 {
                    0b000 => Some(Self::Addi { rd, rs1, imm }),
                    0b010 => Some(Self::Slti { rd, rs1, imm }),
                    0b011 => Some(Self::Sltiu { rd, rs1, imm }),
                    0b100 => Some(Self::Xori { rd, rs1, imm }),
                    0b110 => Some(Self::Ori { rd, rs1, imm }),
                    0b111 => Some(Self::Andi { rd, rs1, imm }),
                    0b001 => {
                        let shamt = (instruction >> 20) & 0b11_1111;
                        let funct6 = (instruction >> 26) & 0b11_1111;
                        if funct6 == 0b000000 {
                            Some(Self::Slli { rd, rs1, shamt })
                        } else {
                            None
                        }
                    }
                    0b101 => {
                        let shamt = (instruction >> 20) & 0b11_1111;
                        let funct6 = (instruction >> 26) & 0b11_1111;
                        match funct6 {
                            0b000000 => Some(Self::Srli { rd, rs1, shamt }),
                            0b010000 => Some(Self::Srai { rd, rs1, shamt }),
                            _ => None,
                        }
                    }
                    _ => None,
                }
            }
            // RV64 I-type W
            0b0011011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let imm = instruction.cast_signed() >> 20;
                // 5-bit for W shifts
                let shamt = (instruction >> 20) & 0b1_1111;
                match funct3 {
                    0b000 => Some(Self::Addiw { rd, rs1, imm }),
                    0b001 => {
                        if funct7 == 0b0000000 {
                            Some(Self::Slliw { rd, rs1, shamt })
                        } else {
                            None
                        }
                    }
                    0b101 => match funct7 {
                        0b0000000 => Some(Self::Srliw { rd, rs1, shamt }),
                        0b0100000 => Some(Self::Sraiw { rd, rs1, shamt }),
                        _ => None,
                    },
                    _ => None,
                }
            }
            // Loads (I-type)
            0b0000011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let imm = instruction.cast_signed() >> 20;
                match funct3 {
                    0b000 => Some(Self::Lb { rd, rs1, imm }),
                    0b001 => Some(Self::Lh { rd, rs1, imm }),
                    0b010 => Some(Self::Lw { rd, rs1, imm }),
                    0b011 => Some(Self::Ld { rd, rs1, imm }),
                    0b100 => Some(Self::Lbu { rd, rs1, imm }),
                    0b101 => Some(Self::Lhu { rd, rs1, imm }),
                    0b110 => Some(Self::Lwu { rd, rs1, imm }),
                    _ => None,
                }
            }
            // Jalr (I-type)
            0b1100111 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                if funct3 == 0b000 {
                    let imm = instruction.cast_signed() >> 20;
                    Some(Self::Jalr { rd, rs1, imm })
                } else {
                    None
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
                    0b000 => Some(Self::Sb { rs2, rs1, imm }),
                    0b001 => Some(Self::Sh { rs2, rs1, imm }),
                    0b010 => Some(Self::Sw { rs2, rs1, imm }),
                    0b011 => Some(Self::Sd { rs2, rs1, imm }),
                    _ => None,
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
                    0b000 => Some(Self::Beq { rs1, rs2, imm }),
                    0b001 => Some(Self::Bne { rs1, rs2, imm }),
                    0b100 => Some(Self::Blt { rs1, rs2, imm }),
                    0b101 => Some(Self::Bge { rs1, rs2, imm }),
                    0b110 => Some(Self::Bltu { rs1, rs2, imm }),
                    0b111 => Some(Self::Bgeu { rs1, rs2, imm }),
                    _ => None,
                }
            }
            // Lui (U-type)
            0b0110111 => {
                let rd = Reg::from_bits(rd_bits)?;
                let imm = (instruction & 0xffff_f000).cast_signed();
                Some(Self::Lui { rd, imm })
            }
            // Auipc (U-type)
            0b0010111 => {
                let rd = Reg::from_bits(rd_bits)?;
                let imm = (instruction & 0xffff_f000).cast_signed();
                Some(Self::Auipc { rd, imm })
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
                Some(Self::Jal { rd, imm })
            }
            // Fence (I-type like, simplified for EM)
            0b0001111 => {
                if funct3 == 0b000 && rd_bits == 0 && rs1_bits == 0 {
                    let pred = ((instruction >> 24) & 0xf) as u8;
                    let succ = ((instruction >> 20) & 0xf) as u8;
                    let fm = ((instruction >> 28) & 0xf) as u8;
                    Some(Self::Fence { pred, succ, fm })
                } else {
                    None
                }
            }
            // System instructions
            0b1110011 => {
                let imm = (instruction >> 20) & 0xfff;
                if funct3 == 0 && rd_bits == 0 && rs1_bits == 0 {
                    match imm {
                        0 => Some(Self::Ecall),
                        1 => Some(Self::Ebreak),
                        _ => None,
                    }
                } else if funct3 == 0b001 && rd_bits == 0 && rs1_bits == 0 && imm == 0xc00 {
                    // `0xc0001073` is emitted as `unimp`/illegal instruction by various compilers,
                    // including Rust when it hits a panic
                    Some(Self::Unimp)
                } else {
                    None
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

impl<Reg> const BaseInstruction for Rv64Instruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn from_base(base: Self::Base) -> Self {
        base
    }

    #[inline]
    fn decode(instruction: u32) -> Self {
        if let Some(instruction) = Self::try_decode(instruction) {
            instruction
        } else {
            Self::Invalid(instruction)
        }
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

            Self::Addw { rd, rs1, rs2 } => write!(f, "addw {}, {}, {}", rd, rs1, rs2),
            Self::Subw { rd, rs1, rs2 } => write!(f, "subw {}, {}, {}", rd, rs1, rs2),
            Self::Sllw { rd, rs1, rs2 } => write!(f, "sllw {}, {}, {}", rd, rs1, rs2),
            Self::Srlw { rd, rs1, rs2 } => write!(f, "srlw {}, {}, {}", rd, rs1, rs2),
            Self::Sraw { rd, rs1, rs2 } => write!(f, "sraw {}, {}, {}", rd, rs1, rs2),

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

            Self::Sb { rs2, rs1, imm } => write!(f, "sb {}, {}({})", rs2, imm, rs1),
            Self::Sh { rs2, rs1, imm } => write!(f, "sh {}, {}({})", rs2, imm, rs1),
            Self::Sw { rs2, rs1, imm } => write!(f, "sw {}, {}({})", rs2, imm, rs1),
            Self::Sd { rs2, rs1, imm } => write!(f, "sd {}, {}({})", rs2, imm, rs1),

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
