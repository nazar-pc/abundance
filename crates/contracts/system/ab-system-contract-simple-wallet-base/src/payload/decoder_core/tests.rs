//! Rules that the core applies, checked through the caller

use crate::payload::test_utils::{CallWith, NUM_SLOT_ARGUMENTS_OFFSET};
use crate::payload::{
    TransactionMethodContext, TransactionPayloadCallError, TransactionPayloadCaller,
    TransactionPayloadDecoderError,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_io_type::alignment::Alignment;
use core::mem::MaybeUninit;
use core::ptr;

fn decode_first_method(payload: &[u8; 64]) -> Result<(), TransactionPayloadDecoderError> {
    let payload = core::array::from_fn::<_, 4, _>(|index| {
        u128::from_ne_bytes(payload.as_chunks::<{ size_of::<u128>() }>().0[index])
    });

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    let mut output_buffer = [MaybeUninit::uninit(); OUTPUT_BUFFER_SIZE];
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let caller = TransactionPayloadCaller::new(
        &payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |method_context| match method_context {
            TransactionMethodContext::Null => MethodContext::Reset,
            TransactionMethodContext::Wallet => MethodContext::Keep,
        },
    );

    // The method is only decoded, its outputs keep size `0`
    match caller.call_next_method_with(&CallWith::new(|_method: PreparedMethod<'_>| Ok(()))) {
        Ok(_maybe_caller) => Ok(()),
        Err(TransactionPayloadCallError::Decoder(error)) => Err(error),
        Err(TransactionPayloadCallError::Method(error)) => {
            unreachable!("The method is not called: {error}")
        }
    }
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
    // No slots, a single empty input value (the first bit) with an alignment of `Alignment::MAX`
    // (power of two in the remaining bits)
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 1] = 1;
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 0b1000_0000 | Alignment::MAX.power;

    decode_first_method(&payload).unwrap();
}

#[test]
fn payload_decode_input_alignment_too_large() {
    let mut payload = [0u8; 64];
    // No slots, a single input value (the first bit) with an alignment of twice `Alignment::MAX`
    // (power of two in the remaining bits)
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 1] = 1;
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 0b1000_0000 | (Alignment::MAX.power + 1);

    assert!(matches!(
        decode_first_method(&payload),
        Err(TransactionPayloadDecoderError::AlignmentPowerTooLarge(alignment_power))
            if alignment_power == Alignment::MAX.power + 1
    ));
}

#[test]
fn payload_decode_output_max_alignment() {
    // An output (the first bit) and a return value
    for output_type in [0b1000_0000, 0] {
        let mut payload = [0u8; 64];
        // No slots and inputs, a single one with capacity `0`, followed by an alignment of
        // `Alignment::MAX` (power of two in the remaining bits)
        payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 1;
        let capacity_offset = (NUM_SLOT_ARGUMENTS_OFFSET + 3).next_multiple_of(align_of::<u32>());
        payload[capacity_offset + size_of::<u32>()] = output_type | Alignment::MAX.power;

        decode_first_method(&payload).unwrap();
    }
}

#[test]
fn payload_decode_output_alignment_too_large() {
    // An output (the first bit) and a return value
    for output_type in [0b1000_0000, 0] {
        // All bits that a valid alignment power doesn't use are invalid
        for invalid_alignment_power in Alignment::MAX.power + 1..=0b0111_1111 {
            let mut payload = [0u8; 64];
            // No slots and inputs, a single one with capacity `0`, followed by an invalid
            // alignment power
            payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 1;
            let capacity_offset =
                (NUM_SLOT_ARGUMENTS_OFFSET + 3).next_multiple_of(align_of::<u32>());
            payload[capacity_offset + size_of::<u32>()] = output_type | invalid_alignment_power;

            assert!(matches!(
                decode_first_method(&payload),
                Err(TransactionPayloadDecoderError::AlignmentPowerTooLarge(alignment_power))
                    if alignment_power == invalid_alignment_power
            ));
        }
    }
}
