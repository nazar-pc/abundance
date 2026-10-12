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
            // Convert struct with field names to tuple struct
            *io_type_metadata_kind_output = IoTypeMetadataKind::TupleStruct as u8;
            (input, output) = compact_struct(input, output, None, false)?;
        }
        IoTypeMetadataKind::TupleStruct => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_struct(input, output, None, true)?;
        }
        IoTypeMetadataKind::Enum => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, true)?;
        }
        IoTypeMetadataKind::EnumNoFields => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_enum(input, output, false)?;
        }
        IoTypeMetadataKind::Array
        | IoTypeMetadataKind::VariableElements
        | IoTypeMetadataKind::FixedCapacityElements => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 4)?;
            (input, output) = compact_metadata(input, output)?;
        }
        IoTypeMetadataKind::FixedCapacityString => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = copy_n_bytes(input, output, 4)?;
        }
        IoTypeMetadataKind::Unaligned => {
            *io_type_metadata_kind_output = io_type_metadata_kind_input;
            (input, output) = compact_metadata(input, output)?;
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
    has_fields: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    // Remove enum name
    let enum_name_length = *input.split_off_first()?;
    *output.split_off_first_mut()? = 0;
    (_, input) = input.split_at_checked(usize::from(enum_name_length))?;

    let mut variant_count = *input.split_off_first()?;
    *output.split_off_first_mut()? = variant_count;

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
