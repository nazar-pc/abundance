mod compact;
#[cfg(test)]
mod tests;
mod type_details;
mod type_name;

use crate::metadata::compact::compact_metadata;
use crate::metadata::type_details::decode_type_details;
use crate::metadata::type_name::type_name;
use core::num::NonZeroU8;

/// Max capacity for metadata bytes used in fixed size buffers
pub const MAX_METADATA_CAPACITY: usize = 8192;

/// Concatenates metadata sources.
///
/// Returns both a scratch memory and number of bytes in it that correspond to metadata
pub const fn concat_metadata_sources(sources: &[&[u8]]) -> ([u8; MAX_METADATA_CAPACITY], usize) {
    let mut metadata_scratch = [0u8; MAX_METADATA_CAPACITY];
    let mut remainder = metadata_scratch.as_mut_slice();

    // For loops are not yet usable in const environment
    let mut i = 0;
    while i < sources.len() {
        let source = sources[i];
        let target;
        (target, remainder) = remainder.split_at_mut(source.len());
        target.copy_from_slice(source);
        i += 1;
    }

    let remainder_len = remainder.len();
    let size = metadata_scratch.len() - remainder_len;
    (metadata_scratch, size)
}

#[derive(Debug, Copy, Clone)]
pub struct IoTypeDetails {
    /// Recommended capacity that must be allocated by the host.
    ///
    /// If actual data is larger, it will be passed down to the guest as it is, if smaller than the
    /// host must allocate the recommended capacity for guest anyway.
    pub recommended_capacity: u32,
    /// Alignment of the type
    pub alignment: NonZeroU8,
}

impl IoTypeDetails {
    /// Create an instance for regular bytes (alignment 1)
    #[inline(always)]
    pub const fn bytes(recommended_capacity: u32) -> Self {
        Self {
            recommended_capacity,
            alignment: NonZeroU8::new(1).expect("Not zero; qed"),
        }
    }
}

/// Metadata types contained in [`TrivialType::METADATA`] and [`IoType::METADATA`].
///
/// Metadata encoding consists of this enum variant treated as `u8` followed by optional metadata
/// encoding rules specific to metadata type variant (see variant's description).
///
/// This metadata is enough to fully reconstruct the hierarchy of the type to generate language
/// bindings, auto-generate UI forms, etc.
///
/// [`TrivialType::METADATA`]: crate::trivial_type::TrivialType::METADATA
/// [`IoType::METADATA`]: crate::IoType::METADATA
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum IoTypeMetadataKind {
    /// `()`
    Unit,
    /// [`Bool`](crate::bool::Bool)
    Bool,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`
    U64,
    /// `u128`
    U128,
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `i32`
    I32,
    /// `i64`
    I64,
    /// `i128`
    I128,
    /// `struct S {..}`, `struct S(..);` or `struct S;`
    ///
    /// Structs are encoded as follows:
    /// * Length of struct name in bytes (u8)
    /// * Struct name as UTF-8 bytes
    /// * Number of fields (u8)
    ///
    /// Each field is encoded as follows:
    /// * Length of the field name in bytes (u8), zero for fields of tuple structs
    /// * Field name as UTF-8 bytes
    /// * Recursive metadata of the field's type
    ///
    /// Decoders reject structs that have both fields with names and fields with zero-length
    /// names.
    Struct,
    /// `enum E { A, B {..}, C(..) }`
    ///
    /// Enums are encoded as follows:
    /// * Length of enum name in bytes (u8)
    /// * Enum name as UTF-8 bytes
    /// * Number of variants (u8)
    ///
    /// Each variant is encoded as follows:
    /// * Length of the variant name in bytes (u8)
    /// * Variant name as UTF-8 bytes
    /// * Number of fields (u8), zero for variants without fields
    /// * Fields, each encoded like a field of [`Self::Struct`], with zero-length names for fields
    ///   of tuple variants
    ///
    /// Each variant is a 1-byte discriminant followed by its fields, all variants must have the
    /// same size. Decoders reject enums without variants and variants that have both fields with
    /// names and fields with zero-length names.
    Enum,
    /// Array `[T; N]`.
    ///
    /// Encoded as follows:
    /// * 4 bytes number of elements (little-endian)
    /// * Recursive metadata of a contained type
    Array,
    /// [`VariableElements`](crate::variable_elements::VariableElements).
    ///
    /// Encoded as follows:
    /// * 4 bytes recommended allocation in elements (little-endian)
    /// * Recursive metadata of a contained type
    VariableElements,
    /// [`FixedCapacityElements`](crate::fixed_capacity_elements::FixedCapacityElements).
    ///
    /// Encoded as follows:
    /// * 4 bytes capacity in elements (little-endian)
    /// * Recursive metadata of a contained type
    ///
    /// Elements follow a little-endian `u32` length, padded with zeroes to the alignment of
    /// elements if it is larger.
    FixedCapacityElements,
    /// [`FixedCapacityString`](crate::fixed_capacity_string::FixedCapacityString).
    ///
    /// This is a string only by convention, there is no runtime verification done, contents is
    /// treated as regular bytes.
    ///
    /// Encoded as follows:
    /// * 4 bytes capacity in bytes (little-endian)
    ///
    /// Bytes follow a little-endian `u32` length.
    FixedCapacityString,
    /// Unaligned wrapper over another [`TrivialType`].
    ///
    /// [`TrivialType`]: crate::trivial_type::TrivialType
    ///
    /// Encoded as follows:
    /// * Recursive metadata of a contained type
    Unaligned,
}

const impl TryFrom<u8> for IoTypeMetadataKind {
    type Error = ();

    #[inline]
    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        Ok(match byte {
            0 => Self::Unit,
            1 => Self::Bool,
            2 => Self::U8,
            3 => Self::U16,
            4 => Self::U32,
            5 => Self::U64,
            6 => Self::U128,
            7 => Self::I8,
            8 => Self::I16,
            9 => Self::I32,
            10 => Self::I64,
            11 => Self::I128,
            12 => Self::Struct,
            13 => Self::Enum,
            14 => Self::Array,
            15 => Self::VariableElements,
            16 => Self::FixedCapacityElements,
            17 => Self::FixedCapacityString,
            18 => Self::Unaligned,
            _ => {
                return Err(());
            }
        })
    }
}

impl IoTypeMetadataKind {
    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Produce compact metadata.
    ///
    /// Compact metadata retains the shape, but throws some details. Specifically, names of structs,
    /// fields, enums and enum variants are replaced with zero-length names, so a struct with named
    /// fields has the same compact metadata as a tuple struct with fields of the same types.
    /// Compact metadata of valid metadata is valid metadata too.
    ///
    /// This is typically called by higher-level functions and doesn't need to be used directly.
    ///
    /// This function takes an `input` that starts with metadata defined in [`IoTypeMetadataKind`]
    /// and `output` where compact metadata must be written. Since input might have other data past
    /// the data structure to be processed, the remainder of input and output are returned to the
    /// caller.
    ///
    /// Unexpected metadata kind results in `None` being returned.
    #[inline]
    pub const fn compact<'i, 'o>(
        input: &'i [u8],
        output: &'o mut [u8],
    ) -> Option<(&'i [u8], &'o mut [u8])> {
        compact_metadata(input, output)
    }

    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Decode type name.
    ///
    /// Expected to be UTF-8, but must be parsed before printed as text, which is somewhat costly.
    #[inline]
    pub const fn type_name(metadata: &[u8]) -> Option<&[u8]> {
        type_name(metadata)
    }

    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Decode type, return its recommended capacity that should be allocated by the host.
    ///
    /// If actual data is larger, it will be passed down to the guest as it is, if smaller than host
    /// should allocate recommended capacity for guest anyway.
    ///
    /// Returns type details and whatever slice of bytes from `input` that is left after
    /// type decoding.
    #[inline]
    pub const fn type_details(metadata: &[u8]) -> Option<(IoTypeDetails, &[u8])> {
        decode_type_details(metadata)
    }
}
