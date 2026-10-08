use crate::payload::{
    TransactionMethodContext, TransactionPayloadDecoder, TransactionPayloadDecoderError,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::MAX_ALIGNMENT;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;
use core::ptr;

/// Offset of the number of `#[slot]` arguments of the first method in the payload
const NUM_SLOT_ARGUMENTS_OFFSET: usize = Address::SIZE as usize
    + MethodFingerprint::SIZE as usize
    + size_of::<TransactionMethodContext>();
/// Power of two of [`MAX_ALIGNMENT`] as stored in the payload
const MAX_ALIGNMENT_POWER: u8 = MAX_ALIGNMENT.trailing_zeros() as u8;

fn decode_first_method(payload: &[u8; 64]) -> Result<(), TransactionPayloadDecoderError> {
    let payload = core::array::from_fn::<_, 4, _>(|index| {
        u128::from_ne_bytes(payload.as_chunks::<{ size_of::<u128>() }>().0[index])
    });

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let mut decoder = TransactionPayloadDecoder::new(
        &payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    );

    decoder.decode_next_method().map(|_| ())
}

#[test]
fn payload_decode_too_many_slot_arguments() {
    let mut payload = [0u8; 64];
    // More slots than `MAX_TOTAL_METHOD_ARGS`, followed by zero inputs and outputs
    payload[NUM_SLOT_ARGUMENTS_OFFSET] = 9;

    assert!(matches!(
        decode_first_method(&payload),
        Err(TransactionPayloadDecoderError::TooManyArguments(9))
    ));
}

#[test]
fn payload_decode_too_many_slot_and_input_arguments() {
    let mut payload = [0u8; 64];
    // Slots and inputs are each within `MAX_TOTAL_METHOD_ARGS`, but not together
    payload[NUM_SLOT_ARGUMENTS_OFFSET] = 5;
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 1 + 5] = 5;

    assert!(matches!(
        decode_first_method(&payload),
        Err(TransactionPayloadDecoderError::TooManyArguments(10))
    ));
}

#[test]
fn payload_decode_invalid_method_context() {
    let mut payload = [0u8; 64];
    // Neither `TransactionMethodContext::Null` nor `TransactionMethodContext::Wallet`
    payload[NUM_SLOT_ARGUMENTS_OFFSET - size_of::<TransactionMethodContext>()] = 2;

    assert!(matches!(
        decode_first_method(&payload),
        Err(TransactionPayloadDecoderError::InvalidMethodContext(2))
    ));
}

#[test]
fn payload_decode_input_max_alignment() {
    let mut payload = [0u8; 64];
    // No slots, a single empty input value (the first bit) with an alignment of `MAX_ALIGNMENT`
    // (power of two in the remaining bits)
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 1] = 1;
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 0b1000_0000 | MAX_ALIGNMENT_POWER;

    decode_first_method(&payload).unwrap();
}

#[test]
fn payload_decode_input_alignment_too_large() {
    let mut payload = [0u8; 64];
    // No slots, a single input value (the first bit) with an alignment of twice `MAX_ALIGNMENT`
    // (power of two in the remaining bits)
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 1] = 1;
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 0b1000_0000 | (MAX_ALIGNMENT_POWER + 1);

    assert!(matches!(
        decode_first_method(&payload),
        Err(TransactionPayloadDecoderError::AlignmentPowerTooLarge(alignment_power))
            if alignment_power == MAX_ALIGNMENT_POWER + 1
    ));
}

#[test]
fn payload_decode_output_max_alignment() {
    let mut payload = [0u8; 64];
    // No slots and inputs, a single empty output with an alignment of `MAX_ALIGNMENT` following its
    // capacity
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 1;
    let capacity_offset = (NUM_SLOT_ARGUMENTS_OFFSET + 3).next_multiple_of(align_of::<u32>());
    payload[capacity_offset + size_of::<u32>()] = MAX_ALIGNMENT_POWER;

    decode_first_method(&payload).unwrap();
}

#[test]
fn payload_decode_output_alignment_too_large() {
    let mut payload = [0u8; 64];
    // No slots and inputs, a single output with an alignment of twice `MAX_ALIGNMENT` following its
    // capacity
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 1;
    let capacity_offset = (NUM_SLOT_ARGUMENTS_OFFSET + 3).next_multiple_of(align_of::<u32>());
    payload[capacity_offset + size_of::<u32>()] = MAX_ALIGNMENT_POWER + 1;

    assert!(matches!(
        decode_first_method(&payload),
        Err(TransactionPayloadDecoderError::AlignmentPowerTooLarge(alignment_power))
            if alignment_power == MAX_ALIGNMENT_POWER + 1
    ));
}
