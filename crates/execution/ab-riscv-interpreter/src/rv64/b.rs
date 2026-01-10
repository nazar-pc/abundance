//! RV64 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::rv64::b::zba::execute_zba;
use crate::rv64::b::zbb::execute_zbb;
use crate::rv64::b::zbc::execute_zbc;
use crate::rv64::b::zbs::execute_zbs;
use ab_riscv_primitives::instruction::rv64::b::{Rv64BInstruction, Rv64BZbcInstruction};
use ab_riscv_primitives::registers::{Register, Registers};

/// Execute instructions from B (Zba + Zbb + Zbs) extension
#[inline(always)]
pub fn execute_b<Reg>(regs: &mut Registers<Reg>, instruction: Rv64BInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Rv64BInstruction::Zba(instruction) => execute_zba(regs, instruction),
        Rv64BInstruction::Zbb(instruction) => execute_zbb(regs, instruction),
        Rv64BInstruction::Zbs(instruction) => execute_zbs(regs, instruction),
    }
}

/// Execute instructions from B (Zba + Zbb + Zbs) + Zbc extensions
#[inline(always)]
pub fn execute_b_zbc<Reg>(regs: &mut Registers<Reg>, instruction: Rv64BZbcInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Rv64BZbcInstruction::Zba(instruction) => execute_zba(regs, instruction),
        Rv64BZbcInstruction::Zbb(instruction) => execute_zbb(regs, instruction),
        Rv64BZbcInstruction::Zbc(instruction) => execute_zbc(regs, instruction),
        Rv64BZbcInstruction::Zbs(instruction) => execute_zbs(regs, instruction),
    }
}
