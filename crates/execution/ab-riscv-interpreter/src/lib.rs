#![feature(const_convert, const_trait_impl, control_flow_ok, widening_mul)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
// TODO: This feature is not actually used in this crate, but is added as a workaround for
//  https://github.com/rust-lang/rust/issues/141492
#![feature(generic_const_exprs)]
#![no_std]

pub mod rv64;

use ab_riscv_primitives::instruction::Instruction;
use ab_riscv_primitives::registers::Register;
use core::marker::PhantomData;
use core::ops::ControlFlow;

type Address<I> = <<I as Instruction>::Reg as Register>::Type;

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
pub enum ProgramCounterError<Address, Custom> {
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
pub trait ProgramCounter<Address, Memory, CustomError> {
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
pub enum ExecutionError<Address, I, Custom> {
    /// Unaligned instruction fetch
    #[error("Unaligned instruction fetch at address {address}")]
    UnalignedInstructionFetch {
        /// Address of the unaligned instruction fetch
        address: Address,
    },
    /// Program counter error
    #[error("Program counter error: {0}")]
    ProgramCounter(#[from] ProgramCounterError<Address, Custom>),
    /// Memory access error
    #[error("Memory access error: {0}")]
    MemoryAccess(#[from] VirtualMemoryError),
    /// Unsupported instruction
    #[error("Unsupported instruction at address {address:#x}: {instruction}")]
    UnsupportedInstruction {
        /// Address of the unsupported instruction
        address: Address,
        /// Instruction that caused the error
        instruction: I,
    },
    /// Unimplemented/illegal instruction
    #[error("Unimplemented/illegal instruction at address {address:#x}")]
    UnimpInstruction {
        /// Address of the `unimp` instruction
        address: Address,
    },
    /// Invalid instruction
    #[error("Invalid instruction at address {address:#x}: {instruction:#010x}")]
    InvalidInstruction {
        /// Address of the invalid instruction
        address: Address,
        /// Instruction that caused the error
        instruction: u32,
    },
    /// Custom error
    #[error("Custom error: {0}")]
    Custom(Custom),
}

impl<Address, BI, Custom> ExecutionError<Address, BI, Custom> {
    /// Map instruction type
    #[inline]
    pub fn map_instruction<I>(self, map: fn(BI) -> I) -> ExecutionError<Address, I, Custom> {
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
                instruction: map(instruction),
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
pub trait InstructionFetcher<I, Memory, CustomError>
where
    Self: ProgramCounter<Address<I>, Memory, CustomError>,
    I: Instruction,
{
    /// Fetch a single instruction at a specified address and advance the program counter
    fn fetch_instruction(
        &mut self,
        memory: &mut Memory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>, I, CustomError>>;
}

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
        memory: &mut Memory,
        pc: Address<I>,
    ) -> Result<ControlFlow<()>, ProgramCounterError<Address<I>, CustomError>> {
        if pc == self.return_trap_address {
            return Ok(ControlFlow::Break(()));
        }

        if !pc.into().is_multiple_of(u64::from(I::alignment())) {
            return Err(ProgramCounterError::UnalignedInstruction { address: pc });
        }

        memory.read::<u32>(pc.into())?;

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
        memory: &mut Memory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>, I, CustomError>> {
        // SAFETY: Constructor guarantees that the last instruction is a jump, which means going
        // through `Self::set_pc()` method that does bound check. Otherwise, advancing forward by
        // one instruction can't result in out-of-bounds access.
        let instruction = unsafe { memory.read_unchecked(self.pc.into()) };
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

/// Trait for executable instructions
pub trait ExecutableInstruction<State, CustomError>
where
    Self: Instruction,
{
    /// Execute instruction
    fn execute(
        self,
        state: &mut State,
    ) -> Result<ControlFlow<()>, ExecutionError<Address<Self>, Self, CustomError>>;
}
