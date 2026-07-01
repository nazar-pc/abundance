//! Zvbc extension

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

/// RISC-V Zvbc vector carryless multiplication instruction.
///
/// All use the OP-V major opcode (0b101_0111). Encoding spaces:
///
/// - `vclmul.[vv,vx]`:  funct6=0b001100, OPMVV/OPMVX; `vm` controls masking (0=masked, 1=unmasked)
/// - `vclmulh.[vv,vx]`: funct6=0b001101, OPMVV/OPMVX; `vm` controls masking
///
/// Both instructions compute a carryless (GF(2)) polynomial product of two SEW-wide elements.
/// `vclmul` produces the lower SEW bits of the 2*SEW-bit result; `vclmulh` produces the upper
/// SEW bits. Together they implement full-width carry-less multiplication, as required for
/// GCM/GHASH (`vclmulh` gives the reduction term) and for CRC computation.
///
/// For the `vm` field: `vm=true` means unmasked (process all body elements);
/// `vm=false` means masked by v0 (skip elements where v0\[i]=0, leaving them undisturbed).
#[instruction(
    inherit = [ZveXxInstruction],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[rustfmt::skip]
pub enum ZvbcInstruction<Reg> {
    // vclmul: lower SEW bits of the carry-less 2*SEW product
    /// `vclmul.vv vd, vs2, vs1, vm`
    VclmulVv  { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vclmul.vx vd, vs2, rs1, vm`
    VclmulVx  { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
    // vclmulh: upper SEW bits of the carry-less 2*SEW product
    /// `vclmulh.vv vd, vs2, vs1, vm`
    VclmulhVv { vd: VReg, vs2: VReg, vs1: VReg, vm: bool },
    /// `vclmulh.vx vd, vs2, rs1, vm`
    VclmulhVx { vd: VReg, vs2: VReg, rs1: Reg, vm: bool },
}

#[instruction]
const impl<Reg> Instruction for ZvbcInstruction<Reg>
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
            // OPMVV: vclmul.vv, vclmulh.vv
            0b010 => {
                let vs1 = VReg::from_bits(vs1_bits)?;
                match funct6 {
                    0b00_1100 => Some(Self::VclmulVv { vd, vs2, vs1, vm }),
                    0b00_1101 => Some(Self::VclmulhVv { vd, vs2, vs1, vm }),
                    _ => None,
                }
            }
            // OPMVX: vclmul.vx, vclmulh.vx
            0b110 => {
                let rs1 = Reg::from_bits(vs1_bits)?;
                match funct6 {
                    0b00_1100 => Some(Self::VclmulVx { vd, vs2, rs1, vm }),
                    0b00_1101 => Some(Self::VclmulhVx { vd, vs2, rs1, vm }),
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
impl<Reg> fmt::Display for ZvbcInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[rustfmt::skip]
        match self {
            Self::VclmulVv  { vd, vs2, vs1, vm } => write!(f, "vclmul.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VclmulVx  { vd, vs2, rs1, vm } => write!(f, "vclmul.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
            Self::VclmulhVv { vd, vs2, vs1, vm } => write!(f, "vclmulh.vv {vd}, {vs2}, {vs1}{}", mask_suffix(vm)),
            Self::VclmulhVx { vd, vs2, rs1, vm } => write!(f, "vclmulh.vx {vd}, {vs2}, {rs1}{}", mask_suffix(vm)),
        }
    }
}

/// Format mask suffix for display
#[inline(always)]
fn mask_suffix(vm: &bool) -> &'static str {
    if *vm { "" } else { ", v0.t" }
}
