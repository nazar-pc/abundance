//! RV32 Zba extension

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
    > for Rv32ZbaInstruction<Reg>
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
            Self::Sh1add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh2add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 2).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh3add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 3).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
