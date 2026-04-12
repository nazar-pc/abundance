//! RV32 Zkn extension

pub mod zknd;
pub mod zkne;
pub mod zknh;

use crate::instructions::Instruction;
use crate::instructions::rv32::b::zbb::Rv32ZbbInstruction;
use crate::instructions::rv32::b::zbc::Rv32ZbcInstruction;
use crate::instructions::rv32::zk::zbkb::Rv32ZbkbInstruction;
use crate::instructions::rv32::zk::zbkx::Rv32ZbkxInstruction;
use crate::instructions::rv32::zk::zkn::zknd::{Rv32AesBs, Rv32ZkndInstruction};
use crate::instructions::rv32::zk::zkn::zkne::Rv32ZkneInstruction;
use crate::instructions::rv32::zk::zkn::zknh::Rv32ZknhInstruction;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rv32ZknInstruction<Reg> {}

#[instruction]
impl<Reg> const Instruction for Rv32ZknInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
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
impl<Reg> fmt::Display for Rv32ZknInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}
