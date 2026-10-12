use crate::metadata::IoTypeMetadataKind;
use crate::metadata::stack::Stack;

/// A compound type whose contained types are being compacted
#[derive(Debug, Copy, Clone)]
enum Frame {
    /// [`IoTypeMetadataKind::Array`], [`IoTypeMetadataKind::VariableElements`],
    /// [`IoTypeMetadataKind::FixedCapacityElements`] or [`IoTypeMetadataKind::Unaligned`], which
    /// contain exactly one type
    Wrapper,
    /// [`IoTypeMetadataKind::Struct`] or [`IoTypeMetadataKind::Enum`], a struct is compacted like
    /// an enum with a single variant whose name and number of fields were already compacted
    Fields {
        /// The number of fields of the current variant that were not reached yet
        remaining_fields: u8,
        /// The number of variants that were not reached yet
        remaining_variants: u8,
    },
}

/// Compact metadata without recursion, metadata nested deeper than `MAX_DEPTH` is rejected
#[inline]
pub(super) const fn compact_metadata<'i, 'o, const MAX_DEPTH: usize>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
) -> Option<(&'i [u8], &'o mut [u8])> {
    // Compound types whose contained types are being compacted, the filler frame is never used
    let mut stack = Stack::<Frame, MAX_DEPTH>::new(Frame::Wrapper);

    loop {
        let io_type_metadata_kind_input = *input.split_off_first()?;
        let io_type_metadata_kind_output = output.split_off_first_mut()?;
        let io_type_metadata_kind =
            IoTypeMetadataKind::try_from(io_type_metadata_kind_input).ok()?;
        *io_type_metadata_kind_output = io_type_metadata_kind_input;

        // A compound type whose contained types follow, `None` for a complete type
        let frame = match io_type_metadata_kind {
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
            | IoTypeMetadataKind::I128 => None,
            IoTypeMetadataKind::Struct => {
                // Remove struct name
                (input, output) = compact_name(input, output)?;

                let field_count = *input.split_off_first()?;
                *output.split_off_first_mut()? = field_count;

                Some(Frame::Fields {
                    remaining_fields: field_count,
                    remaining_variants: 0,
                })
            }
            IoTypeMetadataKind::Enum => {
                // Remove enum name
                (input, output) = compact_name(input, output)?;

                let variant_count = *input.split_off_first()?;
                *output.split_off_first_mut()? = variant_count;

                Some(Frame::Fields {
                    remaining_fields: 0,
                    remaining_variants: variant_count,
                })
            }
            IoTypeMetadataKind::Array
            | IoTypeMetadataKind::VariableElements
            | IoTypeMetadataKind::FixedCapacityElements => {
                (input, output) = copy_n_bytes(input, output, size_of::<u32>())?;

                Some(Frame::Wrapper)
            }
            IoTypeMetadataKind::FixedCapacityString => {
                (input, output) = copy_n_bytes(input, output, size_of::<u32>())?;

                None
            }
            IoTypeMetadataKind::Unaligned => Some(Frame::Wrapper),
        };

        if let Some(frame) = frame {
            stack.push(frame)?;
        }
        // Whether the last compacted type is complete
        let mut complete = frame.is_none();

        // Advance enclosing compound types until one of them expects another contained type
        loop {
            let Some(frame) = stack.last_mut() else {
                // The outermost type is complete
                return Some((input, output));
            };

            let descend = match frame {
                Frame::Wrapper => !complete,
                Frame::Fields {
                    remaining_fields,
                    remaining_variants,
                } => loop {
                    if *remaining_fields > 0 {
                        *remaining_fields -= 1;

                        // Remove field name
                        (input, output) = compact_name(input, output)?;
                        break true;
                    }
                    if *remaining_variants == 0 {
                        break false;
                    }
                    *remaining_variants -= 1;

                    // Remove variant name
                    (input, output) = compact_name(input, output)?;

                    *remaining_fields = *input.split_off_first()?;
                    *output.split_off_first_mut()? = *remaining_fields;
                },
            };

            if descend {
                break;
            }
            stack.pop()?;
            complete = true;
        }
    }
}

/// Replaces a name with a zero-length name
#[inline(always)]
const fn compact_name<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
) -> Option<(&'i [u8], &'o mut [u8])> {
    let name_length = *input.split_off_first()?;
    *output.split_off_first_mut()? = 0;
    // TODO: `split_off()` is not `const fn` yet, even unstably
    input = input.get(usize::from(name_length)..)?;

    Some((input, output))
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
