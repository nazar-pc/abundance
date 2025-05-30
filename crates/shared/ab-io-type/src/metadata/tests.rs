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
        (IoTypeMetadataKind::Struct0, 13),
        (IoTypeMetadataKind::Struct1, 14),
        (IoTypeMetadataKind::Struct2, 15),
        (IoTypeMetadataKind::Struct3, 16),
        (IoTypeMetadataKind::Struct4, 17),
        (IoTypeMetadataKind::Struct5, 18),
        (IoTypeMetadataKind::Struct6, 19),
        (IoTypeMetadataKind::Struct7, 20),
        (IoTypeMetadataKind::Struct8, 21),
        (IoTypeMetadataKind::Struct9, 22),
        (IoTypeMetadataKind::Struct10, 23),
        (IoTypeMetadataKind::TupleStruct, 24),
        (IoTypeMetadataKind::TupleStruct1, 25),
        (IoTypeMetadataKind::TupleStruct2, 26),
        (IoTypeMetadataKind::TupleStruct3, 27),
        (IoTypeMetadataKind::TupleStruct4, 28),
        (IoTypeMetadataKind::TupleStruct5, 29),
        (IoTypeMetadataKind::TupleStruct6, 30),
        (IoTypeMetadataKind::TupleStruct7, 31),
        (IoTypeMetadataKind::TupleStruct8, 32),
        (IoTypeMetadataKind::TupleStruct9, 33),
        (IoTypeMetadataKind::TupleStruct10, 34),
        (IoTypeMetadataKind::Enum, 35),
        (IoTypeMetadataKind::Enum1, 36),
        (IoTypeMetadataKind::Enum2, 37),
        (IoTypeMetadataKind::Enum3, 38),
        (IoTypeMetadataKind::Enum4, 39),
        (IoTypeMetadataKind::Enum5, 40),
        (IoTypeMetadataKind::Enum6, 41),
        (IoTypeMetadataKind::Enum7, 42),
        (IoTypeMetadataKind::Enum8, 43),
        (IoTypeMetadataKind::Enum9, 44),
        (IoTypeMetadataKind::Enum10, 45),
        (IoTypeMetadataKind::EnumNoFields, 46),
        (IoTypeMetadataKind::EnumNoFields1, 47),
        (IoTypeMetadataKind::EnumNoFields2, 48),
        (IoTypeMetadataKind::EnumNoFields3, 49),
        (IoTypeMetadataKind::EnumNoFields4, 50),
        (IoTypeMetadataKind::EnumNoFields5, 51),
        (IoTypeMetadataKind::EnumNoFields6, 52),
        (IoTypeMetadataKind::EnumNoFields7, 53),
        (IoTypeMetadataKind::EnumNoFields8, 54),
        (IoTypeMetadataKind::EnumNoFields9, 55),
        (IoTypeMetadataKind::EnumNoFields10, 56),
        (IoTypeMetadataKind::Array8b, 57),
        (IoTypeMetadataKind::Array16b, 58),
        (IoTypeMetadataKind::Array32b, 59),
        (IoTypeMetadataKind::ArrayU8x8, 60),
        (IoTypeMetadataKind::ArrayU8x16, 61),
        (IoTypeMetadataKind::ArrayU8x32, 62),
        (IoTypeMetadataKind::ArrayU8x64, 63),
        (IoTypeMetadataKind::ArrayU8x128, 64),
        (IoTypeMetadataKind::ArrayU8x256, 65),
        (IoTypeMetadataKind::ArrayU8x512, 66),
        (IoTypeMetadataKind::ArrayU8x1024, 67),
        (IoTypeMetadataKind::ArrayU8x2028, 68),
        (IoTypeMetadataKind::ArrayU8x4096, 69),
        (IoTypeMetadataKind::VariableBytes8b, 70),
        (IoTypeMetadataKind::VariableBytes16b, 71),
        (IoTypeMetadataKind::VariableBytes32b, 72),
        (IoTypeMetadataKind::VariableBytes0, 73),
        (IoTypeMetadataKind::VariableBytes512, 74),
        (IoTypeMetadataKind::VariableBytes1024, 75),
        (IoTypeMetadataKind::VariableBytes2028, 76),
        (IoTypeMetadataKind::VariableBytes4096, 77),
        (IoTypeMetadataKind::VariableBytes8192, 78),
        (IoTypeMetadataKind::VariableBytes16384, 79),
        (IoTypeMetadataKind::VariableBytes32768, 80),
        (IoTypeMetadataKind::VariableBytes65536, 81),
        (IoTypeMetadataKind::VariableBytes131072, 82),
        (IoTypeMetadataKind::VariableBytes262144, 83),
        (IoTypeMetadataKind::VariableBytes524288, 84),
        (IoTypeMetadataKind::VariableBytes1048576, 85),
        (IoTypeMetadataKind::VariableElements8b, 86),
        (IoTypeMetadataKind::VariableElements16b, 87),
        (IoTypeMetadataKind::VariableElements32b, 88),
        (IoTypeMetadataKind::VariableElements0, 89),
        (IoTypeMetadataKind::FixedCapacityBytes8b, 90),
        (IoTypeMetadataKind::FixedCapacityBytes16b, 91),
        (IoTypeMetadataKind::FixedCapacityString8b, 92),
        (IoTypeMetadataKind::FixedCapacityString16b, 93),
        (IoTypeMetadataKind::Unaligned, 94),
        (IoTypeMetadataKind::Address, 128),
        (IoTypeMetadataKind::Balance, 129),
    ];

    for (kind, repr_byte) in known_variants {
        assert_eq!(kind as u8, repr_byte);
        assert_eq!(IoTypeMetadataKind::try_from_u8(repr_byte), Some(kind));
    }
}
