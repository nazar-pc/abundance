//! RV64 Zkn extension

pub mod zknd;
pub mod zkne;
pub mod zknh;

use crate::instructions::Instruction;
use crate::instructions::rv64::b::zbb::Rv64ZbbInstruction;
use crate::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use crate::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use crate::instructions::rv64::zk::zbkx::Rv64ZbkxInstruction;
use crate::instructions::rv64::zk::zkn::zknd::{Rv64ZkndInstruction, Rv64ZkndKsRnum};
use crate::instructions::rv64::zk::zkn::zkne::Rv64ZkneInstruction;
use crate::instructions::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv64ZknInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv64ZknInstruction<Reg>
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
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for Rv64ZknInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
