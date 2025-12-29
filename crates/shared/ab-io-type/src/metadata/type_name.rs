use crate::metadata::IoTypeMetadataKind;

#[inline(always)]
pub(super) const fn type_name(mut metadata: &[u8]) -> Option<&[u8]> {
    if metadata.is_empty() {
        return None;
    }

    let kind = IoTypeMetadataKind::try_from_u8(metadata[0])?;
    metadata = skip_n_bytes(metadata, 1)?;

    Some(match kind {
        IoTypeMetadataKind::Unit => b"()",
        IoTypeMetadataKind::Bool => b"bool",
        IoTypeMetadataKind::U8 => b"u8",
        IoTypeMetadataKind::U16 => b"u16",
        IoTypeMetadataKind::U32 => b"u32",
        IoTypeMetadataKind::U64 => b"u64",
        IoTypeMetadataKind::U128 => b"u128",
        IoTypeMetadataKind::I8 => b"i8",
        IoTypeMetadataKind::I16 => b"i16",
        IoTypeMetadataKind::I32 => b"i32",
        IoTypeMetadataKind::I64 => b"i64",
        IoTypeMetadataKind::I128 => b"i128",
        IoTypeMetadataKind::Struct
        | IoTypeMetadataKind::Struct0
        | IoTypeMetadataKind::Struct1
        | IoTypeMetadataKind::Struct2
        | IoTypeMetadataKind::Struct3
        | IoTypeMetadataKind::Struct4
        | IoTypeMetadataKind::Struct5
        | IoTypeMetadataKind::Struct6
        | IoTypeMetadataKind::Struct7
        | IoTypeMetadataKind::Struct8
        | IoTypeMetadataKind::Struct9
        | IoTypeMetadataKind::Struct10
        | IoTypeMetadataKind::TupleStruct
        | IoTypeMetadataKind::TupleStruct1
        | IoTypeMetadataKind::TupleStruct2
        | IoTypeMetadataKind::TupleStruct3
        | IoTypeMetadataKind::TupleStruct4
        | IoTypeMetadataKind::TupleStruct5
        | IoTypeMetadataKind::TupleStruct6
        | IoTypeMetadataKind::TupleStruct7
        | IoTypeMetadataKind::TupleStruct8
        | IoTypeMetadataKind::TupleStruct9
        | IoTypeMetadataKind::TupleStruct10
        | IoTypeMetadataKind::Enum
        | IoTypeMetadataKind::Enum1
        | IoTypeMetadataKind::Enum2
        | IoTypeMetadataKind::Enum3
        | IoTypeMetadataKind::Enum4
        | IoTypeMetadataKind::Enum5
        | IoTypeMetadataKind::Enum6
        | IoTypeMetadataKind::Enum7
        | IoTypeMetadataKind::Enum8
        | IoTypeMetadataKind::Enum9
        | IoTypeMetadataKind::Enum10
        | IoTypeMetadataKind::EnumNoFields
        | IoTypeMetadataKind::EnumNoFields1
        | IoTypeMetadataKind::EnumNoFields2
        | IoTypeMetadataKind::EnumNoFields3
        | IoTypeMetadataKind::EnumNoFields4
        | IoTypeMetadataKind::EnumNoFields5
        | IoTypeMetadataKind::EnumNoFields6
        | IoTypeMetadataKind::EnumNoFields7
        | IoTypeMetadataKind::EnumNoFields8
        | IoTypeMetadataKind::EnumNoFields9
        | IoTypeMetadataKind::EnumNoFields10 => {
            if metadata.is_empty() {
                return None;
            }

            let type_name_length = metadata[0] as usize;
            metadata = skip_n_bytes(metadata, 1)?;

            if metadata.len() < type_name_length {
                return None;
            }

            let (type_name, _) = metadata.split_at(type_name_length);
            type_name
        }
        IoTypeMetadataKind::Array8b
        | IoTypeMetadataKind::Array16b
        | IoTypeMetadataKind::Array32b => b"[T; N]",
        IoTypeMetadataKind::ArrayU8x8 => b"[u8; 8]",
        IoTypeMetadataKind::ArrayU8x16 => b"[u8; 16]",
        IoTypeMetadataKind::ArrayU8x32 => b"[u8; 32]",
        IoTypeMetadataKind::ArrayU8x64 => b"[u8; 64]",
        IoTypeMetadataKind::ArrayU8x128 => b"[u8; 128]",
        IoTypeMetadataKind::ArrayU8x256 => b"[u8; 256]",
        IoTypeMetadataKind::ArrayU8x512 => b"[u8; 512]",
        IoTypeMetadataKind::ArrayU8x1024 => b"[u8; 1024]",
        IoTypeMetadataKind::ArrayU8x2028 => b"[u8; 2028]",
        IoTypeMetadataKind::ArrayU8x4096 => b"[u8; 4096]",
        IoTypeMetadataKind::VariableBytes8b
        | IoTypeMetadataKind::VariableBytes16b
        | IoTypeMetadataKind::VariableBytes32b
        | IoTypeMetadataKind::VariableBytes0
        | IoTypeMetadataKind::VariableBytes512
        | IoTypeMetadataKind::VariableBytes1024
        | IoTypeMetadataKind::VariableBytes2028
        | IoTypeMetadataKind::VariableBytes4096
        | IoTypeMetadataKind::VariableBytes8192
        | IoTypeMetadataKind::VariableBytes16384
        | IoTypeMetadataKind::VariableBytes32768
        | IoTypeMetadataKind::VariableBytes65536
        | IoTypeMetadataKind::VariableBytes131072
        | IoTypeMetadataKind::VariableBytes262144
        | IoTypeMetadataKind::VariableBytes524288
        | IoTypeMetadataKind::VariableBytes1048576 => b"VariableBytes",
        IoTypeMetadataKind::VariableElements8b
        | IoTypeMetadataKind::VariableElements16b
        | IoTypeMetadataKind::VariableElements32b
        | IoTypeMetadataKind::VariableElements0 => b"VariableElements",
        IoTypeMetadataKind::FixedCapacityBytes8b | IoTypeMetadataKind::FixedCapacityBytes16b => {
            b"FixedCapacityBytes"
        }
        IoTypeMetadataKind::FixedCapacityString8b | IoTypeMetadataKind::FixedCapacityString16b => {
            b"FixedCapacityString"
        }
        IoTypeMetadataKind::Unaligned => b"Unaligned",
        IoTypeMetadataKind::Address => b"Address",
        IoTypeMetadataKind::Balance => b"Balance",
    })
}

/// Skips `n` bytes and return remainder
#[inline(always)]
const fn skip_n_bytes(input: &[u8], n: usize) -> Option<&[u8]> {
    input.get(n..)
}
