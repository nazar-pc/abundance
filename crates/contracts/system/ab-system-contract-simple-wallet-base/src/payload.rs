//! This module contains generic utilities for serializing and deserializing method calls to/from
//! payload bytes.
//!
//! It can be reused to implement a different wallet implementation. [`TransactionPayloadCaller`]
//! calls the methods of a payload one by one. [`TransactionPayloadDecoder`] in [`decoder`] checks a
//! payload and describes its methods without calling them, for example to show a transaction on the
//! screen of a hardware wallet before it is signed. Both use the same decoding code, so they apply
//! the same rules.
//!
//! Methods can pass values to later methods of the same payload through outputs. A payload declares
//! all its outputs upfront, each with a capacity, and every `#[output]` and return value of a
//! method writes one of them. Later methods can refer to what was written last into a declared
//! output as a slot or an input, see [`TransactionOutput`].
//!
//! Builder interface requires heap allocations and can be enabled with `payload-builder` feature,
//! while the rest works in `no_std` environment without a global allocator.
//!
//! [`TransactionPayloadDecoder`]: decoder::TransactionPayloadDecoder

#[cfg(feature = "payload-builder")]
pub mod builder;
mod caller;
pub mod decoder;
mod decoder_core;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;

use crate::MAX_OUTPUTS;
use ab_contracts_common::ContractError;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
pub use caller::TransactionPayloadCaller;
use core::ptr::NonNull;

#[derive(Copy, Clone)]
#[repr(C)]
struct FfiDataSizeCapacityRo {
    data_ptr: NonNull<u8>,
    size: u32,
    capacity: u32,
}

#[derive(Copy, Clone)]
#[repr(C)]
struct FfiDataSizeCapacityRw {
    data_ptr: *mut u8,
    size: u32,
    capacity: u32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(u8)]
pub enum TransactionMethodContext {
    /// Call contract under [`Address::NULL`] context (corresponds to [`MethodContext::Reset`])
    ///
    /// [`Address::NULL`]: ab_core_primitives::address::Address::NULL
    /// [`MethodContext::Reset`]: ab_contracts_common::env::MethodContext::Reset
    Null,
    /// Call contract under context of the wallet (corresponds to [`MethodContext::Keep`])
    ///
    /// [`MethodContext::Keep`]: ab_contracts_common::env::MethodContext::Keep
    Wallet,
}

const impl TryFrom<u8> for TransactionMethodContext {
    type Error = ();

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Null,
            1 => Self::Wallet,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Copy, Eq, PartialEq, Clone)]
pub enum TransactionInputType {
    Value { alignment: Alignment },
    OutputIndex { output_index: u8 },
}

#[derive(Debug, Copy, Eq, PartialEq, Clone)]
pub enum TransactionSlotType {
    Address,
    OutputIndex { output_index: u8 },
}

/// The type of transaction slot could be either explicit slot address or output index.
///
/// A payload declares its outputs upfront, and every `#[output]` and return value of a method
/// writes one of them, see [`TransactionOutput`]. If [`Self::slot_type()`] returns
/// [`TransactionSlotType::OutputIndex`], then the slot is the value that an earlier method wrote
/// last into the declared output with `output_index`, instead of what was specified in
/// `external_args`. This allows composing calls to multiple methods into more sophisticated
/// workflows without writing special contracts for this.
#[derive(Debug, Copy, Clone)]
pub struct TransactionSlot(TransactionSlotType);

impl TransactionSlot {
    /// Explicit slot address
    #[inline(always)]
    pub const fn new_address() -> Self {
        Self(TransactionSlotType::Address)
    }

    /// Output index value.
    ///
    /// Valid index values are below [`MAX_OUTPUTS`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new_output_index(output_index: u8) -> Option<Self> {
        if usize::from(output_index) >= MAX_OUTPUTS {
            return None;
        }

        Some(Self(TransactionSlotType::OutputIndex { output_index }))
    }

    /// Create an instance from `u8`.
    ///
    /// Returns `None` for bytes that [`Self::into_u8()`] never returns, so each slot has one
    /// encoding.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_u8(n: u8) -> Option<Self> {
        // `0b1000_0000` is an explicit slot address, while `0b0xxx_xxxx` is an output index, which
        // must be below `MAX_OUTPUTS`
        if n & 0b1000_0000 == 0 {
            Self::new_output_index(n)
        } else if n == 0b1000_0000 {
            Some(Self(TransactionSlotType::Address))
        } else {
            None
        }
    }

    /// Convert instance into `u8`
    #[inline(always)]
    pub const fn into_u8(self) -> u8 {
        // The highest bit (`0b1000_0000`) is set to 1 for explicit slot address and 0 for output
        // index
        match self.0 {
            TransactionSlotType::Address => 0b1000_0000,
            TransactionSlotType::OutputIndex { output_index } => output_index,
        }
    }

    /// Returns `Some(output_index)` or `None` if explicit slot address
    #[inline(always)]
    pub const fn slot_type(self) -> TransactionSlotType {
        self.0
    }
}

/// The type of transaction input could be either explicit value or output index.
///
/// A payload declares its outputs upfront, and every `#[output]` and return value of a method
/// writes one of them, see [`TransactionOutput`]. If [`Self::input_type()`] returns
/// [`TransactionInputType::OutputIndex`], then the input is the value that an earlier method wrote
/// last into the declared output with `output_index`, instead of what was specified in
/// `external_args`. This allows composing calls to multiple methods into more sophisticated
/// workflows without writing special contracts for this.
#[derive(Debug, Copy, Clone)]
pub struct TransactionInput(TransactionInputType);

impl TransactionInput {
    /// Regular input value with `alignment`
    #[inline(always)]
    pub const fn new_value(alignment: Alignment) -> Self {
        Self(TransactionInputType::Value { alignment })
    }

    /// Output index value.
    ///
    /// Valid index values are below [`MAX_OUTPUTS`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new_output_index(output_index: u8) -> Option<Self> {
        if usize::from(output_index) >= MAX_OUTPUTS {
            return None;
        }

        Some(Self(TransactionInputType::OutputIndex { output_index }))
    }

    /// Create an instance from `u8`.
    ///
    /// Returns `None` for bytes that [`Self::into_u8()`] never returns, so each input has one
    /// encoding.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_u8(n: u8) -> Option<Self> {
        // The highest bit (`0b1000_0000`) is set to 1 for value and 0 for output index, the
        // remaining bits of a value hold the power of two of its alignment
        if n & 0b1000_0000 == 0 {
            return Self::new_output_index(n);
        }

        let Some(alignment) = Alignment::from_power(n & 0b0111_1111) else {
            return None;
        };

        Some(Self(TransactionInputType::Value { alignment }))
    }

    /// Convert instance into `u8`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn into_u8(self) -> u8 {
        // The highest bit (`0b1000_0000`) is set to 1 for value and 0 for output index
        match self.0 {
            TransactionInputType::Value { alignment } => 0b1000_0000 | alignment.power,
            TransactionInputType::OutputIndex { output_index } => output_index,
        }
    }

    /// Returns `Some(output_index)` or `None` if regular input value
    #[inline(always)]
    pub const fn input_type(self) -> TransactionInputType {
        self.0
    }
}

#[derive(Debug, Copy, Eq, PartialEq, Clone)]
pub enum TransactionOutputType {
    Output { output_index: u8 },
    ReturnValue { output_index: u8 },
}

/// The type of transaction output could be either `#[output]` or return value, with the index of
/// the declared output that it writes.
///
/// A payload declares its outputs upfront, each with a capacity, and every `#[output]` and return
/// value of a method writes one of them, which later methods can refer to, see
/// [`TransactionInput`]. Several methods can write the same declared output one after another, and
/// each of them replaces the value. A method writes a declared output at most once, and its slots
/// and inputs don't refer to a declared output that it writes. The memory of every declared output
/// is aligned to [`Alignment::MAX`], the largest alignment of any type that methods accept, so it
/// is aligned for every output type.
///
/// `#[output]` is given to the method empty, whatever the memory held before, and has the size the
/// method wrote into it. The return value doesn't have a size in `ExternalArgs`, executors only
/// write the value, so the payload declares its size, which is the size of its type and must not
/// exceed the capacity of the declared output.
#[derive(Debug, Copy, Clone)]
pub struct TransactionOutput(TransactionOutputType);

impl TransactionOutput {
    /// `#[output]` that writes the declared output with `output_index`.
    ///
    /// Valid index values are below [`MAX_OUTPUTS`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new_output(output_index: u8) -> Option<Self> {
        if usize::from(output_index) >= MAX_OUTPUTS {
            return None;
        }

        Some(Self(TransactionOutputType::Output { output_index }))
    }

    /// Return value that is written into the declared output with `output_index`.
    ///
    /// Valid index values are below [`MAX_OUTPUTS`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new_return_value(output_index: u8) -> Option<Self> {
        if usize::from(output_index) >= MAX_OUTPUTS {
            return None;
        }

        Some(Self(TransactionOutputType::ReturnValue { output_index }))
    }

    /// Create an instance from `u8`.
    ///
    /// Returns `None` for bytes that [`Self::into_u8()`] never returns, so each output has one
    /// encoding.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_u8(n: u8) -> Option<Self> {
        let output_index = Self::tag_output_index(n);
        // The highest bit (`0b1000_0000`) is set to 1 for output and 0 for return value
        if n & 0b1000_0000 == 0 {
            Self::new_return_value(output_index)
        } else {
            Self::new_output(output_index)
        }
    }

    /// Index of the declared output that the output tag `n` holds, which is [`MAX_OUTPUTS`] or
    /// more for a tag that [`Self::from_u8()`] rejects
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub(crate) const fn tag_output_index(n: u8) -> u8 {
        // The highest bit (`0b1000_0000`) holds the kind, the remaining bits hold the index
        n & 0b0111_1111
    }

    /// Convert instance into `u8`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn into_u8(self) -> u8 {
        // The highest bit (`0b1000_0000`) is set to 1 for output and 0 for return value
        match self.0 {
            TransactionOutputType::Output { output_index } => 0b1000_0000 | output_index,
            TransactionOutputType::ReturnValue { output_index } => output_index,
        }
    }

    /// Returns whether this is `#[output]` or return value, with the index of its declared output
    #[inline(always)]
    pub const fn output_type(self) -> TransactionOutputType {
        self.0
    }

    /// Whether this is `#[output]` or return value
    #[inline(always)]
    pub(crate) const fn kind(self) -> TransactionOutputKind {
        match self.0 {
            TransactionOutputType::Output { output_index: _ } => TransactionOutputKind::Output,
            TransactionOutputType::ReturnValue { output_index: _ } => {
                TransactionOutputKind::ReturnValue
            }
        }
    }

    /// Index of the declared output
    #[inline(always)]
    pub(crate) const fn output_index(self) -> u8 {
        match self.0 {
            TransactionOutputType::Output { output_index }
            | TransactionOutputType::ReturnValue { output_index } => output_index,
        }
    }
}

/// Whether an output is `#[output]` or the return value
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TransactionOutputKind {
    /// `#[output]`, which is given to the method empty
    Output,
    /// Return value, which executors write whole
    ReturnValue,
}

/// Errors of decoding a payload with [`TransactionPayloadDecoder`] or
/// [`TransactionPayloadCaller`], whose constructors already read the declared outputs
///
/// [`TransactionPayloadDecoder`]: decoder::TransactionPayloadDecoder
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum TransactionPayloadDecoderError {
    /// Payload too small
    #[error("Payload too small")]
    PayloadTooSmall,
    /// Too many arguments
    #[error("Too many arguments")]
    TooManyArguments(u8),
    /// A slot tag that [`TransactionSlot::from_u8()`] rejects
    #[error("Invalid slot tag: {0:#010b}")]
    InvalidSlotTag(u8),
    /// An input tag that [`TransactionInput::from_u8()`] rejects
    #[error("Invalid input tag: {0:#010b}")]
    InvalidInputTag(u8),
    /// An output tag that [`TransactionOutput::from_u8()`] rejects
    #[error("Invalid output tag: {0:#010b}")]
    InvalidOutputTag(u8),
    /// A method writes an output that the payload doesn't declare, or a slot or an input refers to
    /// an output that the payload doesn't declare or that no earlier method wrote
    #[error("Output index not found: {0}")]
    OutputIndexNotFound(u8),
    /// A method writes the output with this index more than once, as `#[output]` or return value
    #[error("Output index {0} is written more than once by the same method")]
    OutputUsedTwice(u8),
    /// A slot or an input refers to the output with this index, which the same method writes as
    /// `#[output]` or return value
    #[error("Output index {0} is referred to by the method that writes it")]
    OutputReferencedBySameMethod(u8),
    /// The return value with this output index is followed by another output of its method,
    /// while a method has at most one return value, which is its last output
    #[error("Return value with output index {0} is not the last output of its method")]
    ReturnValueNotLast(u8),
    /// The size that the payload declares for a return value exceeds the capacity of its output.
    ///
    /// The capacity is not included, so that errors fit into 8 bytes, which keeps decoding fast.
    #[error("Return value with output index {output_index} has size {size}, more than capacity")]
    ReturnValueTooLarge { output_index: u8, size: u32 },
    /// An output used as a slot doesn't have the size of an address: the payload declares a
    /// different size for a return value, or a method wrote a different size into its `#[output]`
    #[error("Invalid output index {output_index} size for slot: {size}")]
    InvalidSlotOutputIndexSize { output_index: u8, size: u32 },
    /// Invalid method context
    #[error("Invalid method context: {0}")]
    InvalidMethodContext(u8),
    /// The declared outputs don't fit into the output buffer of [`TransactionPayloadCaller`]
    #[error("Output buffer too small")]
    OutputBufferTooSmall,
    /// The payload declares more than [`MAX_OUTPUTS`] outputs
    #[error("Too many declared outputs: {0}")]
    TooManyDeclaredOutputs(u8),
    /// Decoding failed before, so the decoder can't continue
    #[error("Decoding failed before")]
    FailedEarlier,
}

/// Errors for [`TransactionPayloadCaller::call_next_method()`]
#[derive(Debug, thiserror::Error)]
pub enum TransactionPayloadCallError {
    /// Decoder error
    #[error("Decoder error: {0}")]
    Decoder(TransactionPayloadDecoderError),
    /// Method call error
    #[error("Method call error: {0}")]
    Method(ContractError),
}

/// Output that a payload declares, with what is known about the value that was written into it
/// last, see [`TransactionPayloadDecoder::new()`] and [`TransactionPayloadCaller::new()`]
///
/// [`TransactionPayloadDecoder::new()`]: decoder::TransactionPayloadDecoder::new
#[derive(Debug, Copy, Clone)]
#[expect(
    clippy::partial_pub_fields,
    reason = "Offset, size and state are details of decoding and calling methods"
)]
pub struct DeclaredOutput {
    /// Offset of the output's memory in the output buffer of [`TransactionPayloadCaller`], `0`
    /// without an output buffer
    offset: u32,
    /// Capacity of the output's memory in bytes, as the payload declares it
    pub mut(crate) capacity: u32,
    /// Size of the value that was written last: the size that the payload declares for a return
    /// value, which executors write whole, while `#[output]` has size `0` until its method
    /// returned `Ok(())`, and the size it wrote after that
    size: u32,
    /// What was written into the output last
    state: OutputState,
}

impl DeclaredOutput {
    /// Record that a method writes the output as `kind`, after which it has `size`: `0` for
    /// `#[output]`, which is given to the method empty, and the declared size for a return value
    #[inline(always)]
    fn set_written(&mut self, kind: TransactionOutputKind, size: u32) {
        self.size = size;
        self.state = match kind {
            TransactionOutputKind::Output => OutputState::Output,
            TransactionOutputKind::ReturnValue => OutputState::ReturnValue,
        };
    }
}

/// What was written into a declared output last
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
enum OutputState {
    /// No method wrote it yet. It is `0`, which a reference checks with a single instruction.
    NotWritten = 0,
    /// An `#[output]` of an earlier method wrote it
    Output = 1,
    /// A return value of an earlier method was written into it
    ReturnValue = 2,
}
