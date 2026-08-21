//! Zawrs extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands, ThreadedExecutableInstruction, ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

/// Custom handler for `Zawrs` extension's `wrs.nto`/`wrs.sto` instructions.
///
/// These are hint instructions that may complete for any reason, so a no-op is a valid
/// implementation for both.
pub const trait WrsHandler {
    /// Handle a `wrs.nto` instruction (Wait-on-Reservation-Set, no timeout)
    #[inline(always)]
    fn handle_wrs_nto(&mut self) {
        // NOP by default
    }

    /// Handle a `wrs.sto` instruction (Wait-on-Reservation-Set, short timeout)
    #[inline(always)]
    fn handle_wrs_sto(&mut self) {
        // NOP by default
    }
}

// Convenience for threaded execution
const impl<T> WrsHandler for &mut T
where
    T: [const] WrsHandler,
{
    #[inline(always)]
    fn handle_wrs_nto(&mut self) {
        T::handle_wrs_nto(self);
    }

    #[inline(always)]
    fn handle_wrs_sto(&mut self) {
        T::handle_wrs_sto(self);
    }
}

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZawrsInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for ZawrsInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for ZawrsInstruction<Reg>
where
    Reg: [const] Register,
    InstructionHandler: [const] WrsHandler,
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
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::WrsNto => {
                system_instruction_handler.handle_wrs_nto();
                ExecutionResult::ContinueNoWrite
            }
            Self::WrsSto => {
                system_instruction_handler.handle_wrs_sto();
                ExecutionResult::ContinueNoWrite
            }
        }
    }
}
