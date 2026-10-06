//! Zkr extension

#[cfg(test)]
mod tests;

use crate::hart::HartConfig;
use crate::instructions::Instruction;
use crate::instructions::zicsr::ZicsrInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

// TODO: CSR composition?
/// CSR index of the `seed` register defined by the `Zkr` extension
pub const SEED_CSR_INDEX: u16 = 0x015;

/// RISC-V Zkr instruction.
///
/// `Zkr` (the entropy source extension) defines no instructions of its own: it only adds the
/// `seed` CSR (see [`SEED_CSR_INDEX`]), which is accessed through ordinary `Zicsr` instructions.
#[instruction(inherit = [ZicsrInstruction])]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum ZkrInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for ZkrInstruction<Hart>
where
    Reg: [const] Register,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Hart = Hart;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg, Hart> fmt::Display for ZkrInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
