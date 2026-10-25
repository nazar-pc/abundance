//! Decoding of methods in a payload, shared by everything that reads payloads.
//!
//! [`PayloadDecoderCore`] applies every rule that depends only on the payload, and gives each
//! argument of a method to a [`MethodSink`] as soon as it is decoded. This way the code that calls
//! methods writes their `ExternalArgs` in the same pass. The core never creates pointers into
//! memory other than the payload.
//!
//! The rules about references to declared outputs are separate functions, [`output_ref()`] and
//! [`slot_output_ref()`], which `TransactionPayloadBuilder` applies to the method calls that it
//! adds as well, together with [`writes_output()`] for the rules about writing them.

#[cfg(test)]
mod tests;

use crate::MAX_OUTPUTS;
use crate::payload::decoder::{DecodedInput, DecodedOutput, DecodedSlot, OutputRef};
use crate::payload::{
    DeclaredOutput, OutputState, TransactionInput, TransactionInputType, TransactionMethodContext,
    TransactionOutput, TransactionOutputKind, TransactionPayloadDecoderError, TransactionSlot,
    TransactionSlotType,
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
/// refer to declared outputs that earlier methods wrote and that the method doesn't write, and the
/// outputs of a method are different declared outputs. A slot only refers to a return value that
/// has the size of an address.
pub(crate) trait MethodSink<'payload> {
    /// Receives a `#[slot]`.
    ///
    /// The core checks the size of a return value used as a slot, which the payload declares. The
    /// size of an `#[output]` is only known after its method was called, so a sink that calls
    /// methods must check the size of an `#[output]` used as a slot with
    /// [`check_slot_output_size()`].
    fn slot(&mut self, slot: DecodedSlot<'payload>) -> Result<(), TransactionPayloadDecoderError>;

    /// Receives an `#[input]`
    fn input(&mut self, input: DecodedInput<'payload>);

    /// Receives an `#[output]` or the return value, with `offset` of its declared output's memory
    /// from [`OutputAllocator::alloc()`]
    fn output(&mut self, output: DecodedOutput, offset: u32);
}

/// Allocates the memory of declared outputs, see [`PayloadDecoderCore::new()`]
pub(crate) trait OutputAllocator {
    /// Allocate memory of `capacity` bytes for the next declared output, returns its offset
    fn alloc(&mut self, capacity: u32) -> Result<u32, TransactionPayloadDecoderError>;
}

/// Reads values from a payload one after another and skips padding before each of them
#[derive(Debug, Copy, Clone)]
struct PayloadReader<'payload> {
    /// The part of the payload that wasn't read yet. Padding is skipped based on its address,
    /// which works because the payload starts aligned to [`Alignment::MAX`].
    payload: &'payload [u8],
}

impl<'payload> PayloadReader<'payload> {
    /// Create a reader of `payload`, which must be aligned to [`Alignment::MAX`]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn new(payload: &'payload [u128]) -> Result<Self, TransactionPayloadDecoderError> {
        if !Alignment::MAX.is_slice_aligned(payload) {
            return Err(TransactionPayloadDecoderError::PayloadMisaligned);
        }

        // SAFETY: Memory is valid and bound by an argument's lifetime
        let payload =
            unsafe { slice::from_raw_parts(payload.as_ptr().cast::<u8>(), size_of_val(payload)) };

        Ok(Self { payload })
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

    /// Read `len` [`AnyBitPattern`] values in place
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn read_trivial_slice<T>(
        &mut self,
        len: u8,
    ) -> Result<&'payload [T], TransactionPayloadDecoderError>
    where
        T: AnyBitPattern,
    {
        self.ensure_alignment(Alignment::of::<T>())?;
        let bytes = self.read_unaligned_bytes(usize::from(len) * size_of::<T>())?;

        // SAFETY: `bytes` are aligned for `T`, initialized and have the size of `len` values of
        // `T` as ensured above, so they are valid values of `T`, which accepts every bit pattern
        // (`AnyBitPattern`), and the shared reference to them doesn't allow writing to the payload,
        // since `T` has no interior mutability (`TrivialType`)
        let values = unsafe { slice::from_raw_parts(bytes.as_ptr().cast::<T>(), usize::from(len)) };

        Ok(values)
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
pub(crate) struct PayloadDecoderCore<'payload, 'outputs> {
    reader: PayloadReader<'payload>,
    /// Exactly the outputs that the payload declares
    declared_outputs: &'outputs mut [DeclaredOutput],
}

impl<'payload, 'outputs> PayloadDecoderCore<'payload, 'outputs> {
    /// Create a new instance and read the outputs that the payload declares into
    /// `declared_outputs`, where `allocator` allocates their memory one after another.
    ///
    /// Every payload starts with the number of declared outputs, so an empty payload is too small.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub(crate) fn new<Allocator>(
        payload: &'payload [u128],
        declared_outputs: &'outputs mut [MaybeUninit<DeclaredOutput>; MAX_OUTPUTS],
        allocator: &mut Allocator,
    ) -> Result<Self, TransactionPayloadDecoderError>
    where
        Allocator: OutputAllocator,
    {
        let mut reader = PayloadReader::new(payload)?;

        let num_declared_outputs = reader.read_u8()?;
        let Some(declared_outputs) = declared_outputs.get_mut(..usize::from(num_declared_outputs))
        else {
            return Err(TransactionPayloadDecoderError::TooManyDeclaredOutputs(
                num_declared_outputs,
            ));
        };
        // All capacities are read before any of them is allocated, so a payload that is too small
        // is rejected the same way with and without an output buffer
        let capacities = reader.read_trivial_slice::<u32>(num_declared_outputs)?;

        for (declared_output, &capacity) in declared_outputs.iter_mut().zip(capacities) {
            declared_output.write(DeclaredOutput {
                offset: allocator.alloc(capacity)?,
                capacity,
                size: 0,
                state: OutputState::NotWritten,
            });
        }
        // SAFETY: `capacities` has as many elements as `declared_outputs`, so every entry was
        // written above
        let declared_outputs = unsafe { declared_outputs.assume_init_mut() };

        Ok(Self {
            reader,
            declared_outputs,
        })
    }

    /// Outputs that the payload declares
    #[inline(always)]
    pub(crate) fn declared_outputs(&self) -> &[DeclaredOutput] {
        self.declared_outputs
    }

    /// Decode the next method (if any) in the payload and give its arguments to `sink`.
    ///
    /// Slots and inputs are checked and given to `sink` before outputs, so for a method with an
    /// invalid slot or input and an invalid output, the error is about the slot or input.
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
        let output_tags = self.reader.read_tags()?;

        // Checked before any argument is given to the sink. `self`, `#[env]` and `#[tmp]` are not
        // in `ExternalArgs`, so they are not counted here, but it is good enough for this context.
        // Wallet buffers only hold arguments that are in `ExternalArgs`, and the executor checks
        // the exact number of arguments with metadata of the method.
        let number_of_arguments = slot_tags.len() + input_tags.len() + output_tags.len();
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
                TransactionSlotType::OutputIndex { output_index } => DecodedSlot::Output(
                    slot_output_ref(self.declared_outputs, output_index, output_tags)?,
                ),
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
                TransactionInputType::OutputIndex { output_index } => DecodedInput::Output(
                    output_ref(self.declared_outputs, output_index, output_tags)?,
                ),
            };
            sink.input(input);
        }

        if !output_tags.is_empty() {
            self.write_outputs(sink, output_tags)?;
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
        // SAFETY: The output exists, as guaranteed by the function contract, so it is one of the
        // declared outputs
        let declared_output = unsafe {
            self.declared_outputs
                .get_unchecked_mut(usize::from(output_index))
        };
        declared_output.size = size;
    }

    /// Decode the outputs of the current method from `output_tags`, write them into their declared
    /// outputs and give them to `sink`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn write_outputs<Sink>(
        &mut self,
        sink: &mut Sink,
        mut output_tags: &'payload [u8],
    ) -> Result<(), TransactionPayloadDecoderError>
    where
        Sink: MethodSink<'payload>,
    {
        while let Some((&output_tag, later_output_tags)) = output_tags.split_first() {
            output_tags = later_output_tags;

            let output = TransactionOutput::from_u8(output_tag)
                .ok_or(TransactionPayloadDecoderError::InvalidOutputTag(output_tag))?;
            let output_index = output.output_index();
            let kind = output.kind();
            // `TransactionPayloadBuilder` applies the same rules to the method calls that it adds.
            // They stay here instead of in a function shared with it, since that made decoding
            // outputs slower.
            let Some(declared_output) = self.declared_outputs.get_mut(usize::from(output_index))
            else {
                return Err(TransactionPayloadDecoderError::OutputIndexNotFound(
                    output_index,
                ));
            };
            // A method has at most one return value, which is its last output, as in
            // `ExternalArgs`
            if kind == TransactionOutputKind::ReturnValue && !later_output_tags.is_empty() {
                return Err(TransactionPayloadDecoderError::ReturnValueNotLast(
                    output_index,
                ));
            }
            // The method would get two pointers to the same memory. Most methods have at most one
            // output, then nothing is compared.
            if !later_output_tags.is_empty() && writes_output(later_output_tags, output_index) {
                return Err(TransactionPayloadDecoderError::OutputUsedTwice(
                    output_index,
                ));
            }

            let size = match kind {
                // Given to the method empty, whatever was written into the output before
                TransactionOutputKind::Output => 0,
                // Executors write the return value through its pointer, but not its size, which
                // `ExternalArgs` don't have, so the payload declares it as the size of the return
                // type. It takes effect for later methods only: nothing reads it while the current
                // method is called, since the method doesn't refer to its own outputs.
                TransactionOutputKind::ReturnValue => {
                    let size = *self.reader.read_trivial_type::<u32>()?;
                    if size > declared_output.capacity {
                        return Err(TransactionPayloadDecoderError::ReturnValueTooLarge {
                            output_index,
                            size,
                        });
                    }
                    size
                }
            };
            declared_output.set_written(kind, size);

            sink.output(
                DecodedOutput {
                    output_index,
                    kind,
                    capacity: declared_output.capacity,
                    size,
                },
                declared_output.offset,
            );
        }

        Ok(())
    }
}

/// Look up the declared output that a slot or an input of a method with `output_tags` refers to,
/// which an earlier method must have written and which the method must not write.
///
/// `declared_outputs` describes what earlier methods wrote. [`PayloadDecoderCore`] and
/// `TransactionPayloadBuilder` check references with it, so that they agree.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn output_ref(
    declared_outputs: &[DeclaredOutput],
    output_index: u8,
    output_tags: &[u8],
) -> Result<OutputRef, TransactionPayloadDecoderError> {
    let Some(&DeclaredOutput {
        offset,
        capacity: _,
        size,
        state,
    }) = declared_outputs.get(usize::from(output_index))
    else {
        return Err(TransactionPayloadDecoderError::OutputIndexNotFound(
            output_index,
        ));
    };
    let kind = match state {
        OutputState::NotWritten => {
            return Err(TransactionPayloadDecoderError::OutputIndexNotFound(
                output_index,
            ));
        }
        OutputState::Output => TransactionOutputKind::Output,
        OutputState::ReturnValue => TransactionOutputKind::ReturnValue,
    };
    // The method would get a pointer to memory that it also writes through another pointer. Most
    // methods have no outputs, then nothing is compared.
    if !output_tags.is_empty() && writes_output(output_tags, output_index) {
        return Err(TransactionPayloadDecoderError::OutputReferencedBySameMethod(output_index));
    }

    Ok(OutputRef {
        output_index,
        kind,
        offset,
        size,
    })
}

/// Look up the declared output that a slot of a method with `output_tags` refers to, see
/// [`output_ref()`], which must hold an address if it is a return value
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn slot_output_ref(
    declared_outputs: &[DeclaredOutput],
    output_index: u8,
    output_tags: &[u8],
) -> Result<OutputRef, TransactionPayloadDecoderError> {
    let output = output_ref(declared_outputs, output_index, output_tags)?;
    // The size of a return value is declared in the payload, while the size of an `#[output]` is
    // only known after its method was called, see `MethodSink`
    if output.kind == TransactionOutputKind::ReturnValue {
        check_slot_output_size(&output)?;
    }

    Ok(output)
}

/// Whether one of `output_tags` writes the output with `output_index`, which is the index of a
/// declared output.
///
/// The tags may be invalid, since decoders check a tag when they write its output: an invalid tag
/// holds an index of [`MAX_OUTPUTS`] or more, which is never the index of a declared output.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn writes_output(output_tags: &[u8], output_index: u8) -> bool {
    output_tags
        .iter()
        .any(|&output_tag| TransactionOutput::tag_output_index(output_tag) == output_index)
}

/// Check the size of an output that a slot refers to, which must hold an address
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn check_slot_output_size(
    output: &OutputRef,
) -> Result<(), TransactionPayloadDecoderError> {
    if output.size != Address::SIZE {
        return Err(TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
            output_index: output.output_index,
            size: output.size,
        });
    }

    Ok(())
}
