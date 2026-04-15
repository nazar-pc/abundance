//! Basic implementations of various interpreter traits

use crate::{
    Address, ExecutionError, FetchInstructionResult, InstructionFetcher, ProgramCounter,
    ProgramCounterError, VirtualMemory,
};
use ab_riscv_primitives::prelude::*;
use core::marker::PhantomData;
use core::ops::ControlFlow;

/// Basic instruction fetcher implementation
#[derive(Debug, Copy, Clone)]
pub struct BasicInstructionFetcher<I, CustomError>
where
    I: Instruction,
{
    return_trap_address: Address<I>,
    pc: Address<I>,
    _phantom: PhantomData<CustomError>,
}

impl<I, Memory, CustomError> ProgramCounter<Address<I>, Memory, CustomError>
    for BasicInstructionFetcher<I, CustomError>
where
    I: Instruction,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn get_pc(&self) -> Address<I> {
        self.pc
    }

    #[inline]
    fn set_pc(
        &mut self,
        memory: &Memory,
        pc: Address<I>,
    ) -> Result<ControlFlow<()>, ProgramCounterError<Address<I>, CustomError>> {
        if pc == self.return_trap_address {
            return Ok(ControlFlow::Break(()));
        }

        if !pc.as_u64().is_multiple_of(u64::from(I::alignment())) {
            return Err(ProgramCounterError::UnalignedInstruction { address: pc });
        }

        memory.read::<u32>(pc.as_u64())?;

        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl<I, Memory, CustomError> InstructionFetcher<I, Memory, CustomError>
    for BasicInstructionFetcher<I, CustomError>
where
    I: Instruction,
    Memory: VirtualMemory,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        memory: &Memory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>, CustomError>> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method that does bound check. Otherwise, advancing forward by
        // one instruction can't result in out-of-bounds access.
        let instruction = unsafe { memory.read_unchecked(self.pc.as_u64()) };
        // SAFETY: All instructions are valid, according to the constructor contract
        let instruction = unsafe { I::try_decode(instruction).unwrap_unchecked() };
        self.pc += instruction.size().into();

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl<I, CustomError> BasicInstructionFetcher<I, CustomError>
where
    I: Instruction,
{
    /// Create a new instance.
    ///
    /// `return_trap_address` is the address at which the interpreter will stop execution
    /// (gracefully).
    ///
    /// # Safety
    /// The program counter must be valid and aligned, the instructions processed must be valid and
    /// end with a jump instruction.
    #[inline(always)]
    pub unsafe fn new(return_trap_address: Address<I>, pc: Address<I>) -> Self {
        Self {
            return_trap_address,
            pc,
            _phantom: PhantomData,
        }
    }
}
