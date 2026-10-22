//! Encodings of tags, and how errors print them

extern crate alloc;

use crate::payload::{
    MAX_OUTPUT_INDEX, TransactionInput, TransactionInputType, TransactionOutput,
    TransactionOutputType, TransactionPayloadDecoderError, TransactionSlot, TransactionSlotType,
};
use ab_io_type::alignment::Alignment;
use alloc::string::ToString;

/// Tag of an explicit slot address
const SLOT_ADDRESS: u8 = 0b1000_0000;
/// Tag of an input value, with the power of two of its alignment in the lower bits
const INPUT_VALUE: u8 = 0b1000_0000;
/// Tag of an `#[output]`, with the power of two of its alignment in the lower bits
const OUTPUT: u8 = 0b1000_0000;
/// Tag of a return value, with the power of two of its alignment in the lower bits
const RETURN_VALUE: u8 = 0b0000_0000;

#[test]
fn payload_decode_tag_errors_in_binary() {
    // Tags are printed in binary, where their bits are easy to read
    assert_eq!(
        TransactionPayloadDecoderError::InvalidSlotTag(0b1000_0001).to_string(),
        "Invalid slot tag: 0b10000001"
    );
    assert_eq!(
        TransactionPayloadDecoderError::InvalidInputTag(0b1000_0101).to_string(),
        "Invalid input tag: 0b10000101"
    );
    assert_eq!(
        TransactionPayloadDecoderError::InvalidOutputTag(0b0000_0101).to_string(),
        "Invalid output tag: 0b00000101"
    );
}

#[test]
fn transaction_slot_encoding() {
    assert_eq!(TransactionSlot::new_address().into_u8(), SLOT_ADDRESS);
    for output_index in 0..=u8::MAX {
        let slot = TransactionSlot::new_output_index(output_index);
        if output_index <= MAX_OUTPUT_INDEX {
            let slot = slot.unwrap();
            assert_eq!(
                slot.slot_type(),
                TransactionSlotType::OutputIndex { output_index }
            );
            assert_eq!(slot.into_u8(), output_index);
        } else {
            assert!(slot.is_none());
        }
    }

    // Only bytes that `into_u8()` returns are accepted, and they round-trip
    for byte in 0..=u8::MAX {
        let accepted = byte <= MAX_OUTPUT_INDEX || byte == SLOT_ADDRESS;
        assert_eq!(
            TransactionSlot::from_u8(byte).map(TransactionSlot::into_u8),
            accepted.then_some(byte),
            "Byte {byte:#010b}"
        );
    }
}

#[test]
fn transaction_input_encoding() {
    for power in 0..=Alignment::MAX.power {
        let alignment = Alignment::from_power(power).unwrap();

        let input = TransactionInput::new_value(alignment);
        assert_eq!(
            input.input_type(),
            TransactionInputType::Value { alignment }
        );
        assert_eq!(input.into_u8(), INPUT_VALUE | power);
    }
    for output_index in 0..=u8::MAX {
        let input = TransactionInput::new_output_index(output_index);
        if output_index <= MAX_OUTPUT_INDEX {
            let input = input.unwrap();
            assert_eq!(
                input.input_type(),
                TransactionInputType::OutputIndex { output_index }
            );
            assert_eq!(input.into_u8(), output_index);
        } else {
            assert!(input.is_none());
        }
    }

    // Only bytes that `into_u8()` returns are accepted, and they round-trip
    for byte in 0..=u8::MAX {
        let accepted = byte <= MAX_OUTPUT_INDEX || byte & !INPUT_VALUE <= Alignment::MAX.power;
        assert_eq!(
            TransactionInput::from_u8(byte).map(TransactionInput::into_u8),
            accepted.then_some(byte),
            "Byte {byte:#010b}"
        );
    }
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
        assert_eq!(output.into_u8(), OUTPUT | power);

        let return_value = TransactionOutput::new_return_value(alignment);
        assert_eq!(
            return_value.output_type(),
            TransactionOutputType::ReturnValue { alignment }
        );
        assert_eq!(return_value.into_u8(), RETURN_VALUE | power);
    }

    // Only bytes that `into_u8()` returns are accepted, and they round-trip
    for byte in 0..=u8::MAX {
        let accepted = byte & !OUTPUT <= Alignment::MAX.power;
        assert_eq!(
            TransactionOutput::from_u8(byte).map(TransactionOutput::into_u8),
            accepted.then_some(byte),
            "Byte {byte:#010b}"
        );
    }
}
