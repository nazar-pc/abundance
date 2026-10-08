//! RV64 Zkn extension

pub mod zknd;
pub mod zkne;
pub mod zknh;

use crate::hart::HartConfig;
use crate::instructions::rv64::b::zbb::Rv64ZbbZbkbSharedInstruction;
use crate::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use crate::instructions::rv64::zk::zbkc::Rv64ZbkcInstruction;
use crate::instructions::rv64::zk::zbkx::Rv64ZbkxInstruction;
use crate::instructions::rv64::zk::zkn::zknd::{
    Rv64ZkndInstruction, Rv64ZkndKsRnum, Rv64ZkndZkneSharedInstruction,
};
use crate::instructions::rv64::zk::zkn::zkne::Rv64ZkneInstruction;
use crate::instructions::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
use crate::instructions::utils::Shamt;
use crate::instructions::{ImplementedExtension, Instruction, IsaExtension};
use crate::registers::general_purpose::Register;
use ab_riscv_macros::instruction;
use core::fmt;

/// RISC-V RV64 Zkn (Zbkb + Zbkc + Zbkx + Zknd + Zkne + Zknh) instruction
#[instruction(
    inherit = [
        Rv64ZbkbInstruction,
        Rv64ZbkcInstruction,
        Rv64ZbkxInstruction,
        Rv64ZkndInstruction,
        Rv64ZkneInstruction,
        Rv64ZknhInstruction,
    ]
)]
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub enum Rv64ZknInstruction<Hart>
where
    Hart: HartConfig, {}

#[instruction]
const impl<Reg, Hart> Instruction for Rv64ZknInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
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
impl<Reg, Hart> fmt::Display for Rv64ZknInstruction<Hart>
where
    Reg: fmt::Display + Copy,
    Hart: HartConfig<Reg = Reg>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
