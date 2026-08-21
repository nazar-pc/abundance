//! RV64 Zbc extension

pub mod rv64_zbc_helpers;
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
const impl<Reg> ExecutableInstructionOperands for Rv64ZbcInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZbcInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZbcInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
            Self::Clmul { rd, rs1: _, rs2: _ } => {
                let a = rs1_value;
                let b = rs2_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv64_zbc_helpers::clmul(a, b),
                }
            }
            Self::Clmulh { rd, rs1: _, rs2: _ } => {
                let a = rs1_value;
                let b = rs2_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv64_zbc_helpers::clmulh(a, b),
                }
            }
            Self::Clmulr { rd, rs1: _, rs2: _ } => {
                let a = rs1_value;
                let b = rs2_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv64_zbc_helpers::clmulr(a, b),
                }
            }
        }
    }
}
