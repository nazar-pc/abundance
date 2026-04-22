//! Zicond extension

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
    > for ZicondInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            // Conditional zero, equal to zero.
            //
            // rd = (rs2 == 0) ? 0 : rs1
            Self::CzeroEqz { rd, rs1, rs2 } => {
                let condition = state.regs.read(rs2);
                let src = state.regs.read(rs1);
                let result = if condition == Reg::Type::from(0u8) {
                    Reg::Type::from(0u8)
                } else {
                    src
                };
                state.regs.write(rd, result);
            }

            // Conditional zero, nonzero.
            //
            // rd = (rs2 != 0) ? 0 : rs1
            Self::CzeroNez { rd, rs1, rs2 } => {
                let condition = state.regs.read(rs2);
                let src = state.regs.read(rs1);
                let result = if condition != Reg::Type::from(0u8) {
                    Reg::Type::from(0u8)
                } else {
                    src
                };
                state.regs.write(rd, result);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
