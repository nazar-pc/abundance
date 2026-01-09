#![feature(bigint_helper_methods, const_convert, const_trait_impl)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![no_std]

pub mod b_64_ext;
pub mod m_64_ext;
pub mod rv64;

use ab_riscv_primitives::instruction::BaseInstruction;
use ab_riscv_primitives::registers::Register;
use core::fmt;
use core::marker::PhantomData;
use core::ops::ControlFlow;

type PC<Instruction> = <<Instruction as BaseInstruction>::Reg as Register>::Type;

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

    /// Unchecked read a value from memory at the specified address.
    ///
    /// # Safety
    /// The address and value must be in-bounds.
    unsafe fn read_unchecked<T>(&self, address: u64) -> T
    where
        T: BasicInt;

    /// Write a value to memory at the specified address
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt;
}

/// Program counter errors
#[derive(Debug, thiserror::Error)]
pub enum ProgramCounterError<Address, Custom>
where
    Address: fmt::Display,
    Custom: fmt::Display,
{
    /// Unaligned instruction
    #[error("Unaligned instruction at address {address}")]
    UnalignedInstruction {
        /// Address of the unaligned instruction fetch
        address: Address,
    },
    /// Memory access error
    #[error("Memory access error: {0}")]
    MemoryAccess(#[from] VirtualMemoryError),
    /// Custom error
    #[error("Custom error: {0}")]
    Custom(Custom),
}

/// Generic program counter
pub trait ProgramCounter<Address, Memory, CustomError>
where
    Address: fmt::Display,
    CustomError: fmt::Display,
{
    /// Get the current value of the program counter
    fn get_pc(&self) -> Address;

    /// Set the current value of the program counter
    fn set_pc(
        &mut self,
        memory: &mut Memory,
        pc: Address,
    ) -> Result<ControlFlow<()>, ProgramCounterError<Address, CustomError>>;
}

/// Execution errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError<Instruction, Custom>
where
    Instruction: BaseInstruction,
    Custom: fmt::Display,
{
    /// Unaligned instruction fetch
    #[error("Unaligned instruction fetch at address {address}")]
    UnalignedInstructionFetch {
        /// Address of the unaligned instruction fetch
        address: PC<Instruction>,
    },
    /// Program counter error
    #[error("Program counter error: {0}")]
    ProgramCounter(#[from] ProgramCounterError<PC<Instruction>, Custom>),
    /// Memory access error
    #[error("Memory access error: {0}")]
    MemoryAccess(#[from] VirtualMemoryError),
    /// Unsupported instruction
    #[error("Unsupported instruction at address {address:#x}: {instruction}")]
    UnsupportedInstruction {
        /// Address of the unsupported instruction
        address: PC<Instruction>,
        /// Instruction that caused the error
        instruction: Instruction,
    },
    /// Unimplemented/illegal instruction
    #[error("Unimplemented/illegal instruction at address {address:#x}")]
    UnimpInstruction {
        /// Address of the `unimp` instruction
        address: PC<Instruction>,
    },
    /// Invalid instruction
    #[error("Invalid instruction at address {address:#x}: {instruction:#010x}")]
    InvalidInstruction {
        /// Address of the invalid instruction
        address: PC<Instruction>,
        /// Instruction that caused the error
        instruction: u32,
    },
    /// Custom error
    #[error("Custom error: {0}")]
    Custom(Custom),
}

impl<BI, Custom> ExecutionError<BI, Custom>
where
    BI: BaseInstruction,
    Custom: fmt::Display,
{
    /// Map instruction type from lower-level base instruction
    #[inline]
    pub fn map_from_base<Instruction>(self) -> ExecutionError<Instruction, Custom>
    where
        Instruction: BaseInstruction<Reg = BI::Reg, Base = BI>,
    {
        match self {
            Self::UnalignedInstructionFetch { address } => {
                ExecutionError::UnalignedInstructionFetch { address }
            }
            Self::MemoryAccess(error) => ExecutionError::MemoryAccess(error),
            Self::ProgramCounter(error) => ExecutionError::ProgramCounter(error),
            Self::UnsupportedInstruction {
                address,
                instruction,
            } => ExecutionError::UnsupportedInstruction {
                address,
                instruction: Instruction::from_base(instruction),
            },
            Self::UnimpInstruction { address } => ExecutionError::UnimpInstruction { address },
            Self::InvalidInstruction {
                address,
                instruction,
            } => ExecutionError::InvalidInstruction {
                address,
                instruction,
            },
            Self::Custom(error) => ExecutionError::Custom(error),
        }
    }
}

/// Result of [`InstructionFetcher::fetch_instruction()`] call
#[derive(Debug, Copy, Clone)]
pub enum FetchInstructionResult<Instruction> {
    /// Instruction fetched successfully
    Instruction(Instruction),
    /// Control flow instruction encountered
    ControlFlow(ControlFlow<()>),
}

/// Generic instruction fetcher
pub trait InstructionFetcher<Instruction, Memory, CustomError>:
    ProgramCounter<PC<Instruction>, Memory, CustomError>
where
    Instruction: BaseInstruction,
    CustomError: fmt::Display,
{
    /// Fetch a single instruction at a specified address and advance the program counter
    fn fetch_instruction(
        &mut self,
        memory: &mut Memory,
    ) -> Result<FetchInstructionResult<Instruction>, ExecutionError<Instruction, CustomError>>;
}

/// Basic instruction fetcher implementation
#[derive(Debug, Copy, Clone)]
pub struct BasicInstructionFetcher<Instruction, CustomError>
where
    Instruction: BaseInstruction,
{
    return_trap_address: PC<Instruction>,
    pc: PC<Instruction>,
    _phantom: PhantomData<CustomError>,
}

impl<Instruction, Memory, CustomError> ProgramCounter<PC<Instruction>, Memory, CustomError>
    for BasicInstructionFetcher<Instruction, CustomError>
where
    Instruction: BaseInstruction,
    Memory: VirtualMemory,
    CustomError: fmt::Display,
{
    #[inline(always)]
    fn get_pc(&self) -> PC<Instruction> {
        self.pc
    }

    #[inline]
    fn set_pc(
        &mut self,
        memory: &mut Memory,
        pc: PC<Instruction>,
    ) -> Result<ControlFlow<()>, ProgramCounterError<PC<Instruction>, CustomError>> {
        if pc == self.return_trap_address {
            return Ok(ControlFlow::Break(()));
        }

        if !pc.into().is_multiple_of(size_of::<u32>() as u64) {
            return Err(ProgramCounterError::UnalignedInstruction { address: pc });
        }

        memory.read::<u32>(pc.into())?;

        self.pc = pc;

        Ok(ControlFlow::Continue(()))
    }
}

impl<Instruction, Memory, CustomError> InstructionFetcher<Instruction, Memory, CustomError>
    for BasicInstructionFetcher<Instruction, CustomError>
where
    Instruction: BaseInstruction,
    Memory: VirtualMemory,
    CustomError: fmt::Display,
{
    #[inline]
    fn fetch_instruction(
        &mut self,
        memory: &mut Memory,
    ) -> Result<FetchInstructionResult<Instruction>, ExecutionError<Instruction, CustomError>> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method that does bound check. Otherwise, advancing forward by
        // one instruction can't result in out-of-bounds access.
        let instruction = unsafe { memory.read_unchecked(self.pc.into()) };
        let instruction = Instruction::decode(instruction);
        self.pc += instruction.size().into();

        Ok(FetchInstructionResult::Instruction(instruction))
    }
}

impl<Instruction, CustomError> BasicInstructionFetcher<Instruction, CustomError>
where
    Instruction: BaseInstruction,
    [(); Instruction::Reg::N]:,
{
    /// Create a new instance.
    ///
    /// `return_trap_address` is the address at which the interpreter will stop execution
    /// (gracefully).
    ///
    /// # Safety
    /// The program counter must be valid and aligned, the instructions processed must end with a
    /// jump instruction.
    #[inline(always)]
    pub unsafe fn new(return_trap_address: PC<Instruction>, pc: PC<Instruction>) -> Self {
        Self {
            return_trap_address,
            pc,
            _phantom: PhantomData,
        }
    }
}
