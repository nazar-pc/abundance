//! RV32 Zkn extension

pub mod zknd;
pub mod zkne;
pub mod zknh;

use crate::hart::HartConfig;
use crate::instructions::rv32::b::zbb::Rv32ZbbZbkbSharedInstruction;
use crate::instructions::rv32::zk::zbkb::Rv32ZbkbInstruction;
use crate::instructions::rv32::zk::zbkc::Rv32ZbkcInstruction;
use crate::instructions::rv32::zk::zbkx::Rv32ZbkxInstruction;
use crate::instructions::rv32::zk::zkn::zknd::{Rv32AesBs, Rv32ZkndInstruction};
use crate::instructions::rv32::zk::zkn::zkne::Rv32ZkneInstruction;
use crate::instructions::rv32::zk::zkn::zknh::Rv32ZknhInstruction;
use crate::instructions::utils::Shamt;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV32 Zkn (Zbkb + Zbkc + Zbkx + Zknd + Zkne + Zknh) instruction
#[instruction(
    inherit = [
        Rv32ZbkbInstruction,
        Rv32ZbkcInstruction,
        Rv32ZbkxInstruction,
        Rv32ZkndInstruction,
        Rv32ZkneInstruction,
        Rv32ZknhInstruction,
    ]
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum Rv32ZknInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for Rv32ZknInstruction<Hart>
where
    Reg: [const] Register<Type = u32>,
    Hart: [const] HartConfig<Reg = Reg>,
{
    const OWN_ISA_EXTENSIONS: &'static [IsaExtension] = &[IsaExtension::new("zkn", 1, 0)];

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
impl<Reg, Hart> fmt::Display for Rv32ZknInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
