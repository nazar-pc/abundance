//! RV32 Zkne extension

pub mod rv32_zkne_helpers;
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
    > for Rv32ZkneInstruction<Reg>
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
            Self::Aes32Esi { rd, rs1, rs2, bs } => {
                let v1 = state.regs.read(rs1);
                let v2 = state.regs.read(rs2);
                state
                    .regs
                    .write(rd, rv32_zkne_helpers::aes32esi(v1, v2, bs));
            }
            Self::Aes32Esmi { rd, rs1, rs2, bs } => {
                let v1 = state.regs.read(rs1);
                let v2 = state.regs.read(rs2);
                state
                    .regs
                    .write(rd, rv32_zkne_helpers::aes32esmi(v1, v2, bs));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
