//! Zicond extension

#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::zicond::ZicondInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for ZicondInstruction<Reg>
where
    Reg: Register,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
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
