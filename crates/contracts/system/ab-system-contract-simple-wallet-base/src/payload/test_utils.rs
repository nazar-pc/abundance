//! Helpers shared by tests of the payload modules

use crate::payload::caller::CallPrepared;
use crate::payload::decoder::{
    DecodedInput, DecodedOutput, DecodedSlot, TransactionPayloadDecoder,
};
use crate::payload::{
    TransactionInput, TransactionMethodContext, TransactionOutput, TransactionOutputKind,
    TransactionPayloadCallError, TransactionPayloadCaller, TransactionPayloadDecoderError,
    TransactionSlot,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::MethodFingerprint;
use ab_contracts_common::{ContractError, MAX_TOTAL_METHOD_ARGS};
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;
use core::ptr;

/// More entries than any test needs, of which tests use a part
pub(crate) const MAX_OUTPUT_BUFFER_DETAILS: usize = 256;

/// Calls prepared methods with a function instead of the host
pub(crate) struct CallWith<Call>(Call);

impl<Call> CallWith<Call>
where
    Call: Fn(PreparedMethod<'_>) -> Result<(), ContractError>,
{
    /// Call methods with `call`
    pub(crate) fn new(call: Call) -> Self {
        Self(call)
    }
}

// SAFETY: Functions in tests write outputs and return values that later methods refer to the way
// `ExecutorContext` requires, or tests zero the output buffer, so that return values are
// initialized. Other `#[output]` arguments keep size `0` that they were given.
unsafe impl<Call> CallPrepared for CallWith<Call>
where
    Call: Fn(PreparedMethod<'_>) -> Result<(), ContractError>,
{
    fn call_prepared(&self, method: PreparedMethod<'_>) -> Result<(), ContractError> {
        (self.0)(method)
    }
}

/// [`Alignment`] of `bytes`, which must be a power of two up to `Alignment::MAX`
pub(crate) fn alignment(bytes: u8) -> Alignment {
    assert!(bytes.is_power_of_two(), "{bytes} is not a power of two");
    let Some(alignment) = u8::try_from(bytes.ilog2())
        .ok()
        .and_then(Alignment::from_power)
    else {
        panic!("{bytes} is larger than `Alignment::MAX`");
    };

    alignment
}

/// Slot tag in a [`TestPayload`]
#[derive(Debug, Copy, Clone)]
pub(crate) enum SlotTag {
    /// The byte of a [`TransactionSlot`]
    Valid(TransactionSlot),
    /// A byte that [`TransactionSlot::from_u8()`] rejects
    Invalid(u8),
}

impl SlotTag {
    /// Explicit slot address
    pub(crate) fn address() -> Self {
        Self::Valid(TransactionSlot::new_address())
    }

    /// Slot from the output with `output_index`
    pub(crate) fn output_index(output_index: u8) -> Self {
        Self::Valid(TransactionSlot::new_output_index(output_index).unwrap())
    }

    /// `byte` as a valid tag if [`TransactionSlot::from_u8()`] accepts it, an invalid one
    /// otherwise
    pub(crate) fn from_byte(byte: u8) -> Self {
        match TransactionSlot::from_u8(byte) {
            Some(slot) => Self::Valid(slot),
            None => Self::Invalid(byte),
        }
    }

    fn into_u8(self) -> u8 {
        match self {
            Self::Valid(slot) => slot.into_u8(),
            Self::Invalid(byte) => {
                assert!(
                    TransactionSlot::from_u8(byte).is_none(),
                    "Slot tag {byte:#010b} is valid"
                );
                byte
            }
        }
    }
}

/// Input tag in a [`TestPayload`]
#[derive(Debug, Copy, Clone)]
pub(crate) enum InputTag {
    /// The byte of a [`TransactionInput`]
    Valid(TransactionInput),
    /// A byte that [`TransactionInput::from_u8()`] rejects
    Invalid(u8),
}

impl InputTag {
    /// Explicit value with an alignment of `bytes`
    pub(crate) fn value(bytes: u8) -> Self {
        Self::Valid(TransactionInput::new_value(alignment(bytes)))
    }

    /// Input from the output with `output_index`
    pub(crate) fn output_index(output_index: u8) -> Self {
        Self::Valid(TransactionInput::new_output_index(output_index).unwrap())
    }

    /// `byte` as a valid tag if [`TransactionInput::from_u8()`] accepts it, an invalid one
    /// otherwise
    pub(crate) fn from_byte(byte: u8) -> Self {
        match TransactionInput::from_u8(byte) {
            Some(input) => Self::Valid(input),
            None => Self::Invalid(byte),
        }
    }

    fn into_u8(self) -> u8 {
        match self {
            Self::Valid(input) => input.into_u8(),
            Self::Invalid(byte) => {
                assert!(
                    TransactionInput::from_u8(byte).is_none(),
                    "Input tag {byte:#010b} is valid"
                );
                byte
            }
        }
    }
}

/// Output tag in a [`TestPayload`]
#[derive(Debug, Copy, Clone)]
pub(crate) enum OutputTag {
    /// The byte of a [`TransactionOutput`]
    Valid(TransactionOutput),
    /// A byte that [`TransactionOutput::from_u8()`] rejects
    Invalid(u8),
}

impl OutputTag {
    /// `#[output]` with an alignment of `bytes`
    pub(crate) fn output(bytes: u8) -> Self {
        Self::Valid(TransactionOutput::new_output(alignment(bytes)))
    }

    /// Return value with an alignment of `bytes`
    pub(crate) fn return_value(bytes: u8) -> Self {
        Self::Valid(TransactionOutput::new_return_value(alignment(bytes)))
    }

    /// `byte` as a valid tag if [`TransactionOutput::from_u8()`] accepts it, an invalid one
    /// otherwise
    pub(crate) fn from_byte(byte: u8) -> Self {
        match TransactionOutput::from_u8(byte) {
            Some(output) => Self::Valid(output),
            None => Self::Invalid(byte),
        }
    }

    fn into_u8(self) -> u8 {
        match self {
            Self::Valid(output) => output.into_u8(),
            Self::Invalid(byte) => {
                assert!(
                    TransactionOutput::from_u8(byte).is_none(),
                    "Output tag {byte:#010b} is valid"
                );
                byte
            }
        }
    }
}

/// Method context in a [`TestPayload`]
#[derive(Debug, Copy, Clone)]
pub(crate) enum ContextByte {
    /// The byte of a [`TransactionMethodContext`]
    Valid(TransactionMethodContext),
    /// A byte that isn't a [`TransactionMethodContext`]
    Invalid(u8),
}

impl ContextByte {
    fn into_u8(self) -> u8 {
        match self {
            Self::Valid(context) => context as u8,
            Self::Invalid(byte) => {
                assert!(
                    TransactionMethodContext::try_from(byte).is_err(),
                    "Method context {byte} is valid"
                );
                byte
            }
        }
    }
}

/// Payload written byte by byte, with the same padding as `TransactionPayloadBuilder`, so that
/// tests can write tags and values that the builder never writes
pub(crate) struct TestPayload {
    words: [u128; 256],
    /// Number of bytes written
    len: usize,
}

impl TestPayload {
    /// Contract of every method
    pub(crate) const CONTRACT: Address = Address::SYSTEM_CODE;
    /// Bytes of the fingerprint of every method
    const FINGERPRINT: [u8; MethodFingerprint::SIZE as usize] =
        [7; MethodFingerprint::SIZE as usize];

    pub(crate) fn new() -> Self {
        Self {
            words: [0; 256],
            len: 0,
        }
    }

    /// Fingerprint of every method
    pub(crate) fn fingerprint() -> &'static MethodFingerprint {
        MethodFingerprint::try_from_bytes_exact(&Self::FINGERPRINT).unwrap()
    }

    /// Write `bytes` after zeroes up to `alignment`
    pub(crate) fn push(&mut self, bytes: &[u8], alignment: usize) -> &mut Self {
        self.len = self.len.next_multiple_of(alignment);
        for &byte in bytes {
            let word = &mut self.words[self.len / size_of::<u128>()];
            let mut word_bytes = word.to_ne_bytes();
            word_bytes[self.len % size_of::<u128>()] = byte;
            *word = u128::from_ne_bytes(word_bytes);
            self.len += 1;
        }

        self
    }

    /// Write the header of a method with `context`
    pub(crate) fn method_with_context(
        &mut self,
        context: ContextByte,
        slot_tags: &[SlotTag],
        input_tags: &[InputTag],
        num_outputs: u8,
    ) -> &mut Self {
        self.push(Self::CONTRACT.as_bytes(), align_of::<Address>())
            .push(&Self::FINGERPRINT, align_of::<MethodFingerprint>())
            .push(&[context.into_u8()], 1)
            .push(&[u8::try_from(slot_tags.len()).unwrap()], 1);
        for slot_tag in slot_tags {
            self.push(&[slot_tag.into_u8()], 1);
        }
        self.push(&[u8::try_from(input_tags.len()).unwrap()], 1);
        for input_tag in input_tags {
            self.push(&[input_tag.into_u8()], 1);
        }

        self.push(&[num_outputs], 1)
    }

    /// Write the header of a method with [`TransactionMethodContext::Wallet`]
    pub(crate) fn method(
        &mut self,
        slot_tags: &[SlotTag],
        input_tags: &[InputTag],
        num_outputs: u8,
    ) -> &mut Self {
        self.method_with_context(
            ContextByte::Valid(TransactionMethodContext::Wallet),
            slot_tags,
            input_tags,
            num_outputs,
        )
    }

    /// Write an explicit slot address
    pub(crate) fn address(&mut self, address: &Address) -> &mut Self {
        self.push(address.as_bytes(), align_of::<Address>())
    }

    /// Write the size and the bytes of an input value
    pub(crate) fn input_value(&mut self, bytes: &[u8], alignment: usize) -> &mut Self {
        self.push(
            &u32::try_from(bytes.len()).unwrap().to_le_bytes(),
            align_of::<u32>(),
        )
        .push(bytes, alignment)
    }

    /// Write the capacity and the tag of an output
    pub(crate) fn output(&mut self, capacity: u32, tag: OutputTag) -> &mut Self {
        self.push(&capacity.to_le_bytes(), align_of::<u32>())
            .push(&[tag.into_u8()], 1)
    }

    /// The payload, with zeroes up to a multiple of `u128`
    pub(crate) fn as_u128_slice(&self) -> &[u128] {
        &self.words[..self.len.div_ceil(size_of::<u128>())]
    }
}

/// Payload whose methods have `num_outputs` `#[output]` arguments without capacity together, as
/// many as possible in each method
pub(crate) fn payload_with_outputs(num_outputs: usize) -> TestPayload {
    let mut payload = TestPayload::new();
    let mut remaining_outputs = num_outputs;
    while remaining_outputs > 0 {
        let num_method_outputs = remaining_outputs.min(usize::from(MAX_TOTAL_METHOD_ARGS));
        payload.method(&[], &[], num_method_outputs as u8);
        for _ in 0..num_method_outputs {
            payload.output(0, OutputTag::output(1));
        }
        remaining_outputs -= num_method_outputs;
    }

    payload
}

/// What [`TransactionPayloadDecoder`] is expected to describe for one method
pub(crate) struct ExpectedMethod<'a> {
    pub(crate) contract: &'a Address,
    pub(crate) fingerprint: &'a MethodFingerprint,
    pub(crate) context: TransactionMethodContext,
    pub(crate) slots: &'a [DecodedSlot<'a>],
    pub(crate) inputs: &'a [DecodedInput<'a>],
    pub(crate) outputs: &'a [DecodedOutput],
}

/// Check that [`TransactionPayloadDecoder`] describes exactly the methods in `expected`
pub(crate) fn assert_decoded(payload: &[u128], expected: &[ExpectedMethod<'_>]) {
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];
    let mut decoder = TransactionPayloadDecoder::new(payload, &mut output_buffer_details);

    for (method_index, expected) in expected.iter().enumerate() {
        let method = decoder.next_method().unwrap().unwrap();

        assert_eq!(
            method.contract(),
            expected.contract,
            "Method {method_index}"
        );
        assert_eq!(
            method.fingerprint(),
            expected.fingerprint,
            "Method {method_index}"
        );
        assert_eq!(method.context(), expected.context, "Method {method_index}");
        assert_eq!(method.slots(), expected.slots, "Method {method_index}");
        assert_eq!(method.inputs(), expected.inputs, "Method {method_index}");
        assert_eq!(method.outputs(), expected.outputs, "Method {method_index}");
        let expected_return_value = expected
            .outputs
            .last()
            .filter(|output| output.kind == TransactionOutputKind::ReturnValue);
        assert_eq!(
            method.return_value(),
            expected_return_value,
            "Method {method_index}"
        );
    }

    assert!(decoder.next_method().unwrap().is_none());
}

/// Decode all methods of `payload` with [`TransactionPayloadDecoder`], returns their number
pub(crate) fn decode_all(
    payload: &[u128],
    num_output_buffer_details: usize,
) -> Result<usize, TransactionPayloadDecoderError> {
    let mut output_buffer_details = [MaybeUninit::uninit(); MAX_OUTPUT_BUFFER_DETAILS];
    let mut decoder = TransactionPayloadDecoder::new(
        payload,
        &mut output_buffer_details[..num_output_buffer_details],
    );

    let mut num_methods = 0;
    while decoder.next_method()?.is_some() {
        num_methods += 1;
    }

    Ok(num_methods)
}

/// Call all methods of `payload` with [`TransactionPayloadCaller`] and methods that write nothing,
/// returns their number.
///
/// The output buffer is zeroed, so return values are initialized, while `#[output]` arguments keep
/// size `0`.
pub(crate) fn call_all(
    payload: &[u128],
    output_buffer: &mut [MaybeUninit<u128>],
    num_output_buffer_details: usize,
) -> Result<usize, TransactionPayloadDecoderError> {
    output_buffer.fill(MaybeUninit::new(0));
    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); MAX_OUTPUT_BUFFER_DETAILS];
    let mut caller = TransactionPayloadCaller::new(
        payload,
        &mut external_args_buffer,
        output_buffer,
        &mut output_buffer_details[..num_output_buffer_details],
        |_method_context| MethodContext::Reset,
    );

    let mut num_methods = 0;
    loop {
        let call = CallWith::new(|_method: PreparedMethod<'_>| Ok(()));
        caller = match caller.call_next_method_with(&call) {
            Ok(Some(caller)) => caller,
            Ok(None) => return Ok(num_methods),
            Err(TransactionPayloadCallError::Decoder(error)) => return Err(error),
            Err(TransactionPayloadCallError::Method(error)) => {
                unreachable!("Methods don't fail: {error}")
            }
        };
        num_methods += 1;
    }
}

/// Decode all methods of `payload` with [`TransactionPayloadDecoder`] and call them with
/// [`TransactionPayloadCaller`], with `num_output_buffer_details` entries each. Both must give the
/// same result, which is returned.
pub(crate) fn decode_and_call_with_details(
    payload: &[u128],
    num_output_buffer_details: usize,
) -> Result<usize, TransactionPayloadDecoderError> {
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];

    let decoded = decode_all(payload, num_output_buffer_details);
    let called = call_all(payload, &mut output_buffer, num_output_buffer_details);
    assert_eq!(decoded, called);

    decoded
}

/// [`decode_and_call_with_details()`] with as many entries as `SimpleWalletBase` has
pub(crate) fn decode_and_call(payload: &[u128]) -> Result<usize, TransactionPayloadDecoderError> {
    decode_and_call_with_details(payload, OUTPUT_BUFFER_OFFSETS_SIZE)
}
