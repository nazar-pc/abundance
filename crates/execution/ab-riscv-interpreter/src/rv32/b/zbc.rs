//! RV32 Zbc extension

pub mod rv32_zbc_helpers;
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
    > for Rv32ZbcInstruction<Reg>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Clmul { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                state.regs.write(rd, rv32_zbc_helpers::clmul(a, b));
            }
            Self::Clmulh { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                state.regs.write(rd, rv32_zbc_helpers::clmulh(a, b));
            }
            Self::Clmulr { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                state.regs.write(rd, rv32_zbc_helpers::clmulr(a, b));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
