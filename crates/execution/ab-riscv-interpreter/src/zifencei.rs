//! Zifencei extension

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

/// Custom handler for `Zifencei` extension's `fence.i` instruction
pub const trait FenceIHandler {
    // TODO: Figure out the correct API for this method
    /// Handle a `fence.i` instruction
    #[inline(always)]
    fn handle_fence_i(&mut self) {
        // NOP by default
    }
}

// Convenience for threaded execution
const impl<T> FenceIHandler for &mut T
where
    T: [const] FenceIHandler,
{
    #[inline(always)]
    fn handle_fence_i(&mut self) {
        T::handle_fence_i(self);
    }
}

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZifenceiInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for ZifenceiInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZifenceiInstruction<Reg>
where
    Reg: [const] Register,
    Env: [const] FenceIHandler,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value: _,
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        _memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::FenceI => {
                env.handle_fence_i();
                ExecutionResult::ContinueNoWrite
            }
        }
    }
}
