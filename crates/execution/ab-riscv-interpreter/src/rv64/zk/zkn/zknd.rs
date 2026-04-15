//! RV64 Zknd extension

pub mod rv64_zknd_helpers;
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
    > for Rv64ZkndInstruction<Reg>
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
            Self::Aes64Ds { rd, rs1, rs2 } => {
                let v1 = state.regs.read(rs1);
                let v2 = state.regs.read(rs2);
                state.regs.write(rd, rv64_zknd_helpers::aes64ds(v1, v2));
            }
            Self::Aes64Dsm { rd, rs1, rs2 } => {
                let v1 = state.regs.read(rs1);
                let v2 = state.regs.read(rs2);
                state.regs.write(rd, rv64_zknd_helpers::aes64dsm(v1, v2));
            }
            Self::Aes64Im { rd, rs1 } => {
                let v1 = state.regs.read(rs1);
                state.regs.write(rd, rv64_zknd_helpers::aes64im(v1));
            }
            Self::Aes64Ks1i { rd, rs1, rnum } => {
                let v1 = state.regs.read(rs1);
                state.regs.write(rd, rv64_zknd_helpers::aes64ks1i(v1, rnum));
            }
            Self::Aes64Ks2 { rd, rs1, rs2 } => {
                let v1 = state.regs.read(rs1);
                let v2 = state.regs.read(rs2);
                state.regs.write(rd, rv64_zknd_helpers::aes64ks2(v1, v2));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
