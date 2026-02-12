//! RV64 Zve64x widening, narrowing, and extension instructions

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::VReg;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x widening/narrowing/extension instruction.
///
/// Includes:
/// - Widening integer add/subtract (2*SEW = SEW op SEW)
/// - Widening integer add/subtract with a wide source (2*SEW = 2*SEW op SEW)
/// - Narrowing integer right shifts (SEW = 2*SEW >> SEW)
/// - Integer zero/sign extension (vzext, vsext)
///
/// All instructions use the OP-V major opcode (0b1010111).
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xWidenNarrowInstruction<Reg> {
    // Widening unsigned integer add, 2*SEW = SEW + SEW

    /// `vwaddu.vv vd, vs2, vs1, vm`
    VwadduVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwaddu.vx vd, vs2, rs1, vm`
    VwadduVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening signed integer add, 2*SEW = SEW + SEW

    /// `vwadd.vv vd, vs2, vs1, vm`
    VwaddVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwadd.vx vd, vs2, rs1, vm`
    VwaddVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening unsigned integer subtract, 2*SEW = SEW - SEW

    /// `vwsubu.vv vd, vs2, vs1, vm`
    VwsubuVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwsubu.vx vd, vs2, rs1, vm`
    VwsubuVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening signed integer subtract, 2*SEW = SEW - SEW

    /// `vwsub.vv vd, vs2, vs1, vm`
    VwsubVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwsub.vx vd, vs2, rs1, vm`
    VwsubVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening unsigned integer add, 2*SEW = 2*SEW + SEW

    /// `vwaddu.wv vd, vs2, vs1, vm`
    VwadduWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwaddu.wx vd, vs2, rs1, vm`
    VwadduWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening signed integer add, 2*SEW = 2*SEW + SEW

    /// `vwadd.wv vd, vs2, vs1, vm`
    VwaddWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwadd.wx vd, vs2, rs1, vm`
    VwaddWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening unsigned integer subtract, 2*SEW = 2*SEW - SEW

    /// `vwsubu.wv vd, vs2, vs1, vm`
    VwsubuWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwsubu.wx vd, vs2, rs1, vm`
    VwsubuWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Widening signed integer subtract, 2*SEW = 2*SEW - SEW

    /// `vwsub.wv vd, vs2, vs1, vm`
    VwsubWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwsub.wx vd, vs2, rs1, vm`
    VwsubWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // Narrowing integer right shift logical, SEW = (2*SEW) >> SEW

    /// `vnsrl.wv vd, vs2, vs1, vm`
    VnsrlWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vnsrl.wx vd, vs2, rs1, vm`
    VnsrlWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vnsrl.wi vd, vs2, uimm, vm`
    VnsrlWi { vd: VReg, vs2: VReg, uimm: u8, vm: bool },

    // Narrowing integer right shift arithmetic, SEW = (2*SEW) >> SEW

    /// `vnsra.wv vd, vs2, vs1, vm`
    VnsraWv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vnsra.wx vd, vs2, rs1, vm`
    VnsraWx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vnsra.wi vd, vs2, uimm, vm`
    VnsraWi { vd: VReg, vs2: VReg, uimm: u8, vm: bool },

    // Integer zero-extension

    /// `vzext.vf2 vd, vs2, vm` - zero-extend SEW/2 source to SEW destination
    VzextVf2 { vd: VReg, vs2: VReg, vm: bool },
    /// `vzext.vf4 vd, vs2, vm` - zero-extend SEW/4 source to SEW destination
    VzextVf4 { vd: VReg, vs2: VReg, vm: bool },
    /// `vzext.vf8 vd, vs2, vm` - zero-extend SEW/8 source to SEW destination
    VzextVf8 { vd: VReg, vs2: VReg, vm: bool },

    // Integer sign-extension

    /// `vsext.vf2 vd, vs2, vm` - sign-extend SEW/2 source to SEW destination
    VsextVf2 { vd: VReg, vs2: VReg, vm: bool },
    /// `vsext.vf4 vd, vs2, vm` - sign-extend SEW/4 source to SEW destination
    VsextVf4 { vd: VReg, vs2: VReg, vm: bool },
    /// `vsext.vf8 vd, vs2, vm` - sign-extend SEW/8 source to SEW destination
    VsextVf8 { vd: VReg, vs2: VReg, vm: bool },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xWidenNarrowInstruction<Reg>
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
        let rs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let vs2_bits = ((instruction >> 20) & 0x1f) as u8;
        let vm = ((instruction >> 25) & 1) != 0;
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        let vd = VReg::from_bits(vd_bits)?;
        let vs2 = VReg::from_bits(vs2_bits)?;

        match funct6 {
            // Widening unsigned add, 2*SEW = SEW + SEW
            0b110000 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwadduVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwadduVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Widening signed add, 2*SEW = SEW + SEW
            0b110001 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwaddVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwaddVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Widening unsigned sub, 2*SEW = SEW - SEW
            0b110010 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwsubuVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwsubuVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Widening signed sub, 2*SEW = SEW - SEW
            0b110011 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwsubVv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwsubVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Widening unsigned add, 2*SEW = 2*SEW + SEW
            0b110100 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwadduWv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwadduWx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Widening signed add, 2*SEW = 2*SEW + SEW
            0b110101 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwaddWv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwaddWx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Widening unsigned sub, 2*SEW = 2*SEW - SEW
            0b110110 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwsubuWv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwsubuWx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Widening signed sub, 2*SEW = 2*SEW - SEW
            0b110111 => match funct3 {
                // OPMVV
                0b010 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VwsubWv { vd, vs2, vs1, vm })
                }
                // OPMVX
                0b110 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VwsubWx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // Narrowing shift right logical
            0b101100 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VnsrlWv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VnsrlWx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let uimm = vs1_bits;
                    Some(Self::VnsrlWi { vd, vs2, uimm, vm })
                }
                _ => None,
            },
            // Narrowing shift right arithmetic
            0b101101 => match funct3 {
                // OPIVV
                0b000 => {
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VnsraWv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let rs1 = Reg::from_bits(rs1_bits)?;
                    Some(Self::VnsraWx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let uimm = vs1_bits;
                    Some(Self::VnsraWi { vd, vs2, uimm, vm })
                }
                _ => None,
            },
            // VXUNARY0: integer extension instructions
            // funct6=010010, funct3=OPMVV(010)
            // vs1 field selects the specific operation
            0b010010 => match funct3 {
                // OPMVV
                0b010 => match vs1_bits {
                    0b00010 => Some(Self::VzextVf8 { vd, vs2, vm }),
                    0b00011 => Some(Self::VsextVf8 { vd, vs2, vm }),
                    0b00100 => Some(Self::VzextVf4 { vd, vs2, vm }),
                    0b00101 => Some(Self::VsextVf4 { vd, vs2, vm }),
                    0b00110 => Some(Self::VzextVf2 { vd, vs2, vm }),
                    0b00111 => Some(Self::VsextVf2 { vd, vs2, vm }),
                    _ => None,
                },
                _ => None,
            },
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

impl<Reg> fmt::Display for Rv64Zve64xWidenNarrowInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::VwadduVv { vd, vs2, vs1, vm } => write!(f, "vwaddu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwadduVx { vd, vs2, rs1, vm } => write!(f, "vwaddu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwaddVv { vd, vs2, vs1, vm } => write!(f, "vwadd.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwaddVx { vd, vs2, rs1, vm } => write!(f, "vwadd.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwsubuVv { vd, vs2, vs1, vm } => write!(f, "vwsubu.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwsubuVx { vd, vs2, rs1, vm } => write!(f, "vwsubu.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwsubVv { vd, vs2, vs1, vm } => write!(f, "vwsub.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwsubVx { vd, vs2, rs1, vm } => write!(f, "vwsub.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwadduWv { vd, vs2, vs1, vm } => write!(f, "vwaddu.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwadduWx { vd, vs2, rs1, vm } => write!(f, "vwaddu.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwaddWv { vd, vs2, vs1, vm } => write!(f, "vwadd.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwaddWx { vd, vs2, rs1, vm } => write!(f, "vwadd.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwsubuWv { vd, vs2, vs1, vm } => write!(f, "vwsubu.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwsubuWx { vd, vs2, rs1, vm } => write!(f, "vwsubu.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwsubWv { vd, vs2, vs1, vm } => write!(f, "vwsub.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwsubWx { vd, vs2, rs1, vm } => write!(f, "vwsub.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VnsrlWv { vd, vs2, vs1, vm } => write!(f, "vnsrl.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VnsrlWx { vd, vs2, rs1, vm } => write!(f, "vnsrl.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VnsrlWi { vd, vs2, uimm, vm } => write!(f, "vnsrl.wi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            Self::VnsraWv { vd, vs2, vs1, vm } => write!(f, "vnsra.wv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VnsraWx { vd, vs2, rs1, vm } => write!(f, "vnsra.wx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VnsraWi { vd, vs2, uimm, vm } => write!(f, "vnsra.wi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            Self::VzextVf2 { vd, vs2, vm } => write!(f, "vzext.vf2 {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VzextVf4 { vd, vs2, vm } => write!(f, "vzext.vf4 {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VzextVf8 { vd, vs2, vm } => write!(f, "vzext.vf8 {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VsextVf2 { vd, vs2, vm } => write!(f, "vsext.vf2 {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VsextVf4 { vd, vs2, vm } => write!(f, "vsext.vf4 {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VsextVf8 { vd, vs2, vm } => write!(f, "vsext.vf8 {vd}, {vs2}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
