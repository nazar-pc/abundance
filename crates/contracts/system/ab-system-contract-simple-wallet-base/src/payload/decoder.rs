//! Checking a payload and describing its methods without calling them

#[cfg(test)]
mod tests;

use crate::payload::decoder_core::{
    MethodHeader, MethodSink, PayloadDecoderCore, check_slot_output_size,
};
use crate::payload::{
    OutputBufferDetails, TransactionMethodContext, TransactionOutputKind,
    TransactionPayloadDecoderError,
};
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use core::hint;
use core::mem::MaybeUninit;

/// Reference from a slot or an input to the value of an output of an earlier method
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[expect(
    clippy::partial_pub_fields,
    reason = "Offset and size are details of calling methods, see `Self::return_value_size()`"
)]
pub struct OutputRef {
    /// Index of the output
    pub mut(crate) output_index: u8,
    /// Whether the output is `#[output]` or a return value
    pub mut(crate) kind: TransactionOutputKind,
    /// Alignment of the output's memory, as the payload declares it
    pub mut(crate) alignment: Alignment,
    /// Offset of the output's memory, see [`OutputBufferDetails`]
    pub(crate) offset: u32,
    /// Size of the value, see [`OutputBufferDetails`]
    pub(crate) size: u32,
}

impl OutputRef {
    /// Size of a return value, which the payload declares.
    ///
    /// `None` for `#[output]`, whose size is only known after its method was called.
    #[inline(always)]
    pub const fn return_value_size(&self) -> Option<u32> {
        match self.kind {
            TransactionOutputKind::Output => None,
            TransactionOutputKind::ReturnValue => Some(self.size),
        }
    }
}

/// `#[slot]` argument
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DecodedSlot<'payload> {
    /// Explicit address in the payload
    Address(&'payload Address),
    /// Contents of an output of an earlier method, whose memory is declared with at least the
    /// alignment of an address. A return value is declared with the size of an address.
    Output(OutputRef),
}

/// `#[input]` argument
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DecodedInput<'payload> {
    /// Explicit value in the payload, whose bytes are aligned to `alignment`
    Value {
        bytes: &'payload [u8],
        alignment: Alignment,
    },
    /// Contents of an output of an earlier method
    Output(OutputRef),
}

/// `#[output]` or return value, which gets the next output index
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[expect(
    clippy::partial_pub_fields,
    reason = "The size is only meaningful for a return value, see `Self::return_value_size()`"
)]
pub struct DecodedOutput {
    /// Index of the output, which later methods refer to
    pub mut(crate) output_index: u8,
    /// Whether the output is `#[output]` or a return value
    pub mut(crate) kind: TransactionOutputKind,
    /// Alignment of the output's memory
    pub mut(crate) alignment: Alignment,
    /// Capacity of the output's memory
    pub mut(crate) capacity: u32,
    /// Size that the method gets: `0` for `#[output]`, the capacity for the return value
    pub(crate) size: u32,
}

impl DecodedOutput {
    /// Size of a return value, which is its capacity, since executors write the whole value.
    ///
    /// `None` for `#[output]`, which the method gets empty.
    #[inline(always)]
    pub const fn return_value_size(&self) -> Option<u32> {
        match self.kind {
            TransactionOutputKind::Output => None,
            TransactionOutputKind::ReturnValue => Some(self.size),
        }
    }
}

/// Placeholder for unused entries of [`DecodedArguments::slots`]
const UNUSED_SLOT: DecodedSlot<'static> = DecodedSlot::Address(&Address::NULL);
/// Placeholder for unused entries of [`DecodedArguments::inputs`]
const UNUSED_INPUT: DecodedInput<'static> = DecodedInput::Value {
    bytes: &[],
    alignment: Alignment::of::<u8>(),
};
/// Placeholder for unused entries of [`DecodedArguments::outputs`]
const UNUSED_OUTPUT: DecodedOutput = DecodedOutput {
    output_index: 0,
    kind: TransactionOutputKind::Output,
    alignment: Alignment::of::<u8>(),
    capacity: 0,
    size: 0,
};

/// Arguments of a method, which are collected as they are decoded
#[derive(Debug)]
struct DecodedArguments<'payload> {
    /// Entries before `num_slots` are arguments of the method
    slots: [DecodedSlot<'payload>; MAX_TOTAL_METHOD_ARGS as usize],
    num_slots: u8,
    /// Entries before `num_inputs` are arguments of the method
    inputs: [DecodedInput<'payload>; MAX_TOTAL_METHOD_ARGS as usize],
    num_inputs: u8,
    /// Entries before `num_outputs` are arguments of the method
    outputs: [DecodedOutput; MAX_TOTAL_METHOD_ARGS as usize],
    num_outputs: u8,
}

// The core gives at most `MAX_TOTAL_METHOD_ARGS` arguments per method to the sink, so there is
// always a free entry
impl<'payload> MethodSink<'payload> for DecodedArguments<'payload> {
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn slot(&mut self, slot: DecodedSlot<'payload>) -> Result<(), TransactionPayloadDecoderError> {
        // The size of a return value is declared in the payload, while the size of an `#[output]`
        // is only known after its method was called
        if let DecodedSlot::Output(output) = &slot
            && let Some(size) = output.return_value_size()
        {
            check_slot_output_size(output, size)?;
        }

        if let Some(entry) = self.slots.get_mut(usize::from(self.num_slots)) {
            *entry = slot;
            self.num_slots += 1;
        }

        Ok(())
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn input(&mut self, input: DecodedInput<'payload>) {
        if let Some(entry) = self.inputs.get_mut(usize::from(self.num_inputs)) {
            *entry = input;
            self.num_inputs += 1;
        }
    }

    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn output(&mut self, output: DecodedOutput) -> Result<u32, TransactionPayloadDecoderError> {
        if let Some(entry) = self.outputs.get_mut(usize::from(self.num_outputs)) {
            *entry = output;
            self.num_outputs += 1;
        }

        // There is no output buffer, so all outputs have offset `0`
        Ok(0)
    }
}

impl<'payload> DecodedArguments<'payload> {
    #[inline(always)]
    fn new() -> Self {
        Self {
            slots: [UNUSED_SLOT; MAX_TOTAL_METHOD_ARGS as usize],
            num_slots: 0,
            inputs: [UNUSED_INPUT; MAX_TOTAL_METHOD_ARGS as usize],
            num_inputs: 0,
            outputs: [UNUSED_OUTPUT; MAX_TOTAL_METHOD_ARGS as usize],
            num_outputs: 0,
        }
    }

    /// Forget the arguments of the previous method
    #[inline(always)]
    fn clear(&mut self) {
        self.num_slots = 0;
        self.num_inputs = 0;
        self.num_outputs = 0;
    }

    /// Slots of the method
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn slots(&self) -> &[DecodedSlot<'payload>] {
        // SAFETY: The fields are private to this module, where `num_slots` is only set to `0` and
        // incremented by `Self::slot()` after it wrote an entry before the end of `slots`, so it
        // never exceeds the length of `slots`
        unsafe {
            hint::assert_unchecked(usize::from(self.num_slots) <= self.slots.len());
        }

        self.slots
            .get(..usize::from(self.num_slots))
            .expect("Within the length of `slots`, as asserted above; qed")
    }

    /// Inputs of the method
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn inputs(&self) -> &[DecodedInput<'payload>] {
        // SAFETY: The fields are private to this module, where `num_inputs` is only set to `0` and
        // incremented by `Self::input()` after it wrote an entry before the end of `inputs`, so it
        // never exceeds the length of `inputs`
        unsafe {
            hint::assert_unchecked(usize::from(self.num_inputs) <= self.inputs.len());
        }

        self.inputs
            .get(..usize::from(self.num_inputs))
            .expect("Within the length of `inputs`, as asserted above; qed")
    }

    /// Outputs of the method
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn outputs(&self) -> &[DecodedOutput] {
        // SAFETY: The fields are private to this module, where `num_outputs` is only set to `0`
        // and incremented by `Self::output()` after it wrote an entry before the end of `outputs`,
        // so it never exceeds the length of `outputs`
        unsafe {
            hint::assert_unchecked(usize::from(self.num_outputs) <= self.outputs.len());
        }

        self.outputs
            .get(..usize::from(self.num_outputs))
            .expect("Within the length of `outputs`, as asserted above; qed")
    }
}

/// One method of the payload, see [`TransactionPayloadDecoder::next_method()`].
///
/// Arguments of each kind are in payload order, which is the order in which the method declares
/// them. So the `n`-th slot is the `n`-th `#[slot]` in the method's metadata, and the same holds
/// for inputs and outputs. A method has at most one return value, which is its last output, as
/// the decoder checks.
#[derive(Debug, Copy, Clone)]
pub struct DecodedMethod<'decoder, 'payload> {
    header: MethodHeader<'payload>,
    arguments: &'decoder DecodedArguments<'payload>,
}

impl<'decoder, 'payload> DecodedMethod<'decoder, 'payload> {
    /// Contract to call
    #[inline(always)]
    pub fn contract(&self) -> &'payload Address {
        self.header.contract
    }

    /// Fingerprint of the method to call
    #[inline(always)]
    pub fn fingerprint(&self) -> &'payload MethodFingerprint {
        self.header.fingerprint
    }

    /// Context to call the method with
    #[inline(always)]
    pub fn context(&self) -> TransactionMethodContext {
        self.header.context
    }

    /// `#[slot]` arguments
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn slots(&self) -> &'decoder [DecodedSlot<'payload>] {
        self.arguments.slots()
    }

    /// `#[input]` arguments
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn inputs(&self) -> &'decoder [DecodedInput<'payload>] {
        self.arguments.inputs()
    }

    /// `#[output]` arguments and the return value
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn outputs(&self) -> &'decoder [DecodedOutput] {
        self.arguments.outputs()
    }

    /// The return value, which is the last output if there is one
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn return_value(&self) -> Option<&'decoder DecodedOutput> {
        self.outputs()
            .last()
            .filter(|output| output.kind == TransactionOutputKind::ReturnValue)
    }
}

/// Decoder that checks a payload created using `TransactionPayloadBuilder` and describes its
/// methods without calling them, for example to show a transaction to the user before it is
/// signed.
///
/// It runs the same decoding code as [`TransactionPayloadCaller`], so it applies every rule that
/// depends only on the payload. A payload that it accepts can still be rejected by the caller in
/// two cases, which depend on execution: the outputs don't fit into the output buffer of the
/// caller, or an `#[output]` that a later method uses as a slot doesn't hold an address after its
/// method was called. The caller checks these two cases when it reaches them. So when the caller
/// rejects a payload for one of them, the decoder either accepts the payload or rejects it with an
/// error from later in the payload.
///
/// Descriptions contain no pointers into memory other than the payload, so any payload can be
/// decoded safely.
///
/// [`TransactionPayloadCaller`]: crate::payload::TransactionPayloadCaller
#[derive(Debug)]
pub struct TransactionPayloadDecoder<'payload, 'details> {
    core: PayloadDecoderCore<'payload, 'details>,
    /// Arguments of the method that was decoded last
    arguments: DecodedArguments<'payload>,
    /// The core must not be used anymore after an error
    failed: bool,
}

impl<'payload, 'details> TransactionPayloadDecoder<'payload, 'details> {
    /// Create a new instance.
    ///
    /// The size of `output_buffer_details` defines how many `#[output]` arguments and return values
    /// could exist in all methods of the payload together, up to 128, since tags refer to output
    /// indices up to 127. With [`OUTPUT_BUFFER_OFFSETS_SIZE`] entries, the decoder rejects payloads
    /// with more outputs than [`SimpleWalletBase::execute()`] accepts.
    ///
    /// [`OUTPUT_BUFFER_OFFSETS_SIZE`]: crate::OUTPUT_BUFFER_OFFSETS_SIZE
    /// [`SimpleWalletBase::execute()`]: crate::SimpleWalletBase::execute
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn new(
        payload: &'payload [u128],
        output_buffer_details: &'details mut [MaybeUninit<OutputBufferDetails>],
    ) -> Self {
        Self {
            core: PayloadDecoderCore::new(payload, output_buffer_details),
            arguments: DecodedArguments::new(),
            failed: false,
        }
    }

    /// Decode and check the next method, `None` if there are no more methods in the payload.
    ///
    /// The method is returned only after all of it was checked. Its arguments are stored in the
    /// decoder and replaced by the next call. After an error, every call returns
    /// [`TransactionPayloadDecoderError::FailedEarlier`].
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn next_method(
        &mut self,
    ) -> Result<Option<DecodedMethod<'_, 'payload>>, TransactionPayloadDecoderError> {
        if self.failed {
            return Err(TransactionPayloadDecoderError::FailedEarlier);
        }

        self.arguments.clear();
        match self.core.decode_next_method(&mut self.arguments) {
            Ok(Some(header)) => Ok(Some(DecodedMethod {
                header,
                arguments: &self.arguments,
            })),
            Ok(None) => Ok(None),
            Err(error) => {
                self.failed = true;
                Err(error)
            }
        }
    }
}
