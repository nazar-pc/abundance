use crate::metadata::{IoTypeDetails, IoTypeMetadataKind};
use crate::trivial_type::TrivialType;
use core::num::NonZeroU8;

#[inline(always)]
pub(super) const fn decode_type_details(mut metadata: &[u8]) -> Option<(IoTypeDetails, &[u8])> {
    let kind = IoTypeMetadataKind::try_from(*metadata.split_off_first()?).ok()?;

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
        IoTypeMetadataKind::Struct => struct_type_details(metadata, None),
        IoTypeMetadataKind::Enum => enum_capacity(metadata, true),
        IoTypeMetadataKind::EnumNoFields => enum_capacity(metadata, false),
        IoTypeMetadataKind::Array | IoTypeMetadataKind::VariableElements => {
            if metadata.is_empty() {
                return None;
            }

            let mut num_elements = [0; const { size_of::<u32>() }];
            (metadata, _) = copy_n_bytes(metadata, &mut num_elements, size_of::<u32>())?;
            let num_elements = u32::from_le_bytes(num_elements);

            let type_details;
            (type_details, metadata) = decode_type_details(metadata)?;
            let recommended_capacity = type_details
                .recommended_capacity
                .checked_mul(num_elements)?;
            Some((
                IoTypeDetails {
                    recommended_capacity,
                    alignment: type_details.alignment,
                },
                metadata,
            ))
        }
        IoTypeMetadataKind::FixedCapacityElements => {
            let capacity;
            (capacity, metadata) = metadata.split_first_chunk()?;
            let capacity = u32::from_le_bytes(*capacity);

            let type_details;
            (type_details, metadata) = decode_type_details(metadata)?;
            // The length is stored as `u32`, extended to the alignment of elements if it is larger,
            // such that elements are aligned
            let len_size = u32::from(type_details.alignment.get()).max(u32::SIZE);
            let recommended_capacity = type_details
                .recommended_capacity
                .checked_mul(capacity)?
                .checked_add(len_size)?;

            Some((
                IoTypeDetails {
                    recommended_capacity,
                    alignment: type_details.alignment,
                },
                metadata,
            ))
        }
        IoTypeMetadataKind::FixedCapacityString => {
            let capacity;
            (capacity, metadata) = metadata.split_first_chunk()?;
            // The length is stored as `u32`
            let recommended_capacity = u32::from_le_bytes(*capacity).checked_add(u32::SIZE)?;

            Some((IoTypeDetails::bytes(recommended_capacity), metadata))
        }
        IoTypeMetadataKind::Unaligned => {
            if metadata.is_empty() {
                return None;
            }

            let type_details;
            (type_details, metadata) = decode_type_details(metadata)?;

            Some((
                IoTypeDetails::bytes(type_details.recommended_capacity),
                metadata,
            ))
        }
    }
}

#[inline(always)]
const fn struct_type_details(
    mut input: &[u8],
    field_count: Option<u8>,
) -> Option<(IoTypeDetails, &[u8])> {
    // Skip struct name
    let struct_name_length = *input.split_off_first()?;
    // TODO: `split_off()` is not `const fn` yet, even unstably
    input = input.get(usize::from(struct_name_length)..)?;

    let mut field_count = if let Some(field_count) = field_count {
        field_count
    } else {
        *input.split_off_first()?
    };

    // Whether fields are named, either all fields have names or all of them have zero-length names
    // like fields of tuple structs
    let mut named_fields = None;
    // Capacity of arguments
    let mut capacity = 0u32;
    let mut alignment = 1u8;
    while field_count > 0 {
        // Skip field name
        let field_name_length = *input.split_off_first()?;
        // TODO: `split_off()` is not `const fn` yet, even unstably
        input = input.get(usize::from(field_name_length)..)?;

        let named_field = field_name_length > 0;
        if let Some(named_fields) = named_fields
            && named_fields != named_field
        {
            return None;
        }
        named_fields = Some(named_field);

        // Capacity of argument's type
        let type_details;
        (type_details, input) = decode_type_details(input)?;
        capacity = capacity.checked_add(type_details.recommended_capacity)?;
        alignment = alignment.max(type_details.alignment.get());

        field_count -= 1;
    }

    Some((
        IoTypeDetails {
            recommended_capacity: capacity,
            alignment: NonZeroU8::new(alignment).expect("At least one; qed"),
        },
        input,
    ))
}

#[inline(always)]
const fn enum_capacity(mut input: &[u8], has_fields: bool) -> Option<(IoTypeDetails, &[u8])> {
    // Skip enum name
    let enum_name_length = *input.split_off_first()?;
    // TODO: `split_off()` is not `const fn` yet, even unstably
    input = input.get(usize::from(enum_name_length)..)?;

    let mut variant_count = *input.split_off_first()?;

    // Capacity of variants
    let mut enum_capacity = None;
    let mut alignment = 1u8;
    while variant_count > 0 {
        if input.is_empty() {
            return None;
        }

        let variant_type_details;

        // Variant capacity as if it was a struct
        (variant_type_details, input) =
            struct_type_details(input, if has_fields { None } else { Some(0) })?;
        // `+ 1` is for the discriminant
        let variant_capacity = variant_type_details.recommended_capacity.checked_add(1)?;
        alignment = alignment.max(variant_type_details.alignment.get());

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

    let enum_capacity = enum_capacity.unwrap_or_default();

    Some((
        IoTypeDetails {
            recommended_capacity: enum_capacity,
            alignment: NonZeroU8::new(alignment).expect("At least one; qed"),
        },
        input,
    ))
}

/// Copies `n` bytes from input to output and returns both input and output after `n` bytes offset
#[inline(always)]
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
