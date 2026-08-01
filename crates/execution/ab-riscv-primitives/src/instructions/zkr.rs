//! Zkr extension

#[cfg(test)]
mod tests;

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
pub enum ZkrInstruction<Reg> {}

#[instruction]
const impl<Reg> Instruction for ZkrInstruction<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
impl<Reg> fmt::Display for ZkrInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
