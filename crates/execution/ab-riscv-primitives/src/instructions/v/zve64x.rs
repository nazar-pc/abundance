//! Zve64x extension (Vector Extension for Embedded Processors, ELEN=64, integer-only)

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
use crate::instructions::v::zve64x::arith::Zve64xArithInstruction;
use crate::instructions::v::zve64x::carry::Zve64xCarryInstruction;
use crate::instructions::v::zve64x::config::Zve64xConfigInstruction;
use crate::instructions::v::zve64x::fixed_point::Zve64xFixedPointInstruction;
use crate::instructions::v::zve64x::load::{LoadStoreNreg, Nf, SegVmNf, Zve64xLoadInstruction};
use crate::instructions::v::zve64x::mask::Zve64xMaskInstruction;
use crate::instructions::v::zve64x::muldiv::Zve64xMulDivInstruction;
use crate::instructions::v::zve64x::perm::Zve64xPermInstruction;
use crate::instructions::v::zve64x::reduction::Zve64xReductionInstruction;
use crate::instructions::v::zve64x::store::Zve64xStoreInstruction;
use crate::instructions::v::zve64x::widen_narrow::Zve64xWidenNarrowInstruction;
use crate::instructions::zicsr::ZicsrInstruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::VReg;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V Zve64x instruction
#[instruction(
    inherit = [
        Zve64xConfigInstruction,
        Zve64xLoadInstruction,
        Zve64xStoreInstruction,
        Zve64xArithInstruction,
        Zve64xCarryInstruction,
        Zve64xMulDivInstruction,
        Zve64xWidenNarrowInstruction,
        Zve64xFixedPointInstruction,
        Zve64xMaskInstruction,
        Zve64xReductionInstruction,
        Zve64xPermInstruction,
        ZicsrInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zve64xInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Zve64xInstruction<Reg>
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
impl<Reg> fmt::Display for Zve64xInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
