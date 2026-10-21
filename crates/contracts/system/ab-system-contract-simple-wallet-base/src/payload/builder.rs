//! Transaction payload creation utilities

#[cfg(test)]
mod tests;

use crate::payload::{
    FfiDataSizeCapacityRo, TransactionInput, TransactionMethodContext, TransactionOutput,
    TransactionSlot,
};
use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::metadata::decode::{
    ArgumentKind, MetadataDecodingError, MethodKind, MethodMetadataDecoder, MethodMetadataItem,
    MethodsContainerKind,
};
use ab_contracts_common::method::{ExternalArgs, MethodFingerprint};
use ab_core_primitives::address::Address;
use ab_io_type::MAX_ALIGNMENT;
use ab_io_type::metadata::IoTypeDetails;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::num::NonZeroU8;
use core::ptr::NonNull;
use core::{hint, slice};

const {
    // Make sure bit flags for all arguments will fit into a single u8 below
    assert!(MAX_TOTAL_METHOD_ARGS as u32 == u8::BITS);
}

/// Read from an external arguments pointer and move it forward.
///
/// # Safety
/// `external_args` must have enough capacity for the read value, and the current offset must
/// have the correct alignment for the type being read.
#[inline(always)]
unsafe fn read_external_args<T>(external_args: &mut NonNull<c_void>) -> T {
    // SAFETY: guaranteed by this function signature
    unsafe {
        let value = external_args.cast::<T>().read();
        *external_args = external_args.byte_add(size_of::<T>());
        value
    }
}

/// Errors for [`TransactionPayloadBuilder`]
#[derive(Debug, thiserror::Error)]
pub enum TransactionPayloadBuilderError<'a> {
    /// Metadata decoding error
    #[error("Metadata decoding error: {0}")]
    MetadataDecodingError(MetadataDecodingError<'a>),
    /// Too many arguments
    #[error("Too many arguments")]
    TooManyArguments(u8),
    /// Invalid alignment
    #[error("Invalid alignment: {0}")]
    InvalidAlignment(NonZeroU8),
    /// Invalid output index
    #[error("Invalid output index: {0}")]
    InvalidOutputIndex(u8),
    /// The payload doesn't fit into the maximum size given to [`TransactionPayloadBuilder::new()`]
    #[error("The payload is too large")]
    PayloadTooLarge,
}

/// Builder for payload to be used with [`TxHandler`] (primarily for [`SimpleWallet`]).
///
/// Methods in the payload are decoded and called with [`TransactionPayloadDecoder`]
///
/// [`TxHandler`]: ab_contracts_standards::tx_handler::TxHandler
/// [`SimpleWallet`]: crate::SimpleWalletBase
/// [`TransactionPayloadDecoder`]: crate::payload::TransactionPayloadDecoder
#[derive(Debug, Clone)]
pub struct TransactionPayloadBuilder {
    payload: OwnedAlignedBuffer,
}

impl TransactionPayloadBuilder {
    /// Create a new instance for a payload of up to `max_size` bytes.
    ///
    /// Memory for the whole payload is allocated here and never reallocated, a method call that
    /// doesn't fit returns [`TransactionPayloadBuilderError::PayloadTooLarge`]. The payload is
    /// padded to a multiple of [`MAX_ALIGNMENT`], so `max_size` is rounded down to a multiple of
    /// it.
    #[inline]
    pub fn new(max_size: u32) -> Self {
        Self {
            payload: OwnedAlignedBuffer::with_capacity(
                max_size / u32::from(MAX_ALIGNMENT) * u32::from(MAX_ALIGNMENT),
            ),
        }
    }

    /// Add method call to the payload.
    ///
    /// The wallet will call this method in addition order.
    ///
    /// `slot_output_index` and `input_output_index` are used for referencing earlier outputs as
    /// slots or inputs of this method, its values are optional, see [`TransactionInput`] for more
    /// details.
    ///
    /// Outputs are given to the method empty, with size `0`, so methods called from transactions
    /// must accept empty outputs, like [`MaybeData`] and [`VariableElements`]. A method with an
    /// `#[output]` of a [`TrivialType`] can't be called directly from a transaction, since its
    /// value has to be initialized first, for example by a separate method. Initializing values
    /// is domain-specific, so the payload format doesn't do it. Executors that check arguments
    /// against the method's metadata reject such calls, while the native executor trusts
    /// transactions, so with it such a call violates the requirements of the called method.
    ///
    /// The return value is given the size of its type, since executors write the whole value, so
    /// later methods can refer to it as an input or a slot. The state returned by an `#[init]`
    /// method is the exception: executors store it as the state of the contract instead of writing
    /// it to the caller, so it is encoded as an output with capacity `0`, like the state in the
    /// last `#[output]` of `#[init]` without a return value, and later methods referring to it get
    /// it empty.
    ///
    /// On error, part of the method call might have already been added to the payload.
    ///
    /// [`MaybeData`]: ab_io_type::maybe_data::MaybeData
    /// [`VariableElements`]: ab_io_type::variable_elements::VariableElements
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn with_method_call<Args>(
        &mut self,
        contract: &Address,
        external_args: &Args,
        method_context: TransactionMethodContext,
        slot_output_index: &[Option<u8>],
        input_output_index: &[Option<u8>],
    ) -> Result<(), TransactionPayloadBuilderError<'static>>
    where
        Args: ExternalArgs,
    {
        let external_args = NonNull::from_ref(external_args).cast::<*const c_void>();

        // SAFETY: Called with statically valid data
        unsafe {
            self.with_method_call_untyped(
                contract,
                external_args,
                Args::METADATA,
                &Args::FINGERPRINT,
                method_context,
                slot_output_index,
                input_output_index,
            )
        }
    }

    /// Other than unsafe API, this method is identical to [`Self::with_method_call()`].
    ///
    /// # Safety
    /// `external_args` must correspond to `method_metadata` and `method_fingerprint`. Outputs are
    /// never read from `external_args` and inputs that have corresponding `input_output_index`
    /// are not read either.
    #[expect(
        clippy::too_many_arguments,
        reason = "Only exceeds the limit due to being untyped, while above typed version is not"
    )]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub unsafe fn with_method_call_untyped<'a>(
        &mut self,
        contract: &Address,
        external_args: NonNull<*const c_void>,
        mut method_metadata: &'a [u8],
        method_fingerprint: &MethodFingerprint,
        method_context: TransactionMethodContext,
        slot_output_index: &[Option<u8>],
        input_output_index: &[Option<u8>],
    ) -> Result<(), TransactionPayloadBuilderError<'a>> {
        let external_args = &mut external_args.cast::<c_void>();

        let (metadata_decoder, method_metadata_item) =
            MethodMetadataDecoder::new(&mut method_metadata, MethodsContainerKind::Unknown)
                .decode_next()
                .map_err(TransactionPayloadBuilderError::MetadataDecodingError)?;
        let mut metadata_decoder = metadata_decoder.without_auto_drain();

        #[expect(
            clippy::rest_pattern_accessible_field,
            reason = "Do not need other fields"
        )]
        let MethodMetadataItem {
            method_kind,
            num_arguments,
            ..
        } = method_metadata_item;
        let number_of_arguments = num_arguments.saturating_add(u8::from(method_kind.has_self()));

        if number_of_arguments > MAX_TOTAL_METHOD_ARGS {
            return Err(TransactionPayloadBuilderError::TooManyArguments(
                number_of_arguments,
            ));
        }

        self.extend_payload_with_alignment(contract.as_bytes(), align_of_val(contract))?;
        self.extend_payload_with_alignment(
            method_fingerprint.as_bytes(),
            align_of_val(method_fingerprint),
        )?;
        self.push_payload_byte(method_context as u8)?;

        let mut num_slot_arguments = 0u8;
        let mut num_input_arguments = 0u8;
        let mut num_output_arguments = 0u8;
        // Offset of the return value among outputs if executors write it to the caller
        let mut return_value_offset = None;

        let mut input_output_type_details =
            [MaybeUninit::<IoTypeDetails>::uninit(); MAX_TOTAL_METHOD_ARGS as usize];
        // Collect information about all arguments so everything below is able to be purely additive
        while let Some(item) = metadata_decoder
            .decode_next()
            .transpose()
            .map_err(TransactionPayloadBuilderError::MetadataDecodingError)?
        {
            match item.argument_kind {
                ArgumentKind::EnvRo
                | ArgumentKind::EnvRw
                | ArgumentKind::TmpRo
                | ArgumentKind::TmpRw => {
                    // Not represented in external args
                }
                ArgumentKind::SlotRo | ArgumentKind::SlotRw => {
                    num_slot_arguments += 1;
                }
                ArgumentKind::Input => {
                    // SAFETY: The counters only count arguments decoded before this one, so they
                    // and their sum are below `num_arguments`, the number of items that the
                    // metadata decoder returns (see `MethodMetadataItem::num_arguments`). The check
                    // of the number of arguments above limits `num_arguments` to
                    // `MAX_TOTAL_METHOD_ARGS`.
                    unsafe {
                        hint::assert_unchecked(num_input_arguments < MAX_TOTAL_METHOD_ARGS);
                    }
                    input_output_type_details[usize::from(num_input_arguments)]
                        .write(item.type_details.unwrap_or(IoTypeDetails::bytes(0)));
                    num_input_arguments += 1;
                }
                ArgumentKind::Output | ArgumentKind::Return => {
                    // The return value is the last argument (see
                    // `ArgumentsMetadataDecoder::decode_next()`), so it is also the last output. In
                    // `#[init]` it is the state, which executors store instead of writing it to the
                    // caller, so it is encoded as an output then, like the state in the last
                    // `#[output]` of `#[init]` without a return value.
                    if matches!(item.argument_kind, ArgumentKind::Return)
                        && !matches!(method_kind, MethodKind::Init)
                    {
                        return_value_offset = Some(usize::from(num_output_arguments));
                    }
                    // SAFETY: Same as for inputs above
                    unsafe {
                        hint::assert_unchecked(
                            num_input_arguments + num_output_arguments < MAX_TOTAL_METHOD_ARGS,
                        );
                    }
                    // After all inputs, which come before outputs
                    input_output_type_details
                        [usize::from(num_input_arguments + num_output_arguments)]
                    .write(item.type_details.unwrap_or(IoTypeDetails::bytes(0)));
                    num_output_arguments += 1;
                }
            }
        }
        // SAFETY: As for the writes above, the counters and their sum are at most `num_arguments`,
        // so they stay within the `MAX_TOTAL_METHOD_ARGS` elements of the array. The metadata
        // decoder returns all inputs before outputs (see
        // `ArgumentsMetadataDecoder::decode_next()`), so the first `num_input_arguments` elements
        // were initialized above with inputs and the next `num_output_arguments` ones with
        // outputs.
        let (input_type_details, output_type_details) = unsafe {
            let (input_type_details, output_type_details) =
                input_output_type_details.split_at_unchecked(usize::from(num_input_arguments));
            let output_type_details =
                output_type_details.get_unchecked(..usize::from(num_output_arguments));

            (
                input_type_details.assume_init_ref(),
                output_type_details.assume_init_ref(),
            )
        };

        // Store number of slots and `TransactionSlot` for each slot
        self.push_payload_byte(num_slot_arguments)?;
        for slot_offset in 0..usize::from(num_slot_arguments) {
            let slot_type = if let Some(&Some(output_index)) = slot_output_index.get(slot_offset) {
                TransactionSlot::new_output_index(output_index).ok_or(
                    TransactionPayloadBuilderError::InvalidOutputIndex(output_index),
                )?
            } else {
                TransactionSlot::new_address()
            };
            self.push_payload_byte(slot_type.into_u8())?;
        }

        // Store number of inputs and `TransactionInput` for each input
        self.push_payload_byte(num_input_arguments)?;
        for (input_offset, type_details) in input_type_details.iter().enumerate() {
            let input_type = if let Some(&Some(output_index)) = input_output_index.get(input_offset)
            {
                TransactionInput::new_output_index(output_index).ok_or(
                    TransactionPayloadBuilderError::InvalidOutputIndex(output_index),
                )?
            } else {
                TransactionInput::new_value(type_details.alignment).ok_or(
                    TransactionPayloadBuilderError::InvalidAlignment(type_details.alignment),
                )?
            };
            self.push_payload_byte(input_type.into_u8())?;
        }

        // Store number of outputs
        self.push_payload_byte(num_output_arguments)?;

        for slot_offset in 0..usize::from(num_slot_arguments) {
            // SAFETY: Method description requires the layout to correspond to metadata, where all
            // slots come before inputs and outputs (see `ArgumentsMetadataDecoder::decode_next()`)
            let address = unsafe { read_external_args::<NonNull<Address>>(external_args).as_ref() };

            if slot_output_index
                .get(slot_offset)
                .copied()
                .flatten()
                .is_none()
            {
                self.extend_payload_with_alignment(address.as_bytes(), align_of_val(address))?;
            }
        }

        for (input_offset, type_details) in input_type_details.iter().enumerate() {
            // SAFETY: Method description requires the layout to correspond to metadata, where the
            // inputs come right after the slots (see `ArgumentsMetadataDecoder::decode_next()`)
            let (size, data) = unsafe {
                let FfiDataSizeCapacityRo {
                    data_ptr,
                    size,
                    capacity: _,
                } = read_external_args(external_args);

                let data = slice::from_raw_parts(data_ptr.as_ptr().cast_const(), size as usize);

                (size, data)
            };

            if input_output_index
                .get(input_offset)
                .copied()
                .flatten()
                .is_none()
            {
                self.extend_payload_with_alignment(&size.to_le_bytes(), align_of_val(&size))?;
                self.extend_payload_with_alignment(data, type_details.alignment.get() as usize)?;
            }
        }

        for (output_offset, type_details) in output_type_details.iter().enumerate() {
            let output_type = if return_value_offset == Some(output_offset) {
                TransactionOutput::new_return_value(type_details.alignment)
            } else {
                TransactionOutput::new_output(type_details.alignment)
            }
            .ok_or(TransactionPayloadBuilderError::InvalidAlignment(
                type_details.alignment,
            ))?;
            self.extend_payload_with_alignment(
                &type_details.recommended_capacity.to_le_bytes(),
                align_of_val(&type_details.recommended_capacity),
            )?;
            self.push_payload_byte(output_type.into_u8())?;
        }

        Ok(())
    }

    /// Returns 16-byte aligned bytes, padded to a multiple of `u128` size.
    ///
    /// The contents is a concatenated sequence of method calls with their arguments. All data
    /// structures are correctly aligned in the returned byte buffer with `0` used as padding when
    /// necessary. [`OwnedAlignedBuffer::as_u128_slice()`] returns the whole payload as `u128`s.
    ///
    /// Each method is serialized in the following way:
    /// * Contract to call: [`Address`]
    /// * Fingerprint of the method to call: [`MethodFingerprint`]
    /// * Method context: [`TransactionMethodContext`]
    /// * Number of `#[slot]` arguments: `u8`
    /// * For each `#[slot]` argument:
    ///     * [`TransactionSlot`] as `u8`
    /// * Number of `#[input]` arguments: `u8`
    /// * For each `#[input]` argument:
    ///     * [`TransactionInput`] for each `#[input]` argument as `u8`
    /// * Number of `#[output]` arguments, including the return value: `u8`
    /// * For each [`TransactionSlot`], whose type is [`TransactionSlotType::Address`]:
    ///     * [`Address`]
    /// * For each [`TransactionInput`], whose type is [`TransactionInputType::Value`]:
    ///     * Input size as little-endian `u32` followed by the input itself
    /// * For each `#[output]` and the return value:
    ///     * recommended capacity as little-endian `u32` followed by [`TransactionOutput`] as `u8`
    ///
    /// [`TransactionSlotType::Address`]: crate::payload::TransactionSlotType::Address
    /// [`TransactionInputType::Value`]: crate::payload::TransactionInputType::Value
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn into_aligned_bytes(mut self) -> OwnedAlignedBuffer {
        // SAFETY: Constructor guarantees that capacity is a multiple of `MAX_ALIGNMENT` and is
        // never exceeded
        unsafe {
            self.ensure_alignment(usize::from(MAX_ALIGNMENT))
                .unwrap_unchecked();
        }

        self.payload
    }

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn extend_payload_with_alignment(
        &mut self,
        bytes: &[u8],
        alignment: usize,
    ) -> Result<(), TransactionPayloadBuilderError<'static>> {
        self.ensure_alignment(alignment)?;
        self.extend_payload(bytes)
    }

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn ensure_alignment(
        &mut self,
        alignment: usize,
    ) -> Result<(), TransactionPayloadBuilderError<'static>> {
        debug_assert!(alignment <= usize::from(MAX_ALIGNMENT));

        // Optimized version of the following that expects `alignment` to be a power of 2:
        // let unaligned_by = self.payload.len() % alignment;
        let unaligned_by = self.payload.len() as usize & (alignment - 1);
        if unaligned_by > 0 {
            // SAFETY: Subtracted value is always smaller than alignment
            let padding_bytes = unsafe { alignment.unchecked_sub(unaligned_by) };
            // Byte by byte rather than as a slice of zeroes, which the compiler can't prove to be
            // in bounds for alignments that come from metadata
            for _ in 0..padding_bytes {
                self.push_payload_byte(0)?;
            }
        }

        Ok(())
    }

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn push_payload_byte(
        &mut self,
        byte: u8,
    ) -> Result<(), TransactionPayloadBuilderError<'static>> {
        self.extend_payload(&[byte])
    }

    /// Append `bytes` to the payload if they fit into its capacity, it is never reallocated
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn extend_payload(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), TransactionPayloadBuilderError<'static>> {
        // The same checks as in `OwnedAlignedBuffer::append()`, so the compiler can prove that it
        // never reallocates
        let fits = u32::try_from(bytes.len())
            .ok()
            .and_then(|len| self.payload.len().checked_add(len))
            .is_some_and(|new_len| new_len <= self.payload.capacity());
        if !(fits && self.payload.append(bytes)) {
            return Err(TransactionPayloadBuilderError::PayloadTooLarge);
        }

        Ok(())
    }
}
