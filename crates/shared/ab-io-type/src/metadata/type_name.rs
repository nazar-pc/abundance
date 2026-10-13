use crate::metadata::IoTypeMetadataKind;

#[inline(always)]
pub(super) const fn type_name(mut metadata: &[u8]) -> Option<&[u8]> {
    let kind = IoTypeMetadataKind::try_from(*metadata.split_off_first()?).ok()?;

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
        IoTypeMetadataKind::Struct | IoTypeMetadataKind::Enum => {
            let type_name_length = *metadata.split_off_first()?;

            metadata.get(..usize::from(type_name_length))?
        }
        IoTypeMetadataKind::Array => b"[T; N]",
        IoTypeMetadataKind::MaybeData => b"MaybeData",
        IoTypeMetadataKind::VariableElements => b"VariableElements",
        IoTypeMetadataKind::FixedCapacityElements => b"FixedCapacityElements",
        IoTypeMetadataKind::FixedCapacityString => b"FixedCapacityString",
        IoTypeMetadataKind::Unaligned => b"Unaligned",
    })
}
