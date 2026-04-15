//! RV64 Zcb extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zcb compressed instruction set.
///
/// All register operands are prime-field (x8–x15) registers.
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZcbInstruction<Reg> {
    // Q00 loads / stores
    /// C.LBU  rd' = zero_extend(mem8\[rs1' + uimm])  uimm ∈ {0,1,2,3}
    CLbu { rd: Reg, rs1: Reg, uimm: u8 },
    /// C.LH   rd' = sign_extend(mem16\[rs1' + uimm])  uimm ∈ {0,2}
    CLh { rd: Reg, rs1: Reg, uimm: u8 },
    /// C.LHU  rd' = zero_extend(mem16\[rs1' + uimm])  uimm ∈ {0,2}
    CLhu { rd: Reg, rs1: Reg, uimm: u8 },
    /// C.SB   mem8\[rs1' + uimm] = rs2'  uimm ∈ {0,1,2,3}
    CSb { rs1: Reg, rs2: Reg, uimm: u8 },
    /// C.SH   mem16\[rs1' + uimm] = rs2'  uimm ∈ {0,2}
    CSh { rs1: Reg, rs2: Reg, uimm: u8 },

    // Q01 unary bit-manipulation
    /// C.ZEXT.B  rd' = rd' & 0xff
    CZextB { rd: Reg },
    /// C.SEXT.B  rd' = sext(rd'\[7:0])  (requires Zbb)
    CSextB { rd: Reg },
    /// C.ZEXT.H  rd' = rd' & 0xffff  (requires Zbb)
    CZextH { rd: Reg },
    /// C.SEXT.H  rd' = sext(rd'\[15:0])  (requires Zbb)
    CSextH { rd: Reg },
    /// C.ZEXT.W  rd' = rd' & 0xffff_ffff  (requires Zba)
    CZextW { rd: Reg },
    /// C.NOT  rd' = ~rd'
    CNot { rd: Reg },

    // Q01 binary
    /// C.MUL  rd' = (rd' * rs2')\[XLEN-1:0]  (requires M or Zmmul)
    CMul { rd: Reg, rs2: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZcbInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        /// Map a 3-bit "prime" register field to an absolute register number
        #[inline(always)]
        const fn prime_reg_bits(bits: u8) -> u8 {
            bits + 8
        }

        let inst = instruction as u16;
        let quadrant = inst & 0b11;
        let funct3 = ((inst >> 13) & 0b111) as u8;

        match quadrant {
            // Q00 funct3=100: C.LBU / C.LHU / C.LH / C.SB / C.SH
            0b00 if funct3 == 0b100 => {
                let sub = ((inst >> 10) & 0b111) as u8;
                let rs1_bits = prime_reg_bits(((inst >> 7) & 0b111) as u8);
                let rd_rs2_bits = prime_reg_bits(((inst >> 2) & 0b111) as u8);

                match sub {
                    // C.LBU  uimm[1]=inst[5], uimm[0]=inst[6]
                    0b000 => {
                        let uimm = ((((inst >> 5) & 1) << 1) | ((inst >> 6) & 1)) as u8;
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        let rd = Reg::from_bits(rd_rs2_bits)?;
                        Some(Self::CLbu { rd, rs1, uimm })
                    }
                    // C.LHU (funct1=inst[6]=0) / C.LH (funct1=inst[6]=1)
                    // uimm[1]=inst[5], uimm[0]=0 (halfword aligned)
                    0b001 => {
                        let funct1 = ((inst >> 6) & 1) as u8;
                        let uimm = (((inst >> 5) & 1) as u8) << 1;
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        let rd = Reg::from_bits(rd_rs2_bits)?;
                        if funct1 == 0 {
                            Some(Self::CLhu { rd, rs1, uimm })
                        } else {
                            Some(Self::CLh { rd, rs1, uimm })
                        }
                    }
                    // C.SB  uimm[1]=inst[5], uimm[0]=inst[6]
                    0b010 => {
                        let uimm = ((((inst >> 5) & 1) << 1) | ((inst >> 6) & 1)) as u8;
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        let rs2 = Reg::from_bits(rd_rs2_bits)?;
                        Some(Self::CSb { rs1, rs2, uimm })
                    }
                    // C.SH  funct1=inst[6]=0, uimm[1]=inst[5]
                    0b011 => {
                        if ((inst >> 6) & 1) != 0 {
                            None?;
                        }
                        let uimm = (((inst >> 5) & 1) as u8) << 1;
                        let rs1 = Reg::from_bits(rs1_bits)?;
                        let rs2 = Reg::from_bits(rd_rs2_bits)?;
                        Some(Self::CSh { rs1, rs2, uimm })
                    }
                    _ => None,
                }
            }

            // Q01 funct3=100, funct2[11:10]=11, bit12=1: unary ops and C.MUL
            //
            // Encoding layout (per ratified Zcb spec):
            //   funct2b = inst[6:5]
            //   0b11 => unary ops, sub-op = inst[4:2]
            //   0b10 => C.MUL, rs2' = inst[4:2]
            //   0b00, 0b01 => reserved
            0b01 if funct3 == 0b100 => {
                let funct2_11_10 = ((inst >> 10) & 0b11) as u8;
                let bit12 = (inst >> 12) & 1;

                if funct2_11_10 != 0b11 || bit12 == 0 {
                    None?;
                }

                let rd_rs1_bits = prime_reg_bits(((inst >> 7) & 0b111) as u8);
                let funct2b = ((inst >> 5) & 0b11) as u8;
                let rs2_sub = ((inst >> 2) & 0b111) as u8;

                match funct2b {
                    // Unary ops: funct2b=0b11, sub-op in inst[4:2]
                    0b11 => {
                        let rd = Reg::from_bits(rd_rs1_bits)?;
                        match rs2_sub {
                            0b000 => Some(Self::CZextB { rd }),
                            0b001 => Some(Self::CSextB { rd }),
                            0b010 => Some(Self::CZextH { rd }),
                            0b011 => Some(Self::CSextH { rd }),
                            0b100 => Some(Self::CZextW { rd }),
                            0b101 => Some(Self::CNot { rd }),
                            // 110, 111 reserved
                            _ => None,
                        }
                    }
                    // C.MUL: funct2b=0b10, rs2' = inst[4:2]
                    0b10 => {
                        let rd = Reg::from_bits(rd_rs1_bits)?;
                        let rs2 = Reg::from_bits(prime_reg_bits(rs2_sub))?;
                        Some(Self::CMul { rd, rs2 })
                    }
                    // funct2b=0b00 and funct2b=0b01 are reserved in Zcb
                    _ => None,
                }
            }

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

impl<Reg> fmt::Display for Rv64ZcbInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CLbu { rd, rs1, uimm } => write!(f, "c.lbu {rd}, {uimm}({rs1})"),
            Self::CLh { rd, rs1, uimm } => write!(f, "c.lh {rd}, {uimm}({rs1})"),
            Self::CLhu { rd, rs1, uimm } => write!(f, "c.lhu {rd}, {uimm}({rs1})"),
            Self::CSb { rs1, rs2, uimm } => write!(f, "c.sb {rs2}, {uimm}({rs1})"),
            Self::CSh { rs1, rs2, uimm } => write!(f, "c.sh {rs2}, {uimm}({rs1})"),
            Self::CZextB { rd } => write!(f, "c.zext.b {rd}"),
            Self::CSextB { rd } => write!(f, "c.sext.b {rd}"),
            Self::CZextH { rd } => write!(f, "c.zext.h {rd}"),
            Self::CSextH { rd } => write!(f, "c.sext.h {rd}"),
            Self::CZextW { rd } => write!(f, "c.zext.w {rd}"),
            Self::CNot { rd } => write!(f, "c.not {rd}"),
            Self::CMul { rd, rs2 } => write!(f, "c.mul {rd}, {rs2}"),
        }
    }
}
