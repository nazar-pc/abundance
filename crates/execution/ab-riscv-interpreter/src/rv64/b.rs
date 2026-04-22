//! RV64 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::rv64::b::zbb::rv64_zbb_helpers;
use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64BInstruction<Reg>
where
    Reg: Register<Type = u64>,
{
    #[inline(always)]
    fn execute(
        self,
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
