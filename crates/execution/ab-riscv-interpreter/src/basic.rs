//! Basic implementations of various interpreter traits

#[cfg(test)]
mod tests;

use crate::{
    Address, BasicInt, CustomErrorPlaceholder, ExecutableInstruction, ExecutionError,
    FetchInstructionResult, InstructionFetcher, ProgramCounter, ProgramCounterError, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands, SystemInstructionHandler, VirtualMemory,
    VirtualMemoryError,
};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;
use core::marker::PhantomData;
use core::ops::ControlFlow;
use replace_with::replace_with_or_abort_and_return;

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

impl<Regs, ExtState, Memory, IF, InstructionHandler>
    BasicInterpreterState<Regs, ExtState, Memory, IF, InstructionHandler>
{
    /// Execute the program with a given basic interpreter state.
    ///
    /// The implementation is designed to be efficient with little left to optimize further. Though
    /// it is still possible to improve performance by applying additional constraints on the
    /// program.
    pub fn execute<I>(&mut self) -> Result<(), ExecutionError<Address<I>>>
    where
        Regs: RegisterFile<<I as Instruction>::Reg>,
        I: ExecutableInstruction<Regs, ExtState, Memory, IF, InstructionHandler>,
        Memory: VirtualMemory,
        IF: InstructionFetcher<I, Memory> + ProgramCounter<Address<I>, Memory>,
    {
        replace_with_or_abort_and_return(
            &mut self.instruction_fetcher,
            #[inline(always)]
            |mut instruction_fetcher| {
                loop {
                    let instruction = match instruction_fetcher.fetch_instruction(&self.memory) {
                        Ok(FetchInstructionResult::Instruction(instruction)) => instruction,
                        Ok(FetchInstructionResult::ControlFlow(ControlFlow::Continue(()))) => {
                            cold_path();
                            continue;
                        }
                        Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(()))) => {
                            cold_path();
                            break;
                        }
                        Err(error) => {
                            cold_path();
                            return (Err(error), instruction_fetcher);
                        }
                    };

                    let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
                    let rs1rs2_values = Rs1Rs2OperandValues {
                        rs1_value: self.regs.read(rs1),
                        rs2_value: self.regs.read(rs2),
                    };

                    match instruction.execute(
                        rs1rs2_values,
                        &mut self.regs,
                        &mut self.ext_state,
                        &mut self.memory,
                        &mut instruction_fetcher,
                        &mut self.system_instruction_handler,
                    ) {
                        Ok(ControlFlow::Continue((rd, rd_value))) => {
                            self.regs.write(rd, rd_value);
                        }
                        Ok(ControlFlow::Break(())) => {
                            cold_path();
                            break;
                        }
                        Err(error) => {
                            cold_path();
                            return (Err(error), instruction_fetcher);
                        }
                    }
                }

                (Ok(()), instruction_fetcher)
            },
        )
    }
}

/// Basic memory implementation.
///
/// Flat structure, no rwx protections, no alignment requirements. It uses stack, so for larger
/// allocation it'll need to be boxed (zero-initialized is fine) or a custom implementation to be
/// used.
///
/// This implementation is intentionally basic and correct, but not the most performant. It is
/// possible to have a more efficient implementation that skips certain checks by placing additional
/// constraints on the program.
///
/// This works for simpler cases, while a more sophisticated implementation might prevent certain
/// memory from being writable, supporting actual virtual memory with dynamically allocated memory
/// pages, etc.
#[derive(Debug, Copy, Clone)]
#[repr(align(16))]
pub struct BasicMemory<const BASE_ADDR: u64, const SIZE: usize> {
    data: [u8; SIZE],
}

impl<const BASE_ADDR: u64, const SIZE: usize> VirtualMemory for BasicMemory<BASE_ADDR, SIZE> {
    #[inline(always)]
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt,
    {
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        };

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        // SAFETY: Only reading basic integers from initialized memory
        unsafe {
            Ok(self
                .data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
                .read_unaligned())
        }
    }

    #[inline(always)]
    unsafe fn read_unchecked<T>(&self, address: u64) -> T
    where
        T: BasicInt,
    {
        // SAFETY: Guaranteed by function contract
        unsafe {
            let offset = address.unchecked_sub(BASE_ADDR) as usize;
            self.data
                .as_ptr()
                .cast::<T>()
                .byte_add(offset)
                .read_unaligned()
        }
    }

    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError> {
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        };

        if offset > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        }

        self.data
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })
    }

    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8] {
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
            cold_path();
            return &[];
        };

        if offset > self.data.len() as u64 {
            cold_path();
            return &[];
        }

        let remaining = self.data.get(offset as usize..).unwrap_or_default();
        remaining.get(..len as usize).unwrap_or(remaining)
    }

    #[inline(always)]
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        };

        if offset.saturating_add(size_of::<T>() as u64) > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        // SAFETY: Only writing basic integers to initialized memory
        unsafe {
            self.data
                .as_mut_ptr()
                .cast::<T>()
                .byte_add(offset as usize)
                .write_unaligned(value);
        }

        Ok(())
    }

    fn write_slice(&mut self, address: u64, data: &[u8]) -> Result<(), VirtualMemoryError> {
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        };

        if offset > self.data.len() as u64 {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        }

        let len = data.len();
        let Some(target_data) = self
            .data
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len))
        else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsWrite { address });
        };

        target_data.copy_from_slice(data);

        Ok(())
    }
}

impl<const BASE_ADDR: u64, const SIZE: usize> Default for BasicMemory<BASE_ADDR, SIZE> {
    #[inline(always)]
    fn default() -> Self {
        Self { data: [0; _] }
    }
}

impl<const BASE_ADDR: u64, const SIZE: usize> BasicMemory<BASE_ADDR, SIZE> {
    /// Get a mutable slice of memory.
    ///
    /// This is primarily useful for setting up the program and should not be used beyond that.
    pub fn get_mut_bytes(
        &mut self,
        address: u64,
        size: usize,
    ) -> Result<&mut [u8], VirtualMemoryError> {
        let Some(offset) = address.checked_sub(BASE_ADDR) else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        };
        let offset = offset as usize;

        let Some(slice) = self
            .data
            .get_mut(offset..)
            .and_then(|data| data.get_mut(..size))
        else {
            cold_path();
            return Err(VirtualMemoryError::OutOfBoundsRead { address });
        };

        Ok(slice)
    }
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
