//! Basic implementations of various interpreter traits

#[cfg(test)]
mod tests;

use crate::{
    Address, CustomErrorPlaceholder, ExecutionError, FetchInstructionResult, InstructionFetcher,
    ProgramCounter, ProgramCounterError, RegisterFile, SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;
use core::marker::PhantomData;
use core::ops::ControlFlow;

/// Basic general purpose register to be used with [`BasicRegisters`]
///
/// # Safety
/// `Self::offset()` must return values in `0..Self::N` range. `Self::from_bits()` must return
/// `Some()` for `0..=31` if `Self::RVE = false` and `0..=15` if `Self::RVE = true`.
pub const unsafe trait BasicRegister
where
    Self: [const] Register,
{
    /// The number of general purpose registers.
    ///
    /// Canonically 32 unless E extension is used, in which case 16.
    const N: usize;

    /// Offset in a set of registers
    fn offset(self) -> u8;
}

// SAFETY: `Self::offset()` returns values within `0..Self::N` range
unsafe impl<Type> const BasicRegister for EReg<Type>
where
    Self: [const] Register,
{
    const N: usize = 16;

    #[inline(always)]
    fn offset(self) -> u8 {
        // SAFETY: Enum is `#[repr(u8)]` and doesn't have any fields
        unsafe { core::mem::transmute::<Self, u8>(self) }
    }
}

// SAFETY: `Self::offset()` returns values within `0..Self::N` range
unsafe impl<Type> const BasicRegister for Reg<Type>
where
    Self: [const] Register,
{
    const N: usize = 32;

    #[inline(always)]
    fn offset(self) -> u8 {
        // SAFETY: Enum is `#[repr(u8)]` and doesn't have any fields
        unsafe { core::mem::transmute::<Self, u8>(self) }
    }
}

/// A basic set of RISC-V GPRs (General Purpose Registers)
#[derive(Debug, Clone, Copy)]
#[repr(align(16))]
pub struct BasicRegisters<Reg>
where
    Reg: BasicRegister,
    [(); Reg::N]:,
{
    regs: [Reg::Type; Reg::N],
}

impl<Reg> Default for BasicRegisters<Reg>
where
    Reg: BasicRegister,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            regs: [Reg::Type::default(); Reg::N],
        }
    }
}

impl<Reg> const RegisterFile<Reg> for BasicRegisters<Reg>
where
    Reg: [const] BasicRegister,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn read(&self, reg: Reg) -> Reg::Type {
        if reg == Reg::ZERO {
            // Always zero
            return Reg::Type::default();
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(usize::from(reg.offset())) }
    }

    #[inline(always)]
    fn write(&mut self, reg: Reg, value: Reg::Type) {
        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked_mut(usize::from(reg.offset())) } = value;
    }
}

/// Basic interpreter state.
///
/// This is a simple container, which is not required to be used, is helpful for storing the whole
/// state related to the interpreter together.
#[derive(Debug)]
pub struct BasicInterpreterState<Regs, ExtState, Memory, IF, InstructionHandler> {
    /// General purpose registers
    pub regs: Regs,
    /// Extended state.
    ///
    /// Extensions might use this to place additional constraints on `ExtState` to require
    /// additional registers or other resources. If no such extension is used, `()` can be used as
    /// a placeholder.
    pub ext_state: ExtState,
    /// Memory
    pub memory: Memory,
    /// Instruction fetcher
    pub instruction_fetcher: IF,
    /// System instruction handler
    pub system_instruction_handler: InstructionHandler,
}

/// Basic instruction fetcher implementation.
///
/// This implementation is intentionally basic and correct, but not the most performant. It is
/// possible to have a more efficient implementation that skips certain checks by placing additional
/// constraints on the constructor.
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
            cold_path();
            return Ok(ControlFlow::Break(()));
        }

        if !pc.as_u64().is_multiple_of(u64::from(I::alignment())) {
            cold_path();
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
        let instruction = match memory.read(self.pc.as_u64()).or_else(|error| {
            cold_path();
            // Attempt to read a 16-bit compressed instruction
            if let Ok(instruction) = memory.read::<u16>(self.pc.as_u64())
                && (instruction & 0b11) != 0b11
            {
                return Ok(u32::from(instruction));
            }
            Err(error)
        }) {
            Ok(instruction) => instruction,
            Err(error) => {
                cold_path();
                return Err(ExecutionError::MemoryAccess(error));
            }
        };

        let Some(instruction) = I::try_decode(instruction) else {
            cold_path();
            return Err(ExecutionError::IllegalInstruction { address: self.pc });
        };
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

/// System instruction handler that results in illegal instruction for all system calls and does
/// nothing for other system instructions
#[derive(Debug, Default, Clone, Copy)]
pub struct IllegalEcallSystemInstructionHandler;

impl<Reg, Regs, Memory, PC, CustomError>
    SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>
    for IllegalEcallSystemInstructionHandler
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(size_of::<u32>() as u8),
        })
    }
}
