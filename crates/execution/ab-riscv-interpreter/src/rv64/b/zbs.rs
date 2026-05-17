//! RV64 Zbs extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Rv64ZbsInstruction<Reg> where Reg: Register<Type = u64> {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv64ZbsInstruction<Reg>
where
    Reg: Register<Type = u64>,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZbsInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::Bset { rd, rs1: _, rs2: _ } => {
                // Only the bottom 6 bits for RV64
                let index = rs2_value & 0x3f;
                let result = rs1_value | (1u64 << index);
                Ok(ControlFlow::Continue((rd, result)))
            }
            Self::Bseti { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value | (1u64 << index);
                Ok(ControlFlow::Continue((rd, result)))
            }
            Self::Bclr { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x3f;
                let result = rs1_value & !(1u64 << index);
                Ok(ControlFlow::Continue((rd, result)))
            }
            Self::Bclri { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value & !(1u64 << index);
                Ok(ControlFlow::Continue((rd, result)))
            }
            Self::Binv { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x3f;
                let result = rs1_value ^ (1u64 << index);
                Ok(ControlFlow::Continue((rd, result)))
            }
            Self::Binvi { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value ^ (1u64 << index);
                Ok(ControlFlow::Continue((rd, result)))
            }
            Self::Bext { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x3f;
                let result = (rs1_value >> index) & 1;
                Ok(ControlFlow::Continue((rd, result)))
            }
            Self::Bexti { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = (rs1_value >> index) & 1;
                Ok(ControlFlow::Continue((rd, result)))
            }
        }
    }
}
