//! Transaction payload creation utilities

#[cfg(test)]
mod tests;

use crate::MAX_OUTPUTS;
use crate::payload::decoder_core::{output_ref, slot_output_ref, writes_output};
use crate::payload::{
    DeclaredOutput, FfiDataSizeCapacityRo, OutputState, TransactionInput, TransactionMethodContext,
    TransactionOutput, TransactionOutputKind, TransactionPayloadDecoderError, TransactionSlot,
};
use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::metadata::decode::{
    ArgumentKind, MetadataDecodingError, MethodKind, MethodMetadataDecoder, MethodMetadataItem,
    MethodsContainerKind,
};
use ab_contracts_common::method::{ExternalArgs, MethodFingerprint};
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::metadata::IoTypeDetails;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::mem::MaybeUninit;
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
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
    /// An output index of [`MAX_OUTPUTS`] or more, which tags can't hold
    #[error("Invalid output index: {0}")]
    InvalidOutputIndex(u8),
    /// The payload doesn't fit into the maximum size given to [`TransactionPayloadBuilder::new()`]
    #[error("The payload is too large")]
    PayloadTooLarge,
    /// More than [`MAX_OUTPUTS`] outputs declared
    #[error("Too many outputs")]
    TooManyOutputs,
    /// [`MethodCallReferences::outputs`] doesn't have exactly one entry for each output of the
    /// method
    #[error("The method has {expected} outputs, but {actual} output indices are given")]
    NumberOfOutputsMismatch { expected: u8, actual: usize },
    /// [`MethodCallReferences::slots`] has more entries than the method has slots
    #[error("The method has {num_slots} slots, but {num_references} slot references are given")]
    TooManySlotReferences {
        num_slots: u8,
        num_references: usize,
    },
    /// [`MethodCallReferences::inputs`] has more entries than the method has inputs
    #[error("The method has {num_inputs} inputs, but {num_references} input references are given")]
    TooManyInputReferences {
        num_inputs: u8,
        num_references: usize,
    },
    /// No method call writes the declared output with this index, see
    /// [`TransactionPayloadBuilder::into_aligned_bytes()`]
    #[error("No method call writes the output with output index {0}")]
    UnusedOutput(u8),
    /// The method call breaks a rule of the payload format, with the error that
    /// [`TransactionPayloadDecoder`] returns for such a payload, see
    /// [`TransactionPayloadBuilder::with_method_call()`]
    ///
    /// [`TransactionPayloadDecoder`]: crate::payload::decoder::TransactionPayloadDecoder
    #[error("The method call breaks a rule of the payload format: {0}")]
    PayloadRule(TransactionPayloadDecoderError),
}

/// Declared output that no method call wrote yet, with capacity `0`
const UNWRITTEN_OUTPUT: DeclaredOutput = DeclaredOutput {
    offset: 0,
    capacity: 0,
    size: 0,
    state: OutputState::NotWritten,
};

/// References of a method call to declared outputs, by position among the method's arguments of
/// each kind, see [`TransactionPayloadBuilder::with_method_call()`]
#[derive(Debug, Default, Copy, Clone)]
pub struct MethodCallReferences<'a> {
    /// Output index for each `#[slot]`, whose value the slot gets instead of an explicit address.
    /// `None` or no entry for an explicit address.
    pub slots: &'a [Option<u8>],
    /// Output index for each `#[input]`, whose value the input gets instead of a value in the
    /// payload. `None` or no entry for a value in the payload.
    pub inputs: &'a [Option<u8>],
    /// Output index for each `#[output]` and the return value (last), which they write. Exactly
    /// one entry for each output of the method.
    pub outputs: &'a [u8],
}

/// Builder for payload to be used with [`TxHandler`] (primarily for [`SimpleWallet`]).
///
/// The number of outputs that the payload declares is given to [`Self::new()`]. Method calls,
/// added with [`Self::with_method_call()`], write these outputs and refer to them by output index,
/// and the builder computes the capacity of every declared output from the method calls that write
/// it.
///
/// Methods in the payload are decoded and called with [`TransactionPayloadCaller`], and can be
/// described without calling them with [`TransactionPayloadDecoder`].
///
/// [`TxHandler`]: ab_contracts_standards::tx_handler::TxHandler
/// [`SimpleWallet`]: crate::SimpleWalletBase
/// [`TransactionPayloadCaller`]: crate::payload::caller::TransactionPayloadCaller
/// [`TransactionPayloadDecoder`]: crate::payload::decoder::TransactionPayloadDecoder
#[derive(Debug, Clone)]
pub struct TransactionPayloadBuilder {
    /// Starts with the header, which declares the outputs, with a capacity for each of them that
    /// method calls raise, see [`Self::into_aligned_bytes()`]
    payload: OwnedAlignedBuffer,
    /// Entries before `num_outputs` are the outputs that the payload declares, with their
    /// capacities and what the method calls added so far wrote into them last, as decoders see
    /// them after those method calls
    declared_outputs: [DeclaredOutput; MAX_OUTPUTS],
    /// Number of outputs that the payload declares, at most [`MAX_OUTPUTS`]
    num_outputs: u8,
}

impl TransactionPayloadBuilder {
    /// Create a new instance for a payload of up to `max_size` bytes that declares `num_outputs`
    /// outputs.
    ///
    /// Memory for the whole payload is allocated here and never reallocated, a method call that
    /// doesn't fit returns [`TransactionPayloadBuilderError::PayloadTooLarge`]. The payload is
    /// padded to a multiple of [`Alignment::MAX`], so `max_size` is rounded down to a multiple of
    /// it.
    ///
    /// `#[output]` arguments and return values of method calls write the declared outputs, and
    /// slots and inputs of later method calls can refer to what was written last, by output index
    /// from `0` to `num_outputs - 1`, see [`MethodCallReferences`]. The header of the payload has
    /// room for the capacity of every declared output, which starts at `0`, and every method call
    /// that writes an output raises its capacity to what the method needs, so capacities don't
    /// have to be known upfront. The memory of every declared output is aligned for every output
    /// type. [`SimpleWalletBase::execute()`] only calls the methods of a payload whose declared
    /// outputs fit into [`OUTPUT_BUFFER_SIZE`] together, see [`TransactionPayloadDecoder`].
    ///
    /// The header is written right away, so a payload without method calls is the header.
    ///
    /// Returns an error if `num_outputs` is larger than [`MAX_OUTPUTS`] or the header doesn't fit
    /// into `max_size`.
    ///
    /// [`SimpleWalletBase::execute()`]: crate::SimpleWalletBase::execute
    /// [`OUTPUT_BUFFER_SIZE`]: crate::OUTPUT_BUFFER_SIZE
    /// [`TransactionPayloadDecoder`]: crate::payload::decoder::TransactionPayloadDecoder
    // TODO: Not annotated with `no_panic`, since allocating the payload can panic
    pub fn new(
        max_size: u32,
        num_outputs: u8,
    ) -> Result<Self, TransactionPayloadBuilderError<'static>> {
        if usize::from(num_outputs) > MAX_OUTPUTS {
            return Err(TransactionPayloadBuilderError::TooManyOutputs);
        }

        let mut builder = Self {
            payload: OwnedAlignedBuffer::with_capacity(
                max_size / u32::from(Alignment::MAX.get()) * u32::from(Alignment::MAX.get()),
            ),
            declared_outputs: [UNWRITTEN_OUTPUT; MAX_OUTPUTS],
            num_outputs,
        };
        builder.push_payload_byte(num_outputs)?;
        // Method calls that write an output raise its capacity
        for _ in 0..num_outputs {
            builder.extend_payload_with_alignment(&0u32.to_le_bytes(), Alignment::of::<u32>())?;
        }

        Ok(builder)
    }

    /// Add method call to the payload.
    ///
    /// The wallet will call this method in addition order.
    ///
    /// `references` refer to outputs that the payload declares (see [`Self::new()`]): the output
    /// index for each `#[slot]` and `#[input]` that gets the value that an earlier method call
    /// wrote last into that output instead of the value in `external_args`, and the output index
    /// that each `#[output]` and the return value write. See [`TransactionInput`] for more details.
    /// The capacity of every output that the method call writes is raised to the recommended
    /// capacity of the `#[output]` or the size of the return value if it is smaller.
    ///
    /// Every output index must be below [`MAX_OUTPUTS`]
    /// ([`TransactionPayloadBuilderError::InvalidOutputIndex`]). The method call is also checked
    /// against the rules of the payload format about outputs, mostly with the code that decoders
    /// use, so that [`TransactionPayloadDecoder`] accepts the payload, and a broken rule is
    /// returned as [`TransactionPayloadBuilderError::PayloadRule`] with the error that the
    /// decoder returns for it, in the order in which the decoder checks the rules:
    /// * a slot or an input only refers to a declared output that an earlier method call wrote
    ///   ([`TransactionPayloadDecoderError::OutputIndexNotFound`]) and that this method call
    ///   doesn't write ([`TransactionPayloadDecoderError::OutputReferencedBySameMethod`])
    /// * a slot only refers to a return value with the size of an address
    ///   ([`TransactionPayloadDecoderError::InvalidSlotOutputIndexSize`]); an `#[output]` used as a
    ///   slot gets its size when its method is called, so [`TransactionPayloadCaller`] checks it
    /// * the method call only writes declared outputs
    ///   ([`TransactionPayloadDecoderError::OutputIndexNotFound`]), each at most once
    ///   ([`TransactionPayloadDecoderError::OutputUsedTwice`])
    ///
    /// The other rules about outputs hold by construction: the return value is the last output,
    /// and every output gets a capacity that is enough for every method call that writes it.
    ///
    /// Outputs are given to the method empty, with size `0`, whatever an earlier method call wrote
    /// into them, so methods called from transactions must accept empty outputs, like
    /// [`MaybeData`] and [`VariableElements`]. A method with an `#[output]` of a [`TrivialType`]
    /// can't be called directly from a transaction, since its value has to be initialized first,
    /// for example by a separate method. Initializing values is domain-specific, so the payload
    /// format doesn't do it. Executors that check arguments against the method's metadata reject
    /// such calls, while the native executor trusts transactions, so with it such a call violates
    /// the requirements of the called method.
    ///
    /// The return value is given the size of its type, since executors write the whole value, so
    /// later method calls can refer to it as an input or a slot. The state returned by an `#[init]`
    /// method is the exception: executors store it as the state of the contract instead of writing
    /// it to the caller, so it is encoded as an `#[output]` that needs no capacity, like the state
    /// in the last `#[output]` of `#[init]` without a return value, and later method calls
    /// referring to it get it empty.
    ///
    /// On error, the builder is left unchanged, so other method calls can still be added.
    ///
    /// [`TransactionPayloadDecoder`]: crate::payload::decoder::TransactionPayloadDecoder
    /// [`TransactionPayloadCaller`]: crate::payload::caller::TransactionPayloadCaller
    /// [`MaybeData`]: ab_io_type::maybe_data::MaybeData
    /// [`VariableElements`]: ab_io_type::variable_elements::VariableElements
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn with_method_call<Args>(
        &mut self,
        contract: &Address,
        external_args: &Args,
        method_context: TransactionMethodContext,
        references: MethodCallReferences<'_>,
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
                references,
            )
        }
    }

    /// Other than unsafe API, this method is identical to [`Self::with_method_call()`].
    ///
    /// # Safety
    /// `external_args` must correspond to `method_metadata` and `method_fingerprint`. Outputs are
    /// never read from `external_args`. The pointers of slots and inputs that refer to declared
    /// outputs in `references` are never dereferenced, so they may dangle, but they must not be
    /// null, since external arguments hold every pointer as [`NonNull`].
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub unsafe fn with_method_call_untyped<'a>(
        &mut self,
        contract: &Address,
        external_args: NonNull<*const c_void>,
        method_metadata: &'a [u8],
        method_fingerprint: &MethodFingerprint,
        method_context: TransactionMethodContext,
        references: MethodCallReferences<'_>,
    ) -> Result<(), TransactionPayloadBuilderError<'a>> {
        let payload_len = self.payload.len();
        // SAFETY: Guaranteed by this function's contract
        let result = unsafe {
            self.add_method_call_untyped(
                contract,
                external_args,
                method_metadata,
                method_fingerprint,
                method_context,
                references,
            )
        };
        if result.is_err() {
            // Remove the part of a method call that didn't fit, declared outputs weren't changed
            // SAFETY: The payload only grows, so its first `payload_len` bytes are still
            // initialized
            unsafe {
                self.payload.set_len(payload_len);
            }
        }

        result
    }

    /// Add the method call to the payload, see [`Self::with_method_call_untyped()`].
    ///
    /// Errors other than [`TransactionPayloadBuilderError::PayloadTooLarge`] are returned before
    /// anything is changed. After that error, part of the method call might have been added to
    /// the payload, while declared outputs are only changed after the whole method call was added.
    ///
    /// # Safety
    /// The same as for [`Self::with_method_call_untyped()`].
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    unsafe fn add_method_call_untyped<'a>(
        &mut self,
        contract: &Address,
        external_args: NonNull<*const c_void>,
        mut method_metadata: &'a [u8],
        method_fingerprint: &MethodFingerprint,
        method_context: TransactionMethodContext,
        references: MethodCallReferences<'_>,
    ) -> Result<(), TransactionPayloadBuilderError<'a>> {
        let external_args = &mut external_args.cast::<c_void>();
        let MethodCallReferences {
            slots: slot_output_indices,
            inputs: input_output_indices,
            outputs: output_indices,
        } = references;

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

        let mut num_slot_arguments = 0u8;
        let mut num_input_arguments = 0u8;
        let mut num_output_arguments = 0u8;
        // Offset of the return value among outputs and its size if executors write it to the
        // caller
        let mut return_value_offset = None;
        let mut return_value_size = None;

        let mut input_output_type_details =
            [MaybeUninit::<IoTypeDetails>::uninit(); MAX_TOTAL_METHOD_ARGS as usize];
        // Collect information about all arguments, so that everything except whether the method
        // call fits is checked before the payload is changed
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
                    // The state of `#[init]` has no type details, so it needs no capacity
                    let type_details = item.type_details.unwrap_or(IoTypeDetails::bytes(0));
                    // The return value is the last argument (see
                    // `ArgumentsMetadataDecoder::decode_next()`), so it is also the last output. In
                    // `#[init]` it is the state, which executors store instead of writing it to the
                    // caller, so it is encoded as an output then, like the state in the last
                    // `#[output]` of `#[init]` without a return value.
                    if matches!(item.argument_kind, ArgumentKind::Return)
                        && !matches!(method_kind, MethodKind::Init)
                    {
                        return_value_offset = Some(usize::from(num_output_arguments));
                        // The size of the return value is the size of its type, which is its
                        // recommended capacity
                        return_value_size = Some(type_details.recommended_capacity);
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
                    .write(type_details);
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

        if slot_output_indices.len() > usize::from(num_slot_arguments) {
            return Err(TransactionPayloadBuilderError::TooManySlotReferences {
                num_slots: num_slot_arguments,
                num_references: slot_output_indices.len(),
            });
        }
        if input_output_indices.len() > usize::from(num_input_arguments) {
            return Err(TransactionPayloadBuilderError::TooManyInputReferences {
                num_inputs: num_input_arguments,
                num_references: input_output_indices.len(),
            });
        }
        if output_indices.len() != usize::from(num_output_arguments) {
            return Err(TransactionPayloadBuilderError::NumberOfOutputsMismatch {
                expected: num_output_arguments,
                actual: output_indices.len(),
            });
        }
        for &output_index in slot_output_indices
            .iter()
            .chain(input_output_indices)
            .flatten()
            .chain(output_indices)
        {
            if usize::from(output_index) >= MAX_OUTPUTS {
                return Err(TransactionPayloadBuilderError::InvalidOutputIndex(
                    output_index,
                ));
            }
        }

        // `TransactionOutput` for each output, the return value last
        let mut output_tags = [0; MAX_TOTAL_METHOD_ARGS as usize];
        for ((output_offset, &output_index), output_tag) in
            output_indices.iter().enumerate().zip(&mut output_tags)
        {
            let output = if return_value_offset == Some(output_offset) {
                TransactionOutput::new_return_value(output_index)
            } else {
                TransactionOutput::new_output(output_index)
            }
            .ok_or(TransactionPayloadBuilderError::InvalidOutputIndex(
                output_index,
            ))?;
            *output_tag = output.into_u8();
        }
        // SAFETY: `num_output_arguments` is at most `MAX_TOTAL_METHOD_ARGS`, as for the details of
        // outputs above, and the loop above wrote a tag for each of the as many output indices
        let output_tags = unsafe { output_tags.get_unchecked(..usize::from(num_output_arguments)) };

        // The rules of the payload format about outputs, checked in the same order as decoders do,
        // with `declared_outputs` in the state that decoders have after the earlier method calls
        for &output_index in slot_output_indices.iter().flatten() {
            slot_output_ref(self.declared_outputs(), output_index, output_tags)
                .map_err(TransactionPayloadBuilderError::PayloadRule)?;
        }
        for &output_index in input_output_indices.iter().flatten() {
            output_ref(self.declared_outputs(), output_index, output_tags)
                .map_err(TransactionPayloadBuilderError::PayloadRule)?;
        }
        // Every output writes a declared output, each at most once, as
        // `PayloadDecoderCore::write_outputs()` checks. Its other rules hold by construction: the
        // tags are valid, and the return value is the last output.
        let mut remaining_output_tags = output_tags;
        while let Some((&output_tag, later_output_tags)) = remaining_output_tags.split_first() {
            remaining_output_tags = later_output_tags;

            let output_index = TransactionOutput::tag_output_index(output_tag);
            if output_index >= self.num_outputs {
                return Err(TransactionPayloadBuilderError::PayloadRule(
                    TransactionPayloadDecoderError::OutputIndexNotFound(output_index),
                ));
            }
            if writes_output(later_output_tags, output_index) {
                return Err(TransactionPayloadBuilderError::PayloadRule(
                    TransactionPayloadDecoderError::OutputUsedTwice(output_index),
                ));
            }
        }

        self.extend_payload_with_alignment(contract.as_bytes(), Alignment::of::<Address>())?;
        self.extend_payload_with_alignment(
            method_fingerprint.as_bytes(),
            Alignment::of::<MethodFingerprint>(),
        )?;
        self.push_payload_byte(method_context as u8)?;

        // Store number of slots and `TransactionSlot` for each slot
        self.push_payload_byte(num_slot_arguments)?;
        for slot_offset in 0..usize::from(num_slot_arguments) {
            let slot_type = if let Some(&Some(output_index)) = slot_output_indices.get(slot_offset)
            {
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
            let input_type =
                if let Some(&Some(output_index)) = input_output_indices.get(input_offset) {
                    // Only the output index is written, so the alignment of the input isn't used
                    TransactionInput::new_output_index(output_index).ok_or(
                        TransactionPayloadBuilderError::InvalidOutputIndex(output_index),
                    )?
                } else {
                    TransactionInput::new_value(type_details.alignment)
                };
            self.push_payload_byte(input_type.into_u8())?;
        }

        // Store number of outputs and `TransactionOutput` for each output
        self.push_payload_byte(num_output_arguments)?;
        for &output_tag in output_tags {
            self.push_payload_byte(output_tag)?;
        }

        for slot_offset in 0..usize::from(num_slot_arguments) {
            // SAFETY: Method description requires the layout to correspond to metadata, where all
            // slots come before inputs and outputs (see `ArgumentsMetadataDecoder::decode_next()`),
            // and the pointer to be non-null even when it dangles
            let address = unsafe { read_external_args::<NonNull<Address>>(external_args) };

            if slot_output_indices
                .get(slot_offset)
                .copied()
                .flatten()
                .is_none()
            {
                // SAFETY: Method description requires the layout to correspond to metadata, and
                // the slot doesn't refer to a declared output, so it points to an address
                let address = unsafe { address.as_ref() };
                self.extend_payload_with_alignment(address.as_bytes(), Alignment::of::<Address>())?;
            }
        }

        for (input_offset, type_details) in input_type_details.iter().enumerate() {
            // SAFETY: Method description requires the layout to correspond to metadata, where the
            // inputs come right after the slots (see `ArgumentsMetadataDecoder::decode_next()`),
            // and the pointer to be non-null even when it dangles
            let FfiDataSizeCapacityRo {
                data_ptr,
                size,
                capacity: _,
            } = unsafe { read_external_args(external_args) };

            if input_output_indices
                .get(input_offset)
                .copied()
                .flatten()
                .is_none()
            {
                // SAFETY: Method description requires the layout to correspond to metadata, and
                // the input doesn't refer to a declared output, so it points to `size` bytes
                let data =
                    unsafe { slice::from_raw_parts(data_ptr.as_ptr().cast_const(), size as usize) };
                self.extend_payload_with_alignment(&size.to_le_bytes(), Alignment::of::<u32>())?;
                self.extend_payload_with_alignment(data, type_details.alignment)?;
            }
        }

        if let Some(size) = return_value_size {
            self.extend_payload_with_alignment(&size.to_le_bytes(), Alignment::of::<u32>())?;
        }

        // Only after the whole method call was added, so that declared outputs don't change if it
        // doesn't fit. An `#[output]` needs its recommended capacity, and the recommended capacity
        // of a return value is its size.
        for (&output_tag, type_details) in output_tags.iter().zip(output_type_details) {
            // SAFETY: The tag was made from a `TransactionOutput` above, and every output index was
            // checked above to refer to a declared output
            unsafe {
                let output = TransactionOutput::from_u8(output_tag).unwrap_unchecked();
                self.write_output(output, type_details.recommended_capacity);
            }
        }

        Ok(())
    }

    /// Returns 16-byte aligned bytes, padded to a multiple of `u128` size.
    ///
    /// The contents is a header with the declared outputs, followed by a concatenated sequence of
    /// method calls with their arguments. All data structures are correctly aligned in the
    /// returned byte buffer with `0` used as padding when necessary.
    /// [`OwnedAlignedBuffer::as_u128_slice()`] returns the whole payload as `u128`s. Every payload
    /// starts with the header, also without declared outputs and method calls.
    ///
    /// The capacity of every declared output is the largest that a method call writing it needs:
    /// the recommended capacity of an `#[output]`, the size of a return value, or `0` for the state
    /// returned by an `#[init]` method.
    ///
    /// Returns [`TransactionPayloadBuilderError::UnusedOutput`] with the first declared output that
    /// no method call writes, since its memory would never be used, which most likely means that a
    /// method call refers to the wrong output index. [`TransactionPayloadDecoder`] and
    /// [`TransactionPayloadCaller`] accept such a payload.
    ///
    /// The header:
    /// * Number of declared outputs: `u8`
    /// * For each declared output:
    ///     * Its capacity as little-endian `u32`
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
    ///     * [`TransactionInput`] as `u8`
    /// * Number of `#[output]` arguments, including the return value: `u8`
    /// * For each `#[output]` argument, followed by the return value if there is one:
    ///     * [`TransactionOutput`] as `u8`
    /// * For each [`TransactionSlot`], whose type is [`TransactionSlotType::Address`]:
    ///     * [`Address`]
    /// * For each [`TransactionInput`], whose type is [`TransactionInputType::Value`]:
    ///     * Input size as little-endian `u32` followed by the input itself
    /// * If the last [`TransactionOutput`] is a return value:
    ///     * Its size as little-endian `u32`
    ///
    /// [`TransactionPayloadDecoder`]: crate::payload::decoder::TransactionPayloadDecoder
    /// [`TransactionPayloadCaller`]: crate::payload::caller::TransactionPayloadCaller
    /// [`TransactionSlotType::Address`]: crate::payload::TransactionSlotType::Address
    /// [`TransactionInputType::Value`]: crate::payload::TransactionInputType::Value
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn into_aligned_bytes(
        mut self,
    ) -> Result<OwnedAlignedBuffer, TransactionPayloadBuilderError<'static>> {
        for (output_index, declared_output) in (0..=u8::MAX).zip(self.declared_outputs()) {
            if declared_output.state == OutputState::NotWritten {
                return Err(TransactionPayloadBuilderError::UnusedOutput(output_index));
            }
        }

        // SAFETY: Constructor guarantees that capacity is a multiple of `Alignment::MAX` and is
        // never exceeded
        unsafe {
            self.ensure_alignment(Alignment::MAX).unwrap_unchecked();
        }

        Ok(self.payload)
    }

    /// Record that a method call writes `output`, which needs `capacity` bytes, and raise the
    /// capacity of its declared output in the header to that if it is smaller.
    ///
    /// # Safety
    /// The output index of `output` must refer to a declared output.
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    unsafe fn write_output(&mut self, output: TransactionOutput, capacity: u32) {
        let output_index = usize::from(output.output_index());
        // Capacities follow the number of declared outputs and 3 bytes of padding
        let offset = size_of::<u32>() * (1 + output_index);
        // SAFETY: The output is declared (function contract), and `Self::new()` declares at most
        // `MAX_OUTPUTS` outputs. `Self::new()` wrote a capacity for every declared output into the
        // header, and the payload only grows after that.
        unsafe {
            hint::assert_unchecked(
                output_index < MAX_OUTPUTS
                    && offset + size_of::<u32>() <= self.payload.len() as usize,
            );
        }

        let declared_output = self
            .declared_outputs
            .get_mut(output_index)
            .expect("Below `MAX_OUTPUTS`, as asserted above; qed");
        declared_output.capacity = declared_output.capacity.max(capacity);
        let kind = output.kind();
        let size = match kind {
            TransactionOutputKind::Output => 0,
            // The capacity that a return value needs is its size
            TransactionOutputKind::ReturnValue => capacity,
        };
        declared_output.set_written(kind, size);

        let declared_capacity = self.payload.as_mut_slice()[offset..]
            .first_chunk_mut::<{ size_of::<u32>() }>()
            .expect("The header has a capacity for every declared output; qed");
        *declared_capacity = declared_output.capacity.to_le_bytes();
    }

    /// Outputs that the payload declares
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn declared_outputs(&self) -> &[DeclaredOutput] {
        // SAFETY: `Self::new()` sets `num_outputs` to at most `MAX_OUTPUTS`, the length of
        // `declared_outputs`, and it never changes after that
        unsafe {
            self.declared_outputs
                .get_unchecked(..usize::from(self.num_outputs))
        }
    }

    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn extend_payload_with_alignment(
        &mut self,
        bytes: &[u8],
        alignment: Alignment,
    ) -> Result<(), TransactionPayloadBuilderError<'static>> {
        self.ensure_alignment(alignment)?;
        self.extend_payload(bytes)
    }

    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn ensure_alignment(
        &mut self,
        alignment: Alignment,
    ) -> Result<(), TransactionPayloadBuilderError<'static>> {
        let alignment = usize::from(alignment.get());

        let unaligned_by = self.payload.len() as usize % alignment;
        if unaligned_by > 0 {
            let padding_bytes = alignment - unaligned_by;
            // Byte by byte rather than as a slice of zeroes, which the compiler can't prove to be
            // in bounds
            for _ in 0..padding_bytes {
                self.push_payload_byte(0)?;
            }
        }

        Ok(())
    }

    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn push_payload_byte(
        &mut self,
        byte: u8,
    ) -> Result<(), TransactionPayloadBuilderError<'static>> {
        self.extend_payload(&[byte])
    }

    /// Append `bytes` to the payload if they fit into its capacity, it is never reallocated
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
