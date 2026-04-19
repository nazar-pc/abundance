//! RV32 Zcmp extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;
use core::hint::unreachable_unchecked;
use core::marker::PhantomData;

/// Values 0..=3 are reserved by the spec; only 4..=15 are valid.
/// Construct via [`ZcmpUrlist::try_from_raw`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ZcmpUrlistInner {
    /// {ra}
    Ra = 4,
    /// {ra, s0}
    RaS0 = 5,
    /// {ra, s0-s1}
    RaS0S1 = 6,
    /// {ra, s0-s2}
    RaS0S2 = 7,
    /// {ra, s0-s3}
    RaS0S3 = 8,
    /// {ra, s0-s4}
    RaS0S4 = 9,
    /// {ra, s0-s5}
    RaS0S5 = 10,
    /// {ra, s0-s6}
    RaS0S6 = 11,
    /// {ra, s0-s7}
    RaS0S7 = 12,
    /// {ra, s0-s8}
    RaS0S8 = 13,
    /// {ra, s0-s9}
    RaS0S9 = 14,
    /// {ra, s0-s11}
    ///
    /// Note: s10 is skipped; urlist=15 maps directly to s0-s11 per spec.
    RaS0S11 = 15,
}

/// Zcmp register list selector.
///
/// Only valid values (4..=15, further restricted to 4..=6 for RVE) are
/// representable; construct via [`ZcmpUrlist::try_from_raw`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZcmpUrlist<Reg> {
    inner: ZcmpUrlistInner,
    reg: PhantomData<Reg>,
}

impl<Reg> ZcmpUrlist<Reg>
where
    Reg: Register,
{
    const XLEN_32: u8 = 32;
    const XLEN_64: u8 = 64;

    /// Create a validated [`ZcmpUrlist`] from a raw `u8` value.
    ///
    /// Returns `None` if `raw` is reserved (0..=3), out of range (>15), or
    /// names a register list inaccessible under the current ISA variant
    /// (e.g., urlist > 6 under RVE, where only ra, s0, s1 exist).
    #[inline(always)]
    pub const fn try_from_raw(raw: u8) -> Option<Self>
    where
        Reg: [const] Register,
    {
        if !(Reg::XLEN == Self::XLEN_32 || Reg::XLEN == Self::XLEN_64) {
            return None;
        }

        let inner = if Reg::RVE {
            // RVE only has access to ra(x1), s0(x8), s1(x9)
            match raw {
                4 => ZcmpUrlistInner::Ra,
                5 => ZcmpUrlistInner::RaS0,
                6 => ZcmpUrlistInner::RaS0S1,
                _ => {
                    return None;
                }
            }
        } else {
            match raw {
                4 => ZcmpUrlistInner::Ra,
                5 => ZcmpUrlistInner::RaS0,
                6 => ZcmpUrlistInner::RaS0S1,
                7 => ZcmpUrlistInner::RaS0S2,
                8 => ZcmpUrlistInner::RaS0S3,
                9 => ZcmpUrlistInner::RaS0S4,
                10 => ZcmpUrlistInner::RaS0S5,
                11 => ZcmpUrlistInner::RaS0S6,
                12 => ZcmpUrlistInner::RaS0S7,
                13 => ZcmpUrlistInner::RaS0S8,
                14 => ZcmpUrlistInner::RaS0S9,
                15 => ZcmpUrlistInner::RaS0S11,
                _ => {
                    return None;
                }
            }
        };

        Some(Self {
            inner,
            reg: PhantomData,
        })
    }

    /// Convert to the raw `u8` discriminant (4..=15).
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self.inner as u8
    }

    /// Iterator over the absolute register numbers in this list.
    ///
    /// Order matches the spec push/pop order: ra first, then s0 ascending.
    /// ra=x1, s0=x8, s1=x9, s2=x18..s9=x25, s10=x26, s11=x27.
    ///
    /// Note: urlist=15 is {ra, s0-s11} (13 registers, including s10);
    /// {ra, s0-s10} has no encoding.
    #[inline]
    pub fn reg_list(self) -> impl Iterator<Item = Reg> {
        let regs: &[u8] = match self.inner {
            ZcmpUrlistInner::Ra => &[1],
            ZcmpUrlistInner::RaS0 => &[1, 8],
            ZcmpUrlistInner::RaS0S1 => &[1, 8, 9],
            ZcmpUrlistInner::RaS0S2 => &[1, 8, 9, 18],
            ZcmpUrlistInner::RaS0S3 => &[1, 8, 9, 18, 19],
            ZcmpUrlistInner::RaS0S4 => &[1, 8, 9, 18, 19, 20],
            ZcmpUrlistInner::RaS0S5 => &[1, 8, 9, 18, 19, 20, 21],
            ZcmpUrlistInner::RaS0S6 => &[1, 8, 9, 18, 19, 20, 21, 22],
            ZcmpUrlistInner::RaS0S7 => &[1, 8, 9, 18, 19, 20, 21, 22, 23],
            ZcmpUrlistInner::RaS0S8 => &[1, 8, 9, 18, 19, 20, 21, 22, 23, 24],
            ZcmpUrlistInner::RaS0S9 => &[1, 8, 9, 18, 19, 20, 21, 22, 23, 24, 25],
            ZcmpUrlistInner::RaS0S11 => &[1, 8, 9, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27],
        };

        regs.iter().map(|&bits| {
            // SAFETY: constructor invariant guarantees Reg::from_bits returns
            // Some for all register numbers in these lists
            unsafe { Reg::from_bits(bits).unwrap_unchecked() }
        })
    }

    /// Stack adjustment base in bytes.
    ///
    /// The minimum stack frame size for this register list, rounded up to a 16-byte alignment. The
    /// full stack adjustment is: `stack_adj_base + spimm * 16`.
    ///
    /// Values sourced from the Zcmp spec Table 3.
    #[inline(always)]
    pub const fn stack_adj_base(self) -> u32 {
        match Reg::XLEN {
            // RV32: each register is 4 bytes; base = ceil(n_regs * 4 / 16) * 16
            Self::XLEN_32 => match self.inner {
                ZcmpUrlistInner::Ra
                | ZcmpUrlistInner::RaS0
                | ZcmpUrlistInner::RaS0S1
                | ZcmpUrlistInner::RaS0S2 => 16,
                ZcmpUrlistInner::RaS0S3
                | ZcmpUrlistInner::RaS0S4
                | ZcmpUrlistInner::RaS0S5
                | ZcmpUrlistInner::RaS0S6 => 32,
                ZcmpUrlistInner::RaS0S7 | ZcmpUrlistInner::RaS0S8 | ZcmpUrlistInner::RaS0S9 => 48,
                ZcmpUrlistInner::RaS0S11 => 64,
            },
            // RV64: each register is 8 bytes; base = ceil(n_regs * 8 / 16) * 16
            Self::XLEN_64 => match self.inner {
                ZcmpUrlistInner::Ra | ZcmpUrlistInner::RaS0 => 16,
                ZcmpUrlistInner::RaS0S1 | ZcmpUrlistInner::RaS0S2 => 32,
                ZcmpUrlistInner::RaS0S3 | ZcmpUrlistInner::RaS0S4 => 48,
                ZcmpUrlistInner::RaS0S5 | ZcmpUrlistInner::RaS0S6 => 64,
                ZcmpUrlistInner::RaS0S7 | ZcmpUrlistInner::RaS0S8 => 80,
                ZcmpUrlistInner::RaS0S9 => 96,
                ZcmpUrlistInner::RaS0S11 => 112,
            },
            _ => {
                // SAFETY: Invariant protected by constructor guarantees that `Reg::XLEN` is one of
                // the two above values
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

impl<Reg> fmt::Display for ZcmpUrlist<Reg> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            ZcmpUrlistInner::Ra => write!(f, "{{ra}}"),
            ZcmpUrlistInner::RaS0 => write!(f, "{{ra, s0}}"),
            ZcmpUrlistInner::RaS0S1 => write!(f, "{{ra, s0-s1}}"),
            ZcmpUrlistInner::RaS0S2 => write!(f, "{{ra, s0-s2}}"),
            ZcmpUrlistInner::RaS0S3 => write!(f, "{{ra, s0-s3}}"),
            ZcmpUrlistInner::RaS0S4 => write!(f, "{{ra, s0-s4}}"),
            ZcmpUrlistInner::RaS0S5 => write!(f, "{{ra, s0-s5}}"),
            ZcmpUrlistInner::RaS0S6 => write!(f, "{{ra, s0-s6}}"),
            ZcmpUrlistInner::RaS0S7 => write!(f, "{{ra, s0-s7}}"),
            ZcmpUrlistInner::RaS0S8 => write!(f, "{{ra, s0-s8}}"),
            ZcmpUrlistInner::RaS0S9 => write!(f, "{{ra, s0-s9}}"),
            ZcmpUrlistInner::RaS0S11 => write!(f, "{{ra, s0-s11}}"),
        }
    }
}

/// Zcmp compressed instruction set
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZcmpInstruction<Reg> {
    /// CM.PUSH - push reg_list, decrement sp by `stack_adj`
    ///
    /// `stack_adj = urlist.stack_adj_base() + spimm * 16` from the encoding.
    CmPush {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u32,
    },
    /// CM.POP - pop reg_list, increment sp by `stack_adj` (no return)
    CmPop {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u32,
    },
    /// CM.POPRETZ - pop reg_list, set a0=0, increment sp, return
    CmPopretz {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u32,
    },
    /// CM.POPRET - pop reg_list, increment sp, return
    CmPopret {
        urlist: ZcmpUrlist<Reg>,
        stack_adj: u32,
    },
    /// CM.MVA01S - a0 = r1s', a1 = r2s'
    CmMva01s { r1s: Reg, r2s: Reg },
    /// CM.MVSA01 - r1s' = a0, r2s' = a1  (r1s' != r2s')
    CmMvsa01 { r1s: Reg, r2s: Reg },
}

#[instruction]
impl<Reg> const Instruction for Rv32ZcmpInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
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
                let spimm = ((inst >> 2) & 0b11) as u32;
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

impl<Reg> fmt::Display for Rv32ZcmpInstruction<Reg>
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
