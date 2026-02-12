//! RV64 Zbs extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::b::zbs::Rv64ZbsInstruction;
use ab_riscv_primitives::registers::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbsInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Bset { rd, rs1, rs2 } => {
                // Only the bottom 6 bits for RV64
                let index = state.regs.read(rs2) & 0x3f;
                let result = state.regs.read(rs1) | (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bseti { rd, rs1, shamt } => {
                let index = shamt;
                let result = state.regs.read(rs1) | (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bclr { rd, rs1, rs2 } => {
                let index = state.regs.read(rs2) & 0x3f;
                let result = state.regs.read(rs1) & !(1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bclri { rd, rs1, shamt } => {
                let index = shamt;
                let result = state.regs.read(rs1) & !(1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Binv { rd, rs1, rs2 } => {
                let index = state.regs.read(rs2) & 0x3f;
                let result = state.regs.read(rs1) ^ (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Binvi { rd, rs1, shamt } => {
                let index = shamt;
                let result = state.regs.read(rs1) ^ (1u64 << index);
                state.regs.write(rd, result);
            }
            Self::Bext { rd, rs1, rs2 } => {
                let index = state.regs.read(rs2) & 0x3f;
                let result = (state.regs.read(rs1) >> index) & 1;
                state.regs.write(rd, result);
            }
            Self::Bexti { rd, rs1, shamt } => {
                let index = shamt;
                let result = (state.regs.read(rs1) >> index) & 1;
                state.regs.write(rd, result);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
