//! RV64 Zba extension

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
const impl<Reg> ExecutableInstructionOperands for Rv64ZbaInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZbaInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZbaInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
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
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::AddUw { rd, rs1: _, rs2: _ } => {
                let rs1_val = u64::from(rs1_value as u32);
                let value = rs1_val.wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sh1add { rd, rs1: _, rs2: _ } => {
                let value = (rs1_value << 1).wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sh1addUw { rd, rs1: _, rs2: _ } => {
                let rs1_val = u64::from(rs1_value as u32);
                let value = (rs1_val << 1).wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sh2add { rd, rs1: _, rs2: _ } => {
                let value = (rs1_value << 2).wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sh2addUw { rd, rs1: _, rs2: _ } => {
                let rs1_val = u64::from(rs1_value as u32);
                let value = (rs1_val << 2).wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sh3add { rd, rs1: _, rs2: _ } => {
                let value = (rs1_value << 3).wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sh3addUw { rd, rs1: _, rs2: _ } => {
                let rs1_val = u64::from(rs1_value as u32);
                let value = (rs1_val << 3).wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::SlliUw { rd, rs1: _, shamt } => {
                let rs1_val = u64::from(rs1_value as u32);
                let value = rs1_val << (shamt & 0x3f);
                ExecutionResult::Continue { rd, value }
            }
        }
    }
}
