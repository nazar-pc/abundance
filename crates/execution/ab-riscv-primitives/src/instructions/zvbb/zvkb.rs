//! Zvkb extension

#[cfg(test)]
mod tests;

use crate::instructions::Instruction;
use crate::instructions::v::Eew;
use crate::instructions::v::zvexx::arith::ZveXxArithInstruction;
use crate::instructions::v::zvexx::carry::ZveXxCarryInstruction;
use crate::instructions::v::zvexx::config::ZveXxConfigInstruction;
use crate::instructions::v::zvexx::fixed_point::ZveXxFixedPointInstruction;
use crate::instructions::v::zvexx::load::{LoadStoreNreg, Nf, SegVmNf, ZveXxLoadInstruction};
use crate::instructions::v::zvexx::mask::ZveXxMaskInstruction;
use crate::instructions::v::zvexx::muldiv::ZveXxMulDivInstruction;
use crate::instructions::v::zvexx::perm::ZveXxPermInstruction;
use crate::instructions::v::zvexx::reduction::ZveXxReductionInstruction;
use crate::instructions::v::zvexx::store::ZveXxStoreInstruction;
use crate::instructions::v::zvexx::widen_narrow::ZveXxWidenNarrowInstruction;
use crate::instructions::zicsr::ZicsrInstruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V Zvkb vector cryptography bit-manipulation instruction.
///
/// All use the OP-V major opcode (0b101_0111). Encoding spaces:
///
/// - `vandn.[vv,vx]`: funct6=0b000001, OPIVV/OPIVX; `vm` controls masking (0=masked, 1=unmasked)
/// - `vbrev8.v`:      funct6=0b010010, OPMVV, vs1=0b01000; `vm` controls masking
/// - `vrev8.v`:       funct6=0b010010, OPMVV, vs1=0b01001; `vm` controls masking
/// - `vrol.[vv,vx]`:  funct6=0b010101, OPIVV/OPIVX; `vm` controls masking
/// - `vror.[vv,vx]`:  funct6=0b010100, OPIVV/OPIVX; `vm` controls masking
/// - `vror.vi`:       funct6=0b010100, OPIVI; 5-bit unsigned immediate in bits\[19:15] (0-31);
///   bit\[25] is the standard `vm` field, independent of the immediate
///
/// For instructions with a `vm` field: `vm=true` means unmasked (process all body elements);
/// `vm=false` means masked by v0 (skip elements where v0\[i]=0, leaving them undisturbed).
#[instruction(
    inherit = [ZveXxInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub enum ZvkbInstruction<Reg> {
    // vandn: vd[i] = ~vs1[i] & vs2[i]  (or ~rs1 & vs2[i])
    // Essential for the Chi step of the Keccak permutation (SHA-3).

    /// `vandn.vv vd, vs2, vs1, vm`
    VandnVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vandn.vx vd, vs2, rs1, vm`
    VandnVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // vbrev8: reverse bits within each byte of each SEW-wide element

    /// `vbrev8.v vd, vs2, vm`
    Vbrev8V { vd: VReg, vs2: VReg, vm: bool },

    // vrev8: reverse bytes within each SEW-wide element

    /// `vrev8.v vd, vs2, vm`
    Vrev8V  { vd: VReg, vs2: VReg, vm: bool },

    // vrol: rotate left; no immediate form (use vror.vi with negated immediate instead)

    /// `vrol.vv vd, vs2, vs1, vm`
    VrolVv  { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vrol.vx vd, vs2, rs1, vm`
    VrolVx  { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },

    // vror: rotate right
    // vror.vi has a 5-bit unsigned immediate in vs1[19:15]; bit[25] is the standard `vm`
    // mask-control bit

    /// `vror.vv vd, vs2, vs1, vm`
    VrorVv  { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vror.vx vd, vs2, rs1, vm`
    VrorVx  { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    /// `vror.vi vd, vs2, uimm, vm` - `uimm` is 5-bit unsigned (0..=31); `vm` is the
    /// standard mask-control bit at bit\[25], orthogonal to the immediate.
    VrorVi  { vd: VReg, vs2: VReg, uimm: u8, vm: bool },
}

#[instruction]
const impl<Reg> Instruction for ZvkbInstruction<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        let opcode = (instruction & 0b111_1111) as u8;

        if opcode != 0b101_0111 {
            None?;
        }

        let vd_bits = ((instruction >> 7) & 0x1f) as u8;
        let funct3 = ((instruction >> 12) & 0b111) as u8;
        let vs1_bits = ((instruction >> 15) & 0x1f) as u8;
        let vs2_bits = ((instruction >> 20) & 0x1f) as u8;
        // vm=1 means unmasked, vm=0 means masked by v0
        let vm = ((instruction >> 25) & 1) as u8 == 1;
        let funct6 = ((instruction >> 26) & 0b11_1111) as u8;

        let vd = VReg::from_bits(vd_bits)?;
        let vs2 = VReg::from_bits(vs2_bits)?;

        match funct3 {
            // OPIVV: vandn.vv, vrol.vv, vror.vv
            0b000 => {
                let vs1 = VReg::from_bits(vs1_bits)?;
                match funct6 {
                    0b00_0001 => Some(Self::VandnVv { vd, vs2, vs1, vm }),
                    0b01_0100 => Some(Self::VrorVv { vd, vs2, vs1, vm }),
                    0b01_0101 => Some(Self::VrolVv { vd, vs2, vs1, vm }),
                    _ => None,
                }
            }
            // OPIVX: vandn.vx, vrol.vx, vror.vx
            0b100 => {
                let rs1 = Reg::from_bits(vs1_bits)?;
                match funct6 {
                    0b00_0001 => Some(Self::VandnVx { vd, vs2, rs1, vm }),
                    0b01_0100 => Some(Self::VrorVx { vd, vs2, rs1, vm }),
                    0b01_0101 => Some(Self::VrolVx { vd, vs2, rs1, vm }),
                    _ => None,
                }
            }
            // OPIVI: vror.vi only - 5-bit unsigned immediate in vs1[19:15]; bit[25] is the standard
            // vm mask-control bit
            0b011 => {
                if funct6 != 0b01_0100 {
                    None?;
                }
                // vm_bit here is imm[5]; reconstruct 6-bit unsigned immediate
                // uimm is 5-bit (0-31); bit[25] is a standard vm field, not imm[5]
                let uimm = vs1_bits;
                Some(Self::VrorVi { vd, vs2, uimm, vm })
            }
            // OPMVV: vbrev8.v and vrev8.v - unary, vs1 encodes the sub-operation
            // funct6=0b010010 (VXUNARY0 sub-space); sub-opcodes 0b01000/0b01001 belong
            // to Zvkb; other sub-opcodes in this space (vzext, vsext) are not claimed here
            0b010 => {
                if funct6 != 0b01_0010 {
                    None?;
                }
                match vs1_bits {
                    0b01000 => Some(Self::Vbrev8V { vd, vs2, vm }),
                    0b01001 => Some(Self::Vrev8V { vd, vs2, vm }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for ZvkbInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::VandnVv { vd, vs2, vs1, vm }  => write!(f, "vandn.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VandnVx { vd, vs2, rs1, vm }  => write!(f, "vandn.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::Vbrev8V { vd, vs2, vm }       => write!(f, "vbrev8.v {vd}, {vs2}{}", mask_suffix(vm)),
            Self::Vrev8V  { vd, vs2, vm }       => write!(f, "vrev8.v {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VrolVv  { vd, vs2, vs1, vm }  => write!(f, "vrol.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VrolVx  { vd, vs2, rs1, vm }  => write!(f, "vrol.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VrorVv  { vd, vs2, vs1, vm }  => write!(f, "vror.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VrorVx  { vd, vs2, rs1, vm }  => write!(f, "vror.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VrorVi  { vd, vs2, uimm, vm } => write!(f, "vror.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
