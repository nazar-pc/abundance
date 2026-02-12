//! RV64 Zve64x extension (Vector Extension for Embedded Processors, ELEN=64, integer-only)

mod arith;
mod config;
mod fixed_point;
mod load;
mod mask;
mod muldiv;
mod perm;
mod reduction;
mod store;
mod widen_narrow;

use crate::instructions::Instruction;
use crate::instructions::rv64::v::zve64x::arith::Rv64Zve64xArithInstruction;
use crate::instructions::rv64::v::zve64x::config::Rv64Zve64xConfigInstruction;
use crate::instructions::rv64::v::zve64x::fixed_point::Rv64Zve64xFixedPointInstruction;
use crate::instructions::rv64::v::zve64x::load::Rv64Zve64xLoadInstruction;
use crate::instructions::rv64::v::zve64x::mask::Rv64Zve64xMaskInstruction;
use crate::instructions::rv64::v::zve64x::muldiv::Rv64Zve64xMulDivInstruction;
use crate::instructions::rv64::v::zve64x::perm::Rv64Zve64xPermInstruction;
use crate::instructions::rv64::v::zve64x::reduction::Rv64Zve64xReductionInstruction;
use crate::instructions::rv64::v::zve64x::store::Rv64Zve64xStoreInstruction;
use crate::instructions::rv64::v::zve64x::widen_narrow::Rv64Zve64xWidenNarrowInstruction;
use crate::registers::general_purpose::Register;
use crate::registers::vector::{Eew, VReg};
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zve64x instruction
#[instruction(
    ignore = [Phantom],
    inherit = [
        Rv64Zve64xConfigInstruction,
        Rv64Zve64xLoadInstruction,
        Rv64Zve64xStoreInstruction,
        Rv64Zve64xArithInstruction,
        Rv64Zve64xMulDivInstruction,
        Rv64Zve64xWidenNarrowInstruction,
        Rv64Zve64xFixedPointInstruction,
        Rv64Zve64xMaskInstruction,
        Rv64Zve64xReductionInstruction,
        Rv64Zve64xPermInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64Zve64xInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv64Zve64xInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
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

#[instruction]
impl<Reg> fmt::Display for Rv64Zve64xInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
