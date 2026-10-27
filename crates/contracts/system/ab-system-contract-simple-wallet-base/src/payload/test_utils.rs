//! Helpers shared by tests of the payload modules

use crate::payload::caller::{
    CallPrepared, OutputChunk, TransactionPayloadCallError, TransactionPayloadCaller,
};
use crate::payload::decoder::{
    DecodedInput, DecodedOutput, DecodedSlot, TransactionPayloadDecoder,
};
use crate::payload::{
    DeclaredOutput, FfiDataSizeCapacityRo, FfiDataSizeCapacityRw, TransactionInput,
    TransactionMethodContext, TransactionOutput, TransactionOutputKind,
    TransactionPayloadDecoderError, TransactionSlot,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, MAX_OUTPUTS, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::ContractError;
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr;

/// Size of the output buffer of `SimpleWalletBase` in bytes
pub(crate) const OUTPUT_BUFFER_BYTES: u32 = (OUTPUT_BUFFER_SIZE * size_of::<OutputChunk>()) as u32;

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
    /// `#[output]` that writes the declared output with `output_index`
    pub(crate) fn output(output_index: u8) -> Self {
        Self::Valid(TransactionOutput::new_output(output_index).unwrap())
    }

    /// Return value that is written into the declared output with `output_index`
    pub(crate) fn return_value(output_index: u8) -> Self {
        Self::Valid(TransactionOutput::new_return_value(output_index).unwrap())
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
/// tests can write tags and values that the builder never writes. It is aligned to
/// [`Alignment::MAX`], like payloads that the builder creates.
#[repr(C, align(16))]
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

    /// Payload without any bytes, not even the number of declared outputs
    pub(crate) fn empty() -> Self {
        Self {
            words: [0; 256],
            len: 0,
        }
    }

    /// Payload that declares outputs with `capacities`, to which methods are added
    pub(crate) fn new(capacities: &[u32]) -> Self {
        let mut payload = Self::empty();
        payload.push(&[u8::try_from(capacities.len()).unwrap()], 1);
        for capacity in capacities {
            payload.push(&capacity.to_le_bytes(), align_of::<u32>());
        }

        payload
    }

    /// Number of bytes written
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Fingerprint of every method
    pub(crate) fn fingerprint() -> &'static MethodFingerprint {
        MethodFingerprint::from_bytes_exact(&Self::FINGERPRINT).unwrap()
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
        output_tags: &[OutputTag],
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
        self.push(&[u8::try_from(output_tags.len()).unwrap()], 1);
        for output_tag in output_tags {
            self.push(&[output_tag.into_u8()], 1);
        }

        self
    }

    /// Write the header of a method with [`TransactionMethodContext::Wallet`]
    pub(crate) fn method(
        &mut self,
        slot_tags: &[SlotTag],
        input_tags: &[InputTag],
        output_tags: &[OutputTag],
    ) -> &mut Self {
        self.method_with_context(
            ContextByte::Valid(TransactionMethodContext::Wallet),
            slot_tags,
            input_tags,
            output_tags,
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

    /// Write the size of a return value
    pub(crate) fn return_value_size(&mut self, size: u32) -> &mut Self {
        self.push(&size.to_le_bytes(), align_of::<u32>())
    }

    /// The payload, with zeroes up to a multiple of `u128`
    pub(crate) fn as_u128_slice(&self) -> &[u128] {
        &self.words[..self.len.div_ceil(size_of::<u128>())]
    }
}

/// What [`TransactionPayloadDecoder`] is expected to describe for one method
#[derive(Copy, Clone)]
pub(crate) struct ExpectedMethod<'a> {
    pub(crate) contract: &'a Address,
    pub(crate) fingerprint: &'a MethodFingerprint,
    pub(crate) context: TransactionMethodContext,
    pub(crate) slots: &'a [DecodedSlot<'a>],
    pub(crate) inputs: &'a [DecodedInput<'a>],
    pub(crate) outputs: &'a [DecodedOutput],
}

/// Check that [`TransactionPayloadDecoder`] describes exactly outputs with `capacities` as the
/// declared outputs and the methods in `expected`
pub(crate) fn assert_decoded(
    payload: &[u128],
    capacities: &[u32],
    expected: &[ExpectedMethod<'_>],
) {
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let mut decoder = TransactionPayloadDecoder::new(payload, &mut declared_outputs).unwrap();

    assert_eq!(decoder.declared_outputs().len(), capacities.len());
    for (output_index, (declared_output, &capacity)) in decoder
        .declared_outputs()
        .iter()
        .zip(capacities)
        .enumerate()
    {
        assert_eq!(
            declared_output.capacity, capacity,
            "Output index {output_index}"
        );
    }

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
pub(crate) fn decode_all(payload: &[u128]) -> Result<usize, TransactionPayloadDecoderError> {
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let mut decoder = TransactionPayloadDecoder::new(payload, &mut declared_outputs)?;

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
    output_buffer: &mut [MaybeUninit<OutputChunk>],
) -> Result<usize, TransactionPayloadDecoderError> {
    // SAFETY: Zero bytes are a valid value of `OutputChunk`, which is plain data. A single bulk
    // write is much faster under Miri than zeroing chunk by chunk like this:
    // output_buffer.fill_with(MaybeUninit::zeroed);
    unsafe {
        output_buffer
            .as_mut_ptr()
            .write_bytes(0, output_buffer.len());
    }
    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let mut caller = TransactionPayloadCaller::new(
        payload,
        &mut external_args_buffer,
        output_buffer,
        &mut declared_outputs,
        |_method_context| MethodContext::Reset,
    )?;

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
/// [`TransactionPayloadCaller`] with the output buffer of `SimpleWalletBase`. Both must give the
/// same result, which is returned.
pub(crate) fn decode_and_call(payload: &[u128]) -> Result<usize, TransactionPayloadDecoderError> {
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];

    let decoded = decode_all(payload);
    let called = call_all(payload, &mut output_buffer);
    assert_eq!(decoded, called);

    decoded
}

/// Create a caller for `payload` with the buffers of `SimpleWalletBase`
pub(crate) fn new_caller<'a>(
    payload: &'a [u128],
    external_args_buffer: &'a mut [*mut c_void; EXTERNAL_ARGS_BUFFER_SIZE],
    output_buffer: &'a mut [MaybeUninit<OutputChunk>; OUTPUT_BUFFER_SIZE],
    declared_outputs: &'a mut [MaybeUninit<DeclaredOutput>; MAX_OUTPUTS],
) -> TransactionPayloadCaller<'a> {
    TransactionPayloadCaller::new(
        payload,
        external_args_buffer,
        output_buffer,
        declared_outputs,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    )
    .unwrap()
}

/// Call the next method of `caller` with `call`, which must succeed
pub(crate) fn call_next<Call>(
    caller: TransactionPayloadCaller<'_>,
    call: Call,
) -> TransactionPayloadCaller<'_>
where
    Call: Fn(PreparedMethod<'_>) -> Result<(), ContractError>,
{
    caller
        .call_next_method_with(&CallWith::new(call))
        .unwrap()
        .unwrap()
}

/// Check that `caller` has no more methods
pub(crate) fn assert_no_more_methods(caller: TransactionPayloadCaller<'_>) {
    assert!(
        caller
            .call_next_method_with(&CallWith::new(|_method: PreparedMethod<'_>| {
                unreachable!("There are no more methods")
            }))
            .unwrap()
            .is_none()
    );
}

/// The only argument of `method`, an `#[output]` or a return value
///
/// # Safety
/// The method must have exactly one argument, which is an `#[output]` or a return value.
pub(crate) unsafe fn only_output<'a>(method: &PreparedMethod<'a>) -> &'a mut FfiDataSizeCapacityRw {
    // SAFETY: Guaranteed by the function contract
    unsafe {
        method
            .external_args
            .cast::<FfiDataSizeCapacityRw>()
            .as_mut()
    }
}

/// The only argument of `method`, an `#[input]`
///
/// # Safety
/// The method must have exactly one argument, which is an `#[input]`.
pub(crate) unsafe fn only_input(method: &PreparedMethod<'_>) -> FfiDataSizeCapacityRo {
    // SAFETY: Guaranteed by the function contract
    unsafe { method.external_args.cast::<FfiDataSizeCapacityRo>().read() }
}

/// The only argument of `method`, a `#[slot]`
///
/// # Safety
/// The method must have exactly one argument, which is a `#[slot]`.
pub(crate) unsafe fn only_slot(method: &PreparedMethod<'_>) -> *const Address {
    // SAFETY: Guaranteed by the function contract
    unsafe { method.external_args.cast::<*const Address>().read() }
}
