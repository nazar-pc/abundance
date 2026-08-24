//! Zkr extension

#[cfg(test)]
mod tests;
pub mod zkr_helpers;

use crate::zicsr::zicsr_helpers;
use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutionError, ExecutionResult, FetchInstructionResult, InstructionFetcher,
    OpaqueThreadedExecutionResult, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    ThreadedExecutableInstruction, ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

/// Result of polling the entropy source behind the `Zkr` extension's `seed` CSR, corresponding to
/// one of the specification's `OPST` states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkrSeedPoll {
    /// Built-in self-test is being performed (`OPST=BIST`).
    ///
    /// If this is returned after some other state was previously observed, it signals a
    /// non-fatal, but noteworthy, self-test alarm.
    Bist,
    /// A sufficient amount of entropy is not yet available (`OPST=WAIT`).
    ///
    /// This is not an error condition and may be observed more often than [`Self::Es16`], since
    /// physical entropy sources often have low bandwidth.
    Wait,
    /// 16 bits of randomness are available (`OPST=ES16`), guaranteed to meet the specification's
    /// minimum entropy requirements regardless of implementation.
    Es16(u16),
    /// An unrecoverable self-test error has occurred (`OPST=DEAD`).
    ///
    /// Once returned, all future polls are expected to also return [`Self::Dead`].
    Dead,
}

/// Entropy source behind the `Zkr` extension's `seed` CSR.
///
/// Every access to `seed` polls the entropy source once.
pub const trait ZkrSeedSource {
    /// Poll the entropy source once, advancing its internal state as necessary
    fn poll_seed(&mut self) -> ZkrSeedPoll;
}

// Convenience for threaded execution
const impl<T> ZkrSeedSource for &mut T
where
    T: [const] ZkrSeedSource,
{
    #[inline(always)]
    fn poll_seed(&mut self) -> ZkrSeedPoll {
        T::poll_seed(self)
    }
}

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZkrInstruction<Reg> where Reg: [const] Register {}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for ZkrInstruction<Reg>
where
    Reg: [const] Register,
    ExtState: [const] ZkrSeedSource,
{
    /// Reads of `seed` are pass-through: the raw stored value already reflects the outcome of the
    /// most recent poll (see [`Self::prepare_csr_write()`]).
    ///
    /// Per specification, only genuine read-write accesses (not e.g. `csrrs`/`csrrc` with
    /// `rs1 = x0`) are legal against `seed`; a pure read is rejected as an illegal access.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn prepare_csr_read(
        _ext_state: &ExtState,
        csr_index: u16,
        will_write: bool,
        raw_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        if csr_index == SEED_CSR_INDEX {
            if will_write {
                *output_value = raw_value;
                Ok(true)
            } else {
                Err(CsrError::IllegalRead { csr_index })
            }
        } else {
            Ok(false)
        }
    }

    /// Writes to `seed` ignore the write value entirely (per specification) and instead poll the
    /// entropy source once, storing the newly encoded [`ZkrSeedPoll`] to be observed by the next
    /// read
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn prepare_csr_write(
        ext_state: &mut ExtState,
        csr_index: u16,
        write_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError> {
        // Necessary to avoid unused variable depending on instructions composition
        let _: Reg::Type = write_value;

        if csr_index == SEED_CSR_INDEX {
            *output_value = zkr_helpers::encode_seed_poll::<Reg>(ext_state.poll_seed());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for ZkrInstruction<Reg>
where
    Reg: Register,
    ExtState: [const] ZkrSeedSource,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        ExecutionResult::ContinueNoWrite
    }
}
