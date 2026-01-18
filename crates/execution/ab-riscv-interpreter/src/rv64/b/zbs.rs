//! RV64 Zbs extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_primitives::instruction::rv64::b::zbs::Rv64ZbsInstruction;
use ab_riscv_primitives::registers::{Register, Registers};
use core::ops::ControlFlow;

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
        execute_zbs(&mut state.regs, self);

        Ok(ControlFlow::Continue(()))
    }
}

/// Execute instructions from Zbs extension
#[inline(always)]
pub fn execute_zbs<Reg>(regs: &mut Registers<Reg>, instruction: Rv64ZbsInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Rv64ZbsInstruction::Bset { rd, rs1, rs2 } => {
            // Only the bottom 6 bits for RV64
            let index = regs.read(rs2) & 0x3f;
            let result = regs.read(rs1) | (1u64 << index);
            regs.write(rd, result);
        }
        Rv64ZbsInstruction::Bseti { rd, rs1, shamt } => {
            let index = shamt;
            let result = regs.read(rs1) | (1u64 << index);
            regs.write(rd, result);
        }
        Rv64ZbsInstruction::Bclr { rd, rs1, rs2 } => {
            let index = regs.read(rs2) & 0x3f;
            let result = regs.read(rs1) & !(1u64 << index);
            regs.write(rd, result);
        }
        Rv64ZbsInstruction::Bclri { rd, rs1, shamt } => {
            let index = shamt;
            let result = regs.read(rs1) & !(1u64 << index);
            regs.write(rd, result);
        }
        Rv64ZbsInstruction::Binv { rd, rs1, rs2 } => {
            let index = regs.read(rs2) & 0x3f;
            let result = regs.read(rs1) ^ (1u64 << index);
            regs.write(rd, result);
        }
        Rv64ZbsInstruction::Binvi { rd, rs1, shamt } => {
            let index = shamt;
            let result = regs.read(rs1) ^ (1u64 << index);
            regs.write(rd, result);
        }
        Rv64ZbsInstruction::Bext { rd, rs1, rs2 } => {
            let index = regs.read(rs2) & 0x3f;
            let result = (regs.read(rs1) >> index) & 1;
            regs.write(rd, result);
        }
        Rv64ZbsInstruction::Bexti { rd, rs1, shamt } => {
            let index = shamt;
            let result = (regs.read(rs1) >> index) & 1;
            regs.write(rd, result);
        }
    }
}
