use crate::metadata::IoTypeMetadataKind;

#[test]
fn check_repr() {
    let known_variants = [
        (IoTypeMetadataKind::Unit, 0),
        (IoTypeMetadataKind::Bool, 1),
        (IoTypeMetadataKind::U8, 2),
        (IoTypeMetadataKind::U16, 3),
        (IoTypeMetadataKind::U32, 4),
        (IoTypeMetadataKind::U64, 5),
        (IoTypeMetadataKind::U128, 6),
        (IoTypeMetadataKind::I8, 7),
        (IoTypeMetadataKind::I16, 8),
        (IoTypeMetadataKind::I32, 9),
        (IoTypeMetadataKind::I64, 10),
        (IoTypeMetadataKind::I128, 11),
        (IoTypeMetadataKind::Struct, 12),
        (IoTypeMetadataKind::Enum, 13),
        (IoTypeMetadataKind::EnumNoFields, 14),
        (IoTypeMetadataKind::Array, 15),
        (IoTypeMetadataKind::VariableElements, 16),
        (IoTypeMetadataKind::FixedCapacityElements, 17),
        (IoTypeMetadataKind::FixedCapacityString, 18),
        (IoTypeMetadataKind::Unaligned, 19),
    ];

    for (kind, repr_byte) in known_variants {
        assert_eq!(kind as u8, repr_byte);
        assert_eq!(IoTypeMetadataKind::try_from(repr_byte), Ok(kind));
    }

    for byte in u8::try_from(known_variants.len()).unwrap()..=u8::MAX {
        assert_eq!(IoTypeMetadataKind::try_from(byte), Err(()));
    }
}
