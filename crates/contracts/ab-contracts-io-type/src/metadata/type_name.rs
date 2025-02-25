use crate::metadata::IoTypeMetadataKind;
use core::str;

/// This macro is necessary to reduce boilerplate due to lack of `?` in const environment
macro_rules! forward_option {
    ($expr:expr) => {{
        let Some(result) = $expr else {
            return None;
        };
        result
    }};
}

pub(super) const fn type_name(mut metadata: &[u8]) -> Option<&str> {
    if metadata.is_empty() {
        return None;
    }

    let kind = forward_option!(IoTypeMetadataKind::try_from_u8(metadata[0]));
    metadata = forward_option!(skip_n_bytes(metadata, 1));

    Some(match kind {
        IoTypeMetadataKind::Unit => "()",
        IoTypeMetadataKind::Bool => "bool",
        IoTypeMetadataKind::U8 => "u8",
        IoTypeMetadataKind::U16 => "u16",
        IoTypeMetadataKind::U32 => "u32",
        IoTypeMetadataKind::U64 => "u64",
        IoTypeMetadataKind::U128 => "u128",
        IoTypeMetadataKind::I8 => "i8",
        IoTypeMetadataKind::I16 => "i16",
        IoTypeMetadataKind::I32 => "i32",
        IoTypeMetadataKind::I64 => "i64",
        IoTypeMetadataKind::I128 => "i128",
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
            metadata = forward_option!(skip_n_bytes(metadata, 1));

            if metadata.len() < type_name_length {
                return None;
            }

            let (type_name, _) = metadata.split_at(type_name_length);
            match str::from_utf8(type_name) {
                Ok(type_name) => type_name,
                Err(_error) => {
                    return None;
                }
            }
        }
        IoTypeMetadataKind::Array8b
        | IoTypeMetadataKind::Array16b
        | IoTypeMetadataKind::Array32b => "[T; N]",
        IoTypeMetadataKind::ArrayU8x8 => "[u8; 8]",
        IoTypeMetadataKind::ArrayU8x16 => "[u8; 16]",
        IoTypeMetadataKind::ArrayU8x32 => "[u8; 32]",
        IoTypeMetadataKind::ArrayU8x64 => "[u8; 64]",
        IoTypeMetadataKind::ArrayU8x128 => "[u8; 128]",
        IoTypeMetadataKind::ArrayU8x256 => "[u8; 256]",
        IoTypeMetadataKind::ArrayU8x512 => "[u8; 512]",
        IoTypeMetadataKind::ArrayU8x1024 => "[u8; 1024]",
        IoTypeMetadataKind::ArrayU8x2028 => "[u8; 2028]",
        IoTypeMetadataKind::ArrayU8x4096 => "[u8; 4096]",
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
        | IoTypeMetadataKind::VariableBytes1048576 => "VariableBytes",
        IoTypeMetadataKind::VariableElements8b
        | IoTypeMetadataKind::VariableElements16b
        | IoTypeMetadataKind::VariableElements32b
        | IoTypeMetadataKind::VariableElements0 => "VariableElements",
        IoTypeMetadataKind::Address => "Address",
        IoTypeMetadataKind::Balance => "Balance",
    })
}

/// Skips `n` bytes and return remainder
const fn skip_n_bytes(input: &[u8], n: usize) -> Option<&[u8]> {
    if n > input.len() {
        return None;
    }

    // `&input[n..]` not supported in const yet
    Some(input.split_at(n).1)
}
