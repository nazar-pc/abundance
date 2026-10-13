use crate::metadata::stack::Stack;
use crate::metadata::{IoTypeDetails, IoTypeMetadataKind};
use crate::trivial_type::TrivialType;
use core::num::NonZeroU8;

/// Fields of a struct or of an enum variant
#[derive(Debug, Copy, Clone)]
struct Fields {
    /// The number of fields that were not reached yet
    remaining: u8,
    /// Whether fields are named, `None` before the first field
    named: Option<bool>,
    /// Capacity of decoded fields, for enum variants including the discriminant before them, which
    /// is also the offset of the next field
    capacity: u32,
    /// Alignment of decoded fields, for enums including fields of previous variants
    alignment: NonZeroU8,
}

impl Fields {
    /// Read the number of fields, the first field is placed at `offset`
    #[inline(always)]
    const fn new(metadata: &mut &[u8], offset: u32, alignment: NonZeroU8) -> Option<Self> {
        Some(Self {
            remaining: *metadata.split_off_first()?,
            named: None,
            capacity: offset,
            alignment,
        })
    }

    /// Move to the next field and skip its name, returns `false` if there are no more fields
    #[inline(always)]
    const fn next(&mut self, metadata: &mut &[u8]) -> Option<bool> {
        if self.remaining == 0 {
            return Some(false);
        }
        self.remaining -= 1;

        // Either all fields have names or all of them have zero-length names like fields of tuple
        // structs
        let named = skip_name(metadata)? > 0;
        if let Some(named_fields) = self.named
            && named_fields != named
        {
            return None;
        }
        self.named = Some(named);

        Some(true)
    }

    /// Add details of a decoded field
    #[inline(always)]
    const fn add(&mut self, type_details: IoTypeDetails) -> Option<()> {
        // Fields are placed one after another, a field at an offset that is not a multiple of its
        // alignment would need padding before it, which `TrivialType` doesn't allow
        if !self
            .capacity
            .is_multiple_of(u32::from(type_details.alignment.get()))
        {
            return None;
        }
        self.capacity = self
            .capacity
            .checked_add(type_details.recommended_capacity)?;
        self.alignment = self.alignment.max(type_details.alignment);

        Some(())
    }

    /// Details of all fields together
    #[inline(always)]
    const fn type_details(self) -> Option<IoTypeDetails> {
        // Capacity that is not a multiple of alignment would need padding after the last field,
        // which `TrivialType` doesn't allow
        if !self
            .capacity
            .is_multiple_of(u32::from(self.alignment.get()))
        {
            return None;
        }

        Some(IoTypeDetails {
            recommended_capacity: self.capacity,
            alignment: self.alignment,
        })
    }
}

/// A compound type whose contained types are being decoded
#[derive(Debug, Copy, Clone)]
enum Frame {
    /// [`IoTypeMetadataKind::Array`] or [`IoTypeMetadataKind::VariableElements`]
    Elements { num_elements: u32 },
    /// [`IoTypeMetadataKind::MaybeData`]
    MaybeData,
    /// [`IoTypeMetadataKind::FixedCapacityElements`]
    FixedCapacityElements { capacity: u32 },
    /// [`IoTypeMetadataKind::Unaligned`]
    Unaligned,
    /// [`IoTypeMetadataKind::Struct`]
    Struct(Fields),
    /// [`IoTypeMetadataKind::Enum`]
    Enum {
        /// Fields of the current variant
        fields: Fields,
        /// The number of variants after the current one
        remaining_variants: u8,
        /// Capacity of decoded variants, `None` before the first variant is decoded
        variant_capacity: Option<u32>,
    },
}

/// What follows after a compound type was advanced
#[derive(Debug)]
enum Step {
    /// Another contained type
    Descend,
    /// Nothing, the compound type is complete
    Complete(IoTypeDetails),
}

/// Decode type details without recursion, metadata nested deeper than `MAX_DEPTH` is rejected
#[inline]
pub(super) const fn decode_type_details<const MAX_DEPTH: usize>(
    mut metadata: &[u8],
) -> Option<(IoTypeDetails, &[u8])> {
    // Compound types whose contained types are being decoded, the filler frame is never used
    let mut stack = Stack::<Frame, MAX_DEPTH>::new(Frame::Unaligned);

    loop {
        let kind = IoTypeMetadataKind::try_from(*metadata.split_off_first()?).ok()?;

        // Details of a complete type, `None` for a compound type whose contained types follow
        let mut type_details = match kind {
            IoTypeMetadataKind::Unit => Some(IoTypeDetails {
                recommended_capacity: 0,
                alignment: NonZeroU8::new(1).expect("Not zero; qed"),
            }),
            IoTypeMetadataKind::Bool | IoTypeMetadataKind::U8 | IoTypeMetadataKind::I8 => {
                Some(IoTypeDetails {
                    recommended_capacity: 1,
                    alignment: NonZeroU8::new(1).expect("Not zero; qed"),
                })
            }
            IoTypeMetadataKind::U16 | IoTypeMetadataKind::I16 => Some(IoTypeDetails {
                recommended_capacity: 2,
                alignment: NonZeroU8::new(2).expect("Not zero; qed"),
            }),
            IoTypeMetadataKind::U32 | IoTypeMetadataKind::I32 => Some(IoTypeDetails {
                recommended_capacity: 4,
                alignment: NonZeroU8::new(4).expect("Not zero; qed"),
            }),
            IoTypeMetadataKind::U64 | IoTypeMetadataKind::I64 => Some(IoTypeDetails {
                recommended_capacity: 8,
                alignment: NonZeroU8::new(8).expect("Not zero; qed"),
            }),
            IoTypeMetadataKind::U128 | IoTypeMetadataKind::I128 => Some(IoTypeDetails {
                recommended_capacity: 16,
                alignment: NonZeroU8::new(16).expect("Not zero; qed"),
            }),
            IoTypeMetadataKind::Struct => {
                // Skip struct name
                skip_name(&mut metadata)?;
                let fields = Fields::new(&mut metadata, 0, NonZeroU8::MIN)?;

                stack.push(Frame::Struct(fields))?;
                None
            }
            IoTypeMetadataKind::Enum => {
                // Skip enum name
                skip_name(&mut metadata)?;
                let variant_count = *metadata.split_off_first()?;
                // Enums without variants have no values, such types are not allowed
                let remaining_variants = variant_count.checked_sub(1)?;

                // Skip name of the first variant
                skip_name(&mut metadata)?;
                // Fields of a variant follow its 1-byte discriminant
                let fields = Fields::new(&mut metadata, 1, NonZeroU8::MIN)?;

                stack.push(Frame::Enum {
                    fields,
                    remaining_variants,
                    variant_capacity: None,
                })?;
                None
            }
            IoTypeMetadataKind::Array => {
                let num_elements = read_u32(&mut metadata)?;

                stack.push(Frame::Elements { num_elements })?;
                None
            }
            IoTypeMetadataKind::MaybeData => {
                // `MaybeData` is not a `TrivialType`, so it can't be contained in other types
                if !stack.is_empty() {
                    return None;
                }

                stack.push(Frame::MaybeData)?;
                None
            }
            IoTypeMetadataKind::VariableElements => {
                // `VariableElements` is not a `TrivialType`, so it can't be contained in other
                // types
                if !stack.is_empty() {
                    return None;
                }
                let num_elements = read_u32(&mut metadata)?;

                stack.push(Frame::Elements { num_elements })?;
                None
            }
            IoTypeMetadataKind::FixedCapacityElements => {
                let capacity = read_u32(&mut metadata)?;

                stack.push(Frame::FixedCapacityElements { capacity })?;
                None
            }
            IoTypeMetadataKind::FixedCapacityString => {
                // The length is stored as `u32`
                let recommended_capacity = read_u32(&mut metadata)?.checked_add(u32::SIZE)?;

                Some(IoTypeDetails::bytes(recommended_capacity))
            }
            IoTypeMetadataKind::Unaligned => {
                stack.push(Frame::Unaligned)?;
                None
            }
        };

        // Advance enclosing compound types until one of them expects another contained type
        loop {
            let Some(frame) = stack.last_mut() else {
                // The outermost type is complete
                return Some((type_details?, metadata));
            };

            match advance(frame, type_details, &mut metadata)? {
                Step::Descend => {
                    break;
                }
                Step::Complete(complete_type_details) => {
                    type_details = Some(complete_type_details);
                    stack.pop()?;
                }
            }
        }
    }
}

/// Advance a compound type right after it was read (`contained` is `None`) or after its contained
/// type was decoded (`contained` has its details)
#[inline(always)]
const fn advance(
    frame: &mut Frame,
    contained: Option<IoTypeDetails>,
    metadata: &mut &[u8],
) -> Option<Step> {
    let type_details = match (frame, contained) {
        (Frame::Struct(fields), contained) => {
            if let Some(contained) = contained {
                fields.add(contained)?;
            }
            if fields.next(metadata)? {
                return Some(Step::Descend);
            }

            fields.type_details()?
        }
        (
            Frame::Enum {
                fields,
                remaining_variants,
                variant_capacity,
            },
            contained,
        ) => {
            if let Some(contained) = contained {
                fields.add(contained)?;
            }
            loop {
                if fields.next(metadata)? {
                    return Some(Step::Descend);
                }

                // The current variant is complete, all variants must have the same capacity
                if let Some(variant_capacity) = *variant_capacity
                    && variant_capacity != fields.capacity
                {
                    return None;
                }
                *variant_capacity = Some(fields.capacity);

                if *remaining_variants == 0 {
                    break fields.type_details()?;
                }
                *remaining_variants -= 1;

                // Skip name of the next variant
                skip_name(metadata)?;
                // Fields of a variant follow its 1-byte discriminant
                *fields = Fields::new(metadata, 1, fields.alignment)?;
            }
        }
        // Other compound types contain exactly one type
        (_, None) => {
            return Some(Step::Descend);
        }
        (Frame::Elements { num_elements }, Some(contained)) => IoTypeDetails {
            recommended_capacity: contained.recommended_capacity.checked_mul(*num_elements)?,
            alignment: contained.alignment,
        },
        (Frame::MaybeData, Some(contained)) => contained,
        (Frame::FixedCapacityElements { capacity }, Some(contained)) => {
            // The length is stored as `u32`, extended to the alignment of elements if it is larger,
            // such that elements are aligned
            let len_size = u32::from(contained.alignment.get()).max(u32::SIZE);

            IoTypeDetails {
                recommended_capacity: contained
                    .recommended_capacity
                    .checked_mul(*capacity)?
                    .checked_add(len_size)?,
                alignment: contained.alignment,
            }
        }
        (Frame::Unaligned, Some(contained)) => IoTypeDetails::bytes(contained.recommended_capacity),
    };

    Some(Step::Complete(type_details))
}

/// Read a little-endian `u32`
#[inline(always)]
const fn read_u32(metadata: &mut &[u8]) -> Option<u32> {
    let bytes;
    (bytes, *metadata) = metadata.split_first_chunk()?;

    Some(u32::from_le_bytes(*bytes))
}

/// Skip a name, returns its length
#[inline(always)]
const fn skip_name(metadata: &mut &[u8]) -> Option<u8> {
    let name_length = *metadata.split_off_first()?;
    // TODO: `split_off()` is not `const fn` yet, even unstably
    *metadata = (*metadata).get(usize::from(name_length)..)?;

    Some(name_length)
}
