//! This module defines the RISC-V instruction set for the RV64EM architecture

use crate::registers::Reg;
use core::fmt;

/// RISC-V (RV64EM) instruction
#[derive(Debug, Clone, Copy)]
pub enum Instruction {
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

    // Invalid instruction
    Invalid(u32),
}

impl Instruction {
    pub const fn decode(instruction: u32) -> Self {
        let opcode = (instruction & 0b111_1111) as u8;
        let rd_bits = (instruction >> 7) & 0x1f;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = (instruction >> 15) & 0x1f;
        let rs2_bits = (instruction >> 20) & 0x1f;
        let funct7 = ((instruction >> 25) & 0b111_1111) as u8;

        match opcode {
            // R-type
            0b0110011 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs2 = match Reg::from_bits(rs2_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                match (funct3, funct7) {
                    (0b000, 0b0000000) => Instruction::Add { rd, rs1, rs2 },
                    (0b000, 0b0100000) => Instruction::Sub { rd, rs1, rs2 },
                    (0b001, 0b0000000) => Instruction::Sll { rd, rs1, rs2 },
                    (0b010, 0b0000000) => Instruction::Slt { rd, rs1, rs2 },
                    (0b011, 0b0000000) => Instruction::Sltu { rd, rs1, rs2 },
                    (0b100, 0b0000000) => Instruction::Xor { rd, rs1, rs2 },
                    (0b101, 0b0000000) => Instruction::Srl { rd, rs1, rs2 },
                    (0b101, 0b0100000) => Instruction::Sra { rd, rs1, rs2 },
                    (0b110, 0b0000000) => Instruction::Or { rd, rs1, rs2 },
                    (0b111, 0b0000000) => Instruction::And { rd, rs1, rs2 },
                    // M extension
                    (0b000, 0b0000001) => Instruction::Mul { rd, rs1, rs2 },
                    (0b001, 0b0000001) => Instruction::Mulh { rd, rs1, rs2 },
                    (0b010, 0b0000001) => Instruction::Mulhsu { rd, rs1, rs2 },
                    (0b011, 0b0000001) => Instruction::Mulhu { rd, rs1, rs2 },
                    (0b100, 0b0000001) => Instruction::Div { rd, rs1, rs2 },
                    (0b101, 0b0000001) => Instruction::Divu { rd, rs1, rs2 },
                    (0b110, 0b0000001) => Instruction::Rem { rd, rs1, rs2 },
                    (0b111, 0b0000001) => Instruction::Remu { rd, rs1, rs2 },
                    _ => Instruction::Invalid(instruction),
                }
            }
            // RV64 R-type W
            0b0111011 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs2 = match Reg::from_bits(rs2_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                match (funct3, funct7) {
                    (0b000, 0b0000000) => Instruction::Addw { rd, rs1, rs2 },
                    (0b000, 0b0100000) => Instruction::Subw { rd, rs1, rs2 },
                    (0b001, 0b0000000) => Instruction::Sllw { rd, rs1, rs2 },
                    (0b101, 0b0000000) => Instruction::Srlw { rd, rs1, rs2 },
                    (0b101, 0b0100000) => Instruction::Sraw { rd, rs1, rs2 },
                    // M extension W
                    (0b000, 0b0000001) => Instruction::Mulw { rd, rs1, rs2 },
                    (0b100, 0b0000001) => Instruction::Divw { rd, rs1, rs2 },
                    (0b101, 0b0000001) => Instruction::Divuw { rd, rs1, rs2 },
                    (0b110, 0b0000001) => Instruction::Remw { rd, rs1, rs2 },
                    (0b111, 0b0000001) => Instruction::Remuw { rd, rs1, rs2 },
                    _ => Instruction::Invalid(instruction),
                }
            }
            // I-type
            0b0010011 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm = (instruction as i32) >> 20;
                match funct3 {
                    0b000 => Instruction::Addi { rd, rs1, imm },
                    0b010 => Instruction::Slti { rd, rs1, imm },
                    0b011 => Instruction::Sltiu { rd, rs1, imm },
                    0b100 => Instruction::Xori { rd, rs1, imm },
                    0b110 => Instruction::Ori { rd, rs1, imm },
                    0b111 => Instruction::Andi { rd, rs1, imm },
                    0b001 => {
                        let shamt = (instruction >> 20) & 0b11_1111;
                        if funct7 == 0b0000000 {
                            Instruction::Slli { rd, rs1, shamt }
                        } else {
                            Instruction::Invalid(instruction)
                        }
                    }
                    0b101 => {
                        let shamt = (instruction >> 20) & 0b11_1111;
                        match funct7 {
                            0b0000000 => Instruction::Srli { rd, rs1, shamt },
                            0b0100000 => Instruction::Srai { rd, rs1, shamt },
                            _ => Instruction::Invalid(instruction),
                        }
                    }
                    _ => Instruction::Invalid(instruction),
                }
            }
            // RV64 I-type W
            0b0011011 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm = (instruction as i32) >> 20;
                let shamt = (instruction >> 20) & 0b1_1111; // 5-bit for W shifts
                match funct3 {
                    0b000 => Instruction::Addiw { rd, rs1, imm },
                    0b001 => {
                        if funct7 == 0b0000000 {
                            Instruction::Slliw { rd, rs1, shamt }
                        } else {
                            Instruction::Invalid(instruction)
                        }
                    }
                    0b101 => match funct7 {
                        0b0000000 => Instruction::Srliw { rd, rs1, shamt },
                        0b0100000 => Instruction::Sraiw { rd, rs1, shamt },
                        _ => Instruction::Invalid(instruction),
                    },
                    _ => Instruction::Invalid(instruction),
                }
            }
            // Loads (I-type)
            0b0000011 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm = (instruction as i32) >> 20;
                match funct3 {
                    0b000 => Instruction::Lb { rd, rs1, imm },
                    0b001 => Instruction::Lh { rd, rs1, imm },
                    0b010 => Instruction::Lw { rd, rs1, imm },
                    0b011 => Instruction::Ld { rd, rs1, imm },
                    0b100 => Instruction::Lbu { rd, rs1, imm },
                    0b101 => Instruction::Lhu { rd, rs1, imm },
                    0b110 => Instruction::Lwu { rd, rs1, imm },
                    _ => Instruction::Invalid(instruction),
                }
            }
            // Jalr (I-type)
            0b1100111 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                if funct3 == 0b000 {
                    let imm = (instruction as i32) >> 20;
                    Instruction::Jalr { rd, rs1, imm }
                } else {
                    Instruction::Invalid(instruction)
                }
            }
            // S-type
            0b0100011 => {
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs2 = match Reg::from_bits(rs2_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm11_5 = ((instruction >> 25) & 0b111_1111) as i32;
                let imm4_0 = ((instruction >> 7) & 0b1_1111) as i32;
                let imm = (imm11_5 << 5) | imm4_0;
                // Sign extend
                let imm = (imm << 20) >> 20;
                match funct3 {
                    0b000 => Instruction::Sb { rs1, rs2, imm },
                    0b001 => Instruction::Sh { rs1, rs2, imm },
                    0b010 => Instruction::Sw { rs1, rs2, imm },
                    0b011 => Instruction::Sd { rs1, rs2, imm },
                    _ => Instruction::Invalid(instruction),
                }
            }
            // B-type
            0b1100011 => {
                let rs1 = match Reg::from_bits(rs1_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let rs2 = match Reg::from_bits(rs2_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm12 = ((instruction >> 31) & 1) as i32;
                let imm10_5 = ((instruction >> 25) & 0b11_1111) as i32;
                let imm4_1 = ((instruction >> 8) & 0b1111) as i32;
                let imm11 = ((instruction >> 7) & 1) as i32;
                let imm = (imm12 << 12) | (imm11 << 11) | (imm10_5 << 5) | (imm4_1 << 1);
                // Sign extend
                let imm = (imm << 19) >> 19;
                match funct3 {
                    0b000 => Instruction::Beq { rs1, rs2, imm },
                    0b001 => Instruction::Bne { rs1, rs2, imm },
                    0b100 => Instruction::Blt { rs1, rs2, imm },
                    0b101 => Instruction::Bge { rs1, rs2, imm },
                    0b110 => Instruction::Bltu { rs1, rs2, imm },
                    0b111 => Instruction::Bgeu { rs1, rs2, imm },
                    _ => Instruction::Invalid(instruction),
                }
            }
            // Lui (U-type)
            0b0110111 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm = (instruction & 0xffff_f000) as i32;
                Instruction::Lui { rd, imm }
            }
            // Auipc (U-type)
            0b0010111 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm = (instruction & 0xffff_f000) as i32;
                Instruction::Auipc { rd, imm }
            }
            // Jal (J-type)
            0b1101111 => {
                let rd = match Reg::from_bits(rd_bits) {
                    Some(r) => r,
                    None => return Instruction::Invalid(instruction),
                };
                let imm20 = ((instruction >> 31) & 1) as i32;
                let imm10_1 = ((instruction >> 21) & 0b11_1111_1111) as i32;
                let imm11 = ((instruction >> 20) & 1) as i32;
                let imm19_12 = ((instruction >> 12) & 0b1111_1111) as i32;
                let imm = (imm20 << 20) | (imm19_12 << 12) | (imm11 << 11) | (imm10_1 << 1);
                // Sign extend
                let imm = (imm << 11) >> 11;
                Instruction::Jal { rd, imm }
            }
            // Fence (I-type like, simplified for EM)
            0b0001111 => {
                if funct3 == 0b000 {
                    if rd_bits != 0 || rs1_bits != 0 {
                        return Instruction::Invalid(instruction);
                    }
                    let pred = ((instruction >> 24) & 0xf) as u8;
                    let succ = ((instruction >> 20) & 0xf) as u8;
                    let fm = ((instruction >> 28) & 0xf) as u8;
                    Instruction::Fence { pred, succ, fm }
                } else {
                    Instruction::Invalid(instruction)
                }
            }
            // System instructions
            0b1110011 => {
                let imm = (instruction >> 20) & 0xfff;
                if funct3 == 0 && rd_bits == 0 && rs1_bits == 0 {
                    match imm {
                        0 => Instruction::Ecall,
                        1 => Instruction::Ebreak,
                        _ => Instruction::Invalid(instruction),
                    }
                } else {
                    Instruction::Invalid(instruction)
                }
            }
            _ => Instruction::Invalid(instruction),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::Add { rd, rs1, rs2 } => write!(f, "add {}, {}, {}", rd, rs1, rs2),
            Instruction::Sub { rd, rs1, rs2 } => write!(f, "sub {}, {}, {}", rd, rs1, rs2),
            Instruction::Sll { rd, rs1, rs2 } => write!(f, "sll {}, {}, {}", rd, rs1, rs2),
            Instruction::Slt { rd, rs1, rs2 } => write!(f, "slt {}, {}, {}", rd, rs1, rs2),
            Instruction::Sltu { rd, rs1, rs2 } => write!(f, "sltu {}, {}, {}", rd, rs1, rs2),
            Instruction::Xor { rd, rs1, rs2 } => write!(f, "xor {}, {}, {}", rd, rs1, rs2),
            Instruction::Srl { rd, rs1, rs2 } => write!(f, "srl {}, {}, {}", rd, rs1, rs2),
            Instruction::Sra { rd, rs1, rs2 } => write!(f, "sra {}, {}, {}", rd, rs1, rs2),
            Instruction::Or { rd, rs1, rs2 } => write!(f, "or {}, {}, {}", rd, rs1, rs2),
            Instruction::And { rd, rs1, rs2 } => write!(f, "and {}, {}, {}", rd, rs1, rs2),

            Instruction::Mul { rd, rs1, rs2 } => write!(f, "mul {}, {}, {}", rd, rs1, rs2),
            Instruction::Mulh { rd, rs1, rs2 } => write!(f, "mulh {}, {}, {}", rd, rs1, rs2),
            Instruction::Mulhsu { rd, rs1, rs2 } => write!(f, "mulhsu {}, {}, {}", rd, rs1, rs2),
            Instruction::Mulhu { rd, rs1, rs2 } => write!(f, "mulhu {}, {}, {}", rd, rs1, rs2),
            Instruction::Div { rd, rs1, rs2 } => write!(f, "div {}, {}, {}", rd, rs1, rs2),
            Instruction::Divu { rd, rs1, rs2 } => write!(f, "divu {}, {}, {}", rd, rs1, rs2),
            Instruction::Rem { rd, rs1, rs2 } => write!(f, "rem {}, {}, {}", rd, rs1, rs2),
            Instruction::Remu { rd, rs1, rs2 } => write!(f, "remu {}, {}, {}", rd, rs1, rs2),

            Instruction::Addw { rd, rs1, rs2 } => write!(f, "addw {}, {}, {}", rd, rs1, rs2),
            Instruction::Subw { rd, rs1, rs2 } => write!(f, "subw {}, {}, {}", rd, rs1, rs2),
            Instruction::Sllw { rd, rs1, rs2 } => write!(f, "sllw {}, {}, {}", rd, rs1, rs2),
            Instruction::Srlw { rd, rs1, rs2 } => write!(f, "srlw {}, {}, {}", rd, rs1, rs2),
            Instruction::Sraw { rd, rs1, rs2 } => write!(f, "sraw {}, {}, {}", rd, rs1, rs2),
            Instruction::Mulw { rd, rs1, rs2 } => write!(f, "mulw {}, {}, {}", rd, rs1, rs2),
            Instruction::Divw { rd, rs1, rs2 } => write!(f, "divw {}, {}, {}", rd, rs1, rs2),
            Instruction::Divuw { rd, rs1, rs2 } => write!(f, "divuw {}, {}, {}", rd, rs1, rs2),
            Instruction::Remw { rd, rs1, rs2 } => write!(f, "remw {}, {}, {}", rd, rs1, rs2),
            Instruction::Remuw { rd, rs1, rs2 } => write!(f, "remuw {}, {}, {}", rd, rs1, rs2),

            Instruction::Addi { rd, rs1, imm } => write!(f, "addi {}, {}, {}", rd, rs1, imm),
            Instruction::Slti { rd, rs1, imm } => write!(f, "slti {}, {}, {}", rd, rs1, imm),
            Instruction::Sltiu { rd, rs1, imm } => write!(f, "sltiu {}, {}, {}", rd, rs1, imm),
            Instruction::Xori { rd, rs1, imm } => write!(f, "xori {}, {}, {}", rd, rs1, imm),
            Instruction::Ori { rd, rs1, imm } => write!(f, "ori {}, {}, {}", rd, rs1, imm),
            Instruction::Andi { rd, rs1, imm } => write!(f, "andi {}, {}, {}", rd, rs1, imm),
            Instruction::Slli { rd, rs1, shamt } => write!(f, "slli {}, {}, {}", rd, rs1, shamt),
            Instruction::Srli { rd, rs1, shamt } => write!(f, "srli {}, {}, {}", rd, rs1, shamt),
            Instruction::Srai { rd, rs1, shamt } => write!(f, "srai {}, {}, {}", rd, rs1, shamt),

            Instruction::Addiw { rd, rs1, imm } => write!(f, "addiw {}, {}, {}", rd, rs1, imm),
            Instruction::Slliw { rd, rs1, shamt } => write!(f, "slliw {}, {}, {}", rd, rs1, shamt),
            Instruction::Srliw { rd, rs1, shamt } => write!(f, "srliw {}, {}, {}", rd, rs1, shamt),
            Instruction::Sraiw { rd, rs1, shamt } => write!(f, "sraiw {}, {}, {}", rd, rs1, shamt),

            Instruction::Lb { rd, rs1, imm } => write!(f, "lb {}, {}({})", rd, imm, rs1),
            Instruction::Lh { rd, rs1, imm } => write!(f, "lh {}, {}({})", rd, imm, rs1),
            Instruction::Lw { rd, rs1, imm } => write!(f, "lw {}, {}({})", rd, imm, rs1),
            Instruction::Ld { rd, rs1, imm } => write!(f, "ld {}, {}({})", rd, imm, rs1),
            Instruction::Lbu { rd, rs1, imm } => write!(f, "lbu {}, {}({})", rd, imm, rs1),
            Instruction::Lhu { rd, rs1, imm } => write!(f, "lhu {}, {}({})", rd, imm, rs1),
            Instruction::Lwu { rd, rs1, imm } => write!(f, "lwu {}, {}({})", rd, imm, rs1),

            Instruction::Jalr { rd, rs1, imm } => write!(f, "jalr {}, {}({})", rd, imm, rs1),

            Instruction::Sb { rs1, rs2, imm } => write!(f, "sb {}, {}({})", rs2, imm, rs1),
            Instruction::Sh { rs1, rs2, imm } => write!(f, "sh {}, {}({})", rs2, imm, rs1),
            Instruction::Sw { rs1, rs2, imm } => write!(f, "sw {}, {}({})", rs2, imm, rs1),
            Instruction::Sd { rs1, rs2, imm } => write!(f, "sd {}, {}({})", rs2, imm, rs1),

            Instruction::Beq { rs1, rs2, imm } => write!(f, "beq {}, {}, {}", rs1, rs2, imm),
            Instruction::Bne { rs1, rs2, imm } => write!(f, "bne {}, {}, {}", rs1, rs2, imm),
            Instruction::Blt { rs1, rs2, imm } => write!(f, "blt {}, {}, {}", rs1, rs2, imm),
            Instruction::Bge { rs1, rs2, imm } => write!(f, "bge {}, {}, {}", rs1, rs2, imm),
            Instruction::Bltu { rs1, rs2, imm } => write!(f, "bltu {}, {}, {}", rs1, rs2, imm),
            Instruction::Bgeu { rs1, rs2, imm } => write!(f, "bgeu {}, {}, {}", rs1, rs2, imm),

            Instruction::Lui { rd, imm } => write!(f, "lui {}, 0x{:x}", rd, imm >> 12),

            Instruction::Auipc { rd, imm } => write!(f, "auipc {}, 0x{:x}", rd, imm >> 12),

            Instruction::Jal { rd, imm } => write!(f, "jal {}, {}", rd, imm),

            Instruction::Fence { pred, succ, fm } => write!(f, "fence {}, {}, {}", pred, succ, fm),

            Instruction::Ecall => write!(f, "ecall"),
            Instruction::Ebreak => write!(f, "ebreak"),

            Instruction::Invalid(inst) => write!(f, "invalid 0x{:08x}", inst),
        }
    }
}
