#![feature(bigint_helper_methods)]
#![no_std]

pub mod b_64_ext;
pub mod m_64_ext;
pub mod rv64;
#[cfg(test)]
mod tests_utils;

use crate::b_64_ext::execute_b_zbc_64_ext;
use crate::m_64_ext::execute_m_64_ext;
use crate::rv64::execute_rv64;
use ab_riscv_primitives::instruction::{
    GenericBaseInstruction, GenericInstruction, Rv64MBZbcInstruction,
};
use ab_riscv_primitives::registers::{GenericRegister64, GenericRegisters64};
use core::fmt;
use core::ops::ControlFlow;

/// Errors for [`VirtualMemory`]
#[derive(Debug, thiserror::Error)]
pub enum VirtualMemoryError {
    /// Out-of-bounds read
    #[error("Out-of-bounds read at address {address}")]
    OutOfBoundsRead {
        /// Address of the out-of-bounds read
        address: u64,
    },
    /// Out-of-bounds write
    #[error("Out-of-bounds write at address {address}")]
    OutOfBoundsWrite {
        /// Address of the out-of-bounds write
        address: u64,
    },
}

mod private {
    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for i8 {}
    impl Sealed for i16 {}
    impl Sealed for i32 {}
    impl Sealed for i64 {}
}

/// Basic integer types that can be read and written to/from memory freely
pub trait BasicInt: Sized + Copy + private::Sealed {}

impl BasicInt for u8 {}
impl BasicInt for u16 {}
impl BasicInt for u32 {}
impl BasicInt for u64 {}
impl BasicInt for i8 {}
impl BasicInt for i16 {}
impl BasicInt for i32 {}
impl BasicInt for i64 {}

/// Virtual memory interface
pub trait VirtualMemory {
    /// Read a value from memory at the specified address
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt;

    /// Write a value to memory at the specified address
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt;
}

/// Execution errors
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError<Instruction, Custom = &'static str>
where
    Instruction: fmt::Display,
    Custom: fmt::Display,
{
    /// Unaligned instruction fetch
    #[error("Unaligned instruction fetch at address {address}")]
    UnalignedInstructionFetch {
        /// Address of the unaligned instruction fetch
        address: u64,
    },
    /// Memory access error
    #[error("Memory access error: {0}")]
    MemoryAccess(#[from] VirtualMemoryError),
    /// Unsupported instruction
    #[error("Unsupported instruction at address {address:#x}: {instruction}")]
    UnsupportedInstruction {
        /// Address of the unsupported instruction
        address: u64,
        /// Instruction that caused the error
        instruction: Instruction,
    },
    /// Unimplemented/illegal instruction
    #[error("Unimplemented/illegal instruction at address {address:#x}")]
    UnimpInstruction {
        /// Address of the `unimp` instruction
        address: u64,
    },
    /// Invalid instruction
    #[error("Invalid instruction at address {address:#x}: {instruction:#010x}")]
    InvalidInstruction {
        /// Address of the invalid instruction
        address: u64,
        /// Instruction that caused the error
        instruction: u32,
    },
    /// Custom error
    #[error("Custom error: {0}")]
    Custom(Custom),
}

/// Result of [`GenericInstructionHandler::fetch_instruction()`] call
#[derive(Debug, Copy, Clone)]
pub enum FetchInstructionResult<Instruction> {
    /// Instruction fetched successfully
    Instruction(Instruction),
    /// Control flow instruction encountered
    ControlFlow(ControlFlow<()>),
}

/// Custom handlers for instructions `ecall` and `ebreak`
pub trait GenericInstructionHandler<Instruction, Registers, Memory, CustomError>
where
    Instruction: GenericInstruction,
    CustomError: fmt::Display,
{
    /// Fetch a single instruction at a specified address and advance the program counter
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers,
        memory: &mut Memory,
        pc: &mut u64,
    ) -> Result<FetchInstructionResult<Instruction>, ExecuteError<Instruction, CustomError>>;

    /// Handle an `ecall` instruction.
    ///
    /// NOTE: the program counter here is the current value, meaning it is already incremented past
    /// the instruction itself.
    fn handle_ecall(
        &mut self,
        regs: &mut Registers,
        memory: &mut Memory,
        pc: &mut u64,
        instruction: Instruction,
    ) -> Result<(), ExecuteError<Instruction, CustomError>>;

    /// Handle an `ebreak` instruction.
    ///
    /// NOTE: the program counter here is the current value, meaning it is already incremented past
    /// the instruction itself.
    #[inline(always)]
    fn handle_ebreak(
        &mut self,
        _regs: &mut Registers,
        _memory: &mut Memory,
        _pc: &mut u64,
        _instruction: Instruction,
    ) -> Result<(), ExecuteError<Instruction, CustomError>> {
        // NOP by default
        Ok(())
    }
}

/// Basic instruction handler implementation.
///
/// `RETURN_TRAP_ADDRESS` is the address at which the interpreter will stop execution (gracefully).
#[derive(Debug, Default, Copy, Clone)]
pub struct BasicInstructionHandler<const RETURN_TRAP_ADDRESS: u64>;

impl<const RETURN_TRAP_ADDRESS: u64, Instruction, Registers, Memory>
    GenericInstructionHandler<Instruction, Registers, Memory, &'static str>
    for BasicInstructionHandler<RETURN_TRAP_ADDRESS>
where
    Instruction: GenericBaseInstruction,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers,
        memory: &mut Memory,
        pc: &mut u64,
    ) -> Result<FetchInstructionResult<Instruction>, ExecuteError<Instruction, &'static str>> {
        let address = *pc;

        if address == RETURN_TRAP_ADDRESS {
            return Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(())));
        }

        if !address.is_multiple_of(size_of::<u32>() as u64) {
            return Err(ExecuteError::UnalignedInstructionFetch { address });
        }

        let instruction = memory.read(address)?;
        let instruction = Instruction::decode(instruction);
        *pc += instruction.size() as u64;

        Ok(FetchInstructionResult::Instruction(instruction))
    }

    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers,
        _memory: &mut Memory,
        pc: &mut u64,
        instruction: Instruction,
    ) -> Result<(), ExecuteError<Instruction, &'static str>> {
        Err(ExecuteError::UnsupportedInstruction {
            address: *pc - instruction.size() as u64,
            instruction,
        })
    }
}

/// Execute RV64IMBZbc/RV64EMBZbc instructions
pub fn execute_rv64mbzbc<Reg, Registers, Memory, InstructionHandler, CustomError>(
    regs: &mut Registers,
    memory: &mut Memory,
    pc: &mut u64,
    instruction_handlers: &mut InstructionHandler,
) -> Result<(), ExecuteError<Rv64MBZbcInstruction<Reg>, CustomError>>
where
    Reg: GenericRegister64,
    Registers: GenericRegisters64<Reg>,
    Memory: VirtualMemory,
    InstructionHandler:
        GenericInstructionHandler<Rv64MBZbcInstruction<Reg>, Registers, Memory, CustomError>,
    CustomError: fmt::Display,
{
    loop {
        let old_pc = *pc;
        let instruction = match instruction_handlers.fetch_instruction(regs, memory, pc)? {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::ControlFlow(ControlFlow::Continue(())) => {
                continue;
            }
            FetchInstructionResult::ControlFlow(ControlFlow::Break(())) => {
                break;
            }
        };

        match instruction {
            Rv64MBZbcInstruction::A(instruction) => {
                execute_m_64_ext(regs, instruction);
            }
            Rv64MBZbcInstruction::B(instruction) => {
                execute_b_zbc_64_ext(regs, instruction);
            }
            Rv64MBZbcInstruction::Base(instruction) => {
                execute_rv64(regs, memory, pc, instruction_handlers, old_pc, instruction)?;
            }
        }
    }

    Ok(())
}
