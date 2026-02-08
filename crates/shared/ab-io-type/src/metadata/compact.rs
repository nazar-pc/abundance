use crate::metadata::IoTypeMetadataKind;

pub(super) const fn compact_metadata<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
) -> Option<(&'i [u8], &'o mut [u8])> {
    let io_type_metadata_kind_input = *input.split_off_first()?;
    let io_type_metadata_kind_output = output.split_off_first_mut()?;
    let io_type_metadata_kind = IoTypeMetadataKind::try_from(io_type_metadata_kind_input).ok()?;

    match io_type_metadata_kind {
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
        | IoTypeMetadataKind::I128 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
        }
        IoTypeMetadataKind::Struct => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, None, false)?;
        }
        IoTypeMetadataKind::Struct0 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(0), false)?;
        }
        IoTypeMetadataKind::Struct1 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct1 as u8;
            (input, output) = compact_struct(input, output, Some(1), false)?;
        }
        IoTypeMetadataKind::Struct2 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct2 as u8;
            (input, output) = compact_struct(input, output, Some(2), false)?;
        }
        IoTypeMetadataKind::Struct3 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct3 as u8;
            (input, output) = compact_struct(input, output, Some(3), false)?;
        }
        IoTypeMetadataKind::Struct4 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct4 as u8;
            (input, output) = compact_struct(input, output, Some(4), false)?;
        }
        IoTypeMetadataKind::Struct5 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct5 as u8;
            (input, output) = compact_struct(input, output, Some(5), false)?;
        }
        IoTypeMetadataKind::Struct6 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct6 as u8;
            (input, output) = compact_struct(input, output, Some(6), false)?;
        }
        IoTypeMetadataKind::Struct7 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct7 as u8;
            (input, output) = compact_struct(input, output, Some(7), false)?;
        }
        IoTypeMetadataKind::Struct8 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct8 as u8;
            (input, output) = compact_struct(input, output, Some(8), false)?;
        }
        IoTypeMetadataKind::Struct9 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct9 as u8;
            (input, output) = compact_struct(input, output, Some(9), false)?;
        }
        IoTypeMetadataKind::Struct10 => {
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct10 as u8;
            (input, output) = compact_struct(input, output, Some(10), false)?;
        }
        IoTypeMetadataKind::TupleStruct => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, None, true)?;
        }
        IoTypeMetadataKind::TupleStruct1 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(1), true)?;
        }
        IoTypeMetadataKind::TupleStruct2 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(2), true)?;
        }
        IoTypeMetadataKind::TupleStruct3 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(3), true)?;
        }
        IoTypeMetadataKind::TupleStruct4 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(4), true)?;
        }
        IoTypeMetadataKind::TupleStruct5 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(5), true)?;
        }
        IoTypeMetadataKind::TupleStruct6 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(6), true)?;
        }
        IoTypeMetadataKind::TupleStruct7 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(7), true)?;
        }
        IoTypeMetadataKind::TupleStruct8 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(8), true)?;
        }
        IoTypeMetadataKind::TupleStruct9 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(9), true)?;
        }
        IoTypeMetadataKind::TupleStruct10 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, Some(10), true)?;
        }
        IoTypeMetadataKind::Enum => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, None, true)?;
        }
        IoTypeMetadataKind::Enum1 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(1), true)?;
        }
        IoTypeMetadataKind::Enum2 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(2), true)?;
        }
        IoTypeMetadataKind::Enum3 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(3), true)?;
        }
        IoTypeMetadataKind::Enum4 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(4), true)?;
        }
        IoTypeMetadataKind::Enum5 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(5), true)?;
        }
        IoTypeMetadataKind::Enum6 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(6), true)?;
        }
        IoTypeMetadataKind::Enum7 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(7), true)?;
        }
        IoTypeMetadataKind::Enum8 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(8), true)?;
        }
        IoTypeMetadataKind::Enum9 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(9), true)?;
        }
        IoTypeMetadataKind::Enum10 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(10), true)?;
        }
        IoTypeMetadataKind::EnumNoFields => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, None, false)?;
        }
        IoTypeMetadataKind::EnumNoFields1 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(1), false)?;
        }
        IoTypeMetadataKind::EnumNoFields2 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(2), false)?;
        }
        IoTypeMetadataKind::EnumNoFields3 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(3), false)?;
        }
        IoTypeMetadataKind::EnumNoFields4 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(4), false)?;
        }
        IoTypeMetadataKind::EnumNoFields5 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(5), false)?;
        }
        IoTypeMetadataKind::EnumNoFields6 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(6), false)?;
        }
        IoTypeMetadataKind::EnumNoFields7 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(7), false)?;
        }
        IoTypeMetadataKind::EnumNoFields8 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(8), false)?;
        }
        IoTypeMetadataKind::EnumNoFields9 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(9), false)?;
        }
        IoTypeMetadataKind::EnumNoFields10 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, Some(10), false)?;
        }
        IoTypeMetadataKind::Array8b | IoTypeMetadataKind::VariableElements8b => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 1)?;
            (input, output) = compact_metadata(input, output)?;
        }
        IoTypeMetadataKind::Array16b | IoTypeMetadataKind::VariableElements16b => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 2)?;
            (input, output) = compact_metadata(input, output)?;
        }
        IoTypeMetadataKind::Array32b | IoTypeMetadataKind::VariableElements32b => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 4)?;
            (input, output) = compact_metadata(input, output)?;
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
        | IoTypeMetadataKind::ArrayU8x4096 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
        }
        IoTypeMetadataKind::VariableBytes8b
        | IoTypeMetadataKind::FixedCapacityBytes8b
        | IoTypeMetadataKind::FixedCapacityString8b => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 1)?;
        }
        IoTypeMetadataKind::VariableBytes16b
        | IoTypeMetadataKind::FixedCapacityBytes16b
        | IoTypeMetadataKind::FixedCapacityString16b => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 2)?;
        }
        IoTypeMetadataKind::VariableBytes32b => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 4)?;
        }
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
        | IoTypeMetadataKind::VariableBytes1048576 => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
        }
        IoTypeMetadataKind::VariableElements0 | IoTypeMetadataKind::Unaligned => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_metadata(input, output)?;
        }
        IoTypeMetadataKind::Address | IoTypeMetadataKind::Balance => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
        }
    }

    Some((input, output))
}

const fn compact_struct<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    arguments_count: Option<u8>,
    tuple: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    // Remove struct name
    let struct_name_length = *input.split_off_first()?;
    *output.split_off_first_mut()? = 0;
    // TODO: `split_off()` is not `const fn` yet, even unstably
    input = input.get(usize::from(struct_name_length)..)?;

    let mut arguments_count = if let Some(arguments_count) = arguments_count {
        arguments_count
    } else {
        let arguments_count = *input.split_off_first()?;
        *output.split_off_first_mut()? = arguments_count;

        arguments_count
    };

    // Compact arguments
    while arguments_count > 0 {
        // Remove field name if needed
        if !tuple {
            let field_name_length = *input.split_off_first()?;
            // TODO: `split_off()` is not `const fn` yet, even unstably
            input = input.get(usize::from(field_name_length)..)?;
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
    // Remove enum name
    let enum_name_length = *input.split_off_first()?;
    *output.split_off_first_mut()? = 0;
    (_, input) = input.split_at_checked(usize::from(enum_name_length))?;

    let mut variant_count = if let Some(variant_count) = variant_count {
        variant_count
    } else {
        let variant_count = *input.split_off_first()?;
        *output.split_off_first_mut()? = variant_count;

        variant_count
    };

    // Compact enum variants
    while variant_count > 0 {
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
