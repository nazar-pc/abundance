//! Composable and generic RISC-V interpreter.
//!
//! This interpreter is designed to work with abstractions from [`ab-riscv-primitives`] crate and is
//! similarly composable with a powerful macro system and trait abstractions over handling of
//! memory, syscalls, etc.
//!
//! [`ab-riscv-primitives`]: ab_riscv_primitives
//!
//! The immediate needs dictate the current set of available instructions and extensions. Consider
//! contributing if you need something not yet available.
//!
//! `ab-riscv-act4-runner` crate in the repository contains a complementary RISC-V Architectural
//! Certification Tests runner for <https://github.com/riscv-non-isa/riscv-arch-test> that ensures
//! correct implementation.
//!
//! Does not require a standard library (`no_std`) or an allocator, never panics, almost 100% of the
//! API abstractions are usable in const, including several extensions beyond base ISA.
//!
//! ## Supported ISA variants and extensions
//!
//! ISA variants:
//! * RV32I (version 2.1)
//! * RV32E (version 2.0)
//! * RV64I (version 2.1)
//! * RV64E (version 2.0)
//!
//! Extensions:
//! * A (version 2.1)
//! * M (version 2.0)
//! * B (version 1.0.0)
//! * Zaamo (version 1.0.0)
//! * Zabha (version 1.0.0)
//! * Zacas (version 1.0.0)
//! * (experimental) Zalasr (version 1.0.0)
//! * Zalrsc (version 1.0.0)
//! * Zawrs (version 1.0.0)
//! * Zba (version 1.0.0)
//! * Zbb (version 1.0.0)
//! * Zbc (version 1.0.0)
//! * Zbkb (version 1.0.1)
//! * Zbkc (version 1.0.1)
//! * Zbkx (version 1.0.1)
//! * Zbs (version 1.0.0)
//! * Zca (version 1.0.0)
//! * Zcb (version 1.0.0)
//! * (experimental) Zcmp (version 1.0.0)
//! * Zicond (version 2.0)
//! * Zicsr (version 2.0)
//! * Zkn (version 1.0.1)
//! * Zknd (version 1.0.1)
//! * Zkne (version 1.0.1)
//! * Zknh (version 1.0.1)
//! * Zkr (version 1.0.1)
//! * Zvbb (version 1.0.0)
//! * Zvbc (version 1.0.0)
//! * ZveXx (version 1.0.0), where `X` is anything allowed by the specification like Zve32x or
//!   Zve64x
//! * Zvkb (version 1.0.0)
//! * Zvl*b (version 1.0.0), where `*` is anything allowed by the specification like Zvl128b or
//!   Zvl512b
//!
//! All extensions except experimental pass all relevant RISC-V Architectural Certification Tests
//! (ACTs) using the ACT4 framework.
//!
//! Any permutation of compatible extensions is supported.
//!
//! Experimental extensions may not have ACT4 tests yet and are not guaranteed to work correctly.
//!
//! ## Design choices
//!
//! This crate was designed with a blockchain use case in mind, though it is in no way tied to any
//! particular blockchain and is completely general purpose. As a result, the implementation is
//! designed to be precise and non-ambiguous.
//!
//! A few key points:
//! * anything "reserved" in the specification is considered to be illegal
//! * anything "optional" in the specification is considered to be illegal
//! * anything "implementation-defined" in the specification is selected to be the most natural and
//!   deterministic
//! * type system is used to make the majority of invalid invariants impossible to represent in code
//!   and/or decode
//!
//! Examples:
//! * `vma`/`vta` are always undisturbed in vector extensions
//! * Zve64x extension instructions are purposefully restricted to what it is required to be capable
//!   of, although it would be cheaper to support the fuller feature set only required by V
//!   extension
//!
//! ### Instruction implementations are assembled, not compiled in place
//!
//! Instruction implementations are not compiled where they are written. `build.rs` calls
//! `ab_riscv_macros::process_instruction_macros()`, which parses the sources marked with the
//! `#[instruction]` and `#[instruction_execution]` attribute macros and writes a generated
//! execution implementation per instruction set into `OUT_DIR`.
//!
//! Macros have documentation about how they work, but the important thing is that `match`
//! expressions are parsed and re-assembled as needed instead of being compiled in place.
//!
//! Two things follow from this, and both constrain how instruction implementations must be written:
//! * the same arm is expanded into more than one crate, so a `crate::`-relative path inside an arm
//!   resolves against whichever crate it landed in and is avoided. Shared code is reached through
//!   helper modules re-exported from [`prelude`] - like `rv64_zbb_helpers` and similar - so that an
//!   arm can name them unqualified wherever it ends up
//! * because each arm is kept separately and names exactly the operands its instruction uses, the
//!   same arms can be emitted as something other than one large `match`. Emitting them as
//!   standalone per-instruction functions, dispatched by tail call, is what allows an interpreter
//!   to avoid decoding operands that the instruction about to run does not use, and to stop paying
//!   for extensions that are compiled in but never executed

#![expect(incomplete_features, reason = "generic_const_*, explicit_tail_calls")]
#![feature(
    adt_const_params,
    const_block_items,
    const_closures,
    const_cmp,
    const_convert,
    const_default,
    const_destruct,
    const_index,
    const_ops,
    const_option_ops,
    const_result_trait_fn,
    const_trait_impl,
    const_try,
    explicit_tail_calls,
    generic_const_args,
    generic_const_items,
    impl_restriction,
    inherent_associated_types,
    integer_widen_truncate,
    macroless_generic_const_args,
    min_generic_const_args,
    signed_bigint_helpers,
    try_trait_v2,
    widening_mul
)]
#![cfg_attr(test, feature(try_blocks))]
#![cfg_attr(
    not(any(
        all(target_arch = "riscv32", target_feature = "zbkx"),
        all(target_arch = "riscv64", target_feature = "zbkx")
    )),
    feature(portable_simd)
)]
#![cfg_attr(
    any(
        all(
            target_arch = "riscv32",
            any(
                target_feature = "zbb",
                target_feature = "zbc",
                target_feature = "zbkb",
                target_feature = "zbkx",
                target_feature = "zknd",
                target_feature = "zkne",
                target_feature = "zknh"
            ),
            not(miri)
        ),
        all(
            target_arch = "riscv64",
            any(
                target_feature = "zbb",
                target_feature = "zbc",
                target_feature = "zbkx",
                target_feature = "zknd",
                target_feature = "zkne",
                target_feature = "zknh"
            ),
            not(miri)
        )
    ),
    feature(riscv_ext_intrinsics)
)]
#![cfg_attr(
    test,
    expect(
        clippy::rest_pattern_accessible_field,
        reason = "Too verbose for tests"
    )
)]
#![no_std]

pub mod basic;
pub mod prelude;
pub mod rv32;
pub mod rv64;
#[cfg(test)]
mod tests;
pub mod v;
pub mod zawrs;
pub mod zicond;
pub mod zicsr;
pub mod zkr;
pub mod zvbb;
pub mod zvbc;

#[cfg(feature = "alloc")]
extern crate alloc;

use ab_riscv_primitives::prelude::*;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::convert::Infallible;
use core::fmt;
use core::hint::cold_path;
use core::marker::{Destruct, PhantomData};
#[cfg(all(target_arch = "x86_64", any(not(miri), target_feature = "avx")))]
use core::mem;
use core::ops::{ControlFlow, FromResidual, Sub};

type RegisterType<I> = <<I as Instruction>::Reg as Register>::Type;
type Address<I> = RegisterType<I>;

/// A GPR (General Purpose Register) file abstraction
pub const trait RegisterFile<Reg>
where
    Reg: [const] Register,
{
    /// Read register value
    fn read(&self, reg: Reg) -> Reg::Type;

    /// Write register value
    fn write(&mut self, reg: Reg, value: Reg::Type);
}

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

/// Basic integer types that can be read and written to/from memory freely
pub impl(self) trait BasicInt: Sized + Copy + 'static {}

impl BasicInt for u8 {}
impl BasicInt for u16 {}
impl BasicInt for u32 {}
impl BasicInt for u64 {}
impl BasicInt for i8 {}
impl BasicInt for i16 {}
impl BasicInt for i32 {}
impl BasicInt for i64 {}

/// Virtual memory interface
pub const trait VirtualMemory {
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

    /// Read a contiguous byte slice from memory
    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError>;

    /// Read as many contiguous bytes as possible starting at `address`, up to `len` bytes total.
    ///
    /// Can return an empty slice in cases like when the address is out of bounds.
    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8];

    /// Write a value to memory at the specified address
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt;

    /// Write a contiguous byte slice to memory
    fn write_slice(&mut self, address: u64, data: &[u8]) -> Result<(), VirtualMemoryError>;
}

#[cfg(feature = "alloc")]
impl<M> VirtualMemory for Box<M>
where
    M: VirtualMemory,
{
    #[inline(always)]
    fn read<T>(&self, address: u64) -> Result<T, VirtualMemoryError>
    where
        T: BasicInt,
    {
        self.as_ref().read(address)
    }

    #[inline(always)]
    unsafe fn read_unchecked<T>(&self, address: u64) -> T
    where
        T: BasicInt,
    {
        // SAFETY: Guaranteed by the caller
        unsafe { self.as_ref().read_unchecked(address) }
    }

    #[inline(always)]
    fn read_slice(&self, address: u64, len: u32) -> Result<&[u8], VirtualMemoryError> {
        self.as_ref().read_slice(address, len)
    }

    #[inline(always)]
    fn read_slice_up_to(&self, address: u64, len: u32) -> &[u8] {
        self.as_ref().read_slice_up_to(address, len)
    }

    #[inline(always)]
    fn write<T>(&mut self, address: u64, value: T) -> Result<(), VirtualMemoryError>
    where
        T: BasicInt,
    {
        self.as_mut().write(address, value)
    }

    #[inline(always)]
    fn write_slice(&mut self, address: u64, data: &[u8]) -> Result<(), VirtualMemoryError> {
        self.as_mut().write_slice(address, data)
    }
}

/// Generic program counter
pub const trait ProgramCounter<Address, Memory>
where
    Address: Copy,
{
    /// Get the current value of the program counter
    fn get_pc(&self) -> Address;

    /// Get the previous value of the program counter before executing an `instruction`.
    ///
    /// This is usually called from under instruction execution when the program counter is already
    /// advanced during instruction fetching. As such, `pc - instruction_size` is expected to never
    /// underflow.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn old_pc(&self, instruction_size: u8) -> Address
    where
        Address: [const] From<u8> + [const] Sub<Output = Address>,
    {
        // TODO: Wrapping subtraction would be nice, but causes a lot of additional generic bounds
        //  that are bad for ergonomics
        self.get_pc() - Address::from(instruction_size)
    }

    /// Apply [`ExecutionResult::Branch`], continuing `offset` bytes from the instruction being
    /// executed.
    ///
    /// A simple implementation can resolve the offset against the program counter and call
    /// [`Self::set_pc()`]. Implementation that keeps the program counter as a position in an
    /// already decoded instruction stream can move within that stream directly, which avoids
    /// converting an address back into a position.
    fn set_pc_relative(
        &mut self,
        memory: &Memory,
        instruction_size: u8,
        offset: i32,
    ) -> Result<ControlFlow<()>, ExecutionError<Address>>;

    /// Set the current value of the program counter
    fn set_pc(
        &mut self,
        memory: &Memory,
        pc: Address,
    ) -> Result<ControlFlow<()>, ExecutionError<Address>>;
}

/// Address wrapper for [`ExecutionError`] with an alignment of 4 rather than its natural one.
///
/// This is needed for [`ExecutionResult`] that contains [`ExecutionError`] to fit within 16 bytes
/// or two native registers on 64-bit platforms.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(C, packed(4))]
pub struct PackedAddress<Address>(Address);

impl<Address> PackedAddress<Address>
where
    Address: Copy,
{
    /// Create a new instance
    #[inline(always)]
    pub const fn new(address: Address) -> Self {
        Self(address)
    }

    /// Read the address back
    #[inline(always)]
    pub const fn get(self) -> Address {
        self.0
    }
}

const impl<Address> From<Address> for PackedAddress<Address>
where
    Address: Copy + [const] Destruct,
{
    #[inline(always)]
    fn from(address: Address) -> Self {
        Self(address)
    }
}

impl<Address> fmt::Debug for PackedAddress<Address>
where
    Address: fmt::Debug + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let address = self.0;
        fmt::Debug::fmt(&address, f)
    }
}

impl<Address> fmt::Display for PackedAddress<Address>
where
    Address: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let address = self.0;
        fmt::Display::fmt(&address, f)
    }
}

impl<Address> fmt::LowerHex for PackedAddress<Address>
where
    Address: fmt::LowerHex + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let address = self.0;
        fmt::LowerHex::fmt(&address, f)
    }
}

/// Execution errors.
///
/// The variants of [`VirtualMemoryError`] and [`CsrError`] are inlined
/// here rather than nested. Nesting cost an extra discriminant per level, which made this type 24
/// bytes; flattened it is 16, which is what lets `Result<_, ExecutionError>` be returned in
/// registers instead of through a hidden out-pointer. `From` implementations for all three are
/// provided, so `?` on them keeps working unchanged.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError<Address>
where
    Address: Copy,
{
    /// Unaligned instruction
    #[error("Unaligned instruction at address {address}")]
    UnalignedInstruction {
        /// Address of the unaligned instruction fetch
        address: PackedAddress<Address>,
    },
    /// Out-of-bounds read
    #[error("Out-of-bounds read at address {address}")]
    OutOfBoundsRead {
        /// Address of the out-of-bounds read
        address: PackedAddress<u64>,
    },
    /// Out-of-bounds write
    #[error("Out-of-bounds write at address {address}")]
    OutOfBoundsWrite {
        /// Address of the out-of-bounds write
        address: PackedAddress<u64>,
    },
    /// Unsupported `ecall` instruction
    #[error("Unsupported `ecall` instruction at address {address:#x}")]
    EcallUnsupported {
        /// Address of the unsupported instruction
        address: PackedAddress<Address>,
    },
    /// Unimplemented/illegal instruction
    #[error("Unimplemented/illegal instruction at address {address:#x}")]
    IllegalInstruction {
        /// Address of the `unimp` instruction
        address: PackedAddress<Address>,
    },
    /// Read only CSR
    #[error("Read only CSR {csr_index:#x}")]
    CsrReadOnly {
        /// Index of CSR where write was attempted
        csr_index: u16,
    },
    /// Illegal read access
    #[error("Illegal read access to CSR {csr_index:#x}")]
    CsrIllegalRead {
        /// Index of the accessed CSR
        csr_index: u16,
    },
    /// Illegal write access
    #[error("Illegal write access to CSR {csr_index:#x}")]
    CsrIllegalWrite {
        /// Index of the accessed CSR
        csr_index: u16,
    },
    /// Unknown CSR
    #[error("Unknown CSR {csr_index:#x}")]
    CsrUnknown {
        /// Index of the accessed CSR
        csr_index: u16,
    },
    /// Insufficient privilege level
    #[error(
        "Insufficient privilege level for CSR {csr_index:#x}: required {required:?}, \
        current {current:?}"
    )]
    CsrInsufficientPrivilege {
        /// Index of the accessed CSR
        csr_index: u16,
        /// Required privilege level
        required: PrivilegeLevel,
        /// Current privilege level
        current: PrivilegeLevel,
    },
    /// Custom error
    #[error("Custom error: {0:?}")]
    Custom([u8; 8]),
    /// Threaded execution is not supported on this platform.
    ///
    /// See [`OpaqueThreadedExecutionResult::platform_supported()`] for what makes a platform
    /// unsupported and why the answer is a run-time one.
    #[error("Threaded execution is not supported on this platform")]
    UnsupportedPlatform,
}

/// Where execution continues after an instruction, or why it could not.
///
/// Instructions describe control flow rather than performing it: they say where execution goes next
/// instead of moving the program counter themselves. This keeps instruction bodies independent of
/// how the program counter is represented, which is what allows the same bodies to drive both an
/// interpreter loop that owns a program counter and one that carries it in a register.
#[derive(Debug)]
pub enum ExecutionResult<Reg>
where
    Reg: Register,
{
    /// Write the register and continue with the instruction that follows this one
    Continue {
        /// Register to write
        rd: Reg,
        /// Value to write into it
        value: Reg::Type,
    },
    /// Continue with the instruction that follows this one without writing to `rd` register
    ContinueNoWrite,
    /// Continue `offset` bytes away from the address of *this* instruction.
    ///
    /// Keeping it relative rather than resolving it against the program counter here means a
    /// pre-decoded interpreter can reach the target by moving within the decoded stream instead of
    /// converting an address back into a position.
    Branch {
        /// Signed byte offset from this instruction
        offset: i32,
    },
    /// Continue at an absolute guest address, as `jalr`-style jumps produce
    Jump {
        /// Address to continue at
        target: Reg::Type,
    },
    /// Stop execution
    Break,
    /// Execution failed
    Err(ExecutionError<Reg::Type>),
}

const {
    // Ensure result type retains its size
    assert!(size_of::<ExecutionResult<Reg<u64>>>() <= 16);
    assert!(size_of::<ExecutionResult<Reg<u32>>>() <= 16);
}

const impl<Reg> From<ExecutionError<Reg::Type>> for ExecutionResult<Reg>
where
    Reg: Register,
{
    #[inline(always)]
    fn from(error: ExecutionError<Reg::Type>) -> Self {
        cold_path();
        Self::Err(error)
    }
}

const impl<Reg> FromResidual<Result<Infallible, ExecutionError<Reg::Type>>> for ExecutionResult<Reg>
where
    Reg: Register,
{
    #[inline(always)]
    fn from_residual(residual: Result<Infallible, ExecutionError<Reg::Type>>) -> Self {
        match residual {
            Ok(never) => match never {},
            Err(error) => {
                cold_path();
                Self::Err(error)
            }
        }
    }
}

const impl<Reg> FromResidual<Result<Infallible, VirtualMemoryError>> for ExecutionResult<Reg>
where
    Reg: Register,
{
    #[inline(always)]
    fn from_residual(residual: Result<Infallible, VirtualMemoryError>) -> Self {
        match residual {
            Ok(never) => match never {},
            Err(error) => {
                cold_path();
                Self::Err(ExecutionError::from(error))
            }
        }
    }
}

const impl<Reg> FromResidual<Result<Infallible, CsrError>> for ExecutionResult<Reg>
where
    Reg: Register,
{
    #[inline(always)]
    fn from_residual(residual: Result<Infallible, CsrError>) -> Self {
        match residual {
            Ok(never) => match never {},
            Err(error) => {
                cold_path();
                Self::Err(ExecutionError::from(error))
            }
        }
    }
}

const impl<Address> From<VirtualMemoryError> for ExecutionError<Address>
where
    Address: Copy,
{
    #[inline(always)]
    fn from(value: VirtualMemoryError) -> Self {
        match value {
            VirtualMemoryError::OutOfBoundsRead { address } => Self::OutOfBoundsRead {
                address: PackedAddress::new(address),
            },
            VirtualMemoryError::OutOfBoundsWrite { address } => Self::OutOfBoundsWrite {
                address: PackedAddress::new(address),
            },
        }
    }
}

const impl<Address> From<CsrError> for ExecutionError<Address>
where
    Address: Copy,
{
    #[inline(always)]
    fn from(value: CsrError) -> Self {
        match value {
            CsrError::ReadOnly { csr_index } => Self::CsrReadOnly { csr_index },
            CsrError::IllegalRead { csr_index } => Self::CsrIllegalRead { csr_index },
            CsrError::IllegalWrite { csr_index } => Self::CsrIllegalWrite { csr_index },
            CsrError::Unknown { csr_index } => Self::CsrUnknown { csr_index },
            CsrError::InsufficientPrivilege {
                csr_index,
                required,
                current,
            } => Self::CsrInsufficientPrivilege {
                csr_index,
                required,
                current,
            },
            CsrError::Custom(error) => Self::Custom(error),
        }
    }
}

/// Result of [`InstructionFetcher::fetch_instruction()`] call
#[derive(Debug)]
pub enum FetchInstructionResult<I>
where
    I: Instruction,
{
    /// Instruction to execute
    Instruction(I),
    /// Nothing to execute here, carry on from wherever the program counter now points
    Continue,
    /// Stop execution
    Break,
    /// Fetching failed
    Err(ExecutionError<Address<I>>),
}

/// Generic instruction fetcher
pub const trait InstructionFetcher<I, Memory>
where
    Self: ProgramCounter<Address<I>, Memory>,
    I: Instruction,
{
    /// Fetch a single instruction at a specified address and advance the program counter on
    /// successful fetch
    fn fetch_instruction(&mut self, memory: &Memory) -> FetchInstructionResult<I>;
}

/// CSR error
#[derive(Debug, thiserror::Error)]
pub enum CsrError {
    /// Read only CSR
    #[error("Read only CSR {csr_index:#x}")]
    ReadOnly {
        /// Index of CSR where write was attempted
        csr_index: u16,
    },
    /// Illegal read access
    #[error("Illegal read access to CSR {csr_index:#x}")]
    IllegalRead {
        /// Index of the accessed CSR
        csr_index: u16,
    },
    /// Illegal write access
    #[error("Illegal write access to CSR {csr_index:#x}")]
    IllegalWrite {
        /// Index of the accessed CSR
        csr_index: u16,
    },
    /// Unknown CSR
    #[error("Unknown CSR {csr_index:#x}")]
    Unknown {
        /// Index of the accessed CSR
        csr_index: u16,
    },
    /// Insufficient privilege level
    #[error(
        "Insufficient privilege level for CSR {csr_index:#x}: required {required:?}, \
        current {current:?}"
    )]
    InsufficientPrivilege {
        /// Index of the accessed CSR
        csr_index: u16,
        /// Required privilege level
        required: PrivilegeLevel,
        /// Current privilege level
        current: PrivilegeLevel,
    },
    /// Custom error
    #[error("Custom error: {0:?}")]
    Custom([u8; 8]),
}

/// CSRs (Control and Status Registers)
pub const trait Csrs<Reg>
where
    Reg: [const] Register,
{
    /// Current privilege level
    #[inline(always)]
    fn privilege_level(&self) -> PrivilegeLevel {
        PrivilegeLevel::Machine
    }

    /// Reads register value
    fn read_csr(&self, csr_index: u16) -> Result<Reg::Type, CsrError>;

    /// Writes register value
    fn write_csr(&mut self, csr_index: u16, value: Reg::Type) -> Result<(), CsrError>;
}

// Convenience for threaded execution
const impl<Reg, T> Csrs<Reg> for &mut T
where
    Reg: [const] Register,
    T: [const] Csrs<Reg>,
{
    #[inline(always)]
    fn privilege_level(&self) -> PrivilegeLevel {
        T::privilege_level(self)
    }

    #[inline(always)]
    fn read_csr(&self, csr_index: u16) -> Result<Reg::Type, CsrError> {
        T::read_csr(self, csr_index)
    }

    #[inline(always)]
    fn write_csr(&mut self, csr_index: u16, value: Reg::Type) -> Result<(), CsrError> {
        T::write_csr(self, csr_index, value)
    }
}

/// Custom handler for system instructions `ecall` and `ebreak`
pub const trait SystemInstructionHandler<Reg, Regs, Memory, PC>
where
    Reg: Register,
{
    // TODO: Figure out the correct API for this method
    /// Handle a `fence` instruction
    #[inline(always)]
    fn handle_fence(&mut self, pred: u8, succ: u8) {
        let _: u8 = pred;
        let _: u8 = succ;
        // NOP by default
    }

    // TODO: Figure out the correct API for this method
    /// Handle a `fence.tso` instruction
    #[inline(always)]
    fn handle_fence_tso(&mut self) {
        // NOP by default
    }

    /// Handle an `ecall` instruction
    fn handle_ecall(
        &mut self,
        regs: &mut Regs,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type>>;

    /// Handle an `ebreak` instruction.
    ///
    /// NOTE: the program counter here is the current value, meaning it is already incremented past
    /// the instruction itself.
    #[inline(always)]
    fn handle_ebreak(&mut self, regs: &mut Regs, memory: &mut Memory, pc: Reg::Type) {
        // These are for cleaner trait API without leading `_` on arguments
        let _: &Regs = regs;
        let _: &mut Memory = memory;
        let _: Reg::Type = pc;
        // NOP by default
    }
}

// Convenience for threaded execution
const impl<Reg, Regs, Memory, PC, T> SystemInstructionHandler<Reg, Regs, Memory, PC> for &mut T
where
    Reg: [const] Register,
    T: [const] SystemInstructionHandler<Reg, Regs, Memory, PC>,
{
    #[inline(always)]
    fn handle_fence(&mut self, pred: u8, succ: u8) {
        T::handle_fence(self, pred, succ);
    }

    #[inline(always)]
    fn handle_fence_tso(&mut self) {
        T::handle_fence_tso(self);
    }

    #[inline(always)]
    fn handle_ecall(
        &mut self,
        regs: &mut Regs,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type>> {
        T::handle_ecall(self, regs, memory, program_counter)
    }

    #[inline(always)]
    fn handle_ebreak(&mut self, regs: &mut Regs, memory: &mut Memory, pc: Reg::Type) {
        T::handle_ebreak(self, regs, memory, pc);
    }
}

/// `rs1`/`rs2` instruction operands
#[derive(Debug, Default, Copy, Clone)]
pub struct Rs1Rs2Operands<Reg> {
    /// `rs1` operand.
    ///
    /// Zero register if `rs1` was missing in the original instruction definition.
    pub rs1: Reg,
    /// `rs2` operand.
    ///
    /// Zero register if `rs2` was missing in the original instruction definition.
    pub rs2: Reg,
}

/// `rs1`/`rs2` instruction operands
#[derive(Debug, Default, Copy, Clone)]
pub struct Rs1Rs2OperandValues<RegType> {
    /// `rs1` operand value.
    ///
    /// Zero if `rs1` was missing in the original instruction definition.
    pub rs1_value: RegType,
    /// `rs2` operand value.
    ///
    /// Zero if `rs2` was missing in the original instruction definition.
    pub rs2_value: RegType,
}

/// `rs1`/`rs2` instruction operands
pub const trait ExecutableInstructionOperands
where
    Self: Instruction,
{
    /// `rs1`/`rs2` instruction operands.
    ///
    /// Returns zero register for `rs1`/`rs2` that were missing in the original instruction
    /// definition.
    fn get_rs1_rs2_operands(self) -> Rs1Rs2Operands<Self::Reg>;
}

pub const trait ExecutableInstructionCsr<ExtState>
where
    Self: Instruction,
{
    /// Prepare CSR read.
    ///
    /// This method is called on each extension one by one with the `raw_value` (contents of the
    /// corresponding CSR register) and initially zero-initialized `output_value`. In return value
    /// every extension can accept (`Ok(true)`), ignore (`Ok(false)`) or reject (`Err(CsrError)`)
    /// read request. For accepted reads the extension must update `output_value` accordingly, which
    /// will be the value used by the `Zicsr` extension handler.
    ///
    /// Some extensions will just copy `raw_value` to output value, others will copy only some bits
    /// or zero some bits of the `raw_value`, as required by the specification.
    ///
    /// `will_write` indicates whether the CSR instruction performing this read will also perform a
    /// write immediately afterward (as part of the same instruction). It is always `true` for
    /// `csrrw{,i}`, and `true` for `csrrs{,i}`/`csrrc{,i}` unless their `rs1`/`zimm` operand is
    /// zero (in which case they are a pure read with no write). Some CSRs (e.g. `Zkr`'s `seed`)
    /// are only legal to access through a genuine read-write instruction and must reject the read
    /// when `will_write` is `false`.
    ///
    /// If no extension returns `Ok(true)`, the read operation is implicitly rejected as illegal
    /// access.
    #[inline(always)]
    fn prepare_csr_read(
        ext_state: &ExtState,
        csr_index: u16,
        will_write: bool,
        raw_value: RegisterType<Self>,
        output_value: &mut RegisterType<Self>,
    ) -> Result<bool, CsrError> {
        // These are for cleaner trait API without leading `_` on arguments
        let _: &ExtState = ext_state;
        let _: u16 = csr_index;
        let _: bool = will_write;
        let _: RegisterType<Self> = raw_value;
        let _: &mut RegisterType<Self> = output_value;
        // The default implementation is to not allow anything
        Ok(false)
    }

    /// Prepare CSR write.
    ///
    /// This method is called on each extension one by one with `write_value` being prepared by the
    /// `Zicsr` extension handler. In return value every extension can accept (`Ok(true)`), ignore
    /// (`Ok(false)`) or reject (`Err(CsrError)`) write request. For accepted writes the extension
    /// must update `output_value` accordingly, which will be written to the corresponding CSR
    /// register.
    ///
    /// Some extensions will just copy `write_value` to output value, others will copy some bits or
    /// zero some bits of the `write_value`, as required by the specification.
    ///
    /// If no extension returns `Ok(true)`, the write operation is implicitly rejected as illegal
    /// access.
    #[inline(always)]
    fn prepare_csr_write(
        ext_state: &mut ExtState,
        csr_index: u16,
        write_value: RegisterType<Self>,
        output_value: &mut RegisterType<Self>,
    ) -> Result<bool, CsrError> {
        // These are for cleaner trait API without leading `_` on arguments
        let _: &mut ExtState = ext_state;
        let _: u16 = csr_index;
        let _: RegisterType<Self> = write_value;
        let _: &mut RegisterType<Self> = output_value;
        // The default implementation is to not allow anything
        Ok(false)
    }
}

/// Trait for executable instructions
pub const trait ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
where
    Self: ExecutableInstructionOperands + ExecutableInstructionCsr<ExtState>,
{
    /// Execute instruction.
    ///
    /// Instructions might place additional constraints on `ExtState` to require additional
    /// registers or other resources. If no such constraint is used, `()` can be used as a
    /// placeholder.
    ///
    /// On success `ExecutionResult::Continue { rd: rd, value: rd_value }` is returned, which will
    /// be written into the register file. In most cases this is the only register that needs to
    /// be written. If no value needs to be written, `ExecutionResult::ContinueNoWrite` should be
    /// returned, which skips the register file write entirely.
    fn execute(
        self,
        rs1rs2_values: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg>;
}

/// Outcome of [`ThreadedExecutableInstruction::execute_threaded()`].
///
/// Unlike [`ExecutionResult`], this is produced exactly once, by the handler that stops the chain
/// and is not on the per-instruction path.
///
/// For performance reasons everything inside has to fit into what the platform returns in
/// registers, in the shape of [`OpaqueThreadedExecutionResult`]. A caller that needs its fetcher
/// after execution keeps its own copy and sets the program counter to the correct value after
/// execution manually.
#[derive(Debug)]
pub struct ThreadedExecutionResult<I>
where
    I: Instruction,
{
    /// Program counter as of the moment execution stopped
    pub program_counter: Address<I>,
    /// Why execution stopped
    pub outcome: Result<(), ExecutionError<Address<I>>>,
}

impl<I> ThreadedExecutionResult<I>
where
    I: Instruction,
{
    /// Execution stopped gracefully
    #[inline(always)]
    pub const fn stopped(program_counter: Address<I>) -> Self {
        Self {
            program_counter,
            outcome: Ok(()),
        }
    }

    /// Execution failed
    #[inline(always)]
    pub const fn failed(program_counter: Address<I>, error: ExecutionError<Address<I>>) -> Self {
        cold_path();
        Self {
            program_counter,
            outcome: Err(error),
        }
    }
}

cfg_select! {
    all(target_arch = "x86_64", any(not(miri), target_feature = "avx")) => {
        /// x86-64 System V only returns an aggregate larger than two eightbytes in registers when
        /// it is a single vector, hence a 256-bit one (a pair of 128-bit vectors classifies as
        /// memory). Moving one of those needs AVX, which is what makes this the only shape here
        /// with a run-time platform requirement.
        type OpaqueLanes = core::arch::x86_64::__m256i;
    }
    all(target_arch = "aarch64", any(not(miri), target_feature = "neon")) => {
        /// AArch64 returns an aggregate larger than 16 bytes in registers only as a homogeneous
        /// aggregate of up to four members, hence three `u64`.
        // TODO: `[f64; 3]` is used temporarily due to compiler bug:
        //  https://github.com/rust-lang/rust/issues/161382
        type OpaqueLanes = [f64; 3];
    }
    _ => {
        type OpaqueLanes = [u64; 3];
    }
}

/// [`ThreadedExecutionResult`] in the shape tail-called handlers return it in.
///
/// Threaded handler internally returns this rather than [`ThreadedExecutionResult`] so that the
/// outcome travels in registers instead of through a hidden out-pointer where possible for
/// performance reasons (primarily on x86-64 due to a limited number of usable GPRs in the ABI).
///
/// On x86-64 the lanes are a 256-bit vector, so producing one of these executes AVX instructions -
/// see [`Self::platform_supported()`] and [`Self::new()`].
#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct OpaqueThreadedExecutionResult<I> {
    lanes: OpaqueLanes,
    phantom: PhantomData<I>,
}

impl<I> OpaqueThreadedExecutionResult<I>
where
    I: Instruction,
{
    const TAG_STOPPED: u64 = 0;
    const TAG_UNALIGNED_INSTRUCTION: u64 = 1;
    const TAG_OUT_OF_BOUNDS_READ: u64 = 2;
    const TAG_OUT_OF_BOUNDS_WRITE: u64 = 3;
    const TAG_ECALL_UNSUPPORTED: u64 = 4;
    const TAG_ILLEGAL_INSTRUCTION: u64 = 5;
    const TAG_CSR_READ_ONLY: u64 = 6;
    const TAG_CSR_ILLEGAL_READ: u64 = 7;
    const TAG_CSR_ILLEGAL_WRITE: u64 = 8;
    const TAG_CSR_UNKNOWN: u64 = 9;
    const TAG_CSR_INSUFFICIENT_PRIVILEGE: u64 = 10;
    const TAG_CUSTOM: u64 = 11;
    const TAG_UNSUPPORTED_PLATFORM: u64 = 12;

    /// Whether this platform can carry an outcome the way [`Self::new()`] does.
    ///
    /// It cannot on an x86-64 CPU without AVX, where the lanes are a 256-bit vector: Rust targets
    /// x86-64-v1 by default, so a runtime check is necessary.
    ///
    /// This is called once within [`ThreadedExecutableInstruction::execute_threaded()`].
    #[cfg_attr(
        all(target_arch = "x86_64", not(target_feature = "avx")),
        expect(
            deprecated,
            reason = "`cpufeatures` uses a deprecated associated function internally"
        )
    )]
    #[inline(always)]
    #[must_use]
    pub fn platform_supported() -> bool {
        cfg_select! {
            all(target_arch = "x86_64", not(target_feature = "avx")) => {
                cpufeatures::new!(cpuid_avx, "avx");

                cpuid_avx::get()
            }
            _ => true,
        }
    }

    /// Serialize an outcome into the shape handlers return.
    ///
    /// # Safety
    /// [`Self::platform_supported()`] must return `true`.
    #[inline(always)]
    pub unsafe fn new(result: ThreadedExecutionResult<I>) -> Self {
        let program_counter = result.program_counter.as_u64();

        let (tag, payload) = match result.outcome {
            Ok(()) => (Self::TAG_STOPPED, 0),
            Err(error) => match error {
                ExecutionError::UnalignedInstruction { address } => {
                    (Self::TAG_UNALIGNED_INSTRUCTION, address.get().as_u64())
                }
                ExecutionError::OutOfBoundsRead { address } => {
                    (Self::TAG_OUT_OF_BOUNDS_READ, address.get())
                }
                ExecutionError::OutOfBoundsWrite { address } => {
                    (Self::TAG_OUT_OF_BOUNDS_WRITE, address.get())
                }
                ExecutionError::EcallUnsupported { address } => {
                    (Self::TAG_ECALL_UNSUPPORTED, address.get().as_u64())
                }
                ExecutionError::IllegalInstruction { address } => {
                    (Self::TAG_ILLEGAL_INSTRUCTION, address.get().as_u64())
                }
                ExecutionError::CsrReadOnly { csr_index } => {
                    (Self::TAG_CSR_READ_ONLY, u64::from(csr_index))
                }
                ExecutionError::CsrIllegalRead { csr_index } => {
                    (Self::TAG_CSR_ILLEGAL_READ, u64::from(csr_index))
                }
                ExecutionError::CsrIllegalWrite { csr_index } => {
                    (Self::TAG_CSR_ILLEGAL_WRITE, u64::from(csr_index))
                }
                ExecutionError::CsrUnknown { csr_index } => {
                    (Self::TAG_CSR_UNKNOWN, u64::from(csr_index))
                }
                ExecutionError::CsrInsufficientPrivilege {
                    csr_index,
                    required,
                    current,
                } => (
                    Self::TAG_CSR_INSUFFICIENT_PRIVILEGE,
                    u64::from(csr_index)
                        | (u64::from(required.to_bits()) << 16)
                        | (u64::from(current.to_bits()) << 24),
                ),
                ExecutionError::Custom(error) => (Self::TAG_CUSTOM, u64::from_le_bytes(error)),
                ExecutionError::UnsupportedPlatform => (Self::TAG_UNSUPPORTED_PLATFORM, 0),
            },
        };

        Self {
            lanes: cfg_select! {
                all(target_arch = "x86_64", any(not(miri), target_feature = "avx")) => {
                    // SAFETY: Method contract guarantees that `Self::platform_supported()` was
                    // called, which ensures that AVX is supported
                    unsafe {
                        core::arch::x86_64::_mm256_setr_epi64x(
                            program_counter.cast_signed(),
                            tag.cast_signed(),
                            payload.cast_signed(),
                            0,
                        )
                    }
                }
                all(target_arch = "aarch64", any(not(miri), target_feature = "neon")) => {
                    [program_counter, tag, payload].map(f64::from_bits)
                }
                _ => [program_counter, tag, payload],
            },
            phantom: PhantomData,
        }
    }

    /// Deserialize what [`Self::new()`] produced.
    ///
    /// The lanes only ever come from there, in this very crate, which is what makes the
    /// unknown-tag arm unreachable.
    #[inline(always)]
    pub fn into_result(self) -> ThreadedExecutionResult<I> {
        let [program_counter, tag, payload] =
            cfg_select! {
                all(target_arch = "x86_64", any(not(miri), target_feature = "avx")) => {
                    {
                        // SAFETY: Same size, alignment is larger than necessary
                        let [program_counter, tag, payload, _] = unsafe {
                            mem::transmute::<core::arch::x86_64::__m256i, [u64; 4]>(self.lanes)
                        };

                        [program_counter, tag, payload]
                    }
                }
                all(target_arch = "aarch64", any(not(miri), target_feature = "neon")) => {
                    self.lanes.map(f64::to_bits)
                }
                _ => self.lanes,
            };

        let program_counter = Address::<I>::truncate_from_u64(program_counter);
        let csr_index = payload as u16;

        let error = match tag {
            Self::TAG_STOPPED => {
                return ThreadedExecutionResult::stopped(program_counter);
            }
            Self::TAG_UNALIGNED_INSTRUCTION => ExecutionError::UnalignedInstruction {
                address: PackedAddress::new(Address::<I>::truncate_from_u64(payload)),
            },
            Self::TAG_OUT_OF_BOUNDS_READ => ExecutionError::OutOfBoundsRead {
                address: PackedAddress::new(payload),
            },
            Self::TAG_OUT_OF_BOUNDS_WRITE => ExecutionError::OutOfBoundsWrite {
                address: PackedAddress::new(payload),
            },
            Self::TAG_ECALL_UNSUPPORTED => ExecutionError::EcallUnsupported {
                address: PackedAddress::new(Address::<I>::truncate_from_u64(payload)),
            },
            Self::TAG_ILLEGAL_INSTRUCTION => ExecutionError::IllegalInstruction {
                address: PackedAddress::new(Address::<I>::truncate_from_u64(payload)),
            },
            Self::TAG_CSR_READ_ONLY => ExecutionError::CsrReadOnly { csr_index },
            Self::TAG_CSR_ILLEGAL_READ => ExecutionError::CsrIllegalRead { csr_index },
            Self::TAG_CSR_ILLEGAL_WRITE => ExecutionError::CsrIllegalWrite { csr_index },
            Self::TAG_CSR_UNKNOWN => ExecutionError::CsrUnknown { csr_index },
            Self::TAG_CSR_INSUFFICIENT_PRIVILEGE => {
                // SAFETY: `::new()` constructor created this value with `to_bits()`
                let required =
                    unsafe { PrivilegeLevel::from_bits((payload >> 16) as u8).unwrap_unchecked() };
                // SAFETY: `::new()` constructor created this value with `to_bits()`
                let current =
                    unsafe { PrivilegeLevel::from_bits((payload >> 24) as u8).unwrap_unchecked() };

                ExecutionError::CsrInsufficientPrivilege {
                    csr_index,
                    required,
                    current,
                }
            }
            Self::TAG_CUSTOM => ExecutionError::Custom(payload.to_le_bytes()),
            Self::TAG_UNSUPPORTED_PLATFORM => ExecutionError::UnsupportedPlatform,
            _ => {
                unreachable!("Lanes are only ever produced by `new()`; qed");
            }
        };

        ThreadedExecutionResult::failed(program_counter, error)
    }
}

/// Tail-call-threaded counterpart of [`ExecutableInstruction`].
///
/// [`ExecutableInstruction::execute()`] describes what a single instruction does and leaves both
/// fetching and control flow to a driver loop. This trait instead runs the whole program: it is
/// generated as one handler function per instruction variant, each of which executes its own
/// instruction and then tail-calls (`become`) the handler of the next one, so there is no loop and
/// no return until execution stops. Each handler touches only the operands its own instruction
/// names, which is what the shared loop cannot do.
///
/// Instruction implementations are unaffected: the handlers are assembled from the very same
/// `match` arms that [`ExecutableInstruction::execute()`] are assembled from, so both paths execute
/// identical logic and a caller picks between them purely on the trade between code size
/// (`execute`) and throughput (`execute_threaded`).
///
/// This trait is deliberately not `const`, unlike [`ExecutableInstruction`]: dispatch goes through
/// a table of function pointers, and calls through a function pointer are not allowed in
/// `const fn`.
pub trait ThreadedExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
where
    Self: ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>,
    PC: InstructionFetcher<Self, Memory>,
{
    /// Execute instructions starting at the instruction fetcher's current position and continue
    /// until execution stops or fails.
    ///
    /// The instruction fetcher is taken by value rather than behind a reference so that it stays in
    /// registers across the whole handler chain.
    ///
    /// `ext_state` and `system_instruction_handler` are taken by value for the same reason and one
    /// more: an owned value can be a zero-sized type, which occupies no argument register at all,
    /// whereas a `&mut` occupies one whether there is anything behind it or not. Most
    /// configurations have no extension state and a stateless system instruction handler, so both
    /// vanish, allowing more registers for other arguments. A configuration that does have a state
    /// passes a `&mut` to it (which is an owned value too and for which there are blanket
    /// implementations for convenience).
    fn execute_threaded(
        instruction_fetcher: PC,
        regs: &mut Regs,
        ext_state: ExtState,
        memory: &mut Memory,
        system_instruction_handler: InstructionHandler,
    ) -> ThreadedExecutionResult<Self>;
}
