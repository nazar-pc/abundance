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

pub(super) const fn recommended_capacity(mut input: &[u8]) -> Option<(u32, &[u8])> {
    if input.is_empty() {
        return None;
    }

    let kind = forward_option!(IoTypeMetadataKind::try_from_u8(input[0]));
    input = forward_option!(skip_n_bytes(input, 1));

    match kind {
        IoTypeMetadataKind::Unit => Some((0, input)),
        IoTypeMetadataKind::Bool | IoTypeMetadataKind::U8 | IoTypeMetadataKind::I8 => {
            Some((1, input))
        }
        IoTypeMetadataKind::U16 | IoTypeMetadataKind::I16 => Some((2, input)),
        IoTypeMetadataKind::U32 | IoTypeMetadataKind::I32 => Some((4, input)),
        IoTypeMetadataKind::U64 | IoTypeMetadataKind::I64 => Some((8, input)),
        IoTypeMetadataKind::U128 | IoTypeMetadataKind::I128 => Some((16, input)),
        IoTypeMetadataKind::Struct => struct_capacity(input, None, false),
        IoTypeMetadataKind::Struct0 => struct_capacity(input, Some(0), false),
        IoTypeMetadataKind::Struct1 => struct_capacity(input, Some(1), false),
        IoTypeMetadataKind::Struct2 => struct_capacity(input, Some(2), false),
        IoTypeMetadataKind::Struct3 => struct_capacity(input, Some(3), false),
        IoTypeMetadataKind::Struct4 => struct_capacity(input, Some(4), false),
        IoTypeMetadataKind::Struct5 => struct_capacity(input, Some(5), false),
        IoTypeMetadataKind::Struct6 => struct_capacity(input, Some(6), false),
        IoTypeMetadataKind::Struct7 => struct_capacity(input, Some(7), false),
        IoTypeMetadataKind::Struct8 => struct_capacity(input, Some(8), false),
        IoTypeMetadataKind::Struct9 => struct_capacity(input, Some(9), false),
        IoTypeMetadataKind::Struct10 => struct_capacity(input, Some(10), false),
        IoTypeMetadataKind::Struct11 => struct_capacity(input, Some(11), false),
        IoTypeMetadataKind::Struct12 => struct_capacity(input, Some(12), false),
        IoTypeMetadataKind::Struct13 => struct_capacity(input, Some(13), false),
        IoTypeMetadataKind::Struct14 => struct_capacity(input, Some(14), false),
        IoTypeMetadataKind::Struct15 => struct_capacity(input, Some(15), false),
        IoTypeMetadataKind::Struct16 => struct_capacity(input, Some(16), false),
        IoTypeMetadataKind::TupleStruct => struct_capacity(input, None, true),
        IoTypeMetadataKind::TupleStruct1 => struct_capacity(input, Some(1), true),
        IoTypeMetadataKind::TupleStruct2 => struct_capacity(input, Some(2), true),
        IoTypeMetadataKind::TupleStruct3 => struct_capacity(input, Some(3), true),
        IoTypeMetadataKind::TupleStruct4 => struct_capacity(input, Some(4), true),
        IoTypeMetadataKind::TupleStruct5 => struct_capacity(input, Some(5), true),
        IoTypeMetadataKind::TupleStruct6 => struct_capacity(input, Some(6), true),
        IoTypeMetadataKind::TupleStruct7 => struct_capacity(input, Some(7), true),
        IoTypeMetadataKind::TupleStruct8 => struct_capacity(input, Some(8), true),
        IoTypeMetadataKind::TupleStruct9 => struct_capacity(input, Some(9), true),
        IoTypeMetadataKind::TupleStruct10 => struct_capacity(input, Some(10), true),
        IoTypeMetadataKind::TupleStruct11 => struct_capacity(input, Some(11), true),
        IoTypeMetadataKind::TupleStruct12 => struct_capacity(input, Some(12), true),
        IoTypeMetadataKind::TupleStruct13 => struct_capacity(input, Some(13), true),
        IoTypeMetadataKind::TupleStruct14 => struct_capacity(input, Some(14), true),
        IoTypeMetadataKind::TupleStruct15 => struct_capacity(input, Some(15), true),
        IoTypeMetadataKind::TupleStruct16 => struct_capacity(input, Some(16), true),
        IoTypeMetadataKind::Enum => enum_capacity(input, None, true),
        IoTypeMetadataKind::Enum1 => enum_capacity(input, Some(1), true),
        IoTypeMetadataKind::Enum2 => enum_capacity(input, Some(2), true),
        IoTypeMetadataKind::Enum3 => enum_capacity(input, Some(3), true),
        IoTypeMetadataKind::Enum4 => enum_capacity(input, Some(4), true),
        IoTypeMetadataKind::Enum5 => enum_capacity(input, Some(5), true),
        IoTypeMetadataKind::Enum6 => enum_capacity(input, Some(6), true),
        IoTypeMetadataKind::Enum7 => enum_capacity(input, Some(7), true),
        IoTypeMetadataKind::Enum8 => enum_capacity(input, Some(8), true),
        IoTypeMetadataKind::Enum9 => enum_capacity(input, Some(9), true),
        IoTypeMetadataKind::Enum10 => enum_capacity(input, Some(10), true),
        IoTypeMetadataKind::Enum11 => enum_capacity(input, Some(11), true),
        IoTypeMetadataKind::Enum12 => enum_capacity(input, Some(12), true),
        IoTypeMetadataKind::Enum13 => enum_capacity(input, Some(13), true),
        IoTypeMetadataKind::Enum14 => enum_capacity(input, Some(14), true),
        IoTypeMetadataKind::Enum15 => enum_capacity(input, Some(15), true),
        IoTypeMetadataKind::Enum16 => enum_capacity(input, Some(16), true),
        IoTypeMetadataKind::EnumNoFields => enum_capacity(input, None, false),
        IoTypeMetadataKind::EnumNoFields1 => enum_capacity(input, Some(1), false),
        IoTypeMetadataKind::EnumNoFields2 => enum_capacity(input, Some(2), false),
        IoTypeMetadataKind::EnumNoFields3 => enum_capacity(input, Some(3), false),
        IoTypeMetadataKind::EnumNoFields4 => enum_capacity(input, Some(4), false),
        IoTypeMetadataKind::EnumNoFields5 => enum_capacity(input, Some(5), false),
        IoTypeMetadataKind::EnumNoFields6 => enum_capacity(input, Some(6), false),
        IoTypeMetadataKind::EnumNoFields7 => enum_capacity(input, Some(7), false),
        IoTypeMetadataKind::EnumNoFields8 => enum_capacity(input, Some(8), false),
        IoTypeMetadataKind::EnumNoFields9 => enum_capacity(input, Some(9), false),
        IoTypeMetadataKind::EnumNoFields10 => enum_capacity(input, Some(10), false),
        IoTypeMetadataKind::EnumNoFields11 => enum_capacity(input, Some(11), false),
        IoTypeMetadataKind::EnumNoFields12 => enum_capacity(input, Some(12), false),
        IoTypeMetadataKind::EnumNoFields13 => enum_capacity(input, Some(13), false),
        IoTypeMetadataKind::EnumNoFields14 => enum_capacity(input, Some(14), false),
        IoTypeMetadataKind::EnumNoFields15 => enum_capacity(input, Some(15), false),
        IoTypeMetadataKind::EnumNoFields16 => enum_capacity(input, Some(16), false),
        IoTypeMetadataKind::Array8b => {
            if input.is_empty() {
                return None;
            }

            let num_elements = input[0] as u32;
            input = forward_option!(skip_n_bytes(input, size_of::<u8>()));

            let capacity;
            (capacity, input) = forward_option!(recommended_capacity(input));
            Some((forward_option!(capacity.checked_mul(num_elements)), input))
        }
        IoTypeMetadataKind::Array16b => {
            let mut num_elements = [0; size_of::<u16>()];
            (input, _) = forward_option!(copy_n_bytes(input, &mut num_elements, size_of::<u16>()));
            let num_elements = u16::from_le_bytes(num_elements) as u32;

            let capacity;
            (capacity, input) = forward_option!(recommended_capacity(input));
            Some((forward_option!(capacity.checked_mul(num_elements)), input))
        }
        IoTypeMetadataKind::Array32b => {
            let mut num_elements = [0; size_of::<u32>()];
            (input, _) = forward_option!(copy_n_bytes(input, &mut num_elements, size_of::<u32>()));
            let num_elements = u32::from_le_bytes(num_elements);

            let capacity;
            (capacity, input) = forward_option!(recommended_capacity(input));
            Some((forward_option!(capacity.checked_mul(num_elements)), input))
        }
        IoTypeMetadataKind::ArrayU8x8 => Some((8, input)),
        IoTypeMetadataKind::ArrayU8x16 => Some((16, input)),
        IoTypeMetadataKind::ArrayU8x32 => Some((32, input)),
        IoTypeMetadataKind::ArrayU8x64 => Some((64, input)),
        IoTypeMetadataKind::ArrayU8x128 => Some((128, input)),
        IoTypeMetadataKind::ArrayU8x256 => Some((256, input)),
        IoTypeMetadataKind::ArrayU8x512 => Some((512, input)),
        IoTypeMetadataKind::ArrayU8x1024 => Some((1024, input)),
        IoTypeMetadataKind::ArrayU8x2028 => Some((2028, input)),
        IoTypeMetadataKind::ArrayU8x4096 => Some((4096, input)),
        IoTypeMetadataKind::VariableBytes8b => {
            if input.is_empty() {
                return None;
            }

            let num_bytes = input[0] as u32;
            input = forward_option!(skip_n_bytes(input, size_of::<u8>()));

            Some((num_bytes, input))
        }
        IoTypeMetadataKind::VariableBytes16b => {
            let mut num_bytes = [0; size_of::<u16>()];
            (input, _) = forward_option!(copy_n_bytes(input, &mut num_bytes, size_of::<u16>()));
            let num_elements = u16::from_le_bytes(num_bytes) as u32;

            Some((num_elements, input))
        }
        IoTypeMetadataKind::VariableBytes32b => {
            let mut num_bytes = [0; size_of::<u32>()];
            (input, _) = forward_option!(copy_n_bytes(input, &mut num_bytes, size_of::<u32>()));
            let num_elements = u32::from_le_bytes(num_bytes);

            Some((num_elements, input))
        }
        IoTypeMetadataKind::VariableBytes512 => Some((512, input)),
        IoTypeMetadataKind::VariableBytes1024 => Some((1024, input)),
        IoTypeMetadataKind::VariableBytes2028 => Some((2028, input)),
        IoTypeMetadataKind::VariableBytes4096 => Some((4096, input)),
        IoTypeMetadataKind::VariableBytes8192 => Some((8192, input)),
        IoTypeMetadataKind::VariableBytes16384 => Some((16384, input)),
        IoTypeMetadataKind::VariableBytes32768 => Some((32768, input)),
        IoTypeMetadataKind::VariableBytes65536 => Some((65536, input)),
        IoTypeMetadataKind::VariableBytes131072 => Some((131072, input)),
        IoTypeMetadataKind::VariableBytes262144 => Some((262144, input)),
        IoTypeMetadataKind::VariableBytes524288 => Some((524288, input)),
        IoTypeMetadataKind::VariableBytes1048576 => Some((1048576, input)),
        IoTypeMetadataKind::VariableBytes2097152 => Some((2097152, input)),
        IoTypeMetadataKind::VariableBytes4194304 => Some((4194304, input)),
        IoTypeMetadataKind::VariableBytes8388608 => Some((8388608, input)),
        IoTypeMetadataKind::VariableBytes16777216 => Some((16777216, input)),
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

        // Remove variant name
        let field_name_length = input[0] as usize;
        input = forward_option!(skip_n_bytes(input, 1 + field_name_length));

        let variant_capacity;
        if has_fields {
            // Variant capacity as if it was a struct
            (variant_capacity, input) = forward_option!(struct_capacity(input, None, false));
        } else {
            // Variant capacity as if it was a struct without fields
            (variant_capacity, input) = forward_option!(struct_capacity(input, Some(0), false));
        }

        match enum_capacity {
            Some(capacity) => {
                if capacity == variant_capacity {
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
