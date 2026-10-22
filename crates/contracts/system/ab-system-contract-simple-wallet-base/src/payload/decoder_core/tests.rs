//! Rules that the core applies, checked through both the decoder and the caller, which must agree

use crate::OUTPUT_BUFFER_OFFSETS_SIZE;
use crate::payload::decoder::{DecodedInput, DecodedOutput};
use crate::payload::test_utils::{
    ContextByte, ExpectedMethod, InputTag, MAX_OUTPUT_BUFFER_DETAILS, OutputTag, SlotTag,
    TestPayload, alignment, assert_decoded, decode_and_call, decode_and_call_with_details,
    payload_with_outputs,
};
use crate::payload::{
    MAX_OUTPUT_INDEX, TransactionMethodContext, TransactionOutputKind,
    TransactionPayloadDecoderError,
};
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;

/// Alignment of an address
const ADDRESS_ALIGNMENT: u8 = Alignment::of::<Address>().get();

/// Every alignment that tags can hold
fn alignments() -> impl Iterator<Item = u8> {
    (0..=Alignment::MAX.power).map(|power| 1 << power)
}

#[test]
fn payload_decode_payload_too_small() {
    // The fingerprint is cut off
    let mut payload = TestPayload::new();
    payload
        .push(TestPayload::CONTRACT.as_bytes(), align_of::<Address>())
        .push(&[0; 17], 1);
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::PayloadTooSmall)
    );

    // The input value is larger than the rest of the payload
    let mut payload = TestPayload::new();
    payload
        .method(&[], &[InputTag::value(1)], 0)
        .push(&64u32.to_le_bytes(), align_of::<u32>());
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::PayloadTooSmall)
    );
}

#[test]
fn payload_decode_too_many_slot_arguments() {
    let mut payload = TestPayload::new();
    // More slots than `MAX_TOTAL_METHOD_ARGS`, followed by zero inputs and outputs
    payload.method(&[SlotTag::address(); 9], &[], 0);

    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::TooManyArguments(9))
    );
}

#[test]
fn payload_decode_too_many_slot_and_input_arguments() {
    // Slots, inputs and outputs are each within `MAX_TOTAL_METHOD_ARGS`, but not together
    for (num_slots, num_inputs, num_outputs) in [(5, 5, 0), (5, 0, 5), (0, 5, 5), (3, 3, 3)] {
        let mut payload = TestPayload::new();
        payload.method(
            &[SlotTag::address(); MAX_TOTAL_METHOD_ARGS as usize][..usize::from(num_slots)],
            &[InputTag::value(1); MAX_TOTAL_METHOD_ARGS as usize][..usize::from(num_inputs)],
            num_outputs,
        );

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::TooManyArguments(
                num_slots + num_inputs + num_outputs
            )),
            "{num_slots} slots, {num_inputs} inputs, {num_outputs} outputs"
        );
    }
}

#[test]
fn payload_decode_invalid_method_context() {
    let mut payload = TestPayload::new();
    // Neither `TransactionMethodContext::Null` nor `TransactionMethodContext::Wallet`
    payload.method_with_context(ContextByte::Invalid(2), &[], &[], 0);

    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::InvalidMethodContext(2))
    );
}

#[test]
fn payload_decode_slot_tags() {
    for byte in 0..=u8::MAX {
        let mut payload = TestPayload::new();
        payload
            .method(&[SlotTag::from_byte(byte)], &[], 0)
            .address(&Address::NULL);

        let expected = if byte <= MAX_OUTPUT_INDEX {
            // Output index, but there are no outputs
            Err(TransactionPayloadDecoderError::OutputIndexNotFound(byte))
        } else if byte == 0b1000_0000 {
            // Explicit slot address
            Ok(1)
        } else {
            Err(TransactionPayloadDecoderError::InvalidSlotTag(byte))
        };
        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            expected,
            "Slot tag {byte:#010b}"
        );
    }
}

#[test]
fn payload_decode_input_tags() {
    for byte in 0..=u8::MAX {
        let mut payload = TestPayload::new();
        payload
            .method(&[], &[InputTag::from_byte(byte)], 0)
            .input_value(&[], usize::from(Alignment::MAX.get()));

        let expected = if byte <= MAX_OUTPUT_INDEX {
            // Output index, but there are no outputs
            Err(TransactionPayloadDecoderError::OutputIndexNotFound(byte))
        } else if byte & 0b0111_1111 <= Alignment::MAX.power {
            // Value with an alignment up to `Alignment::MAX`
            Ok(1)
        } else {
            Err(TransactionPayloadDecoderError::InvalidInputTag(byte))
        };
        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            expected,
            "Input tag {byte:#010b}"
        );
    }
}

#[test]
fn payload_decode_input_max_alignment() {
    let mut payload = TestPayload::new();
    // A single empty input value with an alignment of `Alignment::MAX`
    payload
        .method(&[], &[InputTag::value(Alignment::MAX.get())], 0)
        .input_value(&[], usize::from(Alignment::MAX.get()));

    assert_decoded(
        payload.as_u128_slice(),
        &[ExpectedMethod {
            contract: &TestPayload::CONTRACT,
            fingerprint: TestPayload::fingerprint(),
            context: TransactionMethodContext::Wallet,
            slots: &[],
            inputs: &[DecodedInput::Value {
                bytes: &[],
                alignment: alignment(Alignment::MAX.get()),
            }],
            outputs: &[],
        }],
    );
    assert_eq!(decode_and_call(payload.as_u128_slice()), Ok(1));
}

#[test]
fn payload_decode_output_tags() {
    for byte in 0..=u8::MAX {
        let mut payload = TestPayload::new();
        payload
            .method(&[], &[], 1)
            .output(0, OutputTag::from_byte(byte));

        // `#[output]` (`0b1xxx_xxxx`) or return value (`0b0xxx_xxxx`) with an alignment up to
        // `Alignment::MAX`
        let expected = if byte & 0b0111_1111 <= Alignment::MAX.power {
            Ok(1)
        } else {
            Err(TransactionPayloadDecoderError::InvalidOutputTag(byte))
        };
        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            expected,
            "Output tag {byte:#010b}"
        );
    }
}

#[test]
fn payload_decode_output_max_alignment() {
    for (output_tag, kind) in [
        (
            OutputTag::output(Alignment::MAX.get()),
            TransactionOutputKind::Output,
        ),
        (
            OutputTag::return_value(Alignment::MAX.get()),
            TransactionOutputKind::ReturnValue,
        ),
    ] {
        let mut payload = TestPayload::new();
        // A single one with capacity `0` and an alignment of `Alignment::MAX`
        payload.method(&[], &[], 1).output(0, output_tag);

        assert_decoded(
            payload.as_u128_slice(),
            &[ExpectedMethod {
                contract: &TestPayload::CONTRACT,
                fingerprint: TestPayload::fingerprint(),
                context: TransactionMethodContext::Wallet,
                slots: &[],
                inputs: &[],
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind,
                    alignment: alignment(Alignment::MAX.get()),
                    capacity: 0,
                    size: 0,
                }],
            }],
        );
        assert_eq!(decode_and_call(payload.as_u128_slice()), Ok(1));
    }
}

#[test]
fn payload_decode_return_value_not_last() {
    // A return value followed by an `#[output]` or by another return value
    for second_output_tag in [OutputTag::output(4), OutputTag::return_value(4)] {
        let mut payload = TestPayload::new();
        payload
            .method(&[], &[], 2)
            .output(4, OutputTag::return_value(4))
            .output(4, second_output_tag);

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::ReturnValueNotLast(0)),
            "Second output tag {second_output_tag:?}"
        );
    }

    // The same in a later method, whose outputs get later output indices
    let mut payload = TestPayload::new();
    payload.method(&[], &[], 1).output(4, OutputTag::output(1));
    payload
        .method(&[], &[], 3)
        .output(4, OutputTag::output(1))
        .output(4, OutputTag::return_value(1))
        .output(4, OutputTag::output(1));
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::ReturnValueNotLast(2))
    );

    // A return value after `#[output]` arguments is accepted
    let mut payload = TestPayload::new();
    payload
        .method(&[], &[], 3)
        .output(4, OutputTag::output(1))
        .output(4, OutputTag::output(1))
        .output(4, OutputTag::return_value(1));
    assert_eq!(decode_and_call(payload.as_u128_slice()), Ok(1));
}

#[test]
fn payload_decode_output_index_not_found() {
    // A slot that refers to the output index after the only output
    let mut payload = TestPayload::new();
    payload
        .method(&[], &[], 1)
        .output(Address::SIZE, OutputTag::return_value(ADDRESS_ALIGNMENT));
    payload.method(&[SlotTag::output_index(1)], &[], 0);
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::OutputIndexNotFound(1))
    );

    // An input that does the same
    let mut payload = TestPayload::new();
    payload.method(&[], &[], 1).output(1, OutputTag::output(1));
    payload.method(&[], &[InputTag::output_index(1)], 0);
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::OutputIndexNotFound(1))
    );
}

#[test]
fn payload_decode_slot_output_alignment() {
    // Outputs whose memory is declared with less than the alignment of an address are rejected
    // as slots, even when their memory happens to be aligned, like that of the first output
    for output_alignment in alignments().filter(|&alignment| alignment < ADDRESS_ALIGNMENT) {
        for output_tag in [
            OutputTag::output(output_alignment),
            OutputTag::return_value(output_alignment),
        ] {
            let mut payload = TestPayload::new();
            payload
                .method(&[], &[], 1)
                .output(Address::SIZE, output_tag);
            payload.method(&[SlotTag::output_index(0)], &[], 0);

            assert_eq!(
                decode_and_call(payload.as_u128_slice()),
                Err(
                    TransactionPayloadDecoderError::InvalidSlotOutputIndexAlign {
                        output_index: 0,
                        alignment: u32::from(output_alignment),
                    }
                ),
                "Output tag {output_tag:?}"
            );
        }
    }

    // Return values with at least the alignment of an address are accepted
    for output_alignment in alignments().filter(|&alignment| alignment >= ADDRESS_ALIGNMENT) {
        let mut payload = TestPayload::new();
        payload
            .method(&[], &[], 1)
            .output(Address::SIZE, OutputTag::return_value(output_alignment));
        payload.method(&[SlotTag::output_index(0)], &[], 0);

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Ok(2),
            "Alignment {output_alignment}"
        );
    }
}

#[test]
fn payload_decode_slot_return_value_size() {
    // A return value used as a slot must be declared with the size of an address
    for size in [0, 8, Address::SIZE - 1, Address::SIZE + 1, 32] {
        let mut payload = TestPayload::new();
        payload
            .method(&[], &[], 1)
            .output(size, OutputTag::return_value(ADDRESS_ALIGNMENT));
        payload.method(&[SlotTag::output_index(0)], &[], 0);

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::InvalidSlotOutputIndexSize {
                output_index: 0,
                size,
            }),
            "Size {size}"
        );
    }
}

#[test]
fn payload_decode_too_many_outputs() {
    for num_output_buffer_details in [2, OUTPUT_BUFFER_OFFSETS_SIZE] {
        let payload = payload_with_outputs(num_output_buffer_details);
        assert_eq!(
            decode_and_call_with_details(payload.as_u128_slice(), num_output_buffer_details),
            Ok(num_output_buffer_details.div_ceil(usize::from(MAX_TOTAL_METHOD_ARGS))),
            "{num_output_buffer_details} entries"
        );

        let payload = payload_with_outputs(num_output_buffer_details + 1);
        assert_eq!(
            decode_and_call_with_details(payload.as_u128_slice(), num_output_buffer_details),
            Err(TransactionPayloadDecoderError::OutputBufferOffsetsTooSmall),
            "{num_output_buffer_details} entries"
        );
    }
}

#[test]
fn payload_decode_max_output_index() {
    // Tags refer to output indices up to 127, so outputs after that are rejected even when there
    // are more entries in `output_buffer_details`
    let num_outputs = usize::from(MAX_OUTPUT_INDEX) + 1;

    let payload = payload_with_outputs(num_outputs);
    assert_eq!(
        decode_and_call_with_details(payload.as_u128_slice(), MAX_OUTPUT_BUFFER_DETAILS),
        Ok(num_outputs / usize::from(MAX_TOTAL_METHOD_ARGS))
    );

    let payload = payload_with_outputs(num_outputs + 1);
    assert_eq!(
        decode_and_call_with_details(payload.as_u128_slice(), MAX_OUTPUT_BUFFER_DETAILS),
        Err(TransactionPayloadDecoderError::OutputBufferOffsetsTooSmall)
    );
}
