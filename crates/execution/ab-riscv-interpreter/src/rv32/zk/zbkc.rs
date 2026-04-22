//! RV32 Zbkc extension (subset of Zbc extension)

use crate::rv32::b::zbc::rv32_zbc_helpers;
use crate::{ExecutableInstruction, ExecutionError, InterpreterState, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv32ZbkcInstruction<Reg>
where
    Reg: Register<Type = u32>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
