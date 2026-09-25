use crate::payload::{
    TransactionMethodContext, TransactionPayloadDecoder, TransactionPayloadDecoderError,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;
use core::ptr;

/// Offset of the number of `#[slot]` arguments of the first method in the payload
const NUM_SLOT_ARGUMENTS_OFFSET: usize = Address::SIZE as usize
    + MethodFingerprint::SIZE as usize
    + size_of::<TransactionMethodContext>();

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
