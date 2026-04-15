//! RV32 Zbkx extension

pub mod rv32_zbkx_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv32ZbkxInstruction<Reg>
where
    Reg: Register<Type = u32>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Xperm4 { rd, rs1, rs2 } => {
                let rs1_value = state.regs.read(rs1);
                let rs2_value = state.regs.read(rs2);

                state
                    .regs
                    .write(rd, rv32_zbkx_helpers::xperm4(rs1_value, rs2_value));
            }
            Self::Xperm8 { rd, rs1, rs2 } => {
                let rs1_value = state.regs.read(rs1);
                let rs2_value = state.regs.read(rs2);

                state
                    .regs
                    .write(rd, rv32_zbkx_helpers::xperm8(rs1_value, rs2_value));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
