//! RV32 A extension

pub mod zaamo;
pub mod zalrsc;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands, ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

/// Reservation set used to implement `Zalrsc` extension's `lr`/`sc` instruction pairs.
///
/// `lr` places a reservation on an address, and a subsequent `sc` succeeds only if the reservation
/// is still held for the same address. Regardless of success or failure, executing `sc` always
/// invalidates the reservation, as does a subsequent `lr`.
pub const trait ReservationSet<Reg>
where
    Reg: [const] Register,
{
    /// Returns the address of the currently held reservation, if any
    fn reservation(&self) -> Option<Reg::Type>;

    /// Place a reservation on `address`, replacing any previously held reservation
    fn set_reservation(&mut self, address: Reg::Type);

    /// Clear any currently held reservation
    fn clear_reservation(&mut self);
}

// Convenience for threaded execution
const impl<Reg, T> ReservationSet<Reg> for &mut T
where
    Reg: [const] Register,
    T: [const] ReservationSet<Reg>,
{
    #[inline(always)]
    fn reservation(&self) -> Option<Reg::Type> {
        T::reservation(self)
    }

    #[inline(always)]
    fn set_reservation(&mut self, address: Reg::Type) {
        T::set_reservation(self, address);
    }

    #[inline(always)]
    fn clear_reservation(&mut self) {
        T::clear_reservation(self);
    }
}

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32AInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32AInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for Rv32AInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
    Regs: [const] RegisterFile<Reg>,
    Memory: [const] VirtualMemory,
    ExtState: [const] ReservationSet<Reg>,
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
        ext_state: &mut ExtState,
        memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
