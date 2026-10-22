//! Decoding of methods in a payload, shared by everything that reads payloads.
//!
//! [`PayloadDecoderCore`] applies every rule that depends only on the payload, and gives each
//! argument of a method to a [`MethodSink`] as soon as it is decoded. This way the code that calls
//! methods writes their `ExternalArgs` in the same pass. The core never creates pointers into
//! memory other than the payload.

#[cfg(test)]
mod tests;

use crate::payload::{
    OutputBufferDetails, TransactionInput, TransactionInputType, TransactionMethodContext,
    TransactionOutput, TransactionOutputKind, TransactionOutputType,
    TransactionPayloadDecoderError, TransactionSlot, TransactionSlotType,
};
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::AnyBitPattern;
use core::mem::MaybeUninit;
use core::slice;

/// Contract, fingerprint and context of a decoded method
#[derive(Debug, Copy, Clone)]
pub(crate) struct MethodHeader<'payload> {
    pub(crate) contract: &'payload Address,
    pub(crate) fingerprint: &'payload MethodFingerprint,
    pub(crate) context: TransactionMethodContext,
}

/// Reference from a slot or an input to an output of an earlier method
#[derive(Debug, Copy, Clone)]
pub(crate) struct OutputRef {
    pub(crate) output_index: u8,
    /// What the sink returned from [`MethodSink::output()`] for the output
    pub(crate) offset: u32,
    /// Size of the value, see [`OutputBufferDetails`]
    pub(crate) size: u32,
}

/// `#[slot]` argument
#[derive(Debug, Copy, Clone)]
pub(crate) enum DecodedSlot<'payload> {
    /// Explicit address in the payload
    Address(&'payload Address),
    /// Contents of an output of an earlier method
    Output(OutputRef),
}

/// `#[input]` argument
#[derive(Debug, Copy, Clone)]
pub(crate) enum DecodedInput<'payload> {
    /// Explicit value in the payload, aligned as the payload declares
    Value { bytes: &'payload [u8] },
    /// Contents of an output of an earlier method
    Output(OutputRef),
}

/// `#[output]` or return value, which gets the next output index
#[derive(Debug, Copy, Clone)]
pub(crate) struct DecodedOutput {
    /// Index of the output in `output_buffer_details`
    pub(crate) output_index: usize,
    pub(crate) kind: TransactionOutputKind,
    pub(crate) alignment: Alignment,
    pub(crate) capacity: u32,
    /// Size that the method gets: `0` for `#[output]`, the capacity for the return value
    pub(crate) size: u32,
}

/// Receives the arguments of each method from [`PayloadDecoderCore::decode_next_method()`] in
/// payload order: slots, then inputs, then outputs.
///
/// The core checks the number of arguments of a method before it gives any of them to the sink, so
/// the sink gets at most [`MAX_TOTAL_METHOD_ARGS`] arguments per method. Slots and inputs only
/// refer to outputs of earlier methods, because the outputs of a method get their indices after
/// its slots and inputs are decoded.
pub(crate) trait MethodSink<'payload> {
    /// Receives a `#[slot]`
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
    /// could exist in all methods of the payload together.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub(crate) fn new(
        payload: &'payload [u128],
        output_buffer_details: &'details mut [MaybeUninit<OutputBufferDetails>],
    ) -> Self {
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
            let slot = match TransactionSlot::from_u8(slot_tag).slot_type() {
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
                .ok_or(TransactionPayloadDecoderError::AlignmentPowerTooLarge(
                    input_tag & 0b0111_1111,
                ))?
                .input_type();
            let input = match input_type {
                TransactionInputType::Value { alignment } => {
                    let size = *self.reader.read_trivial_type::<u32>()?;
                    let bytes = self.reader.read_bytes(size, alignment)?;

                    DecodedInput::Value { bytes }
                }
                TransactionInputType::OutputIndex { output_index } => {
                    DecodedInput::Output(self.output_ref(output_index)?)
                }
            };
            sink.input(input);
        }

        for _ in 0..num_outputs {
            let capacity = *self.reader.read_trivial_type::<u32>()?;
            let output_tag = self.reader.read_u8()?;
            let output_type = TransactionOutput::from_u8(output_tag)
                .ok_or(TransactionPayloadDecoderError::AlignmentPowerTooLarge(
                    output_tag & 0b0111_1111,
                ))?
                .output_type();
            let (kind, alignment) = match output_type {
                TransactionOutputType::Output { alignment } => {
                    (TransactionOutputKind::Output, alignment)
                }
                TransactionOutputType::ReturnValue { alignment } => {
                    (TransactionOutputKind::ReturnValue, alignment)
                }
            };

            self.new_output(sink, kind, alignment, capacity)?;
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
        let OutputBufferDetails { offset, size } = unsafe {
            self.output_buffer_details
                .get_unchecked(usize::from(output_index))
                .assume_init()
        };

        Ok(OutputRef {
            output_index,
            offset,
            size,
        })
    }

    /// Give a new output of the current method to `sink` and create its output index
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn new_output<Sink>(
        &mut self,
        sink: &mut Sink,
        kind: TransactionOutputKind,
        alignment: Alignment,
        capacity: u32,
    ) -> Result<(), TransactionPayloadDecoderError>
    where
        Sink: MethodSink<'payload>,
    {
        let output_index = self.num_outputs;
        let Some(output_buffer_details) = self.output_buffer_details.get_mut(output_index) else {
            return Err(TransactionPayloadDecoderError::OutputBufferOffsetsTooSmall);
        };

        let size = match kind {
            TransactionOutputKind::Output => 0,
            // Executors write the return value through its pointer, but not its size, which
            // `ExternalArgs` don't have, so it is set to the capacity right away, which is the size
            // of the return type
            TransactionOutputKind::ReturnValue => capacity,
        };
        let offset = sink.output(DecodedOutput {
            output_index,
            kind,
            alignment,
            capacity,
            size,
        })?;

        output_buffer_details.write(OutputBufferDetails { offset, size });
        self.num_outputs += 1;

        Ok(())
    }
}
