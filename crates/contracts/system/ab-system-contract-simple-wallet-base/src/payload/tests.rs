use crate::payload::{
    FfiDataSizeCapacityRw, TransactionMethodContext, TransactionOutput, TransactionOutputType,
    TransactionPayloadDecoder, TransactionPayloadDecoderError,
};
use crate::{EXTERNAL_ARGS_BUFFER_SIZE, OUTPUT_BUFFER_OFFSETS_SIZE, OUTPUT_BUFFER_SIZE};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
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

    // SAFETY: No methods were decoded before
    unsafe { decoder.decode_next_method() }.map(|_| ())
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

#[test]
fn payload_decode_outputs_aligned_without_padding() {
    let mut payload = [0u8; 80];
    // No slots and inputs, two outputs (the first bit) with capacity 16 and an alignment of
    // `Alignment::MAX` (power of two in the remaining bits)
    payload[NUM_SLOT_ARGUMENTS_OFFSET + 2] = 2;
    let mut capacity_offset = NUM_SLOT_ARGUMENTS_OFFSET + 3;
    for _ in 0..2 {
        capacity_offset = capacity_offset.next_multiple_of(align_of::<u32>());
        payload[capacity_offset..][..size_of::<u32>()].copy_from_slice(&16u32.to_le_bytes());
        payload[capacity_offset + size_of::<u32>()] = 0b1000_0000 | Alignment::MAX.power;
        capacity_offset += size_of::<u32>() + 1;
    }
    let payload = core::array::from_fn::<_, 5, _>(|index| {
        u128::from_ne_bytes(payload.as_chunks::<{ size_of::<u128>() }>().0[index])
    });

    let mut external_args_buffer = [ptr::null_mut(); EXTERNAL_ARGS_BUFFER_SIZE];
    // Exactly the capacity of both outputs
    let mut output_buffer = [MaybeUninit::uninit(); 2];
    let output_buffer_addr = output_buffer.as_ptr().addr();
    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];

    let mut decoder = TransactionPayloadDecoder::new(
        &payload,
        &mut external_args_buffer,
        &mut output_buffer,
        &mut output_buffer_details,
        |_method_context| MethodContext::Reset,
    );

    // SAFETY: No methods were decoded before
    let prepared_method = unsafe { decoder.decode_next_method() }.unwrap().unwrap();
    // SAFETY: The method has two outputs
    let [first_output, second_output] = unsafe {
        prepared_method
            .external_args
            .cast::<[FfiDataSizeCapacityRw; 2]>()
            .read()
    };

    // Outputs whose offset is already aligned are not padded
    assert_eq!(first_output.data_ptr.addr(), output_buffer_addr);
    assert_eq!(second_output.data_ptr.addr(), output_buffer_addr + 16);
}

#[test]
fn transaction_output_encoding() {
    for power in 0..=Alignment::MAX.power {
        let alignment = Alignment::from_power(power).unwrap();

        let output = TransactionOutput::new_output(alignment);
        assert_eq!(
            output.output_type(),
            TransactionOutputType::Output { alignment }
        );
        assert_eq!(output.into_u8(), 0b1000_0000 | power);

        let return_value = TransactionOutput::new_return_value(alignment);
        assert_eq!(
            return_value.output_type(),
            TransactionOutputType::ReturnValue { alignment }
        );
        assert_eq!(return_value.into_u8(), power);
    }

    // Only bytes that `into_u8()` returns are accepted, and they round-trip
    for byte in 0..=u8::MAX {
        let accepted = byte & 0b0111_1111 <= Alignment::MAX.power;
        assert_eq!(
            TransactionOutput::from_u8(byte).map(TransactionOutput::into_u8),
            accepted.then_some(byte),
            "Byte {byte:#010b}"
        );
    }
}
