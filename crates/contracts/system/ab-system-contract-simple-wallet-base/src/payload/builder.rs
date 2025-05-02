//! Transaction payload creation utilities

#[cfg(test)]
mod tests;

extern crate alloc;

use crate::payload::{TransactionInput, TransactionMethodContext, TransactionSlot};
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::metadata::decode::{
    ArgumentKind, MetadataDecodingError, MethodMetadataDecoder, MethodMetadataItem,
    MethodsContainerKind,
};
use ab_contracts_common::method::{ExternalArgs, MethodFingerprint};
use ab_core_primitives::address::Address;
use ab_io_type::MAX_ALIGNMENT;
use ab_io_type::metadata::IoTypeDetails;
use ab_io_type::trivial_type::TrivialType;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::num::NonZeroU8;
use core::ptr::NonNull;
use core::{ptr, slice};

const _: () = {
    // Make sure bit flags for all arguments will fit into a single u8 below
    assert!(MAX_TOTAL_METHOD_ARGS as u32 == u8::BITS);
};

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
}

/// Builder for payload to be used with [`TxHandler`] (primarily for [`SimpleWallet`]).
///
/// Decoding can be done with [`TransactionPayloadDecoder`]
///
/// [`TxHandler`]: ab_contracts_standards::tx_handler::TxHandler
/// [`SimpleWallet`]: crate::SimpleWalletBase
/// [`TransactionPayloadDecoder`]: crate::payload::TransactionPayloadDecoder
#[derive(Debug, Clone)]
pub struct TransactionPayloadBuilder {
    payload: Vec<u8>,
}

impl Default for TransactionPayloadBuilder {
    fn default() -> Self {
        Self {
            payload: Vec::with_capacity(1024),
        }
    }
}

impl TransactionPayloadBuilder {
    /// Add method call to the payload.
    ///
    /// The wallet will call this method in addition order.
    ///
    /// `slot_output_index` and `input_output_index` are used for referencing earlier outputs as
    /// slots or inputs of this method, its values are optional, see [`TransactionInput`] for more
    /// details.
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
                &external_args,
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
    pub unsafe fn with_method_call_untyped<'a>(
        &mut self,
        contract: &Address,
        external_args: &NonNull<*const c_void>,
        mut method_metadata: &'a [u8],
        method_fingerprint: &MethodFingerprint,
        method_context: TransactionMethodContext,
        slot_output_index: &[Option<u8>],
        input_output_index: &[Option<u8>],
    ) -> Result<(), TransactionPayloadBuilderError<'a>> {
        let mut external_args = *external_args;

        let (mut metadata_decoder, method_metadata_item) =
            MethodMetadataDecoder::new(&mut method_metadata, MethodsContainerKind::Unknown)
                .decode_next()
                .map_err(TransactionPayloadBuilderError::MetadataDecodingError)?;

        let MethodMetadataItem {
            method_kind,
            num_arguments,
            ..
        } = method_metadata_item;
        let number_of_arguments =
            num_arguments.saturating_add(if method_kind.has_self() { 1 } else { 0 });

        if number_of_arguments > MAX_TOTAL_METHOD_ARGS {
            return Err(TransactionPayloadBuilderError::TooManyArguments(
                number_of_arguments,
            ));
        }

        self.extend_payload_with_alignment(contract.as_bytes(), align_of_val(contract));
        self.extend_payload_with_alignment(
            method_fingerprint.as_bytes(),
            align_of_val(method_fingerprint),
        );
        self.push_payload_byte(method_context as u8);

        let mut num_slot_arguments = 0u8;
        let mut num_input_arguments = 0u8;
        let mut num_output_arguments = 0u8;

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
                    input_output_type_details[usize::from(num_input_arguments)]
                        .write(item.type_details.unwrap_or(IoTypeDetails::bytes(0)));
                    num_input_arguments += 1;
                }
                ArgumentKind::Output => {
                    input_output_type_details
                        [usize::from(num_input_arguments + num_output_arguments)]
                    .write(item.type_details.unwrap_or(IoTypeDetails::bytes(0)));
                    num_output_arguments += 1;
                }
            }
        }
        // SAFETY: Just initialized elements above
        let (input_type_details, output_type_details) = unsafe {
            let (input_type_details, output_type_details) =
                input_output_type_details.split_at_unchecked(usize::from(num_input_arguments));
            let (output_type_details, _) =
                output_type_details.split_at_unchecked(usize::from(num_output_arguments));

            (
                input_type_details.assume_init_ref(),
                output_type_details.assume_init_ref(),
            )
        };

        // Store number of slots and `TransactionSlot` for each slot
        self.push_payload_byte(num_slot_arguments);
        for slot_offset in 0..usize::from(num_slot_arguments) {
            let slot_type = if let Some(&Some(output_index)) = slot_output_index.get(slot_offset) {
                TransactionSlot::new_output_index(output_index).ok_or(
                    TransactionPayloadBuilderError::InvalidOutputIndex(output_index),
                )?
            } else {
                TransactionSlot::new_address()
            };
            self.push_payload_byte(slot_type.into_u8());
        }

        // Store number of inputs and `TransactionInput` for each input
        self.push_payload_byte(num_input_arguments);
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
            self.push_payload_byte(input_type.into_u8());
        }

        // Store number of outputs
        self.push_payload_byte(num_output_arguments);

        for slot_offset in 0..usize::from(num_slot_arguments) {
            // SAFETY: Method description requires the layout to correspond to metadata
            let address = unsafe {
                let address = external_args.cast::<NonNull<Address>>().read().as_ref();
                external_args = external_args.offset(1);
                address
            };

            if slot_output_index
                .get(slot_offset)
                .copied()
                .flatten()
                .is_none()
            {
                self.extend_payload_with_alignment(address.as_bytes(), align_of_val(address));
            }
        }

        for (input_offset, type_details) in input_type_details.iter().enumerate() {
            // SAFETY: Method description requires the layout to correspond to metadata
            let (size, data) = unsafe {
                let data = external_args.cast::<NonNull<u8>>().read();
                external_args = external_args.offset(1);
                let size = external_args.cast::<NonNull<u32>>().read().read();
                external_args = external_args.offset(1);

                let data = slice::from_raw_parts(data.as_ptr().cast_const(), size as usize);

                (size, data)
            };

            if input_output_index
                .get(input_offset)
                .copied()
                .flatten()
                .is_none()
            {
                self.extend_payload_with_alignment(&size.to_le_bytes(), align_of_val(&size));
                self.extend_payload_with_alignment(data, type_details.alignment.get() as usize);
            }
        }

        for type_details in output_type_details {
            self.extend_payload_with_alignment(
                &type_details.recommended_capacity.to_le_bytes(),
                align_of_val(&type_details.recommended_capacity),
            );
            self.extend_payload_with_alignment(
                &[type_details.alignment.ilog2() as u8],
                align_of::<u8>(),
            );
        }

        Ok(())
    }

    /// Returns 16-byte aligned bytes.
    ///
    /// The contents is a concatenated sequence of method calls with their arguments. All data
    /// structures are correctly aligned in the returned byte buffer with `0` used as padding when
    /// necessary.
    ///
    /// Each method is serialized in the following way:
    /// * Contract to call: [`Address`]
    /// * Fingerprint of the method to call: [`MethodFingerprint`]
    /// * Method context: [`TransactionMethodContext`]
    /// * Number of `#[slot]` arguments: `u8`
    /// * For each `#[slot]` argument:
    ///     * [`TransactionSlot`]  as `u8`
    /// * Number of `#[input]` arguments: `u8`
    /// * For each `#[input]` argument:
    ///     * [`TransactionInput`] for each `#[input]` argument as `u8`
    /// * Number of `#[output]` arguments: `u8`
    /// * For each [`TransactionSlot`], whose type is [`TransactionSlotType::Address`]:
    ///     * [`Address`]
    /// * For each [`TransactionInput`], whose type is [`TransactionInputType::Value`]:
    ///     * Input size as little-endian `u32` followed by the input itself
    /// * For each `#[output]`:
    ///     * recommended capacity as little-endian `u32` followed by alignment power as `u8`
    ///       (`NonZeroU8::ilog2(alignment)`)
    ///
    /// [`TransactionSlotType::Address`]: crate::payload::TransactionSlotType::Address
    /// [`TransactionInputType::Value`]: crate::payload::TransactionInputType::Value
    pub fn into_aligned_bytes(mut self) -> Vec<u128> {
        // Fill bytes to make it multiple of `u128` before creating `u128`-based vector
        self.ensure_alignment(usize::from(MAX_ALIGNMENT));

        let output_len = self.payload.len() / size_of::<u128>();
        let mut output = Vec::<u128>::with_capacity(output_len);

        // SAFETY: Pointers are valid for reads/writes, aligned and not overlapping
        unsafe {
            ptr::copy_nonoverlapping(
                self.payload.as_ptr(),
                output.as_mut_ptr().cast::<u8>(),
                self.payload.len(),
            );
            output.set_len(output_len);
        }

        debug_assert_eq!(align_of_val(output.as_slice()), usize::from(MAX_ALIGNMENT));

        output
    }

    fn extend_payload_with_alignment(&mut self, bytes: &[u8], alignment: usize) {
        self.ensure_alignment(alignment);

        self.payload.extend_from_slice(bytes);
    }

    fn ensure_alignment(&mut self, alignment: usize) {
        debug_assert!(alignment <= usize::from(MAX_ALIGNMENT));

        // Optimized version of the following that expects `alignment` to be a power of 2:
        // let unaligned_by = self.payload.len() % alignment;
        let unaligned_by = self.payload.len() & (alignment - 1);
        if unaligned_by > 0 {
            self.payload
                .resize(self.payload.len() + (alignment - unaligned_by), 0);
        }
    }

    fn push_payload_byte(&mut self, byte: u8) {
        self.payload.push(byte);
    }
}
