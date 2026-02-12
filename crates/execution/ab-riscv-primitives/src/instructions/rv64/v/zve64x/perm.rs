//! RV64 Zve64x permutation instructions

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::VReg;
use crate::registers::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x permutation instruction
///
/// Includes integer scalar moves, slides, gathers, compress, and whole
/// register moves. Floating-point scalar moves and floating-point slides
/// are excluded (they belong to Zve64f/Zve64d).
///
/// All instructions use the OP-V major opcode (0b1010111).
#[instruction]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub(super) enum Rv64Zve64xPermInstruction<Reg> {
    /// `vmv.x.s rd, vs2` - Copy scalar element 0 of vs2 to GPR rd
    ///
    /// funct6=010000, OPMVV, vs1=00000, vm=1
    VmvXS { rd: Reg, vs2: VReg },
    /// `vmv.s.x vd, rs1` - Copy scalar GPR rs1 to element 0 of vd
    ///
    /// funct6=010000, OPMVX, vs2=00000, vm=1
    VmvSX { vd: VReg, rs1: Reg },
    /// `vslideup.vx vd, vs2, rs1, vm` - Slide elements up by scalar amount
    ///
    /// funct6=001110, OPIVX
    VslideupVx { vd: VReg,  vs2: VReg, rs1: Reg, vm: bool },
    /// `vslideup.vi vd, vs2, uimm, vm` - Slide elements up by immediate amount
    ///
    /// funct6=001110, OPIVI
    VslideupVi { vd: VReg,  vs2: VReg, uimm: u8, vm: bool },
    /// `vslidedown.vx vd, vs2, rs1, vm` - Slide elements down by scalar amount
    ///
    /// funct6=001111, OPIVX
    VslidedownVx { vd: VReg,  vs2: VReg, rs1: Reg, vm: bool },
    /// `vslidedown.vi vd, vs2, uimm, vm` - Slide elements down by immediate amount
    ///
    /// funct6=001111, OPIVI
    VslidedownVi { vd: VReg,  vs2: VReg, uimm: u8, vm: bool },
    /// `vslide1up.vx vd, vs2, rs1, vm` - Slide up by 1 and insert scalar at element 0
    ///
    /// funct6=001110, OPMVX
    Vslide1upVx { vd: VReg,  vs2: VReg, rs1: Reg, vm: bool },
    /// `vslide1down.vx vd, vs2, rs1, vm` - Slide down by 1 and insert scalar at top
    ///
    /// funct6=001111, OPMVX
    Vslide1downVx { vd: VReg,  vs2: VReg, rs1: Reg, vm: bool },
    /// `vrgather.vv vd, vs2, vs1, vm` - Gather elements from vs2 using indices in vs1
    ///
    /// funct6=001100, OPIVV
    VrgatherVv { vd: VReg,  vs2: VReg, vs1: VReg, vm: bool },
    /// `vrgather.vx vd, vs2, rs1, vm` - Gather elements from vs2 using scalar index
    ///
    /// funct6=001100, OPIVX
    VrgatherVx { vd: VReg,  vs2: VReg, rs1: Reg, vm: bool },
    /// `vrgather.vi vd, vs2, uimm, vm` - Gather elements from vs2 using immediate index
    ///
    /// funct6=001100, OPIVI
    VrgatherVi { vd: VReg,  vs2: VReg, uimm: u8, vm: bool },
    /// `vrgatherei16.vv vd, vs2, vs1, vm` - Gather with 16-bit indices
    ///
    /// funct6=001110, OPIVV
    Vrgatherei16Vv { vd: VReg,  vs2: VReg, vs1: VReg, vm: bool },
    /// `vcompress.vm vd, vs2, vs1` - Compress active elements from vs2 under mask vs1
    ///
    /// funct6=010111, OPMVV, vm=1 (always unmasked)
    VcompressVm { vd: VReg,  vs2: VReg, vs1: VReg },
    /// `vmv1r.v vd, vs2` - Whole register move (1 register)
    ///
    /// funct6=100111, OPIVI, simm5=00000, vm=1
    Vmv1rV { vd: VReg, vs2: VReg },
    /// `vmv2r.v vd, vs2` - Whole register move (2 registers)
    ///
    /// funct6=100111, OPIVI, simm5=00001, vm=1
    Vmv2rV { vd: VReg, vs2: VReg },
    /// `vmv4r.v vd, vs2` - Whole register move (4 registers)
    ///
    /// funct6=100111, OPIVI, simm5=00011, vm=1
    Vmv4rV { vd: VReg, vs2: VReg },
    /// `vmv8r.v vd, vs2` - Whole register move (8 registers)
    ///
    /// funct6=100111, OPIVI, simm5=00111, vm=1
    Vmv8rV { vd: VReg, vs2: VReg },
}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xPermInstruction<Reg>
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
        let vm = ((instruction >> 25) & 1) != 0;
        let funct6 = ((instruction >> 26) & 0b111111) as u8;

        match funct6 {
            // 010000: vmv.x.s (OPMVV) / vmv.s.x (OPMVX)
            0b010000 => match funct3 {
                // OPMVV
                0b010 => {
                    // vmv.x.s rd, vs2 - vs1=00000, vm=1
                    if vs1_bits != 0 || !vm {
                        None?;
                    }
                    let rd = Reg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    Some(Self::VmvXS { rd, vs2 })
                }
                // OPMVX
                0b110 => {
                    // vmv.s.x vd, rs1 - vs2=00000, vm=1
                    if vs2_bits != 0 || !vm {
                        None?;
                    }
                    let vd = VReg::from_bits(vd_bits)?;
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VmvSX { vd, rs1 })
                }
                _ => None,
            },
            // 001100: vrgather
            0b001100 => match funct3 {
                // OPIVV
                0b000 => {
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VrgatherVv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VrgatherVx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let uimm = vs1_bits;
                    Some(Self::VrgatherVi { vd, vs2, uimm, vm })
                }
                _ => None,
            },
            // 001110: vslideup / vslide1up / vrgatherei16
            0b001110 => match funct3 {
                // OPIVV
                0b000 => {
                    // vrgatherei16.vv
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::Vrgatherei16Vv { vd, vs2, vs1, vm })
                }
                // OPIVX
                0b100 => {
                    // vslideup.vx
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VslideupVx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    // vslideup.vi
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let uimm = vs1_bits;
                    Some(Self::VslideupVi { vd, vs2, uimm, vm })
                }
                // OPMVX
                0b110 => {
                    // vslide1up.vx
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::Vslide1upVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // 001111: vslidedown / vslide1down
            0b001111 => match funct3 {
                // OPIVX
                0b100 => {
                    // vslidedown.vx
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::VslidedownVx { vd, vs2, rs1, vm })
                }
                // OPIVI
                0b011 => {
                    // vslidedown.vi
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let uimm = vs1_bits;
                    Some(Self::VslidedownVi { vd, vs2, uimm, vm })
                }
                // OPMVX
                0b110 => {
                    // vslide1down.vx
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let rs1 = Reg::from_bits(vs1_bits)?;
                    Some(Self::Vslide1downVx { vd, vs2, rs1, vm })
                }
                _ => None,
            },
            // 010111: vcompress.vm (OPMVV)
            0b010111 => match funct3 {
                // OPMVV
                0b010 => {
                    // vcompress.vm - vm must be 1
                    if !vm {
                        None?;
                    }
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let vs1 = VReg::from_bits(vs1_bits)?;
                    Some(Self::VcompressVm { vd, vs2, vs1 })
                }
                _ => None,
            },
            // 100111: vmvNr.v (whole register move)
            0b100111 => match funct3 {
                // OPIVI
                0b011 => {
                    // vm must be 1
                    if !vm {
                        None?;
                    }
                    let vd = VReg::from_bits(vd_bits)?;
                    let vs2 = VReg::from_bits(vs2_bits)?;
                    let nr_hint = vs1_bits;
                    match nr_hint {
                        0b00000 => Some(Self::Vmv1rV { vd, vs2 }),
                        0b00001 => Some(Self::Vmv2rV { vd, vs2 }),
                        0b00011 => Some(Self::Vmv4rV { vd, vs2 }),
                        0b00111 => Some(Self::Vmv8rV { vd, vs2 }),
                        _ => None,
                    }
                }
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

impl<Reg> fmt::Display for Rv64Zve64xPermInstruction<Reg>
where
    Reg: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::VmvXS { rd, vs2 } => write!(f, "vmv.x.s {}, {}", rd, vs2),
            Self::VmvSX { vd, rs1 } => write!(f, "vmv.s.x {}, {}", vd, rs1),
            Self::VslideupVx { vd, vs2, rs1, vm } => write!(f, "vslideup.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VslideupVi { vd, vs2, uimm, vm } => write!(f, "vslideup.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            Self::VslidedownVx { vd, vs2, rs1, vm } => write!(f, "vslidedown.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VslidedownVi { vd, vs2, uimm, vm } => write!(f, "vslidedown.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            Self::Vslide1upVx { vd, vs2, rs1, vm } => write!(f, "vslide1up.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::Vslide1downVx { vd, vs2, rs1, vm } => write!(f, "vslide1down.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VrgatherVv { vd, vs2, vs1, vm } => write!(f, "vrgather.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VrgatherVx { vd, vs2, rs1, vm } => write!(f, "vrgather.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VrgatherVi { vd, vs2, uimm, vm } => write!(f, "vrgather.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
            Self::Vrgatherei16Vv { vd, vs2, vs1, vm } => write!(f, "vrgatherei16.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VcompressVm { vd, vs2, vs1 } => write!(f, "vcompress.vm {vd}, {vs2}, {vs1}"),
            Self::Vmv1rV { vd, vs2 } => write!(f, "vmv1r.v {vd}, {vs2}"),
            Self::Vmv2rV { vd, vs2 } => write!(f, "vmv2r.v {vd}, {vs2}"),
            Self::Vmv4rV { vd, vs2 } => write!(f, "vmv4r.v {vd}, {vs2}"),
            Self::Vmv8rV { vd, vs2 } => write!(f, "vmv8r.v {vd}, {vs2}"),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
