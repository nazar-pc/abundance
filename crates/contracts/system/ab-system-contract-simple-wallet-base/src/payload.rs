//! This module contains generic utilities for serializing and deserializing method calls to/from
//! payload bytes.
//!
//! It can be reused to implement a different wallet implementation as well as read and verify the
//! contents of the transaction (for example, to display it on the screen of the hardware wallet).
//!
//! Builder interface requires heap allocations and can be enabled with `payload-builder` feature,
//! while the rest works in `no_std` environment without a global allocator.

#[cfg(feature = "payload-builder")]
pub mod builder;

use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::MAX_ALIGNMENT;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::num::{NonZeroU8, NonZeroUsize};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::slice;

#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(u8)]
pub enum TransactionMethodContext {
    /// Call contract under [`Address::NULL`] context (corresponds to [`MethodContext::Reset`])
    Null,
    /// Call contract under context of the wallet (corresponds to [`MethodContext::Replace`])
    Wallet,
}

impl TransactionMethodContext {
    // TODO: Implement `TryFrom` once it is available in const environment
    /// Try to create an instance from its `u8` representation
    #[inline(always)]
    pub const fn try_from_u8(n: u8) -> Option<Self> {
        Some(match n {
            0 => Self::Null,
            1 => Self::Wallet,
            _ => {
                return None;
            }
        })
    }
}

#[derive(Debug, Copy, Eq, PartialEq, Clone)]
pub enum TransactionInputType {
    Value { alignment_power: u8 },
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
    /// Regular input value with specified alignment.
    ///
    /// Valid alignment values are: 1, 2, 4, 8, 16.
    #[inline(always)]
    pub const fn new_value(alignment: NonZeroU8) -> Option<Self> {
        match alignment.get() {
            1 | 2 | 4 | 8 | 16 => Some(Self(TransactionInputType::Value {
                alignment_power: alignment.ilog2() as u8,
            })),
            _ => None,
        }
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

    /// Create an instance from `u8`
    #[inline(always)]
    pub const fn from_u8(n: u8) -> Self {
        // The first bit is set to 1 for value and 0 for output index
        if n & 0b1000_0000 == 0 {
            Self(TransactionInputType::OutputIndex { output_index: n })
        } else {
            Self(TransactionInputType::Value {
                alignment_power: n & 0b0111_1111,
            })
        }
    }

    /// Convert instance into `u8`
    #[inline(always)]
    pub const fn into_u8(self) -> u8 {
        // The first bit is set to 1 for value and 0 for output index
        match self.0 {
            TransactionInputType::Value { alignment_power } => 0b1000_0000 | alignment_power,
            TransactionInputType::OutputIndex { output_index } => output_index,
        }
    }

    /// Returns `Some(output_index)` or `None` if regular input value
    #[inline(always)]
    pub const fn input_type(self) -> TransactionInputType {
        self.0
    }
}

/// Errors for [`TransactionPayloadDecoder`]
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
    /// Output buffer too small
    #[error("Output buffer too small")]
    OutputBufferTooSmall,
    /// Output buffer offsets too small
    #[error("Output buffer offsets too small")]
    OutputBufferOffsetsTooSmall,
}

/// Decoder for transaction payload created using `TransactionPayloadBuilder`.
#[derive(Debug)]
pub struct TransactionPayloadDecoder<'a> {
    payload: &'a [u8],
    external_args_buffer: &'a mut [*mut c_void],
    // TODO: Cast `output_buffer` into `&'a mut [MaybeUninit<u8>]` and remove `output_buffer_cursor`
    output_buffer: &'a mut [MaybeUninit<u128>],
    output_buffer_cursor: usize,
    output_buffer_offsets: &'a mut [MaybeUninit<(u32, u32)>],
    output_buffer_offsets_cursor: usize,
    map_context: fn(TransactionMethodContext) -> MethodContext,
}

impl<'a> TransactionPayloadDecoder<'a> {
    /// Create a new instance.
    ///
    /// The size of `external_args_buffer` defines max number of bytes allocated for `ExternalArgs`,
    /// which impacts the number of arguments that can be represented by `ExternalArgs`. The size is
    /// specified in pointers with `#[slot]` argument using one pointer, `#[input]` two pointers and
    /// `#[output]` three pointers each.
    ///
    /// The size of `output_buffer` defines how big the total size of `#[output]` and return values
    /// could be in all methods of the payload together.
    ///
    /// The size of `output_buffer_offsets` defines how many `#[output]` arguments and return values
    /// could exist in all methods of the payload together.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn new(
        payload: &'a [u128],
        external_args_buffer: &'a mut [*mut c_void],
        output_buffer: &'a mut [MaybeUninit<u128>],
        output_buffer_offsets: &'a mut [MaybeUninit<(u32, u32)>],
        map_context: fn(TransactionMethodContext) -> MethodContext,
    ) -> Self {
        debug_assert_eq!(align_of_val(payload), usize::from(MAX_ALIGNMENT));
        debug_assert_eq!(align_of_val(output_buffer), usize::from(MAX_ALIGNMENT));

        // SAFETY: Memory is valid and bound by argument's lifetime
        let payload =
            unsafe { slice::from_raw_parts(payload.as_ptr().cast::<u8>(), size_of_val(payload)) };

        Self {
            payload,
            external_args_buffer,
            output_buffer,
            output_buffer_cursor: 0,
            output_buffer_offsets,
            output_buffer_offsets_cursor: 0,
            map_context,
        }
    }
}

impl<'a> TransactionPayloadDecoder<'a> {
    /// Decode the next method (if any) in the payload
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn decode_next_method(
        &mut self,
    ) -> Result<Option<PreparedMethod<'_>>, TransactionPayloadDecoderError> {
        TransactionPayloadDecoderInternal::<true>(self).decode_next_method()
    }

    /// Decode the next method (if any) in the payload without checking size.
    ///
    /// # Safety
    /// Must be used with trusted input created using `TransactionPayloadBuilder` or pre-verified
    /// using [`Self::decode_next_method()`] earlier.
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub unsafe fn decode_next_method_unchecked(&mut self) -> Option<PreparedMethod<'_>> {
        TransactionPayloadDecoderInternal::<false>(self)
            .decode_next_method()
            .expect("No decoding errors are possible with trusted input; qed")
    }
}

/// # Safety
/// When `VERIFY == false` input must be trusted and created using `TransactionPayloadBuilder` or
/// pre-verified using `VERIFY == true` earlier.
struct TransactionPayloadDecoderInternal<'tmp, 'decoder, const VERIFY: bool>(
    &'tmp mut TransactionPayloadDecoder<'decoder>,
);

impl<'tmp, 'decoder, const VERIFY: bool> Deref
    for TransactionPayloadDecoderInternal<'tmp, 'decoder, VERIFY>
{
    type Target = TransactionPayloadDecoder<'decoder>;

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'tmp, 'decoder, const VERIFY: bool> DerefMut
    for TransactionPayloadDecoderInternal<'tmp, 'decoder, VERIFY>
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'tmp, 'decoder, const VERIFY: bool> TransactionPayloadDecoderInternal<'tmp, 'decoder, VERIFY> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn decode_next_method(
        mut self,
    ) -> Result<Option<PreparedMethod<'decoder>>, TransactionPayloadDecoderError> {
        if self.payload.len() <= usize::from(MAX_ALIGNMENT) {
            return Ok(None);
        }

        let contract = self.get_trivial_type::<Address>()?;
        let method_fingerprint = self.get_trivial_type::<MethodFingerprint>()?;
        let method_context =
            (self.map_context)(*self.get_trivial_type::<TransactionMethodContext>()?);

        let mut transaction_slots_inputs =
            [MaybeUninit::<u8>::uninit(); MAX_TOTAL_METHOD_ARGS as usize];

        let num_slot_arguments = self.read_u8()?;
        for transaction_slot in transaction_slots_inputs
            .iter_mut()
            .take(usize::from(num_slot_arguments))
        {
            transaction_slot.write(self.read_u8()?);
        }

        let num_input_arguments = self.read_u8()?;
        for transaction_input in transaction_slots_inputs
            .iter_mut()
            .skip(usize::from(num_slot_arguments))
            .take(usize::from(num_input_arguments))
        {
            transaction_input.write(self.read_u8()?);
        }

        let num_output_arguments = self.read_u8()?;

        // SAFETY: Just initialized elements above
        let (transaction_slots, transaction_inputs) = unsafe {
            let (transaction_slots, transaction_inputs) =
                transaction_slots_inputs.split_at_unchecked(usize::from(num_slot_arguments));
            let (transaction_inputs, _) =
                transaction_inputs.split_at_unchecked(usize::from(num_input_arguments));

            (
                transaction_slots.assume_init_ref(),
                transaction_inputs.assume_init_ref(),
            )
        };

        // This can be off by 1 due to `self` not included in `ExternalArgs`, but it is good enough
        // for this context
        let number_of_arguments = num_slot_arguments
            .saturating_add(num_input_arguments)
            .saturating_add(num_output_arguments);
        if VERIFY && number_of_arguments > MAX_TOTAL_METHOD_ARGS {
            return Err(TransactionPayloadDecoderError::TooManyArguments(
                number_of_arguments,
            ));
        }

        // Slot needs one address pointer, input needs pointers to data and size, output needs
        // pointers to data, size and capacity
        let expected_external_args_buffer_size =
            usize::from(num_slot_arguments + num_input_arguments * 2 + num_output_arguments * 3);
        if VERIFY && expected_external_args_buffer_size > self.external_args_buffer.len() {
            return Err(TransactionPayloadDecoderError::ExternalArgsBufferTooSmall);
        }

        let external_args =
            NonNull::new(self.external_args_buffer.as_mut_ptr()).expect("Not null; qed");
        {
            let mut external_args_cursor = external_args;

            for &transaction_slot in transaction_slots {
                let address = match TransactionSlot::from_u8(transaction_slot).slot_type() {
                    TransactionSlotType::Address => self.get_trivial_type::<Address>()?,
                    TransactionSlotType::OutputIndex { output_index } => {
                        let (bytes, &size) = self.get_from_output_buffer(output_index)?;

                        if VERIFY && size != Address::SIZE {
                            return Err(
                                TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
                                    output_index,
                                    size,
                                },
                            );
                        }

                        // SAFETY: All bytes are valid as long as alignment and size match
                        let maybe_address = unsafe { Address::from_bytes(bytes) };

                        if VERIFY {
                            let Some(address) = maybe_address else {
                                let error =
                                    TransactionPayloadDecoderError::InvalidSlotOutputIndexAlign {
                                        output_index,
                                        alignment: align_of_val(bytes) as u32,
                                    };
                                return Err(error);
                            };

                            address
                        } else {
                            // SAFETY: The unverified version, see struct description
                            unsafe { maybe_address.unwrap_unchecked() }
                        }
                    }
                };

                // SAFETY: Size of `self.external_args_buffer` checked above, address is correctly
                // aligned
                unsafe {
                    external_args_cursor.cast::<*const Address>().write(address);
                    external_args_cursor = external_args_cursor.offset(1);
                }
            }

            for &transaction_input in transaction_inputs {
                let (bytes, size) = match TransactionInput::from_u8(transaction_input).input_type()
                {
                    TransactionInputType::Value { alignment_power } => {
                        // Optimized version of the following:
                        // let alignment = 2usize.pow(u32::from(alignment_power));
                        let alignment = if VERIFY {
                            1_usize.checked_shl(u32::from(alignment_power)).ok_or(
                                TransactionPayloadDecoderError::AlignmentPowerTooLarge(
                                    alignment_power,
                                ),
                            )?
                        } else {
                            // SAFETY: The unverified version, see struct description
                            unsafe { 1_usize.unchecked_shl(u32::from(alignment_power)) }
                        };

                        let size = self.get_trivial_type::<u32>()?;
                        let bytes = self.get_bytes(
                            *size,
                            NonZeroUsize::new(alignment).expect("Not zero; qed"),
                        )?;

                        (bytes, size)
                    }
                    TransactionInputType::OutputIndex { output_index } => {
                        self.get_from_output_buffer(output_index)?
                    }
                };

                // SAFETY: Size of `self.external_args_buffer` checked above, buffer is correctly
                // aligned
                unsafe {
                    external_args_cursor
                        .cast::<*const u8>()
                        .write(bytes.as_ptr());
                    external_args_cursor = external_args_cursor.offset(1);

                    external_args_cursor.cast::<*const u32>().write(size);
                    external_args_cursor = external_args_cursor.offset(1);
                }
            }

            for _ in 0..num_output_arguments {
                let recommended_capacity = self.get_trivial_type::<u32>()?;
                let alignment_power = *self.get_trivial_type::<u8>()?;
                // Optimized version of the following:
                // let alignment = 2usize.pow(u32::from(alignment_power));
                let alignment = if VERIFY {
                    1_usize.checked_shl(u32::from(alignment_power)).ok_or(
                        TransactionPayloadDecoderError::AlignmentPowerTooLarge(alignment_power),
                    )?
                } else {
                    // SAFETY: The unverified version, see struct description
                    unsafe { 1_usize.unchecked_shl(u32::from(alignment_power)) }
                };

                let (size, data) = self.allocate_output_buffer(
                    *recommended_capacity,
                    NonZeroUsize::new(alignment).expect("Not zero; qed"),
                )?;

                // SAFETY: Size of `self.external_args_buffer` checked above, buffer is correctly
                // aligned
                unsafe {
                    external_args_cursor.cast::<*mut u8>().write(data.as_ptr());
                    external_args_cursor = external_args_cursor.offset(1);

                    external_args_cursor.cast::<*mut u32>().write(size.as_ptr());
                    external_args_cursor = external_args_cursor.offset(1);

                    external_args_cursor
                        .cast::<*const u32>()
                        .write(recommended_capacity);
                    external_args_cursor = external_args_cursor.offset(1);
                }
            }
        }

        Ok(Some(PreparedMethod {
            contract: *contract,
            fingerprint: *method_fingerprint,
            external_args: external_args.cast::<NonNull<c_void>>(),
            method_context,
            phantom: PhantomData,
        }))
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn get_trivial_type<T>(&mut self) -> Result<&'decoder T, TransactionPayloadDecoderError>
    where
        T: TrivialType,
    {
        self.ensure_alignment(NonZeroUsize::new(align_of::<T>()).expect("Not zero; qed"))?;

        let bytes;
        if VERIFY {
            (bytes, self.payload) = self
                .payload
                .split_at_checked(size_of::<T>())
                .ok_or(TransactionPayloadDecoderError::PayloadTooSmall)?;
        } else {
            // SAFETY: The unverified version, see struct description
            (bytes, self.payload) = unsafe { self.payload.split_at_unchecked(size_of::<T>()) };
        }

        // SAFETY: Correctly aligned bytes of correct size
        let value_ref = unsafe { bytes.as_ptr().cast::<T>().as_ref().expect("Not null; qed") };

        Ok(value_ref)
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn get_bytes(
        &mut self,
        size: u32,
        alignment: NonZeroUsize,
    ) -> Result<&'decoder [u8], TransactionPayloadDecoderError> {
        self.ensure_alignment(alignment)?;

        let bytes;
        if VERIFY {
            (bytes, self.payload) = self
                .payload
                .split_at_checked(size as usize)
                .ok_or(TransactionPayloadDecoderError::PayloadTooSmall)?;
        } else {
            // SAFETY: The unverified version, see struct description
            (bytes, self.payload) = unsafe { self.payload.split_at_unchecked(size as usize) };
        }

        Ok(bytes)
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn read_u8(&mut self) -> Result<u8, TransactionPayloadDecoderError> {
        let value;
        if VERIFY {
            (value, self.payload) = self
                .payload
                .split_at_checked(1)
                .ok_or(TransactionPayloadDecoderError::PayloadTooSmall)?;
        } else {
            // SAFETY: The unverified version, see struct description
            (value, self.payload) = unsafe { self.payload.split_at_unchecked(1) };
        }

        Ok(value[0])
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn ensure_alignment(
        &mut self,
        alignment: NonZeroUsize,
    ) -> Result<(), TransactionPayloadDecoderError> {
        let alignment = alignment.get();
        debug_assert!(alignment <= usize::from(MAX_ALIGNMENT));

        // Optimized version of the following that expects `alignment` to be a power of 2:
        // let unaligned_by = self.payload.as_ptr().addr() % alignment;
        let unaligned_by = self.payload.as_ptr().addr() & (alignment - 1);
        if unaligned_by > 0 {
            // SAFETY: Subtracted value is always smaller than alignment
            let padding_bytes = unsafe { alignment.unchecked_sub(unaligned_by) };
            if VERIFY {
                self.payload = self
                    .payload
                    .split_off(padding_bytes..)
                    .ok_or(TransactionPayloadDecoderError::PayloadTooSmall)?;
            } else {
                // SAFETY: Subtracted value is always smaller than alignment
                (_, self.payload) = unsafe { self.payload.split_at_unchecked(padding_bytes) };
            }
        }
        Ok(())
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn allocate_output_buffer(
        &mut self,
        capacity: u32,
        output_alignment: NonZeroUsize,
    ) -> Result<(NonNull<u32>, NonNull<u8>), TransactionPayloadDecoderError> {
        if VERIFY && self.output_buffer_offsets.len() == self.output_buffer_offsets_cursor {
            return Err(TransactionPayloadDecoderError::OutputBufferOffsetsTooSmall);
        }

        let (size_offset, size_ptr) = self
            .allocate_output_buffer_ptr(
                NonZeroUsize::new(align_of::<u32>()).expect("Not zero; qed"),
                size_of::<u32>(),
            )
            .ok_or(TransactionPayloadDecoderError::OutputBufferTooSmall)?;
        let (output_offset, output_ptr) = self
            .allocate_output_buffer_ptr(output_alignment, capacity as usize)
            .ok_or(TransactionPayloadDecoderError::OutputBufferTooSmall)?;

        // SAFETY: Checked above that output buffer offsets is not full
        let output_buffer_offsets = unsafe {
            // Borrow-checker doesn't understand that these variables are disjoint
            let output_buffer_offsets_cursor = self.output_buffer_offsets_cursor;
            self.output_buffer_offsets
                .get_unchecked_mut(output_buffer_offsets_cursor)
        };
        output_buffer_offsets.write((size_offset as u32, output_offset as u32));
        self.output_buffer_offsets_cursor += 1;

        Ok((size_ptr, output_ptr))
    }

    /// Returns `None` if output buffer is not large enough
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn allocate_output_buffer_ptr<T>(
        &mut self,
        alignment: NonZeroUsize,
        size: usize,
    ) -> Option<(usize, NonNull<T>)> {
        let alignment = alignment.get();
        debug_assert!(alignment <= usize::from(MAX_ALIGNMENT));

        // Optimized version of the following that expects `alignment` to be a power of 2:
        // let unaligned_by = self.output_buffer_cursor % alignment;
        let unaligned_by = self.output_buffer_cursor & (alignment - 1);
        // SAFETY: Subtracted value is always smaller than alignment
        let padding_bytes = unsafe { alignment.unchecked_sub(unaligned_by) };

        let new_output_buffer_cursor = if VERIFY {
            let new_output_buffer_cursor = self
                .output_buffer_cursor
                .checked_add(padding_bytes)?
                .checked_add(size)?;

            if new_output_buffer_cursor > size_of_val(self.output_buffer) {
                return None;
            }

            new_output_buffer_cursor
        } else {
            // SAFETY: The unverified version, see struct description
            unsafe {
                self.output_buffer_cursor
                    .unchecked_add(padding_bytes)
                    .unchecked_add(size)
            }
        };

        // SAFETY: Bounds and alignment checks are done above
        let (offset, buffer_ptr) = unsafe {
            let offset = self.output_buffer_cursor.unchecked_add(padding_bytes);
            let buffer_ptr = NonNull::new_unchecked(
                self.output_buffer.as_mut_ptr().byte_add(offset).cast::<T>(),
            );

            (offset, buffer_ptr)
        };
        self.output_buffer_cursor = new_output_buffer_cursor;

        Some((offset, buffer_ptr))
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn get_from_output_buffer(
        &self,
        output_index: u8,
    ) -> Result<(&[u8], &u32), TransactionPayloadDecoderError> {
        let (size_offset, output_offset) = if VERIFY {
            if usize::from(output_index) < self.output_buffer_offsets_cursor {
                // SAFETY: Checked that index is initialized
                unsafe {
                    self.output_buffer_offsets
                        .get_unchecked(usize::from(output_index))
                        .assume_init()
                }
            } else {
                return Err(TransactionPayloadDecoderError::OutputIndexNotFound(
                    output_index,
                ));
            }
        } else {
            // SAFETY: The unverified version, see struct description
            unsafe {
                self.output_buffer_offsets
                    .get_unchecked(usize::from(output_index))
                    .assume_init()
            }
        };

        // SAFETY: Offset was created as the result of writing value at the correct
        // offset into `output_buffer_offsets` earlier
        let size = unsafe {
            self.output_buffer
                .as_ptr()
                .byte_add(size_offset as usize)
                .cast::<u32>()
                .as_ref_unchecked()
        };
        // SAFETY: Offset was created as the result of writing value at the correct
        // offset into `output_buffer_offsets` earlier
        let bytes = unsafe {
            let bytes_ptr = self
                .output_buffer
                .as_ptr()
                .cast::<u8>()
                .add(output_offset as usize);

            slice::from_raw_parts(bytes_ptr, *size as usize)
        };

        Ok((bytes, size))
    }
}
