//! Zbkc extension (subset of Zbc extension)

use crate::rv64::Rv64InterpreterState;
#[cfg(any(miri, not(all(target_arch = "riscv64", target_feature = "zbc"))))]
use crate::rv64::b::zbc::clmul_internal;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::zk::zbkc::Rv64ZbkcInstruction;
use ab_riscv_primitives::registers::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbkcInstruction<Reg>
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
