//! RV32 Zca extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zca compressed instruction set
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Rv32ZcaInstruction<Reg> {
    // Quadrant 00
    /// C.ADDI4SPN  rd' = sp + nzuimm  (nzuimm != 0)
    CAddi4spn { rd: Reg, nzuimm: u16 },
    /// C.LW  rd' = sext(mem32\[rs1' + uimm])
    CLw { rd: Reg, rs1: Reg, uimm: u8 },
    /// C.SW  mem32\[rs1' + uimm] = rs2'
    CSw { rs1: Reg, rs2: Reg, uimm: u8 },

    // Quadrant 01
    /// C.NOP  (ADDI x0, x0, 0 with rd==x0 and nzimm==0)
    CNop,
    /// C.ADDI  rd += nzimm  (rd != x0)
    CAddi { rd: Reg, nzimm: i8 },
    /// C.JAL  ra = pc+2; pc += imm
    CJal { imm: i16 },
    /// C.LI  rd = sext(imm)  (rd=x0 is a HINT)
    CLi { rd: Reg, imm: i8 },
    /// C.ADDI16SP  sp += nzimm  (nzimm != 0)
    CAddi16sp { nzimm: i16 },
    /// C.LUI  rd = sext(nzimm << 12)  (rd != x0, rd != x2, nzimm != 0)
    CLui { rd: Reg, nzimm: i32 },
    /// C.SRLI  rd' >>= shamt  (logical, 5-bit shamt; shamt=0 is a HINT)
    CSrli { rd: Reg, shamt: u8 },
    /// C.SRAI  rd' >>= shamt  (arithmetic, 5-bit shamt; shamt=0 is a HINT)
    CSrai { rd: Reg, shamt: u8 },
    /// C.ANDI  rd' &= sext(imm)
    CAndi { rd: Reg, imm: i8 },
    /// C.SUB  rd' -= rs2'
    CSub { rd: Reg, rs2: Reg },
    /// C.XOR  rd' ^= rs2'
    CXor { rd: Reg, rs2: Reg },
    /// C.OR   rd' |= rs2'
    COr { rd: Reg, rs2: Reg },
    /// C.AND  rd' &= rs2'
    CAnd { rd: Reg, rs2: Reg },
    /// C.J  pc += sext(imm)
    CJ { imm: i16 },
    /// C.BEQZ  if rs1' == 0: pc += sext(imm)
    CBeqz { rs1: Reg, imm: i16 },
    /// C.BNEZ  if rs1' != 0: pc += sext(imm)
    CBnez { rs1: Reg, imm: i16 },

    // Quadrant 10
    /// C.SLLI  rd <<= shamt  (5-bit shamt; rd=x0 or shamt=0 is a HINT)
    CSlli { rd: Reg, shamt: u8 },
    /// C.LWSP  rd = sext(mem32\[sp + uimm])  (rd != x0)
    CLwsp { rd: Reg, uimm: u8 },
    /// C.JR  pc = rs1  (rs1 != x0)
    CJr { rs1: Reg },
    /// C.MV  rd = rs2  (rs2 != x0; rd=x0 is a HINT)
    CMv { rd: Reg, rs2: Reg },
    /// C.EBREAK
    CEbreak,
    /// C.JALR  ra = pc+2; pc = rs1  (rs1 != x0)
    CJalr { rs1: Reg },
    /// C.ADD  rd += rs2  (rs2 != x0; rd=x0 is a HINT)
    CAdd { rd: Reg, rs2: Reg },
    /// C.SWSP  mem32\[sp + uimm] = rs2
    CSwsp { rs2: Reg, uimm: u8 },

    // Unimplemented/illegal
    CUnimp,
}

#[instruction]
impl<Reg> const Instruction for Rv32ZcaInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        /// Map a 3-bit "prime" register field to an absolute register number
        #[inline(always)]
        const fn prime_reg_bits(bits: u8) -> u8 {
            bits + 8
        }

        /// Reconstruct the CB-type branch offset used by C.BEQZ / C.BNEZ.
        ///
        /// Bit layout in the 16-bit instruction word:
        /// ```text
        ///   imm[8]   = inst[12]
        ///   imm[4:3] = inst[11:10]
        ///   imm[7:6] = inst[6:5]
        ///   imm[2:1] = inst[4:3]
        ///   imm[5]   = inst[2]
        /// imm[0] is always 0 (2-byte aligned).
        /// ```
        #[inline(always)]
        const fn decode_cb_branch_imm(inst: u16) -> i16 {
            let imm8 = ((inst >> 12) & 1).cast_signed();
            let imm4_3 = ((inst >> 10) & 0b11).cast_signed();
            let imm7_6 = ((inst >> 5) & 0b11).cast_signed();
            let imm2_1 = ((inst >> 3) & 0b11).cast_signed();
            let imm5 = ((inst >> 2) & 1).cast_signed();
            let raw = (imm8 << 8) | (imm7_6 << 6) | (imm5 << 5) | (imm4_3 << 3) | (imm2_1 << 1);
            // Sign-extend from bit 8 (9-bit immediate -> i16)
            (raw << 7) >> 7
        }

        /// Decode CJ-type jump offset (C.J / C.JAL).
        ///
        /// Bit layout:
        /// ```text
        ///   imm[11]  = inst[12]
        ///   imm[4]   = inst[11]
        ///   imm[9:8] = inst[10:9]
        ///   imm[10]  = inst[8]
        ///   imm[6]   = inst[7]
        ///   imm[7]   = inst[6]
        ///   imm[3:1] = inst[5:3]
        ///   imm[5]   = inst[2]
        /// imm[0] is always 0 (2-byte aligned).
        /// ```
        #[inline(always)]
        const fn decode_cj_imm(inst: u16) -> i16 {
            let imm11 = ((inst >> 12) & 1).cast_signed();
            let imm4 = ((inst >> 11) & 1).cast_signed();
            let imm9_8 = ((inst >> 9) & 0b11).cast_signed();
            let imm10 = ((inst >> 8) & 1).cast_signed();
            let imm6 = ((inst >> 7) & 1).cast_signed();
            let imm7 = ((inst >> 6) & 1).cast_signed();
            let imm3_1 = ((inst >> 3) & 0b111).cast_signed();
            let imm5 = ((inst >> 2) & 1).cast_signed();
            let raw = (imm11 << 11)
                | (imm10 << 10)
                | (imm9_8 << 8)
                | (imm7 << 7)
                | (imm6 << 6)
                | (imm5 << 5)
                | (imm4 << 4)
                | (imm3_1 << 1);
            // Sign-extend from bit 11 (12-bit immediate -> i16)
            (raw << 4) >> 4
        }

        let inst = instruction as u16;
        let quadrant = inst & 0b11;
        let funct3 = ((inst >> 13) & 0b111) as u8;

        match quadrant {
            // Quadrant 00
            0b00 => match funct3 {
                // C.ADDI4SPN
                // nzuimm[5:4]  = inst[12:11]
                // nzuimm[9:6]  = inst[10:7]
                // nzuimm[2]    = inst[6]
                // nzuimm[3]    = inst[5]
                0b000 => {
                    let imm5_4 = (inst >> 11) & 0b11;
                    let imm9_6 = (inst >> 7) & 0xf;
                    let imm2 = (inst >> 6) & 1;
                    let imm3 = (inst >> 5) & 1;
                    let nzuimm = (imm9_6 << 6) | (imm5_4 << 4) | (imm3 << 3) | (imm2 << 2);
                    if nzuimm == 0 {
                        if inst == 0 {
                            Some(Self::CUnimp)
                        } else {
                            // Reserved encoding
                            None
                        }
                    } else {
                        let rd_bits = prime_reg_bits(((inst >> 2) & 0b111) as u8);
                        let rd = Reg::from_bits(rd_bits)?;
                        Some(Self::CAddi4spn { rd, nzuimm })
                    }
                }
                // C.LW
                // uimm[5:3] = inst[12:10], uimm[2] = inst[6], uimm[6] = inst[5]
                0b010 => {
                    let uimm5_3 = ((inst >> 10) & 0b111) as u8;
                    let uimm2 = ((inst >> 6) & 1) as u8;
                    let uimm6 = ((inst >> 5) & 1) as u8;
                    let uimm = (uimm6 << 6) | (uimm5_3 << 3) | (uimm2 << 2);
                    let rs1_bits = prime_reg_bits(((inst >> 7) & 0b111) as u8);
                    let rd_bits = prime_reg_bits(((inst >> 2) & 0b111) as u8);
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    let rd = Reg::from_bits(rd_bits)?;
                    Some(Self::CLw { rd, rs1, uimm })
                }
                // C.SW  (same uimm layout as C.LW)
                0b110 => {
                    let uimm5_3 = ((inst >> 10) & 0b111) as u8;
                    let uimm2 = ((inst >> 6) & 1) as u8;
                    let uimm6 = ((inst >> 5) & 1) as u8;
                    let uimm = (uimm6 << 6) | (uimm5_3 << 3) | (uimm2 << 2);
                    let rs1_bits = prime_reg_bits(((inst >> 7) & 0b111) as u8);
                    let rs2_bits = prime_reg_bits(((inst >> 2) & 0b111) as u8);
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    let rs2 = Reg::from_bits(rs2_bits)?;
                    Some(Self::CSw { rs1, rs2, uimm })
                }
                // funct3=001: C.FLD (Zcd) - not in Zca, reserved
                // funct3=011: C.FLD (Zcd) - not in Zca, reserved
                // funct3=100: used by Zcb
                // funct3=101: C.FSD (Zcd) - not in Zca, reserved
                // funct3=111: C.FSD (Zcd) - not in Zca, reserved
                _ => None,
            },

            // Quadrant 01
            0b01 => match funct3 {
                // C.NOP (rd=x0) / C.ADDI (rd!=x0)
                // nzimm[5] = inst[12], nzimm[4:0] = inst[6:2]
                0b000 => {
                    let rd_bits = ((inst >> 7) & 0x1f) as u8;
                    let imm5 = ((inst >> 12) & 1) as u8;
                    let imm4_0 = ((inst >> 2) & 0x1f) as u8;
                    let imm_raw = (imm5 << 5) | imm4_0;
                    // Sign-extend 6-bit immediate to i8
                    let nzimm = ((imm_raw.cast_signed()) << 2) >> 2;
                    if rd_bits == 0 && nzimm == 0 {
                        Some(Self::CNop)
                    } else {
                        let rd = Reg::from_bits(rd_bits)?;
                        Some(Self::CAddi { rd, nzimm })
                    }
                }
                // C.JAL  (same CJ immediate encoding as C.J)
                0b001 => Some(Self::CJal {
                    imm: decode_cj_imm(inst),
                }),
                // C.LI  rd = sext(imm)  (rd=x0 is a HINT, still decoded)
                // imm[5] = inst[12], imm[4:0] = inst[6:2]
                0b010 => {
                    let rd_bits = ((inst >> 7) & 0x1f) as u8;
                    let rd = Reg::from_bits(rd_bits)?;
                    let imm5 = ((inst >> 12) & 1) as u8;
                    let imm4_0 = ((inst >> 2) & 0x1f) as u8;
                    let imm_raw = (imm5 << 5) | imm4_0;
                    let imm = ((imm_raw.cast_signed()) << 2) >> 2;
                    Some(Self::CLi { rd, imm })
                }
                // C.ADDI16SP (rd=x2) / C.LUI (rd!=x0, rd!=x2)
                0b011 => {
                    let rd_bits = ((inst >> 7) & 0x1f) as u8;
                    if rd_bits == 2 {
                        // C.ADDI16SP
                        // nzimm[9]   = inst[12]
                        // nzimm[4]   = inst[6]
                        // nzimm[6]   = inst[5]
                        // nzimm[8:7] = inst[4:3]
                        // nzimm[5]   = inst[2]
                        let imm9 = ((inst >> 12) & 1).cast_signed();
                        let imm4 = ((inst >> 6) & 1).cast_signed();
                        let imm6 = ((inst >> 5) & 1).cast_signed();
                        let imm8_7 = ((inst >> 3) & 0b11).cast_signed();
                        let imm5 = ((inst >> 2) & 1).cast_signed();
                        let raw =
                            (imm9 << 9) | (imm8_7 << 7) | (imm6 << 6) | (imm5 << 5) | (imm4 << 4);
                        if raw == 0 {
                            None?;
                        }
                        // Sign-extend from bit 9 (10-bit nzimm -> i16)
                        let nzimm = (raw << 6) >> 6;
                        Some(Self::CAddi16sp { nzimm })
                    } else {
                        // C.LUI  (rd=x0 is reserved)
                        if rd_bits == 0 {
                            None?;
                        }
                        let rd = Reg::from_bits(rd_bits)?;
                        // nzimm[17]    = inst[12]
                        // nzimm[16:12] = inst[6:2]
                        let imm17 = ((inst >> 12) & 1) as i32;
                        let imm16_12 = ((inst >> 2) & 0x1f) as i32;
                        let raw = (imm17 << 17) | (imm16_12 << 12);
                        if raw == 0 {
                            None?;
                        }
                        // Sign-extend from bit 17 (18-bit nzimm -> i32)
                        let nzimm = (raw << 14) >> 14;
                        Some(Self::CLui { rd, nzimm })
                    }
                }
                // C.SRLI / C.SRAI / C.ANDI / arithmetic
                // RV32: shamt is 5-bit only (inst[12] must be 0 for shifts, else reserved)
                0b100 => {
                    let funct2 = ((inst >> 10) & 0b11) as u8;
                    let rd_bits = prime_reg_bits(((inst >> 7) & 0b111) as u8);
                    match funct2 {
                        // C.SRLI  shamt[4:0]=inst[6:2]
                        // RV32: shamt[5]=inst[12] must be 0, else reserved (NSE)
                        // shamt=0 is a HINT, still decoded
                        0b00 => {
                            let rd = Reg::from_bits(rd_bits)?;
                            let shamt5 = ((inst >> 12) & 1) as u8;
                            let shamt40 = ((inst >> 2) & 0x1f) as u8;
                            if shamt5 != 0 {
                                None?;
                            }
                            Some(Self::CSrli { rd, shamt: shamt40 })
                        }
                        // C.SRAI  (same shamt layout as C.SRLI)
                        // RV32: shamt[5]=inst[12] must be 0, else reserved (NSE)
                        // shamt=0 is a HINT, still decoded
                        0b01 => {
                            let rd = Reg::from_bits(rd_bits)?;
                            let shamt5 = ((inst >> 12) & 1) as u8;
                            let shamt40 = ((inst >> 2) & 0x1f) as u8;
                            if shamt5 != 0 {
                                None?;
                            }
                            Some(Self::CSrai { rd, shamt: shamt40 })
                        }
                        // C.ANDI  imm[5]=inst[12], imm[4:0]=inst[6:2]
                        0b10 => {
                            let rd = Reg::from_bits(rd_bits)?;
                            let imm5 = ((inst >> 12) & 1) as u8;
                            let imm4_0 = ((inst >> 2) & 0x1f) as u8;
                            let imm_raw = (imm5 << 5) | imm4_0;
                            let imm = ((imm_raw.cast_signed()) << 2) >> 2;
                            Some(Self::CAndi { rd, imm })
                        }
                        // Arithmetic: only bit12=0 variants valid in RV32
                        // bit12=1 (C.SUBW/C.ADDW) does not exist in RV32, reserved
                        0b11 => {
                            let bit12 = (inst >> 12) & 1;
                            if bit12 != 0 {
                                None?;
                            }
                            let funct2b = ((inst >> 5) & 0b11) as u8;
                            let rs2_bits = prime_reg_bits(((inst >> 2) & 0b111) as u8);
                            let rd = Reg::from_bits(rd_bits)?;
                            let rs2 = Reg::from_bits(rs2_bits)?;
                            match funct2b {
                                0b00 => Some(Self::CSub { rd, rs2 }),
                                0b01 => Some(Self::CXor { rd, rs2 }),
                                0b10 => Some(Self::COr { rd, rs2 }),
                                0b11 => Some(Self::CAnd { rd, rs2 }),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                }
                // C.J
                0b101 => Some(Self::CJ {
                    imm: decode_cj_imm(inst),
                }),
                // C.BEQZ
                0b110 => {
                    let rs1_bits = prime_reg_bits(((inst >> 7) & 0b111) as u8);
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::CBeqz {
                        rs1,
                        imm: decode_cb_branch_imm(inst),
                    })
                }
                // C.BNEZ
                0b111 => {
                    let rs1_bits = prime_reg_bits(((inst >> 7) & 0b111) as u8);
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::CBnez {
                        rs1,
                        imm: decode_cb_branch_imm(inst),
                    })
                }
                _ => None,
            },

            // Quadrant 10
            0b10 => match funct3 {
                // C.SLLI  shamt[4:0]=inst[6:2]
                // RV32: shamt[5]=inst[12] must be 0, else reserved (NSE)
                // rd=x0 or shamt=0 is a HINT, still decoded
                0b000 => {
                    let rd_bits = ((inst >> 7) & 0x1f) as u8;
                    let rd = Reg::from_bits(rd_bits)?;
                    let shamt5 = ((inst >> 12) & 1) as u8;
                    let shamt40 = ((inst >> 2) & 0x1f) as u8;
                    if shamt5 != 0 {
                        None?;
                    }
                    Some(Self::CSlli { rd, shamt: shamt40 })
                }
                // C.LWSP  uimm[5]=inst[12], uimm[4:2]=inst[6:4], uimm[7:6]=inst[3:2]
                // rd=x0 is reserved
                0b010 => {
                    let rd_bits = ((inst >> 7) & 0x1f) as u8;
                    if rd_bits == 0 {
                        None?;
                    }
                    let rd = Reg::from_bits(rd_bits)?;
                    let uimm5 = ((inst >> 12) & 1) as u8;
                    let uimm42 = ((inst >> 4) & 0b111) as u8;
                    let uimm76 = ((inst >> 2) & 0b11) as u8;
                    let uimm = (uimm76 << 6) | (uimm5 << 5) | (uimm42 << 2);
                    Some(Self::CLwsp { rd, uimm })
                }
                // funct3=001: C.FLWSP (Zcf, not Zca) - reserved
                // funct3=011: C.FLDSP (Zcd, not Zca) - reserved
                // C.JR / C.MV / C.EBREAK / C.JALR / C.ADD
                0b100 => {
                    let rs1_bits = ((inst >> 7) & 0x1f) as u8;
                    let rs2_bits = ((inst >> 2) & 0x1f) as u8;
                    let bit12 = (inst >> 12) & 1;
                    if bit12 == 0 {
                        if rs2_bits == 0 {
                            // C.JR  (rs1=x0 is reserved)
                            if rs1_bits == 0 {
                                None?;
                            }
                            let rs1 = Reg::from_bits(rs1_bits)?;
                            Some(Self::CJr { rs1 })
                        } else {
                            // C.MV  (rs2!=x0; rd=x0 is a HINT, still decoded)
                            let rd = Reg::from_bits(rs1_bits)?;
                            let rs2 = Reg::from_bits(rs2_bits)?;
                            Some(Self::CMv { rd, rs2 })
                        }
                    } else if rs2_bits == 0 {
                        if rs1_bits == 0 {
                            // C.EBREAK
                            Some(Self::CEbreak)
                        } else {
                            // C.JALR  (rs1!=x0)
                            let rs1 = Reg::from_bits(rs1_bits)?;
                            Some(Self::CJalr { rs1 })
                        }
                    } else {
                        // C.ADD  (rs2!=x0; rd=x0 is a HINT, still decoded)
                        let rd = Reg::from_bits(rs1_bits)?;
                        let rs2 = Reg::from_bits(rs2_bits)?;
                        Some(Self::CAdd { rd, rs2 })
                    }
                }
                // C.SWSP  uimm[5:2]=inst[12:9], uimm[7:6]=inst[8:7]
                0b110 => {
                    let rs2_bits = ((inst >> 2) & 0x1f) as u8;
                    let rs2 = Reg::from_bits(rs2_bits)?;
                    let uimm52 = ((inst >> 9) & 0xf) as u8;
                    let uimm76 = ((inst >> 7) & 0b11) as u8;
                    let uimm = (uimm76 << 6) | (uimm52 << 2);
                    Some(Self::CSwsp { rs2, uimm })
                }
                // funct3=111: C.FSWSP (Zcf, not Zca) - reserved
                _ => None,
            },

            // Quadrant 11 = 32-bit instructions
            _ => None,
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u16>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u16>() as u8
    }
}

impl<Reg> fmt::Display for Rv32ZcaInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CAddi4spn { rd, nzuimm } => write!(f, "c.addi4spn {rd}, sp, {nzuimm}"),
            Self::CLw { rd, rs1, uimm } => write!(f, "c.lw {rd}, {uimm}({rs1})"),
            Self::CSw { rs1, rs2, uimm } => write!(f, "c.sw {rs2}, {uimm}({rs1})"),
            Self::CNop => write!(f, "c.nop"),
            Self::CAddi { rd, nzimm } => write!(f, "c.addi {rd}, {nzimm}"),
            Self::CJal { imm } => write!(f, "c.jal {imm}"),
            Self::CLi { rd, imm } => write!(f, "c.li {rd}, {imm}"),
            Self::CAddi16sp { nzimm } => write!(f, "c.addi16sp sp, {nzimm}"),
            Self::CLui { rd, nzimm } => write!(f, "c.lui {rd}, 0x{:x}", nzimm >> 12),
            Self::CSrli { rd, shamt } => write!(f, "c.srli {rd}, {shamt}"),
            Self::CSrai { rd, shamt } => write!(f, "c.srai {rd}, {shamt}"),
            Self::CAndi { rd, imm } => write!(f, "c.andi {rd}, {imm}"),
            Self::CSub { rd, rs2 } => write!(f, "c.sub {rd}, {rs2}"),
            Self::CXor { rd, rs2 } => write!(f, "c.xor {rd}, {rs2}"),
            Self::COr { rd, rs2 } => write!(f, "c.or {rd}, {rs2}"),
            Self::CAnd { rd, rs2 } => write!(f, "c.and {rd}, {rs2}"),
            Self::CJ { imm } => write!(f, "c.j {imm}"),
            Self::CBeqz { rs1, imm } => write!(f, "c.beqz {rs1}, {imm}"),
            Self::CBnez { rs1, imm } => write!(f, "c.bnez {rs1}, {imm}"),
            Self::CSlli { rd, shamt } => write!(f, "c.slli {rd}, {shamt}"),
            Self::CLwsp { rd, uimm } => write!(f, "c.lwsp {rd}, {uimm}(sp)"),
            Self::CJr { rs1 } => write!(f, "c.jr {rs1}"),
            Self::CMv { rd, rs2 } => write!(f, "c.mv {rd}, {rs2}"),
            Self::CEbreak => write!(f, "c.ebreak"),
            Self::CJalr { rs1 } => write!(f, "c.jalr {rs1}"),
            Self::CAdd { rd, rs2 } => write!(f, "c.add {rd}, {rs2}"),
            Self::CSwsp { rs2, uimm } => write!(f, "c.swsp {rs2}, {uimm}(sp)"),
            Self::CUnimp => write!(f, "c.unimp"),
        }
    }
}
