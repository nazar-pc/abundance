//! Descriptions of methods, and what the decoder does after an error

use crate::MAX_OUTPUTS;
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
use ab_io_type::trivial_type::TrivialType;
use core::mem::MaybeUninit;

/// Method of [`TestPayload`] without arguments, which tests change with struct update syntax
fn method() -> ExpectedMethod<'static> {
    ExpectedMethod {
        contract: &TestPayload::CONTRACT,
        fingerprint: TestPayload::fingerprint(),
        context: TransactionMethodContext::Wallet,
        slots: &[],
        inputs: &[],
        outputs: &[],
    }
}

#[test]
fn payload_decode_description() {
    let address = Address::SYSTEM_STATE;
    let value_u8 = [5];
    let value_u128 = 7u128.to_le_bytes();

    let mut payload = TestPayload::new(&[4, Address::SIZE]);
    // A slot address, inputs with alignments 1 and 16, an `#[output]` that writes declared output
    // 0 and a return value that writes declared output 1
    payload
        .method(
            &[SlotTag::address()],
            &[InputTag::value(1), InputTag::value(16)],
            &[OutputTag::output(0), OutputTag::return_value(1)],
        )
        .address(&address)
        .input_value(&value_u8, 1)
        .input_value(&value_u128, 16)
        .return_value_size(Address::SIZE);
    // A slot from the return value and an input from the `#[output]` of the first method
    payload.method_with_context(
        ContextByte::Valid(TransactionMethodContext::Null),
        &[SlotTag::output_index(1)],
        &[InputTag::output_index(0)],
        &[],
    );

    let output = OutputRef {
        output_index: 0,
        kind: TransactionOutputKind::Output,
        offset: 0,
        size: 0,
    };
    let return_value = OutputRef {
        output_index: 1,
        kind: TransactionOutputKind::ReturnValue,
        offset: 0,
        size: Address::SIZE,
    };
    assert_decoded(
        payload.as_u128_slice(),
        &[4, Address::SIZE],
        &[
            ExpectedMethod {
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
                        capacity: 4,
                        size: 0,
                    },
                    DecodedOutput {
                        output_index: 1,
                        kind: TransactionOutputKind::ReturnValue,
                        capacity: Address::SIZE,
                        size: Address::SIZE,
                    },
                ],
                ..method()
            },
            ExpectedMethod {
                context: TransactionMethodContext::Null,
                slots: &[DecodedSlot::Output(return_value)],
                inputs: &[DecodedInput::Output(output)],
                ..method()
            },
        ],
    );
    assert_eq!(output.return_value_size(), None);
    assert_eq!(return_value.return_value_size(), Some(Address::SIZE));
}

#[test]
fn payload_decode_failed_earlier() {
    // The second method has an invalid context
    let mut payload = TestPayload::new(&[]);
    payload.method(&[], &[], &[]);
    payload.method_with_context(ContextByte::Invalid(2), &[], &[], &[]);
    payload.method(&[], &[], &[]);

    let mut declared_outputs = [MaybeUninit::uninit(); MAX_OUTPUTS];
    let mut decoder =
        TransactionPayloadDecoder::new(payload.as_u128_slice(), &mut declared_outputs).unwrap();
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

#[test]
fn payload_decode_output_reused_as_output() {
    // Three methods write the same declared output as `#[output]`, and a fourth one reads it
    let mut payload = TestPayload::new(&[64]);
    for _ in 0..3 {
        payload.method(&[], &[], &[OutputTag::output(0)]);
    }
    payload.method(&[], &[InputTag::output_index(0)], &[]);

    let output_method = ExpectedMethod {
        outputs: &[DecodedOutput {
            output_index: 0,
            kind: TransactionOutputKind::Output,
            capacity: 64,
            size: 0,
        }],
        ..method()
    };
    assert_decoded(
        payload.as_u128_slice(),
        &[64],
        &[
            output_method,
            output_method,
            output_method,
            ExpectedMethod {
                inputs: &[DecodedInput::Output(OutputRef {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    offset: 0,
                    size: 0,
                })],
                ..method()
            },
        ],
    );
}

#[test]
fn payload_decode_output_reused_by_return_value() {
    // An `#[output]`, then a return value of an address into the same declared output, which
    // later methods get as an input and as a slot
    let mut payload = TestPayload::new(&[64]);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload
        .method(&[], &[], &[OutputTag::return_value(0)])
        .return_value_size(Address::SIZE);
    payload.method(&[], &[InputTag::output_index(0)], &[]);
    payload.method(&[SlotTag::output_index(0)], &[], &[]);

    let return_value = OutputRef {
        output_index: 0,
        kind: TransactionOutputKind::ReturnValue,
        offset: 0,
        size: Address::SIZE,
    };
    assert_decoded(
        payload.as_u128_slice(),
        &[64],
        &[
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    capacity: 64,
                    size: 0,
                }],
                ..method()
            },
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    capacity: 64,
                    size: Address::SIZE,
                }],
                ..method()
            },
            ExpectedMethod {
                inputs: &[DecodedInput::Output(return_value)],
                ..method()
            },
            ExpectedMethod {
                slots: &[DecodedSlot::Output(return_value)],
                ..method()
            },
        ],
    );
}

#[test]
fn payload_decode_return_value_reused_by_output() {
    // A return value of an address, then an `#[output]` into the same declared output, which later
    // methods get as an input and as a slot. The decoder accepts the slot, since the size of an
    // `#[output]` is only known after its method was called.
    let mut payload = TestPayload::new(&[Address::SIZE]);
    payload
        .method(&[], &[], &[OutputTag::return_value(0)])
        .return_value_size(Address::SIZE);
    payload.method(&[], &[], &[OutputTag::output(0)]);
    payload.method(&[], &[InputTag::output_index(0)], &[]);
    payload.method(&[SlotTag::output_index(0)], &[], &[]);

    let output = OutputRef {
        output_index: 0,
        kind: TransactionOutputKind::Output,
        offset: 0,
        size: 0,
    };
    assert_decoded(
        payload.as_u128_slice(),
        &[Address::SIZE],
        &[
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::ReturnValue,
                    capacity: Address::SIZE,
                    size: Address::SIZE,
                }],
                ..method()
            },
            ExpectedMethod {
                outputs: &[DecodedOutput {
                    output_index: 0,
                    kind: TransactionOutputKind::Output,
                    capacity: Address::SIZE,
                    size: 0,
                }],
                ..method()
            },
            ExpectedMethod {
                inputs: &[DecodedInput::Output(output)],
                ..method()
            },
            ExpectedMethod {
                slots: &[DecodedSlot::Output(output)],
                ..method()
            },
        ],
    );
}
