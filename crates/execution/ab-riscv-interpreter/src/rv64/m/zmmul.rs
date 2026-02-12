//! RV64 Zmmul extension (multiplication subset of M extension)

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::m::zmmul::Rv64ZmmulInstruction;
use ab_riscv_primitives::registers::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZmmulInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
