use crate::metadata::IoTypeMetadataKind;

pub(super) const fn compact_metadata<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    let kind = IoTypeMetadataKind::try_from_u8(input[0])?;

    match kind {
        IoTypeMetadataKind::Unit
        | IoTypeMetadataKind::Bool
        | IoTypeMetadataKind::U8
        | IoTypeMetadataKind::U16
        | IoTypeMetadataKind::U32
        | IoTypeMetadataKind::U64
        | IoTypeMetadataKind::U128
        | IoTypeMetadataKind::I8
        | IoTypeMetadataKind::I16
        | IoTypeMetadataKind::I32
        | IoTypeMetadataKind::I64
        | IoTypeMetadataKind::I128 => copy_n_bytes(input, output, 1),
        IoTypeMetadataKind::Struct => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, None, false)
        }
        IoTypeMetadataKind::Struct0 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(0), false)
        }
        IoTypeMetadataKind::Struct1 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct1 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(1), false)
        }
        IoTypeMetadataKind::Struct2 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct2 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(2), false)
        }
        IoTypeMetadataKind::Struct3 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct3 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(3), false)
        }
        IoTypeMetadataKind::Struct4 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct4 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(4), false)
        }
        IoTypeMetadataKind::Struct5 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct5 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(5), false)
        }
        IoTypeMetadataKind::Struct6 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct6 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(6), false)
        }
        IoTypeMetadataKind::Struct7 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct7 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(7), false)
        }
        IoTypeMetadataKind::Struct8 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct8 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(8), false)
        }
        IoTypeMetadataKind::Struct9 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct9 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(9), false)
        }
        IoTypeMetadataKind::Struct10 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct10 as u8;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            compact_struct(input, output, Some(10), false)
        }
        IoTypeMetadataKind::TupleStruct => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, None, true)
        }
        IoTypeMetadataKind::TupleStruct1 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(1), true)
        }
        IoTypeMetadataKind::TupleStruct2 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(2), true)
        }
        IoTypeMetadataKind::TupleStruct3 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(3), true)
        }
        IoTypeMetadataKind::TupleStruct4 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(4), true)
        }
        IoTypeMetadataKind::TupleStruct5 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(5), true)
        }
        IoTypeMetadataKind::TupleStruct6 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(6), true)
        }
        IoTypeMetadataKind::TupleStruct7 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(7), true)
        }
        IoTypeMetadataKind::TupleStruct8 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(8), true)
        }
        IoTypeMetadataKind::TupleStruct9 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(9), true)
        }
        IoTypeMetadataKind::TupleStruct10 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_struct(input, output, Some(10), true)
        }
        IoTypeMetadataKind::Enum => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, None, true)
        }
        IoTypeMetadataKind::Enum1 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(1), true)
        }
        IoTypeMetadataKind::Enum2 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(2), true)
        }
        IoTypeMetadataKind::Enum3 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(3), true)
        }
        IoTypeMetadataKind::Enum4 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(4), true)
        }
        IoTypeMetadataKind::Enum5 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(5), true)
        }
        IoTypeMetadataKind::Enum6 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(6), true)
        }
        IoTypeMetadataKind::Enum7 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(7), true)
        }
        IoTypeMetadataKind::Enum8 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(8), true)
        }
        IoTypeMetadataKind::Enum9 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(9), true)
        }
        IoTypeMetadataKind::Enum10 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(10), true)
        }
        IoTypeMetadataKind::EnumNoFields => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, None, false)
        }
        IoTypeMetadataKind::EnumNoFields1 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(1), false)
        }
        IoTypeMetadataKind::EnumNoFields2 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(2), false)
        }
        IoTypeMetadataKind::EnumNoFields3 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(3), false)
        }
        IoTypeMetadataKind::EnumNoFields4 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(4), false)
        }
        IoTypeMetadataKind::EnumNoFields5 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(5), false)
        }
        IoTypeMetadataKind::EnumNoFields6 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(6), false)
        }
        IoTypeMetadataKind::EnumNoFields7 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(7), false)
        }
        IoTypeMetadataKind::EnumNoFields8 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(8), false)
        }
        IoTypeMetadataKind::EnumNoFields9 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(9), false)
        }
        IoTypeMetadataKind::EnumNoFields10 => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_enum(input, output, Some(10), false)
        }
        IoTypeMetadataKind::Array8b | IoTypeMetadataKind::VariableElements8b => {
            (input, output) = copy_n_bytes(input, output, 1 + 1)?;
            compact_metadata(input, output)
        }
        IoTypeMetadataKind::Array16b | IoTypeMetadataKind::VariableElements16b => {
            (input, output) = copy_n_bytes(input, output, 1 + 2)?;
            compact_metadata(input, output)
        }
        IoTypeMetadataKind::Array32b | IoTypeMetadataKind::VariableElements32b => {
            (input, output) = copy_n_bytes(input, output, 1 + 4)?;
            compact_metadata(input, output)
        }
        IoTypeMetadataKind::ArrayU8x8
        | IoTypeMetadataKind::ArrayU8x16
        | IoTypeMetadataKind::ArrayU8x32
        | IoTypeMetadataKind::ArrayU8x64
        | IoTypeMetadataKind::ArrayU8x128
        | IoTypeMetadataKind::ArrayU8x256
        | IoTypeMetadataKind::ArrayU8x512
        | IoTypeMetadataKind::ArrayU8x1024
        | IoTypeMetadataKind::ArrayU8x2028
        | IoTypeMetadataKind::ArrayU8x4096 => copy_n_bytes(input, output, 1),
        IoTypeMetadataKind::VariableBytes8b
        | IoTypeMetadataKind::FixedCapacityBytes8b
        | IoTypeMetadataKind::FixedCapacityString8b => copy_n_bytes(input, output, 1 + 1),
        IoTypeMetadataKind::VariableBytes16b
        | IoTypeMetadataKind::FixedCapacityBytes16b
        | IoTypeMetadataKind::FixedCapacityString16b => copy_n_bytes(input, output, 1 + 2),
        IoTypeMetadataKind::VariableBytes32b => copy_n_bytes(input, output, 1 + 4),
        IoTypeMetadataKind::VariableBytes0
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
        | IoTypeMetadataKind::VariableBytes1048576 => copy_n_bytes(input, output, 1),
        IoTypeMetadataKind::VariableElements0 | IoTypeMetadataKind::Unaligned => {
            (input, output) = copy_n_bytes(input, output, 1)?;
            compact_metadata(input, output)
        }
        IoTypeMetadataKind::Address | IoTypeMetadataKind::Balance => copy_n_bytes(input, output, 1),
    }
}

const fn compact_struct<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    arguments_count: Option<u8>,
    tuple: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    // Remove struct name
    let struct_name_length = input[0] as usize;
    output[0] = 0;
    (input, output) = skip_n_bytes_io(input, output, 1)?;
    input = skip_n_bytes(input, struct_name_length)?;

    let mut arguments_count = if let Some(arguments_count) = arguments_count {
        arguments_count
    } else {
        if input.is_empty() || output.is_empty() {
            return None;
        }

        let arguments_count = input[0];
        (input, output) = copy_n_bytes(input, output, 1)?;

        arguments_count
    };

    // Compact arguments
    while arguments_count > 0 {
        if input.is_empty() || output.is_empty() {
            return None;
        }

        // Remove field name if needed
        if !tuple {
            let field_name_length = input[0] as usize;
            input = skip_n_bytes(input, 1 + field_name_length)?;
        }

        // Compact argument's type
        (input, output) = compact_metadata(input, output)?;

        arguments_count -= 1;
    }

    Some((input, output))
}

const fn compact_enum<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    variant_count: Option<u8>,
    has_fields: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    // Remove enum name
    let enum_name_length = input[0] as usize;
    output[0] = 0;
    (input, output) = skip_n_bytes_io(input, output, 1)?;
    input = skip_n_bytes(input, enum_name_length)?;

    let mut variant_count = if let Some(variant_count) = variant_count {
        variant_count
    } else {
        if input.is_empty() || output.is_empty() {
            return None;
        }

        let variant_count = input[0];
        (input, output) = copy_n_bytes(input, output, 1)?;

        variant_count
    };

    // Compact enum variants
    while variant_count > 0 {
        if input.is_empty() || output.is_empty() {
            return None;
        }

        if has_fields {
            // Compact variant as if it was a struct
            (input, output) = compact_struct(input, output, None, false)?;
        } else {
            // Compact variant as if it was a struct without fields
            (input, output) = compact_struct(input, output, Some(0), false)?;
        }

        variant_count -= 1;
    }

    Some((input, output))
}

/// Copies `n` bytes from input to output and returns both input and output after `n` bytes offset
const fn copy_n_bytes<'i, 'o>(
    input: &'i [u8],
    output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    let (source, input) = input.split_at_checked(n)?;
    let (target, output) = output.split_at_mut_checked(n)?;

    target.copy_from_slice(source);

    Some((input, output))
}

/// Skips `n` bytes and return remainder
const fn skip_n_bytes(input: &[u8], n: usize) -> Option<&[u8]> {
    input.get(n..)
}

/// Skips `n` bytes in input and output
const fn skip_n_bytes_io<'i, 'o>(
    input: &'i [u8],
    output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    let input = input.get(n..)?;
    let output = output.get_mut(n..)?;

    Some((input, output))
}
