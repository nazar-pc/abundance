//! RV64 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::hart::HartConfig;
use crate::instructions::rv64::b::zba::Rv64ZbaInstruction;
use crate::instructions::rv64::b::zbb::{Rv64ZbbInstruction, Rv64ZbbZbkbSharedInstruction};
use crate::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use crate::instructions::utils::Shamt;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 B (Zba + Zbb + Zbs) instruction
#[instruction(
    inherit = [Rv64ZbaInstruction, Rv64ZbbInstruction, Rv64ZbsInstruction]
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum Rv64BInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64BInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("b", 1, 0)];

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
impl<Reg, Hart> fmt::Display for Rv64BInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
