use crate::trivial_type::TrivialType;
use crate::variable_bytes::VariableBytes;

#[test]
fn read_trivial_type() {
    // Aligned for `u32`, so that both aligned and unaligned reads happen
    let buffer = [
        u32::from_le_bytes([1, 2, 3, 4]),
        u32::from_le_bytes([5, 6, 7, 8]),
    ];
    let buffer = buffer.as_bytes();

    {
        let size = 4;
        let bytes = VariableBytes::<0>::from_buffer(&buffer[..4], &size).unwrap();
        assert_eq!(
            bytes.read_trivial_type::<u32>(),
            Some(u32::from_le_bytes([1, 2, 3, 4]))
        );
    }

    {
        let size = 4;
        let bytes = VariableBytes::<0>::from_buffer(&buffer[1..5], &size).unwrap();
        assert_eq!(
            bytes.read_trivial_type::<u32>(),
            Some(u32::from_le_bytes([2, 3, 4, 5]))
        );
    }

    // Not enough bytes
    {
        let size = 3;
        let bytes = VariableBytes::<0>::from_buffer(&buffer[..3], &size).unwrap();
        assert_eq!(bytes.read_trivial_type::<u32>(), None);
    }
}
