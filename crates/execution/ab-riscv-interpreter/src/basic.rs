//! Basic implementations of various interpreter traits

#[cfg(test)]
mod tests;

use crate::{
    Address, CustomErrorPlaceholder, ExecutionError, FetchInstructionResult, InstructionFetcher,
    ProgramCounter, ProgramCounterError, VirtualMemory,
};
use ab_riscv_primitives::prelude::*;
use core::marker::PhantomData;
use core::ops::ControlFlow;

/// A basic set of RISC-V GPRs (General Purpose Registers)
#[derive(Debug, Clone, Copy)]
#[repr(align(16))]
pub struct BasicRegisters<Reg>
where
    Reg: Register,
    [(); Reg::N]:,
{
    regs: [Reg::Type; Reg::N],
}

impl<Reg> Default for BasicRegisters<Reg>
where
    Reg: Register,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            regs: [Reg::Type::default(); Reg::N],
        }
    }
}

const impl<Reg> BasicRegisters<Reg>
where
    Reg: [const] Register,
    [(); Reg::N]:,
{
    /// Read register value
    #[inline(always)]
    pub fn read(&self, reg: Reg) -> Reg::Type {
        if reg == Reg::ZERO {
            // Always zero
            return Reg::Type::default();
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(usize::from(reg.offset())) }
    }

    /// Write register value
    #[inline(always)]
    pub fn write(&mut self, reg: Reg, value: Reg::Type) {
        if reg == Reg::ZERO {
            // Writes are ignored
            return;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked_mut(usize::from(reg.offset())) } = value;
    }
}

/// Basic instruction fetcher implementation.
///
/// Note that it loads instructions from anywhere in memory. This works, but it is likely that you
/// want to restrict this to a specific executable region of memory.
#[derive(Debug, Copy, Clone)]
pub struct BasicInstructionFetcher<I, CustomError = CustomErrorPlaceholder>
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
        _memory: &Memory,
        pc: Address<I>,
    ) -> Result<ControlFlow<()>, ProgramCounterError<Address<I>, CustomError>> {
        if pc == self.return_trap_address {
            return Ok(ControlFlow::Break(()));
        }

        if !pc.as_u64().is_multiple_of(u64::from(I::alignment())) {
            return Err(ProgramCounterError::UnalignedInstruction { address: pc });
        }

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
        let instruction = memory.read(self.pc.as_u64()).or_else(|error| {
            // Attempt to read a 16-bit compressed instruction
            if let Ok(instruction) = memory.read::<u16>(self.pc.as_u64())
                && (instruction & 0b11) != 0b11
            {
                return Ok(u32::from(instruction));
            }
            Err(error)
        })?;

        let instruction = I::try_decode(instruction)
            .ok_or(ExecutionError::IllegalInstruction { address: self.pc })?;
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
    #[inline(always)]
    pub fn new(return_trap_address: Address<I>, pc: Address<I>) -> Self {
        Self {
            return_trap_address,
            pc,
            _phantom: PhantomData,
        }
    }
}
