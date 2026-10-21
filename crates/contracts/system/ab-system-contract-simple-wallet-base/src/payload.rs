//! This module contains generic utilities for serializing and deserializing method calls to/from
//! payload bytes.
//!
//! It can be reused to implement a different wallet implementation. [`TransactionPayloadCaller`]
//! calls the methods of a payload one by one. [`TransactionPayloadDecoder`] in [`decoder`] checks a
//! payload and describes its methods without calling them, for example to show a transaction on the
//! screen of a hardware wallet before it is signed. Both use the same decoding code, so they apply
//! the same rules.
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

use ab_contracts_common::ContractError;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
pub use caller::TransactionPayloadCaller;
use core::ptr::NonNull;

/// The largest output index that slot and input tags can hold
const MAX_OUTPUT_INDEX: u8 = 0b0111_1111;

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
/// Specifically, if the previous method has `#[output]` or return value, those values are collected
/// and pushed into a virtual "stack". Then, if [`Self::slot_type()`] returns
/// [`TransactionSlotType::OutputIndex`], then the corresponding input will use the value at
/// `output_index` of this stack instead of what was specified in `external_args`. This allows
/// composing calls to multiple methods into more sophisticated workflows without writing special
/// contracts for this.
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
    /// Valid index values are 0..=127.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new_output_index(output_index: u8) -> Option<Self> {
        if output_index > MAX_OUTPUT_INDEX {
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
        // `0b1000_0000` is an explicit slot address, while `0b0xxx_xxxx` is an output index
        if n & 0b1000_0000 == 0 {
            Some(Self(TransactionSlotType::OutputIndex { output_index: n }))
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
/// Specifically, if the previous method has `#[output]` or return value, those values are collected
/// and pushed into a virtual "stack". Then, if [`Self::input_type()`] returns
/// [`TransactionInputType::OutputIndex`], then the corresponding input will use the value at
/// `output_index` of this stack instead of what was specified in `external_args`. This allows
/// composing calls to multiple methods into more sophisticated workflows without writing special
/// contracts for this.
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
    /// Valid index values are 0..=127.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new_output_index(output_index: u8) -> Option<Self> {
        if output_index > MAX_OUTPUT_INDEX {
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
            return Some(Self(TransactionInputType::OutputIndex { output_index: n }));
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
    Output,
    ReturnValue,
}

/// The type of transaction output could be either `#[output]` or return value.
///
/// Both are allocated with the capacity specified in the payload and pushed into a virtual "stack"
/// that later methods can refer to, see [`TransactionInput`]. The memory of every output is aligned
/// to [`Alignment::MAX`], the largest alignment of any type that methods accept, so it is aligned
/// for every output type, and the payload doesn't declare alignments of outputs. `#[output]` is
/// given to the method empty and has the size the method wrote into it. The return value doesn't
/// have a size in `ExternalArgs`, executors only write the value, so its size is set to its
/// capacity right away, which is the size of its type.
#[derive(Debug, Copy, Clone)]
pub struct TransactionOutput(TransactionOutputType);

impl TransactionOutput {
    /// `#[output]`
    #[inline(always)]
    pub const fn new_output() -> Self {
        Self(TransactionOutputType::Output)
    }

    /// Return value
    #[inline(always)]
    pub const fn new_return_value() -> Self {
        Self(TransactionOutputType::ReturnValue)
    }

    /// Create an instance from `u8`.
    ///
    /// Returns `None` for bytes that [`Self::into_u8()`] never returns, so each output has one
    /// encoding.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_u8(n: u8) -> Option<Self> {
        // `0b1000_0000` is `#[output]` and `0b0000_0000` is the return value
        match n {
            0b1000_0000 => Some(Self(TransactionOutputType::Output)),
            0b0000_0000 => Some(Self(TransactionOutputType::ReturnValue)),
            _ => None,
        }
    }

    /// Convert instance into `u8`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn into_u8(self) -> u8 {
        // The highest bit (`0b1000_0000`) is set to 1 for output and 0 for return value, the
        // remaining bits are always 0
        match self.0 {
            TransactionOutputType::Output => 0b1000_0000,
            TransactionOutputType::ReturnValue => 0b0000_0000,
        }
    }

    /// Returns whether this is `#[output]` or return value
    #[inline(always)]
    pub const fn output_type(self) -> TransactionOutputType {
        self.0
    }

    /// Whether this is `#[output]` or return value
    #[inline(always)]
    pub(crate) const fn kind(self) -> TransactionOutputKind {
        match self.0 {
            TransactionOutputType::Output => TransactionOutputKind::Output,
            TransactionOutputType::ReturnValue => TransactionOutputKind::ReturnValue,
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

/// Errors of decoding a payload, see [`TransactionPayloadDecoder::next_method()`] and
/// [`TransactionPayloadCaller::call_next_method()`]
///
/// [`TransactionPayloadDecoder::next_method()`]: decoder::TransactionPayloadDecoder::next_method
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
    /// Output index not found
    #[error("Output index not found: {0}")]
    OutputIndexNotFound(u8),
    /// The return value with this output index is followed by another output of its method,
    /// while a method has at most one return value, which is its last output
    #[error("Return value with output index {0} is not the last output of its method")]
    ReturnValueNotLast(u8),
    /// An output used as a slot doesn't have the size of an address: the payload declares a
    /// different size for a return value, or a method wrote a different size into its `#[output]`
    #[error("Invalid output index {output_index} size for slot: {size}")]
    InvalidSlotOutputIndexSize { output_index: u8, size: u32 },
    /// Invalid method context
    #[error("Invalid method context: {0}")]
    InvalidMethodContext(u8),
    /// Output buffer too small
    #[error("Output buffer too small")]
    OutputBufferTooSmall,
    /// Output buffer offsets too small
    #[error("Output buffer offsets too small")]
    OutputBufferOffsetsTooSmall,
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

/// Details of an output index that are kept between methods, see
/// [`TransactionPayloadDecoder::new()`] and [`TransactionPayloadCaller::new()`]
///
/// [`TransactionPayloadDecoder::new()`]: decoder::TransactionPayloadDecoder::new
#[derive(Debug, Copy, Clone)]
pub struct OutputBufferDetails {
    /// Offset of the output's memory in the output buffer of [`TransactionPayloadCaller`], `0` in
    /// [`TransactionPayloadDecoder`], which has no output buffer
    ///
    /// [`TransactionPayloadDecoder`]: decoder::TransactionPayloadDecoder
    offset: u32,
    /// Size of the value: the capacity of a return value, which executors write whole, while
    /// `#[output]` has size `0` until its method returned `Ok(())`, and the size it wrote after
    /// that
    size: u32,
    kind: TransactionOutputKind,
}
