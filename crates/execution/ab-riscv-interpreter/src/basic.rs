//! Basic implementations of various interpreter traits

#[cfg(test)]
mod tests;

use crate::zawrs::WrsHandler;
use crate::{
    Address, BasicInt, ExecutableInstruction, ExecutionError, ExecutionResult,
    FetchInstructionResult, InstructionFetcher, PackedAddress, ProgramCounter, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands, SystemInstructionHandler, VirtualMemory,
    VirtualMemoryError,
};
use ab_riscv_primitives::prelude::*;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::hint::cold_path;
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
const unsafe impl<Type> BasicRegister for EReg<Type>
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
const unsafe impl<Type> BasicRegister for Reg<Type>
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
{
    regs: [Reg::Type; Reg::N],
}

impl<Reg> Default for BasicRegisters<Reg>
where
    Reg: BasicRegister,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            regs: [Reg::Type::default(); _],
        }
    }
}

const impl<Reg> RegisterFile<Reg> for BasicRegisters<Reg>
where
    Reg: [const] BasicRegister,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn read(&self, reg: Reg) -> Reg::Type {
        if reg == Reg::ZERO {
            // Always zero
            return Reg::Type::default();
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(usize::from(reg.offset())) }
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
    // TODO: It might be impractical to support `no-panic` here directly in a general case, but it
    //  should be possible to do so for small extensions to verify the workflow
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
                        FetchInstructionResult::Instruction(instruction) => instruction,
                        FetchInstructionResult::Continue => {
                            cold_path();
                            continue;
                        }
                        FetchInstructionResult::Break => {
                            cold_path();
                            break;
                        }
                        FetchInstructionResult::Err(error) => {
                            cold_path();
                            return (Err(error), instruction_fetcher);
                        }
                    };

                    let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
                    let rs1rs2_values = Rs1Rs2OperandValues {
                        rs1_value: self.regs.read(rs1),
                        rs2_value: self.regs.read(rs2),
                    };

                    let outcome = instruction.execute(
                        rs1rs2_values,
                        &mut self.regs,
                        &mut self.ext_state,
                        &mut self.memory,
                        &mut instruction_fetcher,
                        &mut self.system_instruction_handler,
                    );

                    let control_flow =
                        match outcome {
                            ExecutionResult::Continue { rd, value } => {
                                self.regs.write(rd, value);
                                continue;
                            }
                            ExecutionResult::ContinueNoWrite => {
                                continue;
                            }
                            ExecutionResult::Branch { offset } => instruction_fetcher
                                .set_pc_relative(&self.memory, instruction.size(), offset),
                            ExecutionResult::Jump { target } => {
                                instruction_fetcher.set_pc(&self.memory, target)
                            }
                            ExecutionResult::Break => {
                                cold_path();
                                break;
                            }
                            ExecutionResult::Err(error) => {
                                cold_path();
                                return (Err(error), instruction_fetcher);
                            }
                        };

                    match control_flow {
                        Ok(ControlFlow::Continue(())) => {}
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

const impl<const BASE_ADDR: u64, const SIZE: usize> VirtualMemory for BasicMemory<BASE_ADDR, SIZE> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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

    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
            .and_then(const |data| data.get(..len as usize))
            .ok_or(VirtualMemoryError::OutOfBoundsRead { address })
    }

    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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

    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
            .and_then(const |data| data.get_mut(..len))
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
    #[cfg(feature = "alloc")]
    pub fn new_boxed() -> Box<Self> {
        // SAFETY: Zeroed memory is a valid invariant
        unsafe { Box::<Self>::new_zeroed().assume_init() }
    }

    /// Get a mutable slice of memory.
    ///
    /// This is primarily useful for setting up the program and should not be used beyond that.
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn get_mut_bytes(
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
            .and_then(const |data| data.get_mut(..size))
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
pub struct BasicInstructionFetcher<I>
where
    I: Instruction,
{
    return_trap_address: Address<I>,
    pc: Address<I>,
}

const impl<I, Memory> ProgramCounter<Address<I>, Memory> for BasicInstructionFetcher<I>
where
    I: [const] Instruction,
    Memory: [const] VirtualMemory,
{
    #[inline(always)]
    fn get_pc(&self) -> Address<I> {
        self.pc
    }

    #[inline(always)]
    fn set_pc_relative(
        &mut self,
        memory: &Memory,
        instruction_size: u8,
        offset: i32,
    ) -> Result<ControlFlow<()>, ExecutionError<Address<I>>> {
        let old_pc = <Self as ProgramCounter<_, Memory>>::old_pc(self, instruction_size);
        self.set_pc(memory, old_pc.wrapping_add_signed(offset))
    }

    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_pc(
        &mut self,
        _memory: &Memory,
        pc: Address<I>,
    ) -> Result<ControlFlow<()>, ExecutionError<Address<I>>> {
        if pc == self.return_trap_address {
            cold_path();
            return Ok(ControlFlow::Break(()));
        }

        if !pc.as_u64().is_multiple_of(u64::from(I::alignment())) {
            cold_path();
            return Err(ExecutionError::UnalignedInstruction {
                address: PackedAddress::new(pc),
            });
        }

        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

const impl<I, Memory> InstructionFetcher<I, Memory> for BasicInstructionFetcher<I>
where
    I: [const] Instruction,
    Memory: [const] VirtualMemory,
{
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn fetch_instruction(&mut self, memory: &Memory) -> FetchInstructionResult<I> {
        let instruction = match memory.read(self.pc.as_u64()).or_else(const |error| {
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
                return FetchInstructionResult::Err(ExecutionError::from(error));
            }
        };

        let Some(instruction) = I::try_decode(instruction) else {
            cold_path();
            return FetchInstructionResult::Err(ExecutionError::IllegalInstruction {
                address: PackedAddress::new(self.pc),
            });
        };
        self.pc += instruction.size().into();

        FetchInstructionResult::Instruction(instruction)
    }
}

impl<I> BasicInstructionFetcher<I>
where
    I: Instruction,
{
    /// Create a new instance.
    ///
    /// `return_trap_address` is the address at which the interpreter will stop execution
    /// (gracefully).
    #[inline(always)]
    pub const fn new(return_trap_address: Address<I>, pc: Address<I>) -> Self {
        Self {
            return_trap_address,
            pc,
        }
    }
}

/// System instruction handler that results in illegal instruction for all system calls and does
/// nothing for other system instructions
#[derive(Debug, Default, Clone, Copy)]
pub struct IllegalEcallSystemInstructionHandler;

const impl<Reg, Regs, Memory, PC> SystemInstructionHandler<Reg, Regs, Memory, PC>
    for IllegalEcallSystemInstructionHandler
where
    Reg: [const] Register,
    PC: [const] ProgramCounter<Reg::Type, Memory>,
{
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type>> {
        Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(size_of::<u32>() as u8)),
        })
    }
}

const impl WrsHandler for IllegalEcallSystemInstructionHandler {}
