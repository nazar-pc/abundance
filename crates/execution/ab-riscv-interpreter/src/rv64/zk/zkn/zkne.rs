//! RV64 Zkne extension

pub mod rv64_zkne_helpers;
#[cfg(test)]
mod tests;

use crate::rv64::zk::zkn::zknd::rv64_zknd_helpers;
use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::zk::zkn::zkne::Rv64ZkneInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZkneInstruction<Reg>
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
            Self::Aes64Es { rd, rs1, rs2 } => {
                let v1 = state.regs.read(rs1);
                let v2 = state.regs.read(rs2);
                state.regs.write(rd, rv64_zkne_helpers::aes64es(v1, v2));
            }
            Self::Aes64Esm { rd, rs1, rs2 } => {
                let v1 = state.regs.read(rs1);
                let v2 = state.regs.read(rs2);
                state.regs.write(rd, rv64_zkne_helpers::aes64esm(v1, v2));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
