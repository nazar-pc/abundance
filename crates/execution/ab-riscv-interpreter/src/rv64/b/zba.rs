//! RV64 Zba extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::b::zba::Rv64ZbaInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbaInstruction<Reg>
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
            Self::AddUw { rd, rs1, rs2 } => {
                let rs1_val = (state.regs.read(rs1) as u32) as u64;
                let value = rs1_val.wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh1add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh1addUw { rd, rs1, rs2 } => {
                let rs1_val = (state.regs.read(rs1) as u32) as u64;
                let value = (rs1_val << 1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh2add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 2).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh2addUw { rd, rs1, rs2 } => {
                let rs1_val = (state.regs.read(rs1) as u32) as u64;
                let value = (rs1_val << 2).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh3add { rd, rs1, rs2 } => {
                let value = (state.regs.read(rs1) << 3).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sh3addUw { rd, rs1, rs2 } => {
                let rs1_val = (state.regs.read(rs1) as u32) as u64;
                let value = (rs1_val << 3).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::SlliUw { rd, rs1, shamt } => {
                let rs1_val = (state.regs.read(rs1) as u32) as u64;
                let value = rs1_val << (shamt & 0x3f);
                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
