#![feature(bigint_helper_methods, const_convert, const_trait_impl)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![no_std]

pub mod b_64_ext;
pub mod m_64_ext;
pub mod rv64;

use crate::rv64::Rv64SystemInstructionHandler;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::instruction::{GenericBaseInstruction, GenericInstruction};
use ab_riscv_primitives::registers::{GenericRegister, Registers};
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
pub enum ExecuteError<Instruction, Custom>
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

impl<BaseInstruction, Custom> ExecuteError<BaseInstruction, Custom>
where
    BaseInstruction: GenericBaseInstruction,
    Custom: fmt::Display,
{
    /// Map instruction type from lower-level base instruction
    #[inline]
    pub fn map_from_base<Instruction>(self) -> ExecuteError<Instruction, Custom>
    where
        Instruction: GenericBaseInstruction<Base = BaseInstruction>,
    {
        match self {
            Self::UnalignedInstructionFetch { address } => {
                ExecuteError::UnalignedInstructionFetch { address }
            }
            Self::MemoryAccess(error) => ExecuteError::MemoryAccess(error),
            Self::UnsupportedInstruction {
                address,
                instruction,
            } => ExecuteError::UnsupportedInstruction {
                address,
                instruction: Instruction::from_base(instruction),
            },
            Self::UnimpInstruction { address } => ExecuteError::UnimpInstruction { address },
            Self::InvalidInstruction {
                address,
                instruction,
            } => ExecuteError::InvalidInstruction {
                address,
                instruction,
            },
            Self::Custom(error) => ExecuteError::Custom(error),
        }
    }
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
pub trait GenericInstructionHandler<Instruction, Memory, CustomError>
where
    Instruction: GenericBaseInstruction,
    [(); Instruction::Reg::N]:,
    CustomError: fmt::Display,
{
    /// Fetch a single instruction at a specified address and advance the program counter
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers<Instruction::Reg>,
        memory: &mut Memory,
        pc: &mut u64,
    ) -> Result<FetchInstructionResult<Instruction>, ExecuteError<Instruction, CustomError>>;
}

/// Basic instruction handler implementation.
///
/// `RETURN_TRAP_ADDRESS` is the address at which the interpreter will stop execution (gracefully).
#[derive(Debug, Default, Copy, Clone)]
pub struct BasicInstructionHandler<const RETURN_TRAP_ADDRESS: u64>;

impl<const RETURN_TRAP_ADDRESS: u64, Instruction, Memory>
    GenericInstructionHandler<Instruction, Memory, &'static str>
    for BasicInstructionHandler<RETURN_TRAP_ADDRESS>
where
    Instruction: GenericBaseInstruction,
    [(); Instruction::Reg::N]:,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn fetch_instruction(
        &mut self,
        _regs: &mut Registers<Instruction::Reg>,
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
}

impl<const RETURN_TRAP_ADDRESS: u64, Reg, Memory>
    Rv64SystemInstructionHandler<Reg, Memory, &'static str>
    for BasicInstructionHandler<RETURN_TRAP_ADDRESS>
where
    Reg: GenericRegister<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
{
    #[inline(always)]
    fn handle_ecall(
        &mut self,
        _regs: &mut Registers<Reg>,
        _memory: &mut Memory,
        pc: &mut u64,
        instruction: Rv64Instruction<Reg>,
    ) -> Result<(), ExecuteError<Rv64Instruction<Reg>, &'static str>> {
        Err(ExecuteError::UnsupportedInstruction {
            address: *pc - instruction.size() as u64,
            instruction,
        })
    }
}
