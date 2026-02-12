//! RV64 Zve64x mask instructions

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x mask instruction.
///
/// Includes mask-register logical operations (Section 16.1), mask population count and find-first
/// (Sections 16.2-16.3), mask set-before/including/only-first (Sections 16.4-16.6), iota
/// (Section 16.8), and element index (Section 16.9).
///
/// All use the OP-V major opcode (0b1010111).
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xMaskInstruction<Reg> {
    /// `vmandn.mm vd, vs2, vs1` - vd = vs2 AND NOT vs1
    ///
    /// funct6=011000, OPMVV, vm=1
    Vmandn { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmand.mm vd, vs2, vs1` - vd = vs2 AND vs1
    ///
    /// funct6=011001, OPMVV, vm=1
    Vmand { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmor.mm vd, vs2, vs1` - vd = vs2 OR vs1
    ///
    /// funct6=011010, OPMVV, vm=1
    Vmor { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmxor.mm vd, vs2, vs1` - vd = vs2 XOR vs1
    ///
    /// funct6=011011, OPMVV, vm=1
    Vmxor { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmorn.mm vd, vs2, vs1` - vd = vs2 OR NOT vs1
    ///
    /// funct6=011100, OPMVV, vm=1
    Vmorn { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmnand.mm vd, vs2, vs1` - vd = NOT(vs2 AND vs1)
    ///
    /// funct6=011101, OPMVV, vm=1
    Vmnand { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmnor.mm vd, vs2, vs1` - vd = NOT(vs2 OR vs1)
    ///
    /// funct6=011110, OPMVV, vm=1
    Vmnor { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vmxnor.mm vd, vs2, vs1` - vd = NOT(vs2 XOR vs1)
    ///
    /// funct6=011111, OPMVV, vm=1
    Vmxnor { vd: VReg, vs2: VReg, vs1: VReg },
    /// `vcpop.m rd, vs2, vm` - rd = population count of mask vs2
    ///
    /// funct6=010000, OPMVV, vs1=10000
    Vcpop { rd: Reg, vs2: VReg, vm: bool },
    /// `vfirst.m rd, vs2, vm` - rd = index of first set bit in mask vs2, or -1
    ///
    /// funct6=010000, OPMVV, vs1=10001
    Vfirst { rd: Reg, vs2: VReg, vm: bool },
    /// `vmsbf.m vd, vs2, vm` - set-before-first mask bit
    ///
    /// funct6=010100, OPMVV, vs1=00001
    Vmsbf { vd: VReg, vs2: VReg, vm: bool },
    /// `vmsof.m vd, vs2, vm` - set-only-first mask bit
    ///
    /// funct6=010100, OPMVV, vs1=00010
    Vmsof { vd: VReg, vs2: VReg, vm: bool },
    /// `vmsif.m vd, vs2, vm` - set-including-first mask bit
    ///
    /// funct6=010100, OPMVV, vs1=00011
    Vmsif { vd: VReg, vs2: VReg, vm: bool },
    /// `viota.m vd, vs2, vm` - iota: vd\[i] = popcount of vs2\[0..i-1]
    ///
    /// funct6=010100, OPMVV, vs1=10000
    Viota { vd: VReg, vs2: VReg, vm: bool },
    /// `vid.v vd, vm` - vector element index: vd\[i] = i
    ///
    /// funct6=010100, OPMVV, vs1=10001, vs2=00000
    Vid { vd: VReg, vm: bool },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xMaskInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;

        // OP-V major opcode
        if opcode != 0b1010111 {
            None?;
        }

        let vd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let vs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let vs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let vm_bit = ((instruction >> 25) & 1) as u8;
        let funct6 = ((instruction >> 26) & 0b111111) as u8;

        // All mask instructions use OPMVV (funct3 = 0b010)
        if funct3 != 0b010 {
            None?;
        }

        match funct6 {
            // Mask-register logical instructions (Section 16.1)
            // These always have vm=1 (unmasked)
            0b011000..=0b011111 => {
                if vm_bit != 1 {
                    None?;
                }
                let vd = VReg::from_bits(vd_bits)?;
                let vs2 = VReg::from_bits(vs2_bits)?;
                let vs1 = VReg::from_bits(vs1_bits)?;
                match funct6 {
                    0b011000 => Some(Self::Vmandn { vd, vs2, vs1 }),
                    0b011001 => Some(Self::Vmand { vd, vs2, vs1 }),
                    0b011010 => Some(Self::Vmor { vd, vs2, vs1 }),
                    0b011011 => Some(Self::Vmxor { vd, vs2, vs1 }),
                    0b011100 => Some(Self::Vmorn { vd, vs2, vs1 }),
                    0b011101 => Some(Self::Vmnand { vd, vs2, vs1 }),
                    0b011110 => Some(Self::Vmnor { vd, vs2, vs1 }),
                    0b011111 => Some(Self::Vmxnor { vd, vs2, vs1 }),
                    _ => None,
                }
            }
            // VWXUNARY0: vcpop.m, vfirst.m
            // funct6=010000, result written to x register rd
            0b010000 => {
                let vm = vm_bit == 1;
                let rd = Reg::from_bits(vd_bits)?;
                let vs2 = VReg::from_bits(vs2_bits)?;
                match vs1_bits {
                    0b10000 => Some(Self::Vcpop { rd, vs2, vm }),
                    0b10001 => Some(Self::Vfirst { rd, vs2, vm }),
                    _ => None,
                }
            }
            // VMUNARY0: vmsbf.m, vmsof.m, vmsif.m, viota.m, vid.v
            // funct6=010100, result written to vector register vd
            0b010100 => {
                let vm = vm_bit == 1;
                let vd = VReg::from_bits(vd_bits)?;
                match vs1_bits {
                    0b00001 => {
                        let vs2 = VReg::from_bits(vs2_bits)?;
                        Some(Self::Vmsbf { vd, vs2, vm })
                    }
                    0b00010 => {
                        let vs2 = VReg::from_bits(vs2_bits)?;
                        Some(Self::Vmsof { vd, vs2, vm })
                    }
                    0b00011 => {
                        let vs2 = VReg::from_bits(vs2_bits)?;
                        Some(Self::Vmsif { vd, vs2, vm })
                    }
                    0b10000 => {
                        let vs2 = VReg::from_bits(vs2_bits)?;
                        Some(Self::Viota { vd, vs2, vm })
                    }
                    0b10001 => Some(Self::Vid { vd, vm }),
                    _ => None,
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

impl<Reg> fmt::Display for Rv64Zve64xMaskInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::Vmandn { vd, vs2, vs1 } => write!(f, "vmandn.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vmand { vd, vs2, vs1 } => write!(f, "vmand.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vmor { vd, vs2, vs1 } => write!(f, "vmor.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vmxor { vd, vs2, vs1 } => write!(f, "vmxor.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vmorn { vd, vs2, vs1 } => write!(f, "vmorn.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vmnand { vd, vs2, vs1 } => write!(f, "vmnand.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vmnor { vd, vs2, vs1 } => write!(f, "vmnor.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vmxnor { vd, vs2, vs1 } => write!(f, "vmxnor.mm {}, {}, {}", vd, vs2, vs1),
            Self::Vcpop { rd, vs2, vm } => write!(f, "vcpop.m {rd}, {vs2}{}", mask_suffix(vm)),
            Self::Vfirst { rd, vs2, vm } => write!(f, "vfirst.m {rd}, {vs2}{}", mask_suffix(vm)),
            Self::Vmsbf { vd, vs2, vm } => write!(f, "vmsbf.m {vd}, {vs2}{}", mask_suffix(vm)),
            Self::Vmsof { vd, vs2, vm } => write!(f, "vmsof.m {vd}, {vs2}{}", mask_suffix(vm)),
            Self::Vmsif { vd, vs2, vm } => write!(f, "vmsif.m {vd}, {vs2}{}", mask_suffix(vm)),
            Self::Viota { vd, vs2, vm } => write!(f, "viota.m {vd}, {vs2}{}", mask_suffix(vm)),
            Self::Vid { vd, vm } => write!(f, "vid.v {vd}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
