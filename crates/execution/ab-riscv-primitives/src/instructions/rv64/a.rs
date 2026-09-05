//! RV64 A extension

pub mod zaamo;
pub mod zalrsc;

use crate::instructions::Instruction;
use crate::instructions::rv64::a::zaamo::Rv64ZaamoInstruction;
use crate::instructions::rv64::a::zalrsc::Rv64ZalrscInstruction;
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 A (Zaamo + Zalrsc) instruction
#[instruction(
    inherit = [Rv64ZaamoInstruction, Rv64ZalrscInstruction]
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum Rv64AInstruction<Reg> {}

#[instruction]
const impl<Reg> Instruction for Rv64AInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
{
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Reg = Reg;

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
impl<Reg> fmt::Display for Rv64AInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
