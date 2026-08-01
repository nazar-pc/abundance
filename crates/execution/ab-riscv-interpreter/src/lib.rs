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
//! * Zkn (version 1.0.1)
//! * Zknd (version 1.0.1)
//! * Zkne (version 1.0.1)
//! * Zknh (version 1.0.1)
//! * Zicond (version 2.0)
//! * Zicsr (version 2.0)
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

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    adt_const_params,
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
    generic_const_args,
    generic_const_items,
    inherent_associated_types,
    integer_widen_truncate,
    macroless_generic_const_args,
    min_generic_const_args,
    signed_bigint_helpers,
    widening_mul
)]
#![cfg_attr(test, feature(const_block_items))]
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
// TODO: Workaround for https://github.com/rust-lang/rust-clippy/issues/17430
#![cfg_attr(
    feature = "no-panic",
    expect(clippy::no_effect_underscore_binding, reason = "False-positive")
)]
#![no_std]

pub mod basic;
pub mod prelude;
mod private;
pub mod rv32;
pub mod rv64;
pub mod v;
pub mod zawrs;
pub mod zicond;
pub mod zicsr;
pub mod zvbb;
pub mod zvbc;

#[cfg(feature = "alloc")]
extern crate alloc;

use crate::private::BasicIntSealed;
use ab_riscv_primitives::prelude::*;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::fmt;
use core::hint::cold_path;
use core::marker::Destruct;
use core::ops::{ControlFlow, Sub};

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
pub trait BasicInt: Sized + Copy + BasicIntSealed + 'static {}

impl BasicIntSealed for u8 {}
impl BasicIntSealed for u16 {}
impl BasicIntSealed for u32 {}
impl BasicIntSealed for u64 {}
impl BasicIntSealed for i8 {}
impl BasicIntSealed for i16 {}
impl BasicIntSealed for i32 {}
impl BasicIntSealed for i64 {}

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

/// Placeholder for custom errors in [`ExecutionError`]
#[derive(Debug, Copy, Clone)]
pub struct CustomErrorPlaceholder;

impl fmt::Display for CustomErrorPlaceholder {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

/// Program counter errors
#[derive(Debug, thiserror::Error)]
pub enum ProgramCounterError<Address, CustomError = CustomErrorPlaceholder> {
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
    Custom(CustomError),
}

/// Generic program counter
pub const trait ProgramCounter<Address, Memory, CustomError = CustomErrorPlaceholder> {
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

    /// Set the current value of the program counter
    fn set_pc(
        &mut self,
        memory: &Memory,
        pc: Address,
    ) -> Result<ControlFlow<()>, ProgramCounterError<Address, CustomError>>;
}

/// Execution errors
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError<Address, CustomError = CustomErrorPlaceholder> {
    /// Program counter error
    #[error("Program counter error: {0}")]
    ProgramCounter(ProgramCounterError<Address, CustomError>),
    /// Memory access error
    #[error("Memory access error: {0}")]
    MemoryAccess(VirtualMemoryError),
    /// Unsupported `ecall` instruction
    #[error("Unsupported `ecall` instruction at address {address:#x}")]
    EcallUnsupported {
        /// Address of the unsupported instruction
        address: Address,
    },
    /// Unimplemented/illegal instruction
    #[error("Unimplemented/illegal instruction at address {address:#x}")]
    IllegalInstruction {
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
    /// CSR error
    #[error("CSR error: {0}")]
    CsrError(CsrError<CustomError>),
    /// Custom error
    #[error("Custom error: {0}")]
    Custom(CustomError),
}

const impl<Address, CustomError> From<ProgramCounterError<Address, CustomError>>
    for ExecutionError<Address, CustomError>
{
    #[inline(always)]
    fn from(value: ProgramCounterError<Address, CustomError>) -> Self {
        Self::ProgramCounter(value)
    }
}

const impl<Address, CustomError> From<VirtualMemoryError> for ExecutionError<Address, CustomError> {
    #[inline(always)]
    fn from(value: VirtualMemoryError) -> Self {
        Self::MemoryAccess(value)
    }
}

const impl<Address, CustomError> From<CsrError<CustomError>>
    for ExecutionError<Address, CustomError>
{
    #[inline(always)]
    fn from(value: CsrError<CustomError>) -> Self {
        Self::CsrError(value)
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
pub const trait InstructionFetcher<I, Memory, CustomError = CustomErrorPlaceholder>
where
    Self: ProgramCounter<Address<I>, Memory, CustomError>,
    I: Instruction,
{
    /// Fetch a single instruction at a specified address and advance the program counter on
    /// successful fetch
    fn fetch_instruction(
        &mut self,
        memory: &Memory,
    ) -> Result<FetchInstructionResult<I>, ExecutionError<Address<I>, CustomError>>;
}

/// CSR error
#[derive(Debug, thiserror::Error)]
pub enum CsrError<CustomError = CustomErrorPlaceholder> {
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
    #[error("Custom error: {0}")]
    Custom(CustomError),
}

/// CSRs (Control and Status Registers)
pub const trait Csrs<Reg, CustomError = CustomErrorPlaceholder>
where
    Reg: [const] Register,
    CustomError: [const] Destruct,
{
    /// Current privilege level
    #[inline(always)]
    fn privilege_level(&self) -> PrivilegeLevel {
        PrivilegeLevel::Machine
    }

    /// Reads register value
    fn read_csr(&self, csr_index: u16) -> Result<Reg::Type, CsrError<CustomError>>;

    /// Writes register value
    fn write_csr(&mut self, csr_index: u16, value: Reg::Type) -> Result<(), CsrError<CustomError>>;

    // TODO: Remove this method once tests do not need to customize it
    /// Process CSR read.
    ///
    /// Must proxy calls to [`ExecutableInstructionCsr::prepare_csr_read()`] of the root instruction
    /// and return the output value on success. The default implementation of the method should be
    /// fine most of the time but can be overridden in special cases or for testing purposes.
    #[doc(hidden)]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn process_csr_read<I>(
        &self,
        csr_index: u16,
        raw_value: Reg::Type,
    ) -> Result<Reg::Type, CsrError<CustomError>>
    where
        I: [const] ExecutableInstructionCsr<Self, CustomError, Reg = Reg>,
    {
        let mut out = Reg::Type::default();
        match I::prepare_csr_read(self, csr_index, raw_value, &mut out) {
            Ok(true) => Ok(out),
            Ok(false) => {
                cold_path();
                Err(CsrError::IllegalRead { csr_index })
            }
            Err(err) => {
                cold_path();
                Err(err)
            }
        }
    }

    // TODO: Remove this method once tests do not need to customize it
    /// Process CSR write.
    ///
    /// Must proxy calls to [`ExecutableInstructionCsr::prepare_csr_write()`] of the root
    /// instruction and return the output value on success. The default implementation of the method
    /// should be fine most of the time but can be overridden in special cases or for testing
    /// purposes.
    #[doc(hidden)]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn process_csr_write<I>(
        &mut self,
        csr_index: u16,
        write_value: Reg::Type,
    ) -> Result<Reg::Type, CsrError<CustomError>>
    where
        I: [const] ExecutableInstructionCsr<Self, CustomError, Reg = Reg>,
    {
        let mut out = Reg::Type::default();
        match I::prepare_csr_write(self, csr_index, write_value, &mut out) {
            Ok(true) => Ok(out),
            Ok(false) => {
                cold_path();
                Err(CsrError::IllegalWrite { csr_index })
            }
            Err(err) => {
                cold_path();
                Err(err)
            }
        }
    }
}

/// Custom handler for system instructions `ecall` and `ebreak`
pub const trait SystemInstructionHandler<
    Reg,
    Regs,
    Memory,
    PC,
    CustomError = CustomErrorPlaceholder,
> where
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
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>>;

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

pub const trait ExecutableInstructionCsr<ExtState, CustomError = CustomErrorPlaceholder>
where
    Self: Instruction,
    ExtState: ?Sized,
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
    /// If no extension returns `Ok(true)`, the read operation is implicitly rejected as illegal
    /// access.
    #[inline(always)]
    fn prepare_csr_read(
        ext_state: &ExtState,
        csr_index: u16,
        raw_value: RegisterType<Self>,
        output_value: &mut RegisterType<Self>,
    ) -> Result<bool, CsrError<CustomError>> {
        // These are for cleaner trait API without leading `_` on arguments
        let _: &ExtState = ext_state;
        let _: u16 = csr_index;
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
    ) -> Result<bool, CsrError<CustomError>> {
        // These are for cleaner trait API without leading `_` on arguments
        let _: &mut ExtState = ext_state;
        let _: u16 = csr_index;
        let _: RegisterType<Self> = write_value;
        let _: &mut RegisterType<Self> = output_value;
        // The default implementation is to not allow anything
        Ok(false)
    }
}

/// Type alias for the result returned by [`ExecutableInstruction::execute`]
pub type ExecutableInstructionResult<T, I, CustomError> = Result<
    ControlFlow<
        T,
        (
            <I as Instruction>::Reg,
            <<I as Instruction>::Reg as Register>::Type,
        ),
    >,
    ExecutionError<Address<I>, CustomError>,
>;

/// Trait for executable instructions
pub const trait ExecutableInstruction<
    Regs,
    ExtState,
    Memory,
    PC,
    InstructionHandler,
    CustomError = CustomErrorPlaceholder,
> where
    Self: ExecutableInstructionOperands + ExecutableInstructionCsr<ExtState, CustomError>,
{
    /// Execute instruction.
    ///
    /// Instructions might place additional constraints on `ExtState` to require additional
    /// registers or other resources. If no such constraint is used, `()` can be used as a
    /// placeholder.
    ///
    /// On success `Ok(ControlFlow::Continue((rd, rd_value)))` is returned, which will be written
    /// into the register file. In most cases this is the only register that needs to be written. If
    /// no value needs to be written, `Ok(ControlFlow::Continue(Default::default()))` should be
    /// returned, which corresponds to `Ok(ControlFlow::Continue(Reg::ZERO, 0))` and is no-op.
    fn execute(
        self,
        rs1rs2_values: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutableInstructionResult<(), Self, CustomError>;
}
