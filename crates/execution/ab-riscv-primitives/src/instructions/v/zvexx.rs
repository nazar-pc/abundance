//! ZveXx extension (Vector Extension for Embedded Processors, ELEN=64, integer-only)

#[doc(hidden)]
pub mod arith;
#[doc(hidden)]
pub mod carry;
#[doc(hidden)]
pub mod config;
#[doc(hidden)]
pub mod fixed_point;
#[doc(hidden)]
pub mod load;
#[doc(hidden)]
pub mod mask;
#[doc(hidden)]
pub mod muldiv;
#[doc(hidden)]
pub mod perm;
#[doc(hidden)]
pub mod reduction;
#[doc(hidden)]
pub mod store;
#[doc(hidden)]
pub mod widen_narrow;

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

/// RISC-V ZveXx instruction.
///
/// `X` is any legal value, according to the RISC-V specification, for example, Zve32x or Zve64x.
/// The actual `ELEN` and `VLEN` values are configured at the execution side and do not impact
/// decoded instructions.
#[instruction(
    inherit = [
        ZveXxConfigInstruction,
        ZveXxLoadInstruction,
        ZveXxStoreInstruction,
        ZveXxArithInstruction,
        ZveXxCarryInstruction,
        ZveXxMulDivInstruction,
        ZveXxWidenNarrowInstruction,
        ZveXxFixedPointInstruction,
        ZveXxMaskInstruction,
        ZveXxReductionInstruction,
        ZveXxPermInstruction,
        ZicsrInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZveXxInstruction<Reg> {}

#[instruction]
const impl<Reg> Instruction for ZveXxInstruction<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
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
impl<Reg> fmt::Display for ZveXxInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
