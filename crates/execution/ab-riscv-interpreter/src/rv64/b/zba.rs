//! RV64 Zba extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_primitives::instruction::rv64::b::zba::Rv64ZbaInstruction;
use ab_riscv_primitives::registers::{Register, Registers};
use core::ops::ControlFlow;

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
        execute_zba(&mut state.regs, self);

        Ok(ControlFlow::Continue(()))
    }
}

/// Execute instructions from Zba extension
#[inline(always)]
pub fn execute_zba<Reg>(regs: &mut Registers<Reg>, instruction: Rv64ZbaInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Rv64ZbaInstruction::AddUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = rs1_val.wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbaInstruction::Sh1add { rd, rs1, rs2 } => {
            let value = (regs.read(rs1) << 1).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbaInstruction::Sh1addUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = (rs1_val << 1).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbaInstruction::Sh2add { rd, rs1, rs2 } => {
            let value = (regs.read(rs1) << 2).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbaInstruction::Sh2addUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = (rs1_val << 2).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbaInstruction::Sh3add { rd, rs1, rs2 } => {
            let value = (regs.read(rs1) << 3).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbaInstruction::Sh3addUw { rd, rs1, rs2 } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = (rs1_val << 3).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbaInstruction::SlliUw { rd, rs1, shamt } => {
            let rs1_val = (regs.read(rs1) as u32) as u64;
            let value = rs1_val << (shamt & 0x3f);
            regs.write(rd, value);
        }
    }
}
