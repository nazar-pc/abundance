//! Zicond extension

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
const impl<Reg> ExecutableInstructionOperands for ZicondInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for ZicondInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for ZicondInstruction<Reg>
where
    Reg: [const] Register,
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
            // Conditional zero, equal to zero.
            //
            // rd = (rs2 == 0) ? 0 : rs1
            Self::CzeroEqz { rd, rs1: _, rs2: _ } => {
                let condition = rs2_value;
                let src = rs1_value;
                let result = if condition == Reg::Type::from(0u8) {
                    Reg::Type::from(0u8)
                } else {
                    src
                };
                ExecutionResult::Continue { rd, value: result }
            }

            // Conditional zero, nonzero.
            //
            // rd = (rs2 != 0) ? 0 : rs1
            Self::CzeroNez { rd, rs1: _, rs2: _ } => {
                let condition = rs2_value;
                let src = rs1_value;
                let result = if condition == Reg::Type::from(0u8) {
                    src
                } else {
                    Reg::Type::from(0u8)
                };
                ExecutionResult::Continue { rd, value: result }
            }
        }
    }
}
