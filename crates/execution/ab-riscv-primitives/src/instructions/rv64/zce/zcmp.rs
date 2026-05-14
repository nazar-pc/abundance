//! RV64 Zcmp extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::rv32::zce::zcmp::{ZcmpRegister, ZcmpUrlist};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// Zcmp compressed instruction set
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZcmpInstruction<Reg> {
    /// CM.PUSH - push reg_list, decrement sp by `stack_adj`
    ///
    /// `stack_adj = urlist.stack_adj_base() + spimm * 16` from the encoding.
    CmPush {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u8,
    },
    /// CM.POP - pop reg_list, increment sp by `stack_adj` (no return)
    CmPop {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u8,
    },
    /// CM.POPRETZ - pop reg_list, set a0=0, increment sp, return
    CmPopretz {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u8,
    },
    /// CM.POPRET - pop reg_list, increment sp, return
    CmPopret {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u8,
    },
    /// CM.MVA01S - a0 = r1s', a1 = r2s'
    CmMva01s { r1s: Reg, r2s: Reg },
    /// CM.MVSA01 - r1s' = a0, r2s' = a1  (r1s' != r2s')
    CmMvsa01 { r1s: Reg, r2s: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv64ZcmpInstruction<Reg>
where
    Reg: [const] ZcmpRegister<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        /// Map the Zcmp 3-bit "s-register" field to an absolute register number.
        /// 000->x8(s0), 001->x9(s1), 010->x18(s2)..111->x23(s7)
        #[inline(always)]
        const fn sreg_bits(field: u8) -> u8 {
            match field {
                0 => 8,
                1 => 9,
                f => f + 16,
            }
        }

        let inst = instruction as u16;
        let quadrant = inst & 0b11;
        let funct3 = ((inst >> 13) & 0b111) as u8;

        // All Zcmp instructions: Q10, funct3=101
        if quadrant != 0b10 || funct3 != 0b101 {
            None?;
        }

        let funct2_12_11 = ((inst >> 11) & 0b11) as u8;

        match funct2_12_11 {
            // CM.PUSH / CM.POP / CM.POPRETZ / CM.POPRET
            0b11 => {
                let op_sel = ((inst >> 9) & 0b11) as u8;
                let urlist = ZcmpUrlist::try_from_raw(((inst >> 4) & 0xf) as u8)?;
                let spimm = ((inst >> 2) & 0b11) as u8;
                let stack_adj = urlist.stack_adj_base() + spimm * 16;
                match op_sel {
                    0b00 => Some(Self::CmPush { urlist, stack_adj }),
                    0b01 => Some(Self::CmPop { urlist, stack_adj }),
                    0b10 => Some(Self::CmPopretz { urlist, stack_adj }),
                    0b11 => Some(Self::CmPopret { urlist, stack_adj }),
                    _ => None,
                }
            }
            // CM.MVA01S / CM.MVSA01: require bit 10 = 1 (full funct6 = 101_011)
            0b01 => {
                if (inst >> 10) & 1 != 1 {
                    None?;
                }

                let r1s_bits = ((inst >> 7) & 0b111) as u8;
                let funct2 = ((inst >> 5) & 0b11) as u8;
                let r2s_bits = ((inst >> 2) & 0b111) as u8;

                // Reg::from_bits returns None for registers inaccessible in the current ISA
                // variant. Under RVE this covers field > 1 (i.e. r1sc/r2sc > 1 in the spec
                // pseudocode), which maps to x18-x23 - registers that do not exist in the E
                // extension.
                let r1s = Reg::from_bits(sreg_bits(r1s_bits))?;
                let r2s = Reg::from_bits(sreg_bits(r2s_bits))?;

                // funct2[6:5]: 0b11 -> CM.MVA01S, 0b01 -> CM.MVSA01, others reserved
                match funct2 {
                    0b11 => Some(Self::CmMva01s { r1s, r2s }),
                    0b01 => {
                        // CM.MVSA01 requires r1s' != r2s'
                        if r1s_bits == r2s_bits {
                            None?;
                        }
                        Some(Self::CmMvsa01 { r1s, r2s })
                    }
                    _ => None,
                }
            }
            // funct2_12_11 values 0b00 and 0b10 are not defined by Zcmp
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

impl<Reg> fmt::Display for Rv64ZcmpInstruction<Reg>
where
    Reg: Register,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CmPush { urlist, stack_adj } => {
                write!(f, "cm.push {urlist}, -{stack_adj}")
            }
            Self::CmPop { urlist, stack_adj } => {
                write!(f, "cm.pop {urlist}, {stack_adj}")
            }
            Self::CmPopretz { urlist, stack_adj } => {
                write!(f, "cm.popretz {urlist}, {stack_adj}")
            }
            Self::CmPopret { urlist, stack_adj } => {
                write!(f, "cm.popret {urlist}, {stack_adj}")
            }
            Self::CmMva01s { r1s, r2s } => write!(f, "cm.mva01s {r1s}, {r2s}"),
            Self::CmMvsa01 { r1s, r2s } => write!(f, "cm.mvsa01 {r1s}, {r2s}"),
        }
    }
}
