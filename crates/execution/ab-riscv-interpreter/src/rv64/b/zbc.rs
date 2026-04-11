//! RV64 Zbc extension

pub mod rv64_zbc_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbcInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Clmul { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                state.regs.write(rd, rv64_zbc_helpers::clmul(a, b));
            }
            Self::Clmulh { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                state.regs.write(rd, rv64_zbc_helpers::clmulh(a, b));
            }
            Self::Clmulr { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                state.regs.write(rd, rv64_zbc_helpers::clmulr(a, b));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
