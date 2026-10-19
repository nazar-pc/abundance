//! Encodings of tags, and how errors print them

extern crate alloc;

use crate::MAX_OUTPUTS;
use crate::payload::{
    TransactionInput, TransactionInputType, TransactionOutput, TransactionOutputType,
    TransactionPayloadDecoderError, TransactionSlot, TransactionSlotType,
};
use ab_io_type::alignment::Alignment;
use alloc::string::ToString;

/// Tag of an explicit slot address
const SLOT_ADDRESS: u8 = 0b1000_0000;
/// Tag of an input value, with the power of two of its alignment in the lower bits
const INPUT_VALUE: u8 = 0b1000_0000;
/// Tag of an `#[output]`, with the index of the declared output in the lower bits
const OUTPUT: u8 = 0b1000_0000;
/// Tag of a return value, with the index of the declared output in the lower bits
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
        TransactionPayloadDecoderError::InvalidOutputTag(0b1001_0000).to_string(),
        "Invalid output tag: 0b10010000"
    );
}

#[test]
fn transaction_slot_encoding() {
    assert_eq!(TransactionSlot::new_address().into_u8(), SLOT_ADDRESS);
    for output_index in 0..=u8::MAX {
        let slot = TransactionSlot::new_output_index(output_index);
        if usize::from(output_index) < MAX_OUTPUTS {
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
        let accepted = usize::from(byte) < MAX_OUTPUTS || byte == SLOT_ADDRESS;
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
        if usize::from(output_index) < MAX_OUTPUTS {
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
        let accepted = usize::from(byte) < MAX_OUTPUTS
            || (byte & INPUT_VALUE != 0 && byte & !INPUT_VALUE <= Alignment::MAX.power);
        assert_eq!(
            TransactionInput::from_u8(byte).map(TransactionInput::into_u8),
            accepted.then_some(byte),
            "Byte {byte:#010b}"
        );
    }
}

#[test]
fn transaction_output_encoding() {
    for output_index in 0..=u8::MAX {
        let output = TransactionOutput::new_output(output_index);
        let return_value = TransactionOutput::new_return_value(output_index);
        if usize::from(output_index) < MAX_OUTPUTS {
            let output = output.unwrap();
            assert_eq!(
                output.output_type(),
                TransactionOutputType::Output { output_index }
            );
            assert_eq!(output.into_u8(), OUTPUT | output_index);

            let return_value = return_value.unwrap();
            assert_eq!(
                return_value.output_type(),
                TransactionOutputType::ReturnValue { output_index }
            );
            assert_eq!(return_value.into_u8(), RETURN_VALUE | output_index);
        } else {
            assert!(output.is_none());
            assert!(return_value.is_none());
        }
    }

    // Only bytes that `into_u8()` returns are accepted, and they round-trip
    for byte in 0..=u8::MAX {
        let output_index = byte & 0b0111_1111;
        let expected = if usize::from(output_index) >= MAX_OUTPUTS {
            None
        } else if byte & OUTPUT == 0 {
            Some(TransactionOutputType::ReturnValue { output_index })
        } else {
            Some(TransactionOutputType::Output { output_index })
        };
        let output = TransactionOutput::from_u8(byte);
        assert_eq!(
            output.map(TransactionOutput::output_type),
            expected,
            "Byte {byte:#010b}"
        );
        assert_eq!(
            output.map(TransactionOutput::into_u8),
            expected.is_some().then_some(byte),
            "Byte {byte:#010b}"
        );
    }
}
