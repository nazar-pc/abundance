//! Descriptions of methods, and what the decoder does after an error

use crate::OUTPUT_BUFFER_OFFSETS_SIZE;
use crate::payload::decoder::{
    DecodedInput, DecodedOutput, DecodedSlot, OutputRef, TransactionPayloadDecoder,
};
use crate::payload::test_utils::{
    ContextByte, ExpectedMethod, InputTag, OutputTag, SlotTag, TestPayload, alignment,
    assert_decoded,
};
use crate::payload::{
    TransactionMethodContext, TransactionOutputKind, TransactionPayloadDecoderError,
};
use ab_core_primitives::address::Address;
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;

#[test]
fn payload_decode_description() {
    let address = Address::SYSTEM_STATE;
    let value_u8 = [5];
    let value_u128 = 7u128.to_le_bytes();
    let address_alignment = Alignment::of::<Address>().get();

    let mut payload = TestPayload::new();
    // A slot address, inputs with alignments 1 and 16, an `#[output]` and a return value
    payload
        .method(
            &[SlotTag::address()],
            &[InputTag::value(1), InputTag::value(16)],
            2,
        )
        .address(&address)
        .input_value(&value_u8, 1)
        .input_value(&value_u128, 16)
        .output(4, OutputTag::output(4))
        .output(Address::SIZE, OutputTag::return_value(address_alignment));
    // A slot from the return value and an input from the `#[output]` of the first method
    payload.method_with_context(
        ContextByte::Valid(TransactionMethodContext::Null),
        &[SlotTag::output_index(1)],
        &[InputTag::output_index(0)],
        0,
    );

    let output = OutputRef {
        output_index: 0,
        kind: TransactionOutputKind::Output,
        alignment: alignment(4),
        offset: 0,
        size: 0,
    };
    let return_value = OutputRef {
        output_index: 1,
        kind: TransactionOutputKind::ReturnValue,
        alignment: alignment(address_alignment),
        offset: 0,
        size: Address::SIZE,
    };
    assert_decoded(
        payload.as_u128_slice(),
        &[
            ExpectedMethod {
                contract: &TestPayload::CONTRACT,
                fingerprint: TestPayload::fingerprint(),
                context: TransactionMethodContext::Wallet,
                slots: &[DecodedSlot::Address(&address)],
                inputs: &[
                    DecodedInput::Value {
                        bytes: &value_u8,
                        alignment: alignment(1),
                    },
                    DecodedInput::Value {
                        bytes: &value_u128,
                        alignment: alignment(16),
                    },
                ],
                outputs: &[
                    DecodedOutput {
                        output_index: 0,
                        kind: TransactionOutputKind::Output,
                        alignment: alignment(4),
                        capacity: 4,
                        size: 0,
                    },
                    DecodedOutput {
                        output_index: 1,
                        kind: TransactionOutputKind::ReturnValue,
                        alignment: alignment(address_alignment),
                        capacity: Address::SIZE,
                        size: Address::SIZE,
                    },
                ],
            },
            ExpectedMethod {
                contract: &TestPayload::CONTRACT,
                fingerprint: TestPayload::fingerprint(),
                context: TransactionMethodContext::Null,
                slots: &[DecodedSlot::Output(return_value)],
                inputs: &[DecodedInput::Output(output)],
                outputs: &[],
            },
        ],
    );
    assert_eq!(output.return_value_size(), None);
    assert_eq!(return_value.return_value_size(), Some(Address::SIZE));
}

#[test]
fn payload_decode_failed_earlier() {
    // The second method has an invalid context
    let mut payload = TestPayload::new();
    payload.method(&[], &[], 0);
    payload.method_with_context(ContextByte::Invalid(2), &[], &[], 0);
    payload.method(&[], &[], 0);

    let mut output_buffer_details = [MaybeUninit::uninit(); OUTPUT_BUFFER_OFFSETS_SIZE];
    let mut decoder =
        TransactionPayloadDecoder::new(payload.as_u128_slice(), &mut output_buffer_details);
    assert!(decoder.next_method().unwrap().is_some());
    assert_eq!(
        decoder.next_method().map(|_maybe_method| ()),
        Err(TransactionPayloadDecoderError::InvalidMethodContext(2))
    );
    // The third method is valid, but decoding doesn't continue after an error
    for _ in 0..2 {
        assert_eq!(
            decoder.next_method().map(|_maybe_method| ()),
            Err(TransactionPayloadDecoderError::FailedEarlier)
        );
    }
}
