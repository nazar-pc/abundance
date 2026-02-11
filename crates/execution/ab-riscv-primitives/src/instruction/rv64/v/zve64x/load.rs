//! RV64 Zve64x vector load instructions

#[cfg(test)]
mod tests;

use crate::instruction::Instruction;
use crate::instruction::rv64::v::zve64x::{Eew, VReg};
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x vector load instruction.
///
/// Encoded under the LOAD-FP major opcode (0x07). All loads use rs1 (GPR) as a base address and vd
/// (vector register) as a destination.
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xLoadInstruction<Reg> {
    /// Unit-stride load: `vle{eew}.v vd, (rs1), vm`
    ///
    /// mop=00, lumop=00000, nf=000
    Vle { vd: VReg, rs1: Reg, vm: bool, eew: Eew },
    /// Unit-stride fault-only-first load: `vle{eew}ff.v vd, (rs1), vm`
    ///
    /// mop=00, lumop=10000, nf=000
    Vleff { vd: VReg, rs1: Reg, vm: bool, eew: Eew },
    /// Unit-stride mask load: `vlm.v vd, (rs1)`
    ///
    /// mop=00, lumop=01011, nf=000, eew=e8, vm=1
    Vlm { vd: VReg, rs1: Reg },
    /// Strided load: `vlse{eew}.v vd, (rs1), rs2, vm`
    ///
    /// mop=10, nf=000
    Vlse { vd: VReg, rs1: Reg, rs2: Reg, vm: bool, eew: Eew },
    /// Indexed-unordered load: `vluxei{eew}.v vd, (rs1), vs2, vm`
    ///
    /// mop=01, nf=000. eew is the index element width.
    Vluxei { vd: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew },
    /// Indexed-ordered load: `vloxei{eew}.v vd, (rs1), vs2, vm`
    ///
    /// mop=11, nf=000. eew is the index element width.
    Vloxei { vd: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew },
    /// Whole-register load: `vl{nreg}re{eew}.v vd, (rs1)`
    ///
    /// mop=00, lumop=01000, vm=1. nreg must be 1, 2, 4, or 8.
    Vlr { vd: VReg, rs1: Reg, nreg: u8, eew: Eew },
    /// Unit-stride segment load: `vlseg{nf}e{eew}.v vd, (rs1), vm`
    ///
    /// mop=00, lumop=00000, nf>0
    Vlseg { vd: VReg, rs1: Reg, vm: bool, eew: Eew, nf: u8 },
    /// Unit-stride fault-only-first segment load: `vlseg{nf}e{eew}ff.v vd, (rs1), vm`
    ///
    /// mop=00, lumop=10000, nf>0
    Vlsegff { vd: VReg, rs1: Reg, vm: bool, eew: Eew, nf: u8 },
    /// Strided segment load: `vlsseg{nf}e{eew}.v vd, (rs1), rs2, vm`
    ///
    /// mop=10, nf>0
    Vlsseg { vd: VReg, rs1: Reg, rs2: Reg, vm: bool, eew: Eew, nf: u8 },
    /// Indexed-unordered segment load: `vluxseg{nf}ei{eew}.v vd, (rs1), vs2, vm`
    ///
    /// mop=01, nf>0
    Vluxseg { vd: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew, nf: u8 },
    /// Indexed-ordered segment load: `vloxseg{nf}ei{eew}.v vd, (rs1), vs2, vm`
    ///
    /// mop=11, nf>0
    Vloxseg { vd: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew, nf: u8 },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xLoadInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;

        // LOAD-FP major opcode
        if opcode != 0b0000111 {
            None?;
        }

        let vd_bits = ((instruction >> 7) & 0x1f) as u8;
        let width = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let vm = ((instruction >> 25) & 1) != 0;
        let mop = ((instruction >> 26) & 0b11) as u8;
        let mew = ((instruction >> 28) & 1) as u8;
        let nf = ((instruction >> 29) & 0b111) as u8;

        // mew must be 0 (reserved for >=128-bit)
        if mew != 0 {
            None?;
        }

        let vd = VReg::from_bits(vd_bits)?;
        let rs1 = Reg::from_bits(rs1_bits)?;

        // nf encodes number of fields minus 1 (nf=0 means 1 field)
        let nf_val = nf + 1;

        match mop {
            // Unit-stride
            0b00 => {
                let lumop = rs2_bits;
                match lumop {
                    // Regular unit-stride load
                    0b00000 => {
                        let eew = Eew::from_width(width)?;
                        if nf == 0 {
                            Some(Self::Vle { vd, rs1, vm, eew })
                        } else {
                            Some(Self::Vlseg {
                                vd,
                                rs1,
                                vm,
                                eew,
                                nf: nf_val,
                            })
                        }
                    }
                    // Whole-register load
                    0b01000 => {
                        // vm must be 1 (unmasked)
                        if !vm {
                            None?;
                        }
                        let eew = Eew::from_width(width)?;
                        // nf encodes nreg: nf+1 must be 1, 2, 4, or 8
                        match nf_val {
                            1 | 2 | 4 | 8 => Some(Self::Vlr {
                                vd,
                                rs1,
                                nreg: nf_val,
                                eew,
                            }),
                            _ => None,
                        }
                    }
                    // Mask load
                    0b01011 => {
                        // Must be eew=e8, vm=1, nf=0
                        if width != 0b000 || !vm || nf != 0 {
                            None?;
                        }
                        Some(Self::Vlm { vd, rs1 })
                    }
                    // Fault-only-first
                    0b10000 => {
                        let eew = Eew::from_width(width)?;
                        if nf == 0 {
                            Some(Self::Vleff { vd, rs1, vm, eew })
                        } else {
                            Some(Self::Vlsegff {
                                vd,
                                rs1,
                                vm,
                                eew,
                                nf: nf_val,
                            })
                        }
                    }
                    _ => None,
                }
            }
            // Indexed-unordered
            0b01 => {
                let eew = Eew::from_width(width)?;
                let vs2 = VReg::from_bits(rs2_bits)?;
                if nf == 0 {
                    Some(Self::Vluxei {
                        vd,
                        rs1,
                        vs2,
                        vm,
                        eew,
                    })
                } else {
                    Some(Self::Vluxseg {
                        vd,
                        rs1,
                        vs2,
                        vm,
                        eew,
                        nf: nf_val,
                    })
                }
            }
            // Strided
            0b10 => {
                let eew = Eew::from_width(width)?;
                let rs2 = Reg::from_bits(rs2_bits)?;
                if nf == 0 {
                    Some(Self::Vlse {
                        vd,
                        rs1,
                        rs2,
                        vm,
                        eew,
                    })
                } else {
                    Some(Self::Vlsseg {
                        vd,
                        rs1,
                        rs2,
                        vm,
                        eew,
                        nf: nf_val,
                    })
                }
            }
            // Indexed-ordered
            0b11 => {
                let eew = Eew::from_width(width)?;
                let vs2 = VReg::from_bits(rs2_bits)?;
                if nf == 0 {
                    Some(Self::Vloxei {
                        vd,
                        rs1,
                        vs2,
                        vm,
                        eew,
                    })
                } else {
                    Some(Self::Vloxseg {
                        vd,
                        rs1,
                        vs2,
                        vm,
                        eew,
                        nf: nf_val,
                    })
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

impl<Reg> fmt::Display for Rv64Zve64xLoadInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::Vle { vd, rs1, vm, eew } => write!(f, "vle{eew}.v {vd}, ({rs1}){}", mask_suffix(vm)),
            Self::Vleff { vd, rs1, vm, eew } => write!(f, "vle{eew}ff.v {vd}, ({rs1}){}", mask_suffix(vm)),
            Self::Vlm { vd, rs1 } => write!(f, "vlm.v {vd}, ({rs1})"),
            Self::Vlse { vd, rs1, rs2, vm, eew } => write!(f, "vlse{eew}.v {vd}, ({rs1}), {rs2}{}", mask_suffix(vm)),
            Self::Vluxei { vd, rs1, vs2, vm, eew } => write!(f, "vluxei{eew}.v {vd}, ({rs1}), {vs2}{}", mask_suffix(vm)),
            Self::Vloxei { vd, rs1, vs2, vm, eew } => write!(f, "vloxei{eew}.v {vd}, ({rs1}), {vs2}{}", mask_suffix(vm)),
            Self::Vlr { vd, rs1, nreg, eew } => write!(f, "vl{}re{}.v {}, ({})", nreg, eew, vd, rs1),
            Self::Vlseg { vd, rs1, vm, eew, nf } => write!(f, "vlseg{nf}e{eew}.v {vd}, ({rs1}){}", mask_suffix(vm)),
            Self::Vlsegff { vd, rs1, vm, eew, nf } => write!(f, "vlseg{nf}e{eew}ff.v {vd}, ({rs1}){}", mask_suffix(vm)),
            Self::Vlsseg { vd, rs1, rs2, vm, eew, nf } => write!(f, "vlsseg{nf}e{eew}.v {vd}, ({rs1}), {rs2}{}", mask_suffix(vm)),
            Self::Vluxseg { vd, rs1, vs2, vm, eew, nf } => write!(f, "vluxseg{nf}ei{eew}.v {vd}, ({rs1}), {vs2}{}", mask_suffix(vm)),
            Self::Vloxseg { vd, rs1, vs2, vm, eew, nf } => write!(f, "vloxseg{nf}ei{eew}.v {vd}, ({rs1}), {vs2}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
