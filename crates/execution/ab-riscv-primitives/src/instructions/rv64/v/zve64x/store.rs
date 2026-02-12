//! RV64 Zve64x vector store instructions

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::{Eew, VReg};
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x vector store instruction.
///
/// Encoded under the STORE-FP major opcode (0x27). All stores use rs1 (GPR) as a base address and
/// vs3 (vector register) as a source.
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xStoreInstruction<Reg> {
    /// Unit-stride store: `vse{eew}.v vs3, (rs1), vm`
    ///
    /// mop=00, sumop=00000, nf=000
    Vse { vs3: VReg, rs1: Reg, vm: bool, eew: Eew },
    /// Unit-stride mask store: `vsm.v vs3, (rs1)`
    ///
    /// mop=00, sumop=01011, nf=000, eew=e8, vm=1
    Vsm { vs3: VReg, rs1: Reg },
    /// Strided store: `vsse{eew}.v vs3, (rs1), rs2, vm`
    ///
    /// mop=10, nf=000
    Vsse { vs3: VReg, rs1: Reg, rs2: Reg, vm: bool, eew: Eew },
    /// Indexed-unordered store: `vsuxei{eew}.v vs3, (rs1), vs2, vm`
    ///
    /// mop=01, nf=000. eew is the index element width.
    Vsuxei { vs3: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew },
    /// Indexed-ordered store: `vsoxei{eew}.v vs3, (rs1), vs2, vm`
    ///
    /// mop=11, nf=000. eew is the index element width.
    Vsoxei { vs3: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew },
    /// Whole-register store: `vs{nreg}r.v vs3, (rs1)`
    ///
    /// mop=00, sumop=01000, vm=1. nreg must be 1, 2, 4, or 8.
    Vsr { vs3: VReg, rs1: Reg, nreg: u8 },
    /// Unit-stride segment store: `vsseg{nf}e{eew}.v vs3, (rs1), vm`
    ///
    /// mop=00, sumop=00000, nf>0
    Vsseg { vs3: VReg, rs1: Reg, vm: bool, eew: Eew, nf: u8 },
    /// Strided segment store: `vssseg{nf}e{eew}.v vs3, (rs1), rs2, vm`
    ///
    /// mop=10, nf>0
    Vssseg { vs3: VReg, rs1: Reg, rs2: Reg, vm: bool, eew: Eew, nf: u8 },
    /// Indexed-unordered segment store: `vsuxseg{nf}ei{eew}.v vs3, (rs1), vs2, vm`
    ///
    /// mop=01, nf>0
    Vsuxseg { vs3: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew, nf: u8 },
    /// Indexed-ordered segment store: `vsoxseg{nf}ei{eew}.v vs3, (rs1), vs2, vm`
    ///
    /// mop=11, nf>0
    Vsoxseg { vs3: VReg, rs1: Reg, vs2: VReg, vm: bool, eew: Eew, nf: u8 },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xStoreInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;

        // STORE-FP major opcode
        if opcode != 0b0100111 {
            None?;
        }

        let vs3_bits = ((instruction >> 7) & 0x1f) as u8;
        let width = ((instruction >> 12) & 0b111) as u8;
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let rs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let vm = ((instruction >> 25) & 1) != 0;
        let mop = ((instruction >> 26) & 0b11) as u8;
        let mew = ((instruction >> 28) & 1) as u8;
        let nf = ((instruction >> 29) & 0b111) as u8;

        // mew must be 0
        if mew != 0 {
            None?;
        }

        let vs3 = VReg::from_bits(vs3_bits)?;
        let rs1 = Reg::from_bits(rs1_bits)?;
        let nf_val = nf + 1;

        match mop {
            // Unit-stride
            0b00 => {
                let sumop = rs2_bits;
                match sumop {
                    // Regular unit-stride store
                    0b00000 => {
                        let eew = Eew::from_width(width)?;
                        if nf == 0 {
                            Some(Self::Vse { vs3, rs1, vm, eew })
                        } else {
                            Some(Self::Vsseg {
                                vs3,
                                rs1,
                                vm,
                                eew,
                                nf: nf_val,
                            })
                        }
                    }
                    // Whole-register store
                    0b01000 => {
                        // vm must be 1, width must be e8 (0b000)
                        if !vm || width != 0b000 {
                            None?;
                        }
                        // nreg must be 1, 2, 4, or 8
                        match nf_val {
                            1 | 2 | 4 | 8 => Some(Self::Vsr {
                                vs3,
                                rs1,
                                nreg: nf_val,
                            }),
                            _ => None,
                        }
                    }
                    // Mask store
                    0b01011 => {
                        // Must be eew=e8, vm=1, nf=0
                        if width != 0b000 || !vm || nf != 0 {
                            None?;
                        }
                        Some(Self::Vsm { vs3, rs1 })
                    }
                    _ => None,
                }
            }
            // Indexed-unordered
            0b01 => {
                let eew = Eew::from_width(width)?;
                let vs2 = VReg::from_bits(rs2_bits)?;
                if nf == 0 {
                    Some(Self::Vsuxei {
                        vs3,
                        rs1,
                        vs2,
                        vm,
                        eew,
                    })
                } else {
                    Some(Self::Vsuxseg {
                        vs3,
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
                    Some(Self::Vsse {
                        vs3,
                        rs1,
                        rs2,
                        vm,
                        eew,
                    })
                } else {
                    Some(Self::Vssseg {
                        vs3,
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
                    Some(Self::Vsoxei {
                        vs3,
                        rs1,
                        vs2,
                        vm,
                        eew,
                    })
                } else {
                    Some(Self::Vsoxseg {
                        vs3,
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

impl<Reg> fmt::Display for Rv64Zve64xStoreInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
       match self {
            Self::Vse { vs3, rs1, vm, eew } => write!(f, "vse{}.v {}, ({}){}", eew, vs3, rs1, mask_suffix(vm)),
            Self::Vsm { vs3, rs1 } => write!(f, "vsm.v {}, ({})", vs3, rs1),
            Self::Vsse { vs3, rs1, rs2, vm, eew } => write!(f, "vsse{eew}.v {vs3}, ({rs1}), {rs2}{}", mask_suffix(vm)),
            Self::Vsuxei { vs3, rs1, vs2, vm, eew } => write!(f, "vsuxei{eew}.v {vs3}, ({rs1}), {vs2}{}", mask_suffix(vm)),
            Self::Vsoxei { vs3, rs1, vs2, vm, eew } => write!(f, "vsoxei{eew}.v {vs3}, ({rs1}), {vs2}{}", mask_suffix(vm)),
            Self::Vsr { vs3, rs1, nreg } => write!(f, "vs{}r.v {}, ({})", nreg, vs3, rs1),
            Self::Vsseg { vs3, rs1, vm, eew, nf } => write!(f, "vsseg{nf}e{eew}.v {vs3}, ({rs1}){}", mask_suffix(vm)),
            Self::Vssseg { vs3, rs1, rs2, vm, eew, nf } => write!(f, "vssseg{nf}e{eew}.v {vs3}, ({rs1}), {rs2}{}", mask_suffix(vm)),
            Self::Vsuxseg { vs3, rs1, vs2, vm, eew, nf } => write!(f, "vsuxseg{nf}ei{eew}.v {vs3}, ({rs1}), {vs2}{}", mask_suffix(vm)),
            Self::Vsoxseg { vs3, rs1, vs2, vm, eew, nf } => write!(f, "vsoxseg{nf}ei{eew}.v {vs3}, ({rs1}), {vs2}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
