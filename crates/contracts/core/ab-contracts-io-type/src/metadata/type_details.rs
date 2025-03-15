use crate::metadata::{IoTypeDetails, IoTypeMetadataKind};
use core::num::NonZeroU8;

/// This macro is necessary to reduce boilerplate due to lack of `?` in const environment
macro_rules! forward_option {
    ($expr:expr) => {{
        let Some(result) = $expr else {
            return None;
        };
        result
    }};
}

#[inline(always)]
pub(super) const fn decode_type_details(mut metadata: &[u8]) -> Option<(IoTypeDetails, &[u8])> {
    if metadata.is_empty() {
        return None;
    }

    let kind = forward_option!(IoTypeMetadataKind::try_from_u8(metadata[0]));
    metadata = forward_option!(skip_n_bytes(metadata, 1));

    match kind {
        IoTypeMetadataKind::Unit => Some((
            IoTypeDetails {
                recommended_capacity: 0,
                alignment: NonZeroU8::new(1).expect("Not zero; qed"),
            },
            metadata,
        )),
        IoTypeMetadataKind::Bool | IoTypeMetadataKind::U8 | IoTypeMetadataKind::I8 => Some((
            IoTypeDetails {
                recommended_capacity: 1,
                alignment: NonZeroU8::new(1).expect("Not zero; qed"),
            },
            metadata,
        )),
        IoTypeMetadataKind::U16 | IoTypeMetadataKind::I16 => Some((
            IoTypeDetails {
                recommended_capacity: 2,
                alignment: NonZeroU8::new(2).expect("Not zero; qed"),
            },
            metadata,
        )),
        IoTypeMetadataKind::U32 | IoTypeMetadataKind::I32 => Some((
            IoTypeDetails {
                recommended_capacity: 4,
                alignment: NonZeroU8::new(4).expect("Not zero; qed"),
            },
            metadata,
        )),
        IoTypeMetadataKind::U64 | IoTypeMetadataKind::I64 => Some((
            IoTypeDetails {
                recommended_capacity: 8,
                alignment: NonZeroU8::new(8).expect("Not zero; qed"),
            },
            metadata,
        )),
        IoTypeMetadataKind::U128 | IoTypeMetadataKind::I128 => Some((
            IoTypeDetails {
                recommended_capacity: 16,
                alignment: NonZeroU8::new(16).expect("Not zero; qed"),
            },
            metadata,
        )),
        IoTypeMetadataKind::Struct => struct_type_details(metadata, None, false),
        IoTypeMetadataKind::Struct0 => struct_type_details(metadata, Some(0), false),
        IoTypeMetadataKind::Struct1 => struct_type_details(metadata, Some(1), false),
        IoTypeMetadataKind::Struct2 => struct_type_details(metadata, Some(2), false),
        IoTypeMetadataKind::Struct3 => struct_type_details(metadata, Some(3), false),
        IoTypeMetadataKind::Struct4 => struct_type_details(metadata, Some(4), false),
        IoTypeMetadataKind::Struct5 => struct_type_details(metadata, Some(5), false),
        IoTypeMetadataKind::Struct6 => struct_type_details(metadata, Some(6), false),
        IoTypeMetadataKind::Struct7 => struct_type_details(metadata, Some(7), false),
        IoTypeMetadataKind::Struct8 => struct_type_details(metadata, Some(8), false),
        IoTypeMetadataKind::Struct9 => struct_type_details(metadata, Some(9), false),
        IoTypeMetadataKind::Struct10 => struct_type_details(metadata, Some(10), false),
        IoTypeMetadataKind::TupleStruct => struct_type_details(metadata, None, true),
        IoTypeMetadataKind::TupleStruct1 => struct_type_details(metadata, Some(1), true),
        IoTypeMetadataKind::TupleStruct2 => struct_type_details(metadata, Some(2), true),
        IoTypeMetadataKind::TupleStruct3 => struct_type_details(metadata, Some(3), true),
        IoTypeMetadataKind::TupleStruct4 => struct_type_details(metadata, Some(4), true),
        IoTypeMetadataKind::TupleStruct5 => struct_type_details(metadata, Some(5), true),
        IoTypeMetadataKind::TupleStruct6 => struct_type_details(metadata, Some(6), true),
        IoTypeMetadataKind::TupleStruct7 => struct_type_details(metadata, Some(7), true),
        IoTypeMetadataKind::TupleStruct8 => struct_type_details(metadata, Some(8), true),
        IoTypeMetadataKind::TupleStruct9 => struct_type_details(metadata, Some(9), true),
        IoTypeMetadataKind::TupleStruct10 => struct_type_details(metadata, Some(10), true),
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
        IoTypeMetadataKind::Array8b | IoTypeMetadataKind::VariableElements8b => {
            if metadata.is_empty() {
                return None;
            }

            let num_elements = metadata[0] as u32;
            metadata = forward_option!(skip_n_bytes(metadata, size_of::<u8>()));

            let type_details;
            (type_details, metadata) = forward_option!(decode_type_details(metadata));
            let recommended_capacity =
                forward_option!(type_details.recommended_capacity.checked_mul(num_elements));
            Some((
                IoTypeDetails {
                    recommended_capacity,
                    alignment: type_details.alignment,
                },
                metadata,
            ))
        }
        IoTypeMetadataKind::Array16b | IoTypeMetadataKind::VariableElements16b => {
            if metadata.is_empty() {
                return None;
            }

            let mut num_elements = [0; size_of::<u16>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_elements, size_of::<u16>()));
            let num_elements = u16::from_le_bytes(num_elements) as u32;

            let type_details;
            (type_details, metadata) = forward_option!(decode_type_details(metadata));
            let recommended_capacity =
                forward_option!(type_details.recommended_capacity.checked_mul(num_elements));
            Some((
                IoTypeDetails {
                    recommended_capacity,
                    alignment: type_details.alignment,
                },
                metadata,
            ))
        }
        IoTypeMetadataKind::Array32b | IoTypeMetadataKind::VariableElements32b => {
            if metadata.is_empty() {
                return None;
            }

            let mut num_elements = [0; size_of::<u32>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_elements, size_of::<u32>()));
            let num_elements = u32::from_le_bytes(num_elements);

            let type_details;
            (type_details, metadata) = forward_option!(decode_type_details(metadata));
            let recommended_capacity =
                forward_option!(type_details.recommended_capacity.checked_mul(num_elements));
            Some((
                IoTypeDetails {
                    recommended_capacity,
                    alignment: type_details.alignment,
                },
                metadata,
            ))
        }
        IoTypeMetadataKind::ArrayU8x8 => Some((IoTypeDetails::bytes(8), metadata)),
        IoTypeMetadataKind::ArrayU8x16 => Some((IoTypeDetails::bytes(16), metadata)),
        IoTypeMetadataKind::ArrayU8x32 => Some((IoTypeDetails::bytes(32), metadata)),
        IoTypeMetadataKind::ArrayU8x64 => Some((IoTypeDetails::bytes(64), metadata)),
        IoTypeMetadataKind::ArrayU8x128 => Some((IoTypeDetails::bytes(128), metadata)),
        IoTypeMetadataKind::ArrayU8x256 => Some((IoTypeDetails::bytes(256), metadata)),
        IoTypeMetadataKind::ArrayU8x512 => Some((IoTypeDetails::bytes(512), metadata)),
        IoTypeMetadataKind::ArrayU8x1024 => Some((IoTypeDetails::bytes(1024), metadata)),
        IoTypeMetadataKind::ArrayU8x2028 => Some((IoTypeDetails::bytes(2028), metadata)),
        IoTypeMetadataKind::ArrayU8x4096 => Some((IoTypeDetails::bytes(4096), metadata)),
        IoTypeMetadataKind::VariableBytes8b => {
            if metadata.is_empty() {
                return None;
            }

            let num_bytes = metadata[0] as u32;
            metadata = forward_option!(skip_n_bytes(metadata, size_of::<u8>()));

            Some((IoTypeDetails::bytes(num_bytes), metadata))
        }
        IoTypeMetadataKind::VariableBytes16b => {
            if metadata.is_empty() {
                return None;
            }

            let mut num_bytes = [0; size_of::<u16>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_bytes, size_of::<u16>()));
            let num_bytes = u16::from_le_bytes(num_bytes) as u32;

            Some((IoTypeDetails::bytes(num_bytes), metadata))
        }
        IoTypeMetadataKind::VariableBytes32b => {
            if metadata.is_empty() {
                return None;
            }

            let mut num_bytes = [0; size_of::<u32>()];
            (metadata, _) =
                forward_option!(copy_n_bytes(metadata, &mut num_bytes, size_of::<u32>()));
            let num_bytes = u32::from_le_bytes(num_bytes);

            Some((IoTypeDetails::bytes(num_bytes), metadata))
        }
        IoTypeMetadataKind::VariableBytes0 => Some((IoTypeDetails::bytes(0), metadata)),
        IoTypeMetadataKind::VariableBytes512 => Some((IoTypeDetails::bytes(512), metadata)),
        IoTypeMetadataKind::VariableBytes1024 => Some((IoTypeDetails::bytes(1024), metadata)),
        IoTypeMetadataKind::VariableBytes2028 => Some((IoTypeDetails::bytes(2028), metadata)),
        IoTypeMetadataKind::VariableBytes4096 => Some((IoTypeDetails::bytes(4096), metadata)),
        IoTypeMetadataKind::VariableBytes8192 => Some((IoTypeDetails::bytes(8192), metadata)),
        IoTypeMetadataKind::VariableBytes16384 => Some((IoTypeDetails::bytes(16384), metadata)),
        IoTypeMetadataKind::VariableBytes32768 => Some((IoTypeDetails::bytes(32768), metadata)),
        IoTypeMetadataKind::VariableBytes65536 => Some((IoTypeDetails::bytes(65536), metadata)),
        IoTypeMetadataKind::VariableBytes131072 => Some((IoTypeDetails::bytes(131_072), metadata)),
        IoTypeMetadataKind::VariableBytes262144 => Some((IoTypeDetails::bytes(262_144), metadata)),
        IoTypeMetadataKind::VariableBytes524288 => Some((IoTypeDetails::bytes(524_288), metadata)),
        IoTypeMetadataKind::VariableBytes1048576 => {
            Some((IoTypeDetails::bytes(1_048_576), metadata))
        }
        IoTypeMetadataKind::VariableElements0 => {
            if metadata.is_empty() {
                return None;
            }

            (_, metadata) = forward_option!(decode_type_details(metadata));
            Some((
                IoTypeDetails {
                    recommended_capacity: 0,
                    alignment: NonZeroU8::new(1).expect("Not zero; qed"),
                },
                metadata,
            ))
        }
        IoTypeMetadataKind::Address | IoTypeMetadataKind::Balance => Some((
            IoTypeDetails {
                recommended_capacity: 16,
                alignment: NonZeroU8::new(8).expect("Not zero; qed"),
            },
            metadata,
        )),
    }
}

#[inline(always)]
const fn struct_type_details(
    mut input: &[u8],
    field_count: Option<u8>,
    tuple: bool,
) -> Option<(IoTypeDetails, &[u8])> {
    if input.is_empty() {
        return None;
    }

    // Skip struct name
    let struct_name_length = input[0] as usize;
    input = forward_option!(skip_n_bytes(input, 1 + struct_name_length));

    let mut field_count = if let Some(field_count) = field_count {
        field_count
    } else {
        if input.is_empty() {
            return None;
        }

        let field_count = input[0];
        input = forward_option!(skip_n_bytes(input, 1));

        field_count
    };

    // Capacity of arguments
    let mut capacity = 0u32;
    let mut alignment = 1u8;
    while field_count > 0 {
        if input.is_empty() {
            return None;
        }

        // Skip field name if needed
        if !tuple {
            let field_name_length = input[0] as usize;
            input = forward_option!(skip_n_bytes(input, 1 + field_name_length));
        }

        // Capacity of argument's type
        let type_details;
        (type_details, input) = forward_option!(decode_type_details(input));
        capacity = forward_option!(capacity.checked_add(type_details.recommended_capacity));
        // TODO: `core::cmp::max()` isn't const yet due to trait bounds
        alignment = if type_details.alignment.get() > alignment {
            type_details.alignment.get()
        } else {
            alignment
        };

        field_count -= 1;
    }

    Some((
        IoTypeDetails {
            recommended_capacity: capacity,
            alignment: NonZeroU8::new(alignment).expect("At least zero; qed"),
        },
        input,
    ))
}

#[inline(always)]
const fn enum_capacity(
    mut input: &[u8],
    variant_count: Option<u8>,
    has_fields: bool,
) -> Option<(IoTypeDetails, &[u8])> {
    if input.is_empty() {
        return None;
    }

    // Skip enum name
    let enum_name_length = input[0] as usize;
    input = forward_option!(skip_n_bytes(input, 1 + enum_name_length));

    let mut variant_count = if let Some(variant_count) = variant_count {
        variant_count
    } else {
        if input.is_empty() {
            return None;
        }

        let variant_count = input[0];
        input = forward_option!(skip_n_bytes(input, 1));

        variant_count
    };

    // Capacity of variants
    let mut enum_capacity = None;
    let mut alignment = 1u8;
    while variant_count > 0 {
        if input.is_empty() {
            return None;
        }

        let variant_type_details;

        // Variant capacity as if it was a struct
        (variant_type_details, input) = forward_option!(struct_type_details(
            input,
            if has_fields { None } else { Some(0) },
            false
        ));
        // `+ 1` is for the discriminant
        let variant_capacity = variant_type_details.recommended_capacity + 1;
        // TODO: `core::cmp::max()` isn't const yet due to trait bounds
        alignment = if variant_type_details.alignment.get() > alignment {
            variant_type_details.alignment.get()
        } else {
            alignment
        };

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

    Some((
        IoTypeDetails {
            recommended_capacity: enum_capacity,
            alignment: NonZeroU8::new(alignment).expect("At least zero; qed"),
        },
        input,
    ))
}

/// Copies `n` bytes from input to output and returns both input and output after `n` bytes offset
#[inline(always)]
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
    target.copy_from_slice(source);

    Some((input, output))
}

/// Skips `n` bytes and return remainder
#[inline(always)]
const fn skip_n_bytes(input: &[u8], n: usize) -> Option<&[u8]> {
    if n > input.len() {
        return None;
    }

    // `&input[n..]` not supported in const yet
    Some(input.split_at(n).1)
}
