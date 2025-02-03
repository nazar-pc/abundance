use crate::metadata::IoTypeMetadataKind;
use core::ptr;

/// This macro is necessary to reduce boilerplate due to lack of `?` in const environment
macro_rules! forward_option {
    ($expr:expr) => {{
        let Some(result) = $expr else {
            return None;
        };
        result
    }};
}

pub(super) const fn recommended_capacity(mut metadata: &[u8]) -> Option<(u32, &[u8])> {
    if metadata.is_empty() {
        return None;
    }

    let kind = forward_option!(IoTypeMetadataKind::try_from_u8(metadata[0]));
    metadata = forward_option!(skip_n_bytes(metadata, 1));

    match kind {
        IoTypeMetadataKind::Unit => Some((0, metadata)),
        IoTypeMetadataKind::Bool | IoTypeMetadataKind::U8 | IoTypeMetadataKind::I8 => {
            Some((1, metadata))
        }
        IoTypeMetadataKind::U16 | IoTypeMetadataKind::I16 => Some((2, metadata)),
        IoTypeMetadataKind::U32 | IoTypeMetadataKind::I32 => Some((4, metadata)),
        IoTypeMetadataKind::U64 | IoTypeMetadataKind::I64 => Some((8, metadata)),
        IoTypeMetadataKind::U128 | IoTypeMetadataKind::I128 => Some((16, metadata)),
        IoTypeMetadataKind::Struct => struct_capacity(metadata, None, false),
        IoTypeMetadataKind::Struct0 => struct_capacity(metadata, Some(0), false),
        IoTypeMetadataKind::Struct1 => struct_capacity(metadata, Some(1), false),
        IoTypeMetadataKind::Struct2 => struct_capacity(metadata, Some(2), false),
        IoTypeMetadataKind::Struct3 => struct_capacity(metadata, Some(3), false),
        IoTypeMetadataKind::Struct4 => struct_capacity(metadata, Some(4), false),
        IoTypeMetadataKind::Struct5 => struct_capacity(metadata, Some(5), false),
        IoTypeMetadataKind::Struct6 => struct_capacity(metadata, Some(6), false),
        IoTypeMetadataKind::Struct7 => struct_capacity(metadata, Some(7), false),
        IoTypeMetadataKind::Struct8 => struct_capacity(metadata, Some(8), false),
        IoTypeMetadataKind::Struct9 => struct_capacity(metadata, Some(9), false),
        IoTypeMetadataKind::Struct10 => struct_capacity(metadata, Some(10), false),
        IoTypeMetadataKind::Struct11 => struct_capacity(metadata, Some(11), false),
        IoTypeMetadataKind::Struct12 => struct_capacity(metadata, Some(12), false),
        IoTypeMetadataKind::Struct13 => struct_capacity(metadata, Some(13), false),
        IoTypeMetadataKind::Struct14 => struct_capacity(metadata, Some(14), false),
        IoTypeMetadataKind::Struct15 => struct_capacity(metadata, Some(15), false),
        IoTypeMetadataKind::Struct16 => struct_capacity(metadata, Some(16), false),
        IoTypeMetadataKind::TupleStruct => struct_capacity(metadata, None, true),
        IoTypeMetadataKind::TupleStruct1 => struct_capacity(metadata, Some(1), true),
        IoTypeMetadataKind::TupleStruct2 => struct_capacity(metadata, Some(2), true),
        IoTypeMetadataKind::TupleStruct3 => struct_capacity(metadata, Some(3), true),
        IoTypeMetadataKind::TupleStruct4 => struct_capacity(metadata, Some(4), true),
        IoTypeMetadataKind::TupleStruct5 => struct_capacity(metadata, Some(5), true),
        IoTypeMetadataKind::TupleStruct6 => struct_capacity(metadata, Some(6), true),
        IoTypeMetadataKind::TupleStruct7 => struct_capacity(metadata, Some(7), true),
        IoTypeMetadataKind::TupleStruct8 => struct_capacity(metadata, Some(8), true),
        IoTypeMetadataKind::TupleStruct9 => struct_capacity(metadata, Some(9), true),
        IoTypeMetadataKind::TupleStruct10 => struct_capacity(metadata, Some(10), true),
        IoTypeMetadataKind::TupleStruct11 => struct_capacity(metadata, Some(11), true),
        IoTypeMetadataKind::TupleStruct12 => struct_capacity(metadata, Some(12), true),
        IoTypeMetadataKind::TupleStruct13 => struct_capacity(metadata, Some(13), true),
        IoTypeMetadataKind::TupleStruct14 => struct_capacity(metadata, Some(14), true),
        IoTypeMetadataKind::TupleStruct15 => struct_capacity(metadata, Some(15), true),
        IoTypeMetadataKind::TupleStruct16 => struct_capacity(metadata, Some(16), true),
        IoTypeMetadataKind::Enum => enum_capacity(metadata, None, true),
        IoTypeMetadataKind::Enum1 => enum_capacity(metadata, Some(1), true),
        IoTypeMetadataKind::Enum2 => enum_capacity(metadata, Some(2), true),
        IoTypeMetadataKind::Enum3 => enum_capacity(metadata, Some(3), true),
        IoTypeMetadataKind::Enum4 => enum_capacity(metadata, Some(4), true),
        IoTypeMetadataKind::Enum5 => enum_capacity(metadata, Some(5), true),
        IoTypeMetadataKind::Enum6 => enum_capacity(metadata, Some(6), true),
        IoTypeMetadataKind::Enum7 => enum_capacity(metadata, Some(7), true),
        IoTypeMetadataKind::Enum8 => enum_capacity(metadata, Some(8), true),
        IoTypeMetadataKind::Enum9 => enum_capacity(metadata, Some(9), true),
        IoTypeMetadataKind::Enum10 => enum_capacity(metadata, Some(10), true),
        IoTypeMetadataKind::Enum11 => enum_capacity(metadata, Some(11), true),
        IoTypeMetadataKind::Enum12 => enum_capacity(metadata, Some(12), true),
        IoTypeMetadataKind::Enum13 => enum_capacity(metadata, Some(13), true),
        IoTypeMetadataKind::Enum14 => enum_capacity(metadata, Some(14), true),
        IoTypeMetadataKind::Enum15 => enum_capacity(metadata, Some(15), true),
        IoTypeMetadataKind::Enum16 => enum_capacity(metadata, Some(16), true),
        IoTypeMetadataKind::EnumNoFields => enum_capacity(metadata, None, false),
        IoTypeMetadataKind::EnumNoFields1 => enum_capacity(metadata, Some(1), false),
        IoTypeMetadataKind::EnumNoFields2 => enum_capacity(metadata, Some(2), false),
        IoTypeMetadataKind::EnumNoFields3 => enum_capacity(metadata, Some(3), false),
        IoTypeMetadataKind::EnumNoFields4 => enum_capacity(metadata, Some(4), false),
        IoTypeMetadataKind::EnumNoFields5 => enum_capacity(metadata, Some(5), false),
        IoTypeMetadataKind::EnumNoFields6 => enum_capacity(metadata, Some(6), false),
        IoTypeMetadataKind::EnumNoFields7 => enum_capacity(metadata, Some(7), false),
        IoTypeMetadataKind::EnumNoFields8 => enum_capacity(metadata, Some(8), false),
        IoTypeMetadataKind::EnumNoFields9 => enum_capacity(metadata, Some(9), false),
        IoTypeMetadataKind::EnumNoFields10 => enum_capacity(metadata, Some(10), false),
        IoTypeMetadataKind::EnumNoFields11 => enum_capacity(metadata, Some(11), false),
        IoTypeMetadataKind::EnumNoFields12 => enum_capacity(metadata, Some(12), false),
        IoTypeMetadataKind::EnumNoFields13 => enum_capacity(metadata, Some(13), false),
        IoTypeMetadataKind::EnumNoFields14 => enum_capacity(metadata, Some(14), false),
        IoTypeMetadataKind::EnumNoFields15 => enum_capacity(metadata, Some(15), false),
        IoTypeMetadataKind::EnumNoFields16 => enum_capacity(metadata, Some(16), false),
        IoTypeMetadataKind::Array8b => {
            if metadata.is_empty() {
                return None;
            }

            let num_elements = metadata[0] as u32;
            metadata = forward_option!(skip_n_bytes(metadata, size_of::<u8>()));

            let capacity;
            (capacity, metadata) = forward_option!(recommended_capacity(metadata));
            Some((
                forward_option!(capacity.checked_mul(num_elements)),
                metadata,
            ))
        }
        IoTypeMetadataKind::Array16b => {
            let mut num_elements = [0; size_of::<u16>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_elements, size_of::<u16>()));
            let num_elements = u16::from_le_bytes(num_elements) as u32;

            let capacity;
            (capacity, metadata) = forward_option!(recommended_capacity(metadata));
            Some((
                forward_option!(capacity.checked_mul(num_elements)),
                metadata,
            ))
        }
        IoTypeMetadataKind::Array32b => {
            let mut num_elements = [0; size_of::<u32>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_elements, size_of::<u32>()));
            let num_elements = u32::from_le_bytes(num_elements);

            let capacity;
            (capacity, metadata) = forward_option!(recommended_capacity(metadata));
            Some((
                forward_option!(capacity.checked_mul(num_elements)),
                metadata,
            ))
        }
        IoTypeMetadataKind::ArrayU8x8 => Some((8, metadata)),
        IoTypeMetadataKind::ArrayU8x16 => Some((16, metadata)),
        IoTypeMetadataKind::ArrayU8x32 => Some((32, metadata)),
        IoTypeMetadataKind::ArrayU8x64 => Some((64, metadata)),
        IoTypeMetadataKind::ArrayU8x128 => Some((128, metadata)),
        IoTypeMetadataKind::ArrayU8x256 => Some((256, metadata)),
        IoTypeMetadataKind::ArrayU8x512 => Some((512, metadata)),
        IoTypeMetadataKind::ArrayU8x1024 => Some((1024, metadata)),
        IoTypeMetadataKind::ArrayU8x2028 => Some((2028, metadata)),
        IoTypeMetadataKind::ArrayU8x4096 => Some((4096, metadata)),
        IoTypeMetadataKind::VariableBytes8b => {
            if metadata.is_empty() {
                return None;
            }

            let num_bytes = metadata[0] as u32;
            metadata = forward_option!(skip_n_bytes(metadata, size_of::<u8>()));

            Some((num_bytes, metadata))
        }
        IoTypeMetadataKind::VariableBytes16b => {
            let mut num_bytes = [0; size_of::<u16>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_bytes, size_of::<u16>()));
            let num_elements = u16::from_le_bytes(num_bytes) as u32;

            Some((num_elements, metadata))
        }
        IoTypeMetadataKind::VariableBytes32b => {
            let mut num_bytes = [0; size_of::<u32>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_bytes, size_of::<u32>()));
            let num_elements = u32::from_le_bytes(num_bytes);

            Some((num_elements, metadata))
        }
        IoTypeMetadataKind::VariableBytes512 => Some((512, metadata)),
        IoTypeMetadataKind::VariableBytes1024 => Some((1024, metadata)),
        IoTypeMetadataKind::VariableBytes2028 => Some((2028, metadata)),
        IoTypeMetadataKind::VariableBytes4096 => Some((4096, metadata)),
        IoTypeMetadataKind::VariableBytes8192 => Some((8192, metadata)),
        IoTypeMetadataKind::VariableBytes16384 => Some((16384, metadata)),
        IoTypeMetadataKind::VariableBytes32768 => Some((32768, metadata)),
        IoTypeMetadataKind::VariableBytes65536 => Some((65536, metadata)),
        IoTypeMetadataKind::VariableBytes131072 => Some((131072, metadata)),
        IoTypeMetadataKind::VariableBytes262144 => Some((262144, metadata)),
        IoTypeMetadataKind::VariableBytes524288 => Some((524288, metadata)),
        IoTypeMetadataKind::VariableBytes1048576 => Some((1048576, metadata)),
        IoTypeMetadataKind::VariableBytes2097152 => Some((2097152, metadata)),
        IoTypeMetadataKind::VariableBytes4194304 => Some((4194304, metadata)),
        IoTypeMetadataKind::VariableBytes8388608 => Some((8388608, metadata)),
        IoTypeMetadataKind::VariableBytes16777216 => Some((16777216, metadata)),
    }
}

const fn struct_capacity(
    mut input: &[u8],
    arguments_count: Option<u8>,
    tuple: bool,
) -> Option<(u32, &[u8])> {
    if input.is_empty() {
        return None;
    }

    // Skip struct name
    let struct_name_length = input[0] as usize;
    input = forward_option!(skip_n_bytes(input, 1 + struct_name_length));

    let mut arguments_count = match arguments_count {
        Some(arguments_count) => arguments_count,
        None => {
            if input.is_empty() {
                return None;
            }

            let arguments_count = input[0];
            input = forward_option!(skip_n_bytes(input, 1));

            arguments_count
        }
    };

    // Capacity of arguments
    let mut capacity = 0u32;
    while arguments_count > 0 {
        if input.is_empty() {
            return None;
        }

        // Skip field name if needed
        if !tuple {
            let field_name_length = input[0] as usize;
            input = forward_option!(skip_n_bytes(input, 1 + field_name_length));
        }

        // Capacity of argument's type
        let argument_capacity;
        (argument_capacity, input) = forward_option!(recommended_capacity(input));
        capacity = forward_option!(capacity.checked_add(argument_capacity));

        arguments_count -= 1;
    }

    Some((capacity, input))
}

const fn enum_capacity(
    mut input: &[u8],
    variant_count: Option<u8>,
    has_fields: bool,
) -> Option<(u32, &[u8])> {
    if input.is_empty() {
        return None;
    }

    // Skip enum name
    let enum_name_length = input[0] as usize;
    input = forward_option!(skip_n_bytes(input, 1 + enum_name_length));

    let mut variant_count = match variant_count {
        Some(variant_count) => variant_count,
        None => {
            if input.is_empty() {
                return None;
            }

            let variant_count = input[0];
            input = forward_option!(skip_n_bytes(input, 1));

            variant_count
        }
    };

    // Capacity of variants
    let mut enum_capacity = None;
    while variant_count > 0 {
        if input.is_empty() {
            return None;
        }

        let mut variant_capacity;

        // Variant capacity as if it was a struct
        (variant_capacity, input) = forward_option!(struct_capacity(
            input,
            if has_fields { None } else { Some(0) },
            false
        ));
        // `+ 1` is for the discriminant
        variant_capacity += 1;

        match enum_capacity {
            Some(capacity) => {
                if capacity != variant_capacity {
                    return None;
                }
            }
            None => {
                enum_capacity.replace(variant_capacity);
            }
        }

        variant_count -= 1;
    }

    // `.unwrap_or_default()` is not const
    let enum_capacity = match enum_capacity {
        Some(enum_capacity) => enum_capacity,
        None => 0,
    };

    Some((enum_capacity, input))
}

/// Copies `n` bytes from input to output and returns both input and output after `n` bytes offset
const fn copy_n_bytes<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if n > input.len() || n > output.len() {
        return None;
    }

    let source;
    let target;
    (source, input) = input.split_at(n);
    (target, output) = output.split_at_mut(n);
    // TODO: Switch to `copy_from_slice` once stable:
    //  https://github.com/rust-lang/rust/issues/131415
    // The same as `target.copy_from_slice(&source);`, but it doesn't work in const environment
    // yet
    // SAFETY: Size is correct due to slicing above, pointers are created from valid independent
    // slices of equal length
    unsafe {
        ptr::copy_nonoverlapping(source.as_ptr(), target.as_mut_ptr(), source.len());
    }

    Some((input, output))
}

/// Skips `n` bytes and return remainder
const fn skip_n_bytes(input: &[u8], n: usize) -> Option<&[u8]> {
    if n > input.len() {
        return None;
    }

    // `&input[n..]` not supported in const yet
    Some(input.split_at(n).1)
}
