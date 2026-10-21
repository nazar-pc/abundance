//! Decoding of methods in a payload, shared by everything that reads payloads.
//!
//! [`PayloadDecoderCore`] applies every rule that depends only on the payload, and gives each
//! argument of a method to a [`MethodSink`] as soon as it is decoded. This way the code that calls
//! methods writes their `ExternalArgs` in the same pass. The core never creates pointers into
//! memory other than the payload.

#[cfg(test)]
mod tests;

use crate::payload::decoder::{DecodedInput, DecodedOutput, DecodedSlot, OutputRef};
use crate::payload::{
    MAX_OUTPUT_INDEX, OutputBufferDetails, TransactionInput, TransactionInputType,
    TransactionMethodContext, TransactionOutput, TransactionOutputKind,
    TransactionPayloadDecoderError, TransactionSlot, TransactionSlotType,
};
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::{AnyBitPattern, TrivialType};
use core::mem::MaybeUninit;
use core::slice;

/// Contract, fingerprint and context of a decoded method
#[derive(Debug, Copy, Clone)]
pub(crate) struct MethodHeader<'payload> {
    pub(crate) contract: &'payload Address,
    pub(crate) fingerprint: &'payload MethodFingerprint,
    pub(crate) context: TransactionMethodContext,
}

/// Receives the arguments of each method from [`PayloadDecoderCore::decode_next_method()`] in
/// payload order: slots, then inputs, then outputs.
///
/// The core checks the number of arguments of a method before it gives any of them to the sink, so
/// the sink gets at most [`MAX_TOTAL_METHOD_ARGS`] arguments per method. Slots and inputs only
/// refer to outputs of earlier methods, because the outputs of a method get their indices after
/// its slots and inputs are decoded.
pub(crate) trait MethodSink<'payload> {
    /// Receives a `#[slot]`.
    ///
    /// The sink must check the size of an output used as a slot with [`check_slot_output_size()`]
    /// as far as it knows the size: the size of a return value is declared in the payload, while
    /// the size of an `#[output]` is only known after its method was called.
    fn slot(&mut self, slot: DecodedSlot<'payload>) -> Result<(), TransactionPayloadDecoderError>;

    /// Receives an `#[input]`
    fn input(&mut self, input: DecodedInput<'payload>);

    /// Receives an `#[output]` or the return value. Returns the offset of its memory in the
    /// sink's output buffer, which the core keeps with its output index for later references.
    fn output(&mut self, output: DecodedOutput) -> Result<u32, TransactionPayloadDecoderError>;
}

/// Reads values from a payload one after another and skips padding before each of them
#[derive(Debug, Copy, Clone)]
struct PayloadReader<'payload> {
    /// The part of the payload that wasn't read yet. Padding is skipped based on its address,
    /// which works because the payload starts aligned to [`Alignment::MAX`].
    payload: &'payload [u8],
}

impl<'payload> PayloadReader<'payload> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn new(payload: &'payload [u128]) -> Self {
        debug_assert_eq!(align_of_val(payload), usize::from(Alignment::MAX.get()));

        // SAFETY: Memory is valid and bound by an argument's lifetime
        let payload =
            unsafe { slice::from_raw_parts(payload.as_ptr().cast::<u8>(), size_of_val(payload)) };

        Self { payload }
    }

    /// Number of bytes that weren't read yet
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn remaining_len(&self) -> usize {
        self.payload.len()
    }

    /// Read a reference to an [`AnyBitPattern`] value
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn read_trivial_type<T>(&mut self) -> Result<&'payload T, TransactionPayloadDecoderError>
    where
        T: AnyBitPattern,
    {
        self.ensure_alignment(Alignment::of::<T>())?;
        let bytes = self.read_unaligned_bytes(size_of::<T>())?;

        // SAFETY: `bytes` are aligned for `T`, initialized and have the size of `T` as ensured
        // above, so they are a valid value of `T`, which accepts every bit pattern
        // (`AnyBitPattern`), and the shared reference to them doesn't allow writing to the payload,
        // since `T` has no interior mutability (`TrivialType`)
        let value_ref = unsafe { bytes.as_ptr().cast::<T>().as_ref().expect("Not null; qed") };

        Ok(value_ref)
    }

    /// Read `size` bytes in place, after padding to `alignment`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn read_bytes(
        &mut self,
        size: u32,
        alignment: Alignment,
    ) -> Result<&'payload [u8], TransactionPayloadDecoderError> {
        self.ensure_alignment(alignment)?;

        self.read_unaligned_bytes(size as usize)
    }

    /// Read the number of tags, followed by the tags, which are returned in place
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn read_tags(&mut self) -> Result<&'payload [u8], TransactionPayloadDecoderError> {
        let num_tags = self.read_u8()?;

        self.read_unaligned_bytes(usize::from(num_tags))
    }

    /// Read [`TransactionMethodContext`] from its byte, which might not be a valid discriminant
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn read_method_context(
        &mut self,
    ) -> Result<TransactionMethodContext, TransactionPayloadDecoderError> {
        let value = self.read_u8()?;

        TransactionMethodContext::try_from(value)
            .map_err(|()| TransactionPayloadDecoderError::InvalidMethodContext(value))
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn read_u8(&mut self) -> Result<u8, TransactionPayloadDecoderError> {
        let value;
        (value, self.payload) = self
            .payload
            .split_first()
            .ok_or(TransactionPayloadDecoderError::PayloadTooSmall)?;

        Ok(*value)
    }

    /// Read `size` bytes in place, without padding
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn read_unaligned_bytes(
        &mut self,
        size: usize,
    ) -> Result<&'payload [u8], TransactionPayloadDecoderError> {
        let bytes;
        (bytes, self.payload) = self
            .payload
            .split_at_checked(size)
            .ok_or(TransactionPayloadDecoderError::PayloadTooSmall)?;

        Ok(bytes)
    }

    /// Skip padding up to `alignment`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn ensure_alignment(
        &mut self,
        alignment: Alignment,
    ) -> Result<(), TransactionPayloadDecoderError> {
        let alignment = usize::from(alignment.get());

        let misalignment = self.payload.as_ptr().addr() % alignment;
        if misalignment > 0 {
            let padding_bytes = alignment - misalignment;
            self.payload = self
                .payload
                .split_off(padding_bytes..)
                .ok_or(TransactionPayloadDecoderError::PayloadTooSmall)?;
        }
        Ok(())
    }
}

/// Decoder of methods in a payload, see the module description
#[derive(Debug)]
pub(crate) struct PayloadDecoderCore<'payload, 'details> {
    reader: PayloadReader<'payload>,
    output_buffer_details: &'details mut [MaybeUninit<OutputBufferDetails>],
    /// Number of output indices created so far, which never exceeds the length of
    /// `output_buffer_details`. Entries before it are initialized.
    num_outputs: usize,
}

impl<'payload, 'details> PayloadDecoderCore<'payload, 'details> {
    /// Create a new instance.
    ///
    /// The size of `output_buffer_details` defines how many `#[output]` arguments and return values
    /// could exist in all methods of the payload together. Only the first 128 entries are used,
    /// since tags refer to output indices up to 127.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub(crate) fn new(
        payload: &'payload [u128],
        mut output_buffer_details: &'details mut [MaybeUninit<OutputBufferDetails>],
    ) -> Self {
        // Tags refer to output indices up to 127, so later entries are left out. A shorter slice
        // is left as it is.
        let _unused_output_buffer_details =
            output_buffer_details.split_off_mut(usize::from(MAX_OUTPUT_INDEX) + 1..);

        Self {
            reader: PayloadReader::new(payload),
            output_buffer_details,
            num_outputs: 0,
        }
    }

    /// Decode the next method (if any) in the payload and give its arguments to `sink`.
    ///
    /// Returns `None` if there are no more methods in the payload. After an error, the core must
    /// not be used anymore.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub(crate) fn decode_next_method<Sink>(
        &mut self,
        sink: &mut Sink,
    ) -> Result<Option<MethodHeader<'payload>>, TransactionPayloadDecoderError>
    where
        Sink: MethodSink<'payload>,
    {
        if self.reader.remaining_len() <= usize::from(Alignment::MAX.get()) {
            return Ok(None);
        }

        let contract = self.reader.read_trivial_type::<Address>()?;
        let fingerprint = self.reader.read_trivial_type::<MethodFingerprint>()?;
        let context = self.reader.read_method_context()?;
        let slot_tags = self.reader.read_tags()?;
        let input_tags = self.reader.read_tags()?;
        let num_outputs = self.reader.read_u8()?;

        // Checked before any argument is given to the sink. This can be off by 1 due to `self` not
        // included in `ExternalArgs`, but it is good enough for this context.
        let number_of_arguments = slot_tags.len() + input_tags.len() + usize::from(num_outputs);
        if number_of_arguments > usize::from(MAX_TOTAL_METHOD_ARGS) {
            return Err(TransactionPayloadDecoderError::TooManyArguments(
                u8::try_from(number_of_arguments).unwrap_or(u8::MAX),
            ));
        }

        for &slot_tag in slot_tags {
            let slot_type = TransactionSlot::from_u8(slot_tag)
                .ok_or(TransactionPayloadDecoderError::InvalidSlotTag(slot_tag))?
                .slot_type();
            let slot = match slot_type {
                TransactionSlotType::Address => {
                    DecodedSlot::Address(self.reader.read_trivial_type::<Address>()?)
                }
                TransactionSlotType::OutputIndex { output_index } => {
                    DecodedSlot::Output(self.output_ref(output_index)?)
                }
            };
            sink.slot(slot)?;
        }

        for &input_tag in input_tags {
            let input_type = TransactionInput::from_u8(input_tag)
                .ok_or(TransactionPayloadDecoderError::InvalidInputTag(input_tag))?
                .input_type();
            let input = match input_type {
                TransactionInputType::Value { alignment } => {
                    let size = *self.reader.read_trivial_type::<u32>()?;
                    let bytes = self.reader.read_bytes(size, alignment)?;

                    DecodedInput::Value { bytes, alignment }
                }
                TransactionInputType::OutputIndex { output_index } => {
                    DecodedInput::Output(self.output_ref(output_index)?)
                }
            };
            sink.input(input);
        }

        if num_outputs > 0 {
            self.new_outputs(sink, usize::from(num_outputs))?;
        }

        Ok(Some(MethodHeader {
            contract,
            fingerprint,
            context,
        }))
    }

    /// Store the size that a method wrote into its `#[output]` with `output_index`.
    ///
    /// # Safety
    /// The output must exist: `output_index` was given to the sink in [`DecodedOutput`] by a call
    /// of [`Self::decode_next_method()`] that returned a method. `size` must not exceed the
    /// capacity of the output, and that many bytes of its memory must be initialized, as they are
    /// after the method returned `Ok(())`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub(crate) unsafe fn set_output_size(&mut self, output_index: u8, size: u32) {
        let output_index = usize::from(output_index);

        // SAFETY: The output exists, as guaranteed by the function contract, so its entry is
        // before `num_outputs`, which never exceeds the length of `output_buffer_details`, and
        // entries before it are initialized
        let output_buffer_details = unsafe {
            self.output_buffer_details
                .get_unchecked_mut(output_index)
                .assume_init_mut()
        };
        output_buffer_details.size = size;
    }

    /// Look up the output that a slot or an input of the current method refers to
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn output_ref(&self, output_index: u8) -> Result<OutputRef, TransactionPayloadDecoderError> {
        if usize::from(output_index) >= self.num_outputs {
            return Err(TransactionPayloadDecoderError::OutputIndexNotFound(
                output_index,
            ));
        }

        // SAFETY: Entries before `num_outputs` are initialized, and it never exceeds the length of
        // `output_buffer_details`
        let &OutputBufferDetails { offset, size, kind } = unsafe {
            self.output_buffer_details
                .get_unchecked(usize::from(output_index))
                .assume_init_ref()
        };

        Ok(OutputRef {
            output_index,
            kind,
            offset,
            size,
        })
    }

    /// Decode `num_outputs` outputs of the current method, give them to `sink` and create their
    /// output indices
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn new_outputs<Sink>(
        &mut self,
        sink: &mut Sink,
        num_outputs: usize,
    ) -> Result<(), TransactionPayloadDecoderError>
    where
        Sink: MethodSink<'payload>,
    {
        let first_output_index = self.num_outputs;
        // Checked once for all outputs of the method. Never underflows, since `self.num_outputs`
        // never exceeds the length of `output_buffer_details`.
        if num_outputs > self.output_buffer_details.len() - first_output_index {
            return Err(TransactionPayloadDecoderError::OutputBufferOffsetsTooSmall);
        }
        let end_output_index = first_output_index + num_outputs;
        // SAFETY: In bounds, as checked above
        let output_buffer_details = unsafe {
            self.output_buffer_details
                .get_unchecked_mut(first_output_index..end_output_index)
        };

        for (output_index, output_buffer_details) in
            (first_output_index..).zip(output_buffer_details)
        {
            let capacity = *self.reader.read_trivial_type::<u32>()?;
            let output_tag = self.reader.read_u8()?;
            let output = TransactionOutput::from_u8(output_tag)
                .ok_or(TransactionPayloadDecoderError::InvalidOutputTag(output_tag))?;

            let kind = output.kind();
            // A method has at most one return value, which is its last output, as in
            // `ExternalArgs`
            if kind == TransactionOutputKind::ReturnValue && output_index + 1 != end_output_index {
                return Err(TransactionPayloadDecoderError::ReturnValueNotLast(
                    // Doesn't truncate, `output_buffer_details` has at most 128 entries
                    output_index as u8,
                ));
            }
            let size = match kind {
                TransactionOutputKind::Output => 0,
                // Executors write the return value through its pointer, but not its size, which
                // `ExternalArgs` don't have, so it is set to the capacity right away, which is the
                // size of the return type
                TransactionOutputKind::ReturnValue => capacity,
            };
            let offset = sink.output(DecodedOutput {
                // Doesn't truncate, `output_buffer_details` has at most 128 entries
                output_index: output_index as u8,
                kind,
                capacity,
                size,
            })?;

            output_buffer_details.write(OutputBufferDetails { offset, size, kind });
        }
        self.num_outputs += num_outputs;

        Ok(())
    }
}

/// Check the size of an output that a slot refers to, which must hold an address.
///
/// `size` is the size of the output as far as the caller of this function knows it, see
/// [`MethodSink::slot()`].
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn check_slot_output_size(
    output: &OutputRef,
    size: u32,
) -> Result<(), TransactionPayloadDecoderError> {
    if size != Address::SIZE {
        return Err(TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
            output_index: output.output_index,
            size,
        });
    }

    Ok(())
}
