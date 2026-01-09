//! RV64 Zbb extension

#[cfg(test)]
mod tests;

use crate::instruction::Instruction;
use crate::instruction::rv64::Rv64Instruction;
use crate::registers::Register;
use core::fmt;

/// RISC-V RV64 Zbb instruction (Basic bit manipulation)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZbbInstruction<Reg> {
    // RV64 Zbb instructions
    Andn { rd: Reg, rs1: Reg, rs2: Reg },
    Orn { rd: Reg, rs1: Reg, rs2: Reg },
    Xnor { rd: Reg, rs1: Reg, rs2: Reg },
    Clz { rd: Reg, rs1: Reg },
    Clzw { rd: Reg, rs1: Reg },
    Ctz { rd: Reg, rs1: Reg },
    Ctzw { rd: Reg, rs1: Reg },
    Cpop { rd: Reg, rs1: Reg },
    Cpopw { rd: Reg, rs1: Reg },
    Max { rd: Reg, rs1: Reg, rs2: Reg },
    Maxu { rd: Reg, rs1: Reg, rs2: Reg },
    Min { rd: Reg, rs1: Reg, rs2: Reg },
    Minu { rd: Reg, rs1: Reg, rs2: Reg },
    Sextb { rd: Reg, rs1: Reg },
    Sexth { rd: Reg, rs1: Reg },
    Zexth { rd: Reg, rs1: Reg },
    Rol { rd: Reg, rs1: Reg, rs2: Reg },
    Rolw { rd: Reg, rs1: Reg, rs2: Reg },
    Ror { rd: Reg, rs1: Reg, rs2: Reg },
    Rori { rd: Reg, rs1: Reg, shamt: u8 },
    Roriw { rd: Reg, rs1: Reg, shamt: u8 },
    Rorw { rd: Reg, rs1: Reg, rs2: Reg },
    Orcb { rd: Reg, rs1: Reg },
    Rev8 { rd: Reg, rs1: Reg },
}

impl<Reg> const Instruction for Rv64ZbbInstruction<Reg>
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
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;
        // bits 25:20 for I-type distinctions
        let low6 = ((instruction >> 20) & 0x3f) as u8;
        let funct12 = ((instruction >> 20) & 0xfff) as u16;

        Some(match opcode {
            // OP-IMM
            0b0010011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    0b001 => {
                        if funct6 == 0b011000 {
                            match low6 {
                                0 => Self::Clz { rd, rs1 },
                                1 => Self::Ctz { rd, rs1 },
                                2 => Self::Cpop { rd, rs1 },
                                4 => Self::Sextb { rd, rs1 },
                                5 => Self::Sexth { rd, rs1 },
                                _ => {
                                    return None;
                                }
                            }
                        } else {
                            return None;
                        }
                    }
                    0b101 => {
                        if funct12 == 0b011010111000 {
                            Self::Rev8 { rd, rs1 }
                        } else if funct6 == 0b011000 {
                            Self::Rori {
                                rd,
                                rs1,
                                shamt: low6,
                            }
                        } else {
                            return None;
                        }
                    }
                    _ => {
                        return None;
                    }
                }
            }
            // OP / R-type
            0b0110011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match funct3 {
                    0b001 => {
                        if funct7 == 0b0110000 {
                            Self::Rol { rd, rs1, rs2 }
                        } else {
                            return None;
                        }
                    }
                    0b010 => {
                        if funct7 == 0b0000101 {
                            Self::Min { rd, rs1, rs2 }
                        } else {
                            return None;
                        }
                    }
                    0b011 => {
                        if funct7 == 0b0000101 {
                            Self::Minu { rd, rs1, rs2 }
                        } else {
                            return None;
                        }
                    }
                    0b100 => match funct7 {
                        0b0100000 => Self::Xnor { rd, rs1, rs2 },
                        0b0000101 => Self::Max { rd, rs1, rs2 },
                        _ => {
                            return None;
                        }
                    },
                    0b101 => match funct7 {
                        0b0110000 => Self::Ror { rd, rs1, rs2 },
                        0b0000101 => {
                            if rs2_bits == 0b00111 {
                                Self::Orcb { rd, rs1 }
                            } else {
                                Self::Maxu { rd, rs1, rs2 }
                            }
                        }
                        _ => {
                            return None;
                        }
                    },
                    0b110 => {
                        if funct7 == 0b0100000 {
                            Self::Orn { rd, rs1, rs2 }
                        } else {
                            return None;
                        }
                    }
                    0b111 => {
                        if funct7 == 0b0100000 {
                            Self::Andn { rd, rs1, rs2 }
                        } else {
                            return None;
                        }
                    }
                    _ => {
                        return None;
                    }
                }
            }
            // OP-IMM-32
            0b0011011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                match funct3 {
                    0b001 => {
                        if funct7 == 0b0110000 {
                            match rs2_bits {
                                0 => Self::Clzw { rd, rs1 },
                                1 => Self::Ctzw { rd, rs1 },
                                2 => Self::Cpopw { rd, rs1 },
                                _ => {
                                    return None;
                                }
                            }
                        } else {
                            return None;
                        }
                    }
                    0b101 => {
                        if funct7 == 0b0110000 {
                            let shamt = rs2_bits;
                            Self::Roriw { rd, rs1, shamt }
                        } else {
                            return None;
                        }
                    }
                    _ => {
                        return None;
                    }
                }
            }
            // OP-32
            0b0111011 => {
                let rd = Reg::from_bits(rd_bits)?;
                let rs1 = Reg::from_bits(rs1_bits)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                match funct3 {
                    0b001 => {
                        if funct7 == 0b0110000 {
                            Self::Rolw { rd, rs1, rs2 }
                        } else {
                            return None;
                        }
                    }
                    0b100 => {
                        if funct7 == 0b0000100 && rs2_bits == 0 {
                            Self::Zexth { rd, rs1 }
                        } else {
                            return None;
                        }
                    }
                    0b101 => {
                        if funct7 == 0b0110000 {
                            Self::Rorw { rd, rs1, rs2 }
                        } else {
                            return None;
                        }
                    }
                    _ => {
                        return None;
                    }
                }
            }
            _ => {
                return None;
            }
        })
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

impl<Reg> fmt::Display for Rv64ZbbInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Andn { rd, rs1, rs2 } => write!(f, "andn {}, {}, {}", rd, rs1, rs2),
            Self::Orn { rd, rs1, rs2 } => write!(f, "orn {}, {}, {}", rd, rs1, rs2),
            Self::Xnor { rd, rs1, rs2 } => write!(f, "xnor {}, {}, {}", rd, rs1, rs2),
            Self::Clz { rd, rs1 } => write!(f, "clz {}, {}", rd, rs1),
            Self::Clzw { rd, rs1 } => write!(f, "clzw {}, {}", rd, rs1),
            Self::Ctz { rd, rs1 } => write!(f, "ctz {}, {}", rd, rs1),
            Self::Ctzw { rd, rs1 } => write!(f, "ctzw {}, {}", rd, rs1),
            Self::Cpop { rd, rs1 } => write!(f, "cpop {}, {}", rd, rs1),
            Self::Cpopw { rd, rs1 } => write!(f, "cpopw {}, {}", rd, rs1),
            Self::Max { rd, rs1, rs2 } => write!(f, "max {}, {}, {}", rd, rs1, rs2),
            Self::Maxu { rd, rs1, rs2 } => write!(f, "maxu {}, {}, {}", rd, rs1, rs2),
            Self::Min { rd, rs1, rs2 } => write!(f, "min {}, {}, {}", rd, rs1, rs2),
            Self::Minu { rd, rs1, rs2 } => write!(f, "minu {}, {}, {}", rd, rs1, rs2),
            Self::Sextb { rd, rs1 } => write!(f, "sext.b {}, {}", rd, rs1),
            Self::Sexth { rd, rs1 } => write!(f, "sext.h {}, {}", rd, rs1),
            Self::Zexth { rd, rs1 } => write!(f, "zext.h {}, {}", rd, rs1),
            Self::Rol { rd, rs1, rs2 } => write!(f, "rol {}, {}, {}", rd, rs1, rs2),
            Self::Rolw { rd, rs1, rs2 } => write!(f, "rolw {}, {}, {}", rd, rs1, rs2),
            Self::Ror { rd, rs1, rs2 } => write!(f, "ror {}, {}, {}", rd, rs1, rs2),
            Self::Rori { rd, rs1, shamt } => write!(f, "rori {}, {}, {}", rd, rs1, shamt),
            Self::Roriw { rd, rs1, shamt } => write!(f, "roriw {}, {}, {}", rd, rs1, shamt),
            Self::Rorw { rd, rs1, rs2 } => write!(f, "rorw {}, {}, {}", rd, rs1, rs2),
            Self::Orcb { rd, rs1 } => write!(f, "orc.b {}, {}", rd, rs1),
            Self::Rev8 { rd, rs1 } => write!(f, "rev8 {}, {}", rd, rs1),
        }
    }
}
