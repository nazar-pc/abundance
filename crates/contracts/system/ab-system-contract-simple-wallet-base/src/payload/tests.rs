//! Encodings of tags

use crate::payload::{TransactionOutput, TransactionOutputType};
use ab_io_type::alignment::Alignment;

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
