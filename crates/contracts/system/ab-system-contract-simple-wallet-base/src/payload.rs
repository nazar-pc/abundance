//! This module contains generic utilities for serializing and deserializing method calls to/from
//! payload bytes.
//!
//! It can be reused to implement a different wallet implementation. [`TransactionPayloadCaller`]
//! decodes methods only to call them, so displaying the contents of the transaction without
//! executing it (for example, on the screen of the hardware wallet) needs a separate parser, which
//! can use the types that describe individual bytes of the payload, like [`TransactionInput`].
//!
//! Builder interface requires heap allocations and can be enabled with `payload-builder` feature,
//! while the rest works in `no_std` environment without a global allocator.

#[cfg(feature = "payload-builder")]
pub mod builder;
mod caller;
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
    pub const fn new_output_index(output_index: u8) -> Option<Self> {
        if output_index > 0b0111_1111 {
            return None;
        }

        Some(Self(TransactionSlotType::OutputIndex { output_index }))
    }

    /// Create an instance from `u8`
    #[inline(always)]
    pub const fn from_u8(n: u8) -> Self {
        // The first bit is set to 1 for explicit slot address and 0 for output index
        if n & 0b1000_0000 == 0 {
            Self(TransactionSlotType::OutputIndex { output_index: n })
        } else {
            Self(TransactionSlotType::Address)
        }
    }

    /// Convert instance into `u8`
    #[inline(always)]
    pub const fn into_u8(self) -> u8 {
        // The first bit is set to 1 for explicit slot address and 0 for output index
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
    pub const fn new_output_index(output_index: u8) -> Option<Self> {
        if output_index > 0b0111_1111 {
            return None;
        }

        Some(Self(TransactionInputType::OutputIndex { output_index }))
    }

    /// Create an instance from `u8`.
    ///
    /// Returns `None` for a value with an alignment larger than [`Alignment::MAX`], which
    /// [`Alignment`] can't hold.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_u8(n: u8) -> Option<Self> {
        // The first bit is set to 1 for value and 0 for output index, the remaining bits of a
        // value hold the power of two of its alignment
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
        // The first bit is set to 1 for value and 0 for output index
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
    Output { alignment: Alignment },
    ReturnValue { alignment: Alignment },
}

/// The type of transaction output could be either `#[output]` or return value.
///
/// Both are allocated with the capacity and alignment specified in the payload and pushed into a
/// virtual "stack" that later methods can refer to, see [`TransactionInput`]. `#[output]` is given
/// to the method empty and has the size the method wrote into it. The return value doesn't have a
/// size in `ExternalArgs`, executors only write the value, so its size is set to its capacity
/// right away, which is the size of its type.
#[derive(Debug, Copy, Clone)]
pub struct TransactionOutput(TransactionOutputType);

impl TransactionOutput {
    /// `#[output]` with `alignment`
    #[inline(always)]
    pub const fn new_output(alignment: Alignment) -> Self {
        Self(TransactionOutputType::Output { alignment })
    }

    /// Return value with `alignment`
    #[inline(always)]
    pub const fn new_return_value(alignment: Alignment) -> Self {
        Self(TransactionOutputType::ReturnValue { alignment })
    }

    /// Create an instance from `u8`.
    ///
    /// Returns `None` for an alignment larger than [`Alignment::MAX`], which [`Alignment`] can't
    /// hold.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_u8(n: u8) -> Option<Self> {
        let Some(alignment) = Alignment::from_power(n & 0b0111_1111) else {
            return None;
        };

        // The first bit is set to 1 for output and 0 for return value
        if n & 0b1000_0000 == 0 {
            Some(Self(TransactionOutputType::ReturnValue { alignment }))
        } else {
            Some(Self(TransactionOutputType::Output { alignment }))
        }
    }

    /// Convert instance into `u8`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn into_u8(self) -> u8 {
        // The first bit is set to 1 for output and 0 for return value
        match self.0 {
            TransactionOutputType::Output { alignment } => 0b1000_0000 | alignment.power,
            TransactionOutputType::ReturnValue { alignment } => alignment.power,
        }
    }

    /// Returns whether this is `#[output]` or return value, with its alignment
    #[inline(always)]
    pub const fn output_type(self) -> TransactionOutputType {
        self.0
    }
}

/// Whether an output is `#[output]` or the return value
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum TransactionOutputKind {
    /// `#[output]`, which is given to the method empty
    Output,
    /// Return value, which executors write whole
    ReturnValue,
}

/// Errors of decoding a payload, see [`TransactionPayloadCaller::call_next_method()`]
#[derive(Debug, thiserror::Error)]
pub enum TransactionPayloadDecoderError {
    /// Payload too small
    #[error("Payload too small")]
    PayloadTooSmall,
    /// Too many arguments
    #[error("Too many arguments")]
    TooManyArguments(u8),
    /// `ExternalArgs` buffer too small
    #[error("`ExternalArgs` buffer too small")]
    ExternalArgsBufferTooSmall,
    /// Output index not found
    #[error("Output index not found: {0}")]
    OutputIndexNotFound(u8),
    /// Invalid output index size for slot
    #[error("Invalid output index {output_index} size for slot: {size}")]
    InvalidSlotOutputIndexSize { output_index: u8, size: u32 },
    /// Invalid output index alignment for slot
    #[error("Invalid output index {output_index} alignment for slot: {alignment}")]
    InvalidSlotOutputIndexAlign { output_index: u8, alignment: u32 },
    /// Alignment power is too large
    #[error("Alignment power is too large: {0}")]
    AlignmentPowerTooLarge(u8),
    /// Invalid method context
    #[error("Invalid method context: {0}")]
    InvalidMethodContext(u8),
    /// Output buffer too small
    #[error("Output buffer too small")]
    OutputBufferTooSmall,
    /// Output buffer offsets too small
    #[error("Output buffer offsets too small")]
    OutputBufferOffsetsTooSmall,
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
/// [`TransactionPayloadCaller::new()`]
#[derive(Debug, Copy, Clone)]
pub struct OutputBufferDetails {
    /// Offset of the output's memory in the output buffer of [`TransactionPayloadCaller`]
    offset: u32,
    /// Size of the value: the capacity of a return value, which executors write whole, while
    /// `#[output]` has size `0` until its method returned `Ok(())`, and the size it wrote after
    /// that
    size: u32,
}
