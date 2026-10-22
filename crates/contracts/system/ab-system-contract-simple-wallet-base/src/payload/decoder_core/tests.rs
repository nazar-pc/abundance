//! Rules that the core applies, checked through both the decoder and the caller, which must agree

use crate::MAX_OUTPUTS;
use crate::payload::decoder::{DecodedInput, DecodedOutput, TransactionPayloadDecoder};
use crate::payload::test_utils::{
    ContextByte, ExpectedMethod, InputTag, OUTPUT_BUFFER_BYTES, OutputTag, SlotTag, TestPayload,
    alignment, assert_decoded, decode_and_call,
};
use crate::payload::{
    TransactionMethodContext, TransactionOutputKind, TransactionPayloadDecoderError,
};
use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;
use core::slice;

#[test]
fn payload_decode_empty() {
    // Every payload starts with the number of declared outputs, so an empty payload is too small
    assert_eq!(
        decode_and_call(TestPayload::empty().as_u128_slice()),
        Err(TransactionPayloadDecoderError::PayloadTooSmall)
    );

    // A payload with only the number of declared outputs declares none and has no methods
    let payload = TestPayload::new(&[]);
    let payload = payload.as_u128_slice();
    assert_eq!(decode_and_call(payload), Ok(0));

    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let decoder = TransactionPayloadDecoder::new(payload, &mut declared_outputs).unwrap();
    assert!(decoder.declared_outputs().is_empty());
}

#[test]
fn payload_decode_payload_too_small() {
    // The fingerprint is cut off
    let mut payload = TestPayload::new(&[]);
    payload
        .push(TestPayload::CONTRACT.as_bytes(), align_of::<Address>())
        .push(&[0; 17], 1);
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::PayloadTooSmall)
    );

    // The input value is larger than the rest of the payload
    let mut payload = TestPayload::new(&[]);
    payload
        .method(&[], &[InputTag::value(1)], &[])
        .push(&64u32.to_le_bytes(), align_of::<u32>());
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::PayloadTooSmall)
    );

    // The size of the return value is cut off: the method ends 3 bytes before a multiple of 16
    let mut payload = TestPayload::new(&[4]);
    payload.method(&[], &[], &[OutputTag::return_value(0)]);
    assert_eq!(payload.len() % size_of::<u128>(), 13);
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::PayloadTooSmall)
    );
}

#[test]
fn payload_decode_header_too_small() {
    // 4 declared outputs, but only 3 capacities before the end of the payload. The first one
    // doesn't fit into the output buffer, but all capacities are read before outputs are
    // allocated, so the caller rejects the payload the same way as the decoder.
    for first_capacity in [4, OUTPUT_BUFFER_BYTES + 1] {
        let mut payload = TestPayload::empty();
        payload
            .push(&[4], 1)
            .push(&first_capacity.to_le_bytes(), align_of::<u32>())
            .push(&4u32.to_le_bytes(), align_of::<u32>())
            .push(&4u32.to_le_bytes(), align_of::<u32>());
        assert_eq!(payload.len(), size_of::<u128>());

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::PayloadTooSmall),
            "First capacity {first_capacity}"
        );
    }
}

#[test]
fn payload_decode_too_many_slot_arguments() {
    let mut payload = TestPayload::new(&[]);
    // More slots than `MAX_TOTAL_METHOD_ARGS`, followed by zero inputs and outputs
    payload.method(&[SlotTag::address(); 9], &[], &[]);

    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::TooManyArguments(9))
    );
}

#[test]
fn payload_decode_too_many_slot_and_input_arguments() {
    // Slots, inputs and outputs are each within `MAX_TOTAL_METHOD_ARGS`, but not together
    for (num_slots, num_inputs, num_outputs) in [(5, 5, 0), (5, 0, 5), (0, 5, 5), (3, 3, 3)] {
        let mut payload = TestPayload::new(&[]);
        payload.method(
            &[SlotTag::address(); MAX_TOTAL_METHOD_ARGS as usize][..usize::from(num_slots)],
            &[InputTag::value(1); MAX_TOTAL_METHOD_ARGS as usize][..usize::from(num_inputs)],
            &[OutputTag::output(0); MAX_TOTAL_METHOD_ARGS as usize][..usize::from(num_outputs)],
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
    let mut payload = TestPayload::new(&[]);
    // Neither `TransactionMethodContext::Null` nor `TransactionMethodContext::Wallet`
    payload.method_with_context(ContextByte::Invalid(2), &[], &[], &[]);

    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::InvalidMethodContext(2))
    );
}

#[test]
fn payload_decode_slot_tags() {
    for byte in 0..=u8::MAX {
        let mut payload = TestPayload::new(&[]);
        payload
            .method(&[SlotTag::from_byte(byte)], &[], &[])
            .address(&Address::NULL);

        let expected = if usize::from(byte) < MAX_OUTPUTS {
            // Output index, but there are no declared outputs
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
        let mut payload = TestPayload::new(&[]);
        payload
            .method(&[], &[InputTag::from_byte(byte)], &[])
            .input_value(&[], usize::from(Alignment::MAX.get()));

        let expected = if usize::from(byte) < MAX_OUTPUTS {
            // Output index, but there are no declared outputs
            Err(TransactionPayloadDecoderError::OutputIndexNotFound(byte))
        } else if byte & 0b1000_0000 != 0 && byte & 0b0111_1111 <= Alignment::MAX.power {
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
    let mut payload = TestPayload::new(&[]);
    // A single empty input value with an alignment of `Alignment::MAX`
    payload
        .method(&[], &[InputTag::value(Alignment::MAX.get())], &[])
        .input_value(&[], usize::from(Alignment::MAX.get()));

    assert_decoded(
        payload.as_u128_slice(),
        &[],
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
fn payload_decode_misaligned() {
    // Padding before the input value depends on where the payload starts
    let mut payload = TestPayload::new(&[]);
    payload
        .method(&[], &[InputTag::value(Alignment::MAX.get())], &[])
        .input_value(&[1], usize::from(Alignment::MAX.get()));
    let payload = payload.as_u128_slice();
    assert_eq!(decode_and_call(payload), Ok(1));

    // A misaligned `u128` slice only exists where `u128` is aligned to 8 bytes
    if align_of::<u128>() < usize::from(Alignment::MAX.get()) {
        let mut buffer = OwnedAlignedBuffer::from_bytes(&[0; 8]);
        for word in payload {
            assert!(buffer.append(&word.to_ne_bytes()));
        }
        // SAFETY: The buffer holds the initialized bytes of the payload 8 bytes after its 16-byte
        // aligned start, so they are aligned for `u128` here
        let misaligned =
            unsafe { slice::from_raw_parts(buffer.as_ptr().byte_add(8), payload.len()) };
        assert_eq!(
            decode_and_call(misaligned),
            Err(TransactionPayloadDecoderError::PayloadMisaligned)
        );
    }
}

#[test]
fn payload_decode_output_tags() {
    // `#[output]` (`0b1xxx_xxxx`) or return value (`0b0xxx_xxxx`) that writes one of
    // `MAX_OUTPUTS` declared outputs, other output indices are invalid tags
    let capacities = [0; MAX_OUTPUTS];
    for byte in 0..=u8::MAX {
        let mut payload = TestPayload::new(&capacities);
        payload.method(&[], &[], &[OutputTag::from_byte(byte)]);
        let kind = if byte & 0b1000_0000 == 0 {
            payload.return_value_size(0);
            TransactionOutputKind::ReturnValue
        } else {
            TransactionOutputKind::Output
        };
        let payload = payload.as_u128_slice();

        if usize::from(byte & 0b0111_1111) >= MAX_OUTPUTS {
            assert_eq!(
                decode_and_call(payload),
                Err(TransactionPayloadDecoderError::InvalidOutputTag(byte)),
                "Output tag {byte:#010b}"
            );
            continue;
        }
        assert_eq!(decode_and_call(payload), Ok(1), "Output tag {byte:#010b}");

        let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
        let mut decoder = TransactionPayloadDecoder::new(payload, &mut declared_outputs).unwrap();
        let method = decoder.next_method().unwrap().unwrap();
        assert_eq!(
            method.outputs(),
            &[DecodedOutput {
                output_index: byte & 0b0111_1111,
                kind,
                capacity: 0,
                size: 0,
            }],
            "Output tag {byte:#010b}"
        );
    }
}

#[test]
fn payload_decode_return_value_not_last() {
    // A return value followed by an `#[output]` or by another return value
    for second_output_tag in [OutputTag::output(1), OutputTag::return_value(1)] {
        let mut payload = TestPayload::new(&[4, 4]);
        payload.method(&[], &[], &[OutputTag::return_value(0), second_output_tag]);

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::ReturnValueNotLast(0)),
            "Second output tag {second_output_tag:?}"
        );
    }

    // The same in a later method
    let mut payload = TestPayload::new(&[4, 4, 4, 4]);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload.method(
        &[],
        &[],
        &[
            OutputTag::output(1),
            OutputTag::return_value(2),
            OutputTag::output(3),
        ],
    );
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::ReturnValueNotLast(2))
    );

    // A return value after `#[output]` arguments is accepted
    let mut payload = TestPayload::new(&[4, 4, 4]);
    payload
        .method(
            &[],
            &[],
            &[
                OutputTag::output(0),
                OutputTag::output(1),
                OutputTag::return_value(2),
            ],
        )
        .return_value_size(4);
    assert_eq!(decode_and_call(payload.as_u128_slice()), Ok(1));
}

#[test]
fn payload_decode_output_index_not_found() {
    // A method writes an output that the payload doesn't declare, as `#[output]` or as return
    // value
    for output_tag in [OutputTag::output(1), OutputTag::return_value(1)] {
        let mut payload = TestPayload::new(&[Address::SIZE]);
        payload
            .method(&[], &[], &[output_tag])
            .return_value_size(Address::SIZE);
        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::OutputIndexNotFound(1)),
            "Output tag {output_tag:?}"
        );
    }

    // A slot or an input that refers to an output that the payload doesn't declare, or that no
    // earlier method wrote
    for capacities in [&[Address::SIZE][..], &[Address::SIZE, Address::SIZE]] {
        for (slot_tags, input_tags) in [
            (&[SlotTag::output_index(1)][..], &[][..]),
            (&[], &[InputTag::output_index(1)]),
        ] {
            let mut payload = TestPayload::new(capacities);
            payload
                .method(&[], &[], &[OutputTag::return_value(0)])
                .return_value_size(Address::SIZE);
            payload.method(slot_tags, input_tags, &[]);

            assert_eq!(
                decode_and_call(payload.as_u128_slice()),
                Err(TransactionPayloadDecoderError::OutputIndexNotFound(1)),
                "{} declared outputs, slots {slot_tags:?}, inputs {input_tags:?}",
                capacities.len()
            );
        }
    }

    // Both refer to an output that an earlier method wrote
    let mut payload = TestPayload::new(&[Address::SIZE, Address::SIZE]);
    payload
        .method(&[], &[], &[OutputTag::return_value(1)])
        .return_value_size(Address::SIZE);
    payload.method(
        &[SlotTag::output_index(1)],
        &[InputTag::output_index(1)],
        &[],
    );
    assert_eq!(decode_and_call(payload.as_u128_slice()), Ok(2));
}

#[test]
fn payload_decode_output_used_twice() {
    // A method writes the same declared output twice, as `#[output]`s or with its return value
    for second_output_tag in [OutputTag::output(0), OutputTag::return_value(0)] {
        let mut payload = TestPayload::new(&[4]);
        payload
            .method(&[], &[], &[OutputTag::output(0), second_output_tag])
            .return_value_size(4);

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::OutputUsedTwice(0)),
            "Second output tag {second_output_tag:?}"
        );
    }

    // The same with an output that an earlier method wrote
    let mut payload = TestPayload::new(&[4, 4]);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload.method(
        &[],
        &[],
        &[
            OutputTag::output(1),
            OutputTag::output(0),
            OutputTag::output(0),
        ],
    );
    assert_eq!(
        decode_and_call(payload.as_u128_slice()),
        Err(TransactionPayloadDecoderError::OutputUsedTwice(0))
    );
}

#[test]
fn payload_decode_output_referenced_by_same_method() {
    // A slot or an input refers to the output that the same method writes as `#[output]` or as
    // return value, after an earlier method wrote it
    for output_tag in [OutputTag::output(0), OutputTag::return_value(0)] {
        for (slot_tags, input_tags) in [
            (&[SlotTag::output_index(0)][..], &[][..]),
            (&[], &[InputTag::output_index(0)]),
        ] {
            let mut payload = TestPayload::new(&[Address::SIZE]);
            payload
                .method(&[], &[], &[OutputTag::return_value(0)])
                .return_value_size(Address::SIZE);
            payload
                .method(slot_tags, input_tags, &[output_tag])
                .return_value_size(Address::SIZE);

            assert_eq!(
                decode_and_call(payload.as_u128_slice()),
                Err(TransactionPayloadDecoderError::OutputReferencedBySameMethod(0)),
                "Output tag {output_tag:?}, slots {slot_tags:?}, inputs {input_tags:?}"
            );
        }
    }
}

#[test]
fn payload_decode_invalid_output_tag_in_scans() {
    // The checks for double use and for references to outputs of the same method scan output tags
    // that weren't checked yet. An invalid tag in them is never a use of a declared output, so the
    // error is about the tag.
    for tag in [0b1001_0000, 0b0001_0000] {
        // After an `#[output]` of the same method
        let mut payload = TestPayload::new(&[Address::SIZE]);
        payload.method(&[], &[], &[OutputTag::output(0), OutputTag::from_byte(tag)]);
        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::InvalidOutputTag(tag)),
            "Invalid tag {tag:#010b} after an output"
        );

        // In a method whose slot or input refers to a declared output that an earlier method wrote
        for (slot_tags, input_tags) in [
            (&[SlotTag::output_index(0)][..], &[][..]),
            (&[], &[InputTag::output_index(0)]),
        ] {
            let mut payload = TestPayload::new(&[Address::SIZE]);
            payload
                .method(&[], &[], &[OutputTag::return_value(0)])
                .return_value_size(Address::SIZE);
            payload.method(slot_tags, input_tags, &[OutputTag::from_byte(tag)]);

            assert_eq!(
                decode_and_call(payload.as_u128_slice()),
                Err(TransactionPayloadDecoderError::InvalidOutputTag(tag)),
                "Invalid tag {tag:#010b}, slots {slot_tags:?}, inputs {input_tags:?}"
            );
        }
    }
}

#[test]
fn payload_decode_return_value_too_large() {
    // The size of a return value can be the capacity of its declared output, but not more
    for (size, expected) in [
        (4, Ok(1)),
        (
            5,
            Err(TransactionPayloadDecoderError::ReturnValueTooLarge {
                output_index: 0,
                size: 5,
            }),
        ),
    ] {
        let mut payload = TestPayload::new(&[4]);
        payload
            .method(&[], &[], &[OutputTag::return_value(0)])
            .return_value_size(size);

        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            expected,
            "Size {size}"
        );
    }
}

#[test]
fn payload_decode_slot_return_value_size() {
    // A return value used as a slot must be declared with the size of an address
    for size in [0, 8, Address::SIZE - 1, Address::SIZE + 1, 32] {
        let mut payload = TestPayload::new(&[32]);
        payload
            .method(&[], &[], &[OutputTag::return_value(0)])
            .return_value_size(size);
        payload.method(&[SlotTag::output_index(0)], &[], &[]);

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
fn payload_decode_too_many_declared_outputs() {
    // A payload declares at most `MAX_OUTPUTS` outputs
    let capacities = [0; usize::from(u8::MAX)];

    let payload = TestPayload::new(&capacities[..MAX_OUTPUTS]);
    assert_eq!(decode_and_call(payload.as_u128_slice()), Ok(0));

    for num_outputs in [MAX_OUTPUTS + 1, capacities.len()] {
        let payload = TestPayload::new(&capacities[..num_outputs]);
        assert_eq!(
            decode_and_call(payload.as_u128_slice()),
            Err(TransactionPayloadDecoderError::TooManyDeclaredOutputs(
                u8::try_from(num_outputs).unwrap()
            )),
            "{num_outputs} declared outputs"
        );
    }
}

#[test]
fn payload_decode_unused_declared_output() {
    // The second declared output is never written, which is accepted, although
    // `TransactionPayloadBuilder` doesn't create such payloads
    let mut payload = TestPayload::new(&[4, 8]);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload.method(&[], &[InputTag::output_index(0)], &[]);

    assert_eq!(decode_and_call(payload.as_u128_slice()), Ok(2));
}
