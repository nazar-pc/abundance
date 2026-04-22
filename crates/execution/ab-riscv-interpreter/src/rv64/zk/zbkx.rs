//! RV64 Zbkx extension

pub mod rv64_zbkx_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbkxInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Xperm4 { rd, rs1, rs2 } => {
                let rs1_value = state.regs.read(rs1);
                let rs2_value = state.regs.read(rs2);

                state
                    .regs
                    .write(rd, rv64_zbkx_helpers::xperm4(rs1_value, rs2_value));
            }
            Self::Xperm8 { rd, rs1, rs2 } => {
                let rs1_value = state.regs.read(rs1);
                let rs2_value = state.regs.read(rs2);

                state
                    .regs
                    .write(rd, rv64_zbkx_helpers::xperm8(rs1_value, rs2_value));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
