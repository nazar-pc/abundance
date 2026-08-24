//! RV32 Zbs extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZbsInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for Rv32ZbsInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv32ZbsInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
    Regs: [const] RegisterFile<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _env: &mut Env,
        _memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Bset { rd, rs1: _, rs2: _ } => {
                // Only the bottom 5 bits for RV32
                let index = rs2_value & 0x1f;
                let result = rs1_value | (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bseti { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value | (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bclr { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x1f;
                let result = rs1_value & !(1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bclri { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value & !(1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Binv { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x1f;
                let result = rs1_value ^ (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Binvi { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = rs1_value ^ (1u32 << index);
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bext { rd, rs1: _, rs2: _ } => {
                let index = rs2_value & 0x1f;
                let result = (rs1_value >> index) & 1;
                ExecutionResult::Continue { rd, value: result }
            }
            Self::Bexti { rd, rs1: _, shamt } => {
                let index = shamt;
                let result = (rs1_value >> index) & 1;
                ExecutionResult::Continue { rd, value: result }
            }
        }
    }
}
