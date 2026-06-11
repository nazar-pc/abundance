//! Zvbb extension

#[cfg(test)]
mod tests;
pub mod zvkb;

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
use crate::instructions::zvbb::zvkb::ZvkbInstruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V Zvbb vector bit-manipulation instruction.
///
/// Zvbb is a strict superset of Zvkb; this type encodes only the instructions unique to Zvbb.
/// The Zvkb subset (vandn, vbrev8, vrev8, vrol, vror) is inherited from [`ZvkbInstruction`].
///
/// All use the OP-V major opcode (0b101_0111). Encoding spaces:
///
/// - `vbrev.v`:       funct6=0b010010, OPMVV, vs1=0b01010; `vm` controls masking
/// - `vclz.v`:        funct6=0b010010, OPMVV, vs1=0b01100; `vm` controls masking
/// - `vctz.v`:        funct6=0b010010, OPMVV, vs1=0b01101; `vm` controls masking
/// - `vcpop.v`:       funct6=0b010010, OPMVV, vs1=0b01110; `vm` controls masking
/// - `vwsll.[vv,vx]`: funct6=0b110101, OPIVV/OPIVX; `vm` controls masking
/// - `vwsll.vi`:      funct6=0b110101, OPIVI; 5-bit unsigned immediate in bits\[19:15] (0-31);
///   bit\[25] is the standard `vm` field, orthogonal to the immediate
///
/// The four unary operations share VXUNARY0 (funct6=0b010010, OPMVV); Zvkb claims vs1
/// sub-opcodes 0b01000 and 0b01001; Zvbb claims 0b01010, 0b01100, 0b01101, and 0b01110.
/// Sub-opcode 0b01011 is an undefined gap between vbrev and vclz.
///
/// Note: `vwsll` funct6 (0b110101) coincides with `vwadd.wv` from the base V extension;
/// these instructions are mutually exclusive within a combined decoder.
///
/// For instructions with a `vm` field: `vm=true` means unmasked (process all body elements);
/// `vm=false` means masked by v0 (skip elements where v0\[i]=0, leaving them undisturbed).
#[instruction(
    inherit = [ZvkbInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub enum ZvbbInstruction<Reg> {
    // vbrev: bit-reverse within each SEW-wide element (element granularity, unlike vbrev8's byte granularity)
    /// `vbrev.v vd, vs2, vm`
    VbrevV  { vd: VReg, vs2: VReg, vm: bool },
    // vclz: count leading zeros within each SEW-wide element; result in [0, SEW]
    /// `vclz.v vd, vs2, vm`
    VclzV   { vd: VReg, vs2: VReg, vm: bool },
    // vctz: count trailing zeros within each SEW-wide element; result in [0, SEW]
    /// `vctz.v vd, vs2, vm`
    VctzV   { vd: VReg, vs2: VReg, vm: bool },
    // vcpop: population count (number of set bits) within each SEW-wide element
    /// `vcpop.v vd, vs2, vm`
    VcpopV  { vd: VReg, vs2: VReg, vm: bool },
    // vwsll: widening shift left logical; result is 2*SEW wide, both sources are SEW wide
    /// `vwsll.vv vd, vs2, vs1, vm`
    VwsllVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vwsll.vx vd, vs2, rs1, vm`
    VwsllVx { vd: VReg, vs2: VReg, rs1: Reg,  vm: bool },
    /// `vwsll.vi vd, vs2, uimm, vm` - `uimm` is 5-bit unsigned (0..=31); `vm` is the
    /// standard mask-control bit at bit\[25], orthogonal to the immediate.
    VwsllVi { vd: VReg, vs2: VReg, uimm: u8,  vm: bool },
}

#[instruction]
impl<Reg> const Instruction for ZvbbInstruction<Reg>
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
            // OPIVV: vwsll.vv
            0b000 => {
                if funct6 != 0b11_0101 {
                    None?;
                }
                let vs1 = VReg::from_bits(vs1_bits)?;
                Some(Self::VwsllVv { vd, vs2, vs1, vm })
            }
            // OPIVX: vwsll.vx
            0b100 => {
                if funct6 != 0b11_0101 {
                    None?;
                }
                let rs1 = Reg::from_bits(vs1_bits)?;
                Some(Self::VwsllVx { vd, vs2, rs1, vm })
            }
            // OPIVI: vwsll.vi - standard 5-bit unsigned immediate in bits[19:15]; vm is bit[25]
            0b011 => {
                if funct6 != 0b11_0101 {
                    None?;
                }
                let uimm = vs1_bits;
                Some(Self::VwsllVi { vd, vs2, uimm, vm })
            }
            // OPMVV: vbrev.v, vclz.v, vctz.v, vcpop.v
            // funct6=0b010010 (VXUNARY0 sub-space); Zvkb claims vs1 0b01000/0b01001;
            // Zvbb claims 0b01010, 0b01100, 0b01101, 0b01110; vs1 0b01011 is an undefined gap
            0b010 => {
                if funct6 != 0b01_0010 {
                    None?;
                }
                match vs1_bits {
                    0b01010 => Some(Self::VbrevV { vd, vs2, vm }),
                    0b01100 => Some(Self::VclzV { vd, vs2, vm }),
                    0b01101 => Some(Self::VctzV { vd, vs2, vm }),
                    0b01110 => Some(Self::VcpopV { vd, vs2, vm }),
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
impl<Reg> fmt::Display for ZvbbInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::VbrevV  { vd, vs2, vm }        => write!(f, "vbrev.v {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VclzV   { vd, vs2, vm }        => write!(f, "vclz.v {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VctzV   { vd, vs2, vm }        => write!(f, "vctz.v {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VcpopV  { vd, vs2, vm }        => write!(f, "vcpop.v {vd}, {vs2}{}", mask_suffix(vm)),
            Self::VwsllVv { vd, vs2, vs1, vm }   => write!(f, "vwsll.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VwsllVx { vd, vs2, rs1, vm }   => write!(f, "vwsll.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VwsllVi { vd, vs2, uimm, vm }  => write!(f, "vwsll.vi {vd}, {vs2}, {uimm}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
