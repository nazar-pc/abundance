//! RV64 Zkn extension

pub mod zknd;
pub mod zkne;
pub mod zknh;

use crate::rv64::b::zbc::rv64_zbc_helpers;
use crate::rv64::zk::zbkx::rv64_zbkx_helpers;
use crate::rv64::zk::zkn::zknd::rv64_zknd_helpers;
use crate::rv64::zk::zkn::zkne::rv64_zkne_helpers;
use crate::rv64::zk::zkn::zknh::rv64_zknh_helpers;
use crate::{ExecutableInstruction, ExecutionError, InterpreterState, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZknInstruction<Reg>
where
    Reg: Register<Type = u64>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
