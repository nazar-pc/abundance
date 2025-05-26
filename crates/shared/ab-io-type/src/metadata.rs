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
    /// If actual data is larger, it will be passed down to the guest as it is, if smaller than host
    /// must allocate the recommended capacity for guest anyway.
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
    /// `struct S {..}`
    ///
    /// Structs with named fields are encoded af follows:
    /// * Length of struct name in bytes (u8)
    /// * Struct name as UTF-8 bytes
    /// * Number of fields (u8)
    ///
    /// Each field is encoded follows:
    /// * Length of the field name in bytes (u8)
    /// * Field name as UTF-8 bytes
    /// * Recursive metadata of the field's type
    Struct,
    /// Similar to [`Self::Struct`], but for exactly `0` struct fields and thus skips number of
    /// fields after struct name
    Struct0,
    /// Similar to [`Self::Struct`], but for exactly `1` struct fields and thus skips number of
    /// fields after struct name
    Struct1,
    /// Similar to [`Self::Struct`], but for exactly `2` struct fields and thus skips number of
    /// fields after struct name
    Struct2,
    /// Similar to [`Self::Struct`], but for exactly `3` struct fields and thus skips number of
    /// fields after struct name
    Struct3,
    /// Similar to [`Self::Struct`], but for exactly `4` struct fields and thus skips number of
    /// fields after struct name
    Struct4,
    /// Similar to [`Self::Struct`], but for exactly `5` struct fields and thus skips number of
    /// fields after struct name
    Struct5,
    /// Similar to [`Self::Struct`], but for exactly `6` struct fields and thus skips number of
    /// fields after struct name
    Struct6,
    /// Similar to [`Self::Struct`], but for exactly `7` struct fields and thus skips number of
    /// fields after struct name
    Struct7,
    /// Similar to [`Self::Struct`], but for exactly `8` struct fields and thus skips number of
    /// fields after struct name
    Struct8,
    /// Similar to [`Self::Struct`], but for exactly `9` struct fields and thus skips number of
    /// fields after struct name
    Struct9,
    /// Similar to [`Self::Struct`], but for exactly `10` struct fields and thus skips number of
    /// fields after struct name
    Struct10,
    /// `struct S(..);`
    ///
    /// Tuple structs are encoded af follows:
    /// * Length of struct name in bytes (u8)
    /// * Struct name as UTF-8 bytes
    /// * Number of fields (u8)
    ///
    /// Each field is encoded follows:
    /// * Recursive metadata of the field's type
    TupleStruct,
    /// Similar to [`Self::TupleStruct`], but for exactly `1` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct1,
    /// Similar to [`Self::TupleStruct`], but for exactly `2` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct2,
    /// Similar to [`Self::TupleStruct`], but for exactly `3` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct3,
    /// Similar to [`Self::TupleStruct`], but for exactly `4` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct4,
    /// Similar to [`Self::TupleStruct`], but for exactly `5` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct5,
    /// Similar to [`Self::TupleStruct`], but for exactly `6` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct6,
    /// Similar to [`Self::TupleStruct`], but for exactly `7` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct7,
    /// Similar to [`Self::TupleStruct`], but for exactly `8` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct8,
    /// Similar to [`Self::TupleStruct`], but for exactly `9` struct fields and thus skips number of
    /// fields after struct name
    TupleStruct9,
    /// Similar to [`Self::TupleStruct`], but for exactly `10` struct fields and thus skips number
    /// of fields after struct name
    TupleStruct10,
    /// `enum E { Variant {..} }`
    ///
    /// Enums with variants that have fields are encoded as follows:
    /// * Length of enum name in bytes (u8)
    /// * Enum name as UTF-8 bytes
    /// * Number of variants (u8)
    /// * Each enum variant as if it was a struct with fields, see [`Self::Struct`] for details
    Enum,
    /// Similar to [`Self::Enum`], but for exactly `1` enum variants and thus skips number of
    /// variants after enum name
    Enum1,
    /// Similar to [`Self::Enum`], but for exactly `2` enum variants and thus skips number of
    /// variants after enum name
    Enum2,
    /// Similar to [`Self::Enum`], but for exactly `3` enum variants and thus skips number of
    /// variants after enum name
    Enum3,
    /// Similar to [`Self::Enum`], but for exactly `4` enum variants and thus skips number of
    /// variants after enum name
    Enum4,
    /// Similar to [`Self::Enum`], but for exactly `5` enum variants and thus skips number of
    /// variants after enum name
    Enum5,
    /// Similar to [`Self::Enum`], but for exactly `6` enum variants and thus skips number of
    /// variants after enum name
    Enum6,
    /// Similar to [`Self::Enum`], but for exactly `7` enum variants and thus skips number of
    /// variants after enum name
    Enum7,
    /// Similar to [`Self::Enum`], but for exactly `8` enum variants and thus skips number of
    /// variants after enum name
    Enum8,
    /// Similar to [`Self::Enum`], but for exactly `9` enum variants and thus skips number of
    /// variants after enum name
    Enum9,
    /// Similar to [`Self::Enum`], but for exactly `10` enum variants and thus skips number of
    /// variants after enum name
    Enum10,
    /// `enum E { A, B }`
    ///
    /// Enums with variants that have no fields are encoded as follows:
    /// * Length of enum name in bytes (u8)
    /// * Enum name as UTF-8 bytes
    /// * Number of variants (u8)
    ///
    /// Each enum variant is encoded follows:
    /// * Length of the variant name in bytes (u8)
    /// * Variant name as UTF-8 bytes
    EnumNoFields,
    /// Similar to [`Self::EnumNoFields`], but for exactly `1` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields1,
    /// Similar to [`Self::EnumNoFields`], but for exactly `2` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields2,
    /// Similar to [`Self::EnumNoFields`], but for exactly `3` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields3,
    /// Similar to [`Self::EnumNoFields`], but for exactly `4` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields4,
    /// Similar to [`Self::EnumNoFields`], but for exactly `5` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields5,
    /// Similar to [`Self::EnumNoFields`], but for exactly `6` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields6,
    /// Similar to [`Self::EnumNoFields`], but for exactly `7` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields7,
    /// Similar to [`Self::EnumNoFields`], but for exactly `8` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields8,
    /// Similar to [`Self::EnumNoFields`], but for exactly `9` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields9,
    /// Similar to [`Self::EnumNoFields`], but for exactly `10` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields10,
    /// Array `[T; N]` with up to 2^8 elements.
    ///
    /// Encoded as follows:
    /// * 1 byte number of elements
    /// * Recursive metadata of a contained type
    Array8b,
    /// Array `[T; N]` with up to 2^16 elements.
    ///
    /// Encoded as follows:
    /// * 2 bytes number of elements (little-endian)
    /// * Recursive metadata of a contained type
    Array16b,
    /// Array `[T; N]` with up to 2^32 elements.
    ///
    /// Encoded as follows:
    /// * 4 bytes number of elements (little-endian)
    /// * Recursive metadata of a contained type
    Array32b,
    /// Compact alias for `[u8; 8]`
    ArrayU8x8,
    /// Compact alias for `[u8; 16]`
    ArrayU8x16,
    /// Compact alias for `[u8; 32]`
    ArrayU8x32,
    /// Compact alias for `[u8; 64]`
    ArrayU8x64,
    /// Compact alias for `[u8; 128]`
    ArrayU8x128,
    /// Compact alias for `[u8; 256]`
    ArrayU8x256,
    /// Compact alias for `[u8; 512]`
    ArrayU8x512,
    /// Compact alias for `[u8; 1024]`
    ArrayU8x1024,
    /// Compact alias for `[u8; 2028]`
    ArrayU8x2028,
    /// Compact alias for `[u8; 4096]`
    ArrayU8x4096,
    /// Variable bytes with up to 2^8 bytes recommended allocation.
    ///
    /// Encoded as follows:
    /// * 1 byte recommended allocation in bytes
    VariableBytes8b,
    /// Variable bytes with up to 2^16 bytes recommended allocation.
    ///
    /// Encoded as follows:
    /// * 2 bytes recommended allocation in bytes (little-endian)
    VariableBytes16b,
    /// Variable bytes with up to 2^32 bytes recommended allocation.
    ///
    /// Encoded as follows:
    /// * 4 bytes recommended allocation in bytes (little-endian)
    VariableBytes32b,
    /// Compact alias [`VariableBytes<0>`](crate::variable_bytes::VariableBytes)
    VariableBytes0,
    /// Compact alias [`VariableBytes<512>`](crate::variable_bytes::VariableBytes)
    VariableBytes512,
    /// Compact alias [`VariableBytes<1024>`](crate::variable_bytes::VariableBytes)
    VariableBytes1024,
    /// Compact alias [`VariableBytes<2028>`](crate::variable_bytes::VariableBytes)
    VariableBytes2028,
    /// Compact alias [`VariableBytes<4096>`](crate::variable_bytes::VariableBytes)
    VariableBytes4096,
    /// Compact alias [`VariableBytes<8192>`](crate::variable_bytes::VariableBytes)
    VariableBytes8192,
    /// Compact alias [`VariableBytes<16384>`](crate::variable_bytes::VariableBytes)
    VariableBytes16384,
    /// Compact alias [`VariableBytes<32768>`](crate::variable_bytes::VariableBytes)
    VariableBytes32768,
    /// Compact alias [`VariableBytes<65536>`](crate::variable_bytes::VariableBytes)
    VariableBytes65536,
    /// Compact alias [`VariableBytes<131072>`](crate::variable_bytes::VariableBytes)
    VariableBytes131072,
    /// Compact alias [`VariableBytes<262144>`](crate::variable_bytes::VariableBytes)
    VariableBytes262144,
    /// Compact alias [`VariableBytes<524288>`](crate::variable_bytes::VariableBytes)
    VariableBytes524288,
    /// Compact alias [`VariableBytes<1048576>`](crate::variable_bytes::VariableBytes)
    VariableBytes1048576,
    /// Variable elements with up to 2^8 elements recommended allocation.
    ///
    /// Encoded as follows:
    /// * 1 byte recommended allocation in bytes
    /// * Recursive metadata of a contained type
    VariableElements8b,
    /// Variable elements with up to 2^16 elements recommended allocation.
    ///
    /// Encoded as follows:
    /// * 2 bytes recommended allocation in elements (little-endian)
    /// * Recursive metadata of a contained type
    VariableElements16b,
    /// Variable elements with up to 2^32 elements recommended allocation.
    ///
    /// Encoded as follows:
    /// * 4 bytes recommended allocation in elements (little-endian)
    /// * Recursive metadata of a contained type
    VariableElements32b,
    /// Compact alias [`VariableElements<0, T>`](crate::variable_elements::VariableElements)
    ///
    /// Encoded as follows:
    /// * Recursive metadata of a contained type
    VariableElements0,
    /// Fixed capacity bytes with up to 2^8 bytes capacity.
    ///
    /// Encoded as follows:
    /// * 1 byte capacity
    FixedCapacityBytes8b,
    /// Fixed capacity bytes with up to 2^16 bytes capacity.
    ///
    /// Encoded as follows:
    /// * 2 bytes capacity (little-endian)
    FixedCapacityBytes16b,
    /// Fixed capacity UTF-8 string with up to 2^8 bytes capacity.
    ///
    /// This is a string only by convention, there is no runtime verification done, contents is
    /// treated as regular bytes.
    ///
    /// Encoded as follows:
    /// * 1 byte capacity
    FixedCapacityString8b,
    /// Fixed capacity UTF-8 bytes with up to 2^16 bytes capacity.
    ///
    /// This is a string only by convention, there is no runtime verification done, contents is
    /// treated as regular bytes.
    ///
    /// Encoded as follows:
    /// * 2 bytes capacity (little-endian)
    FixedCapacityString16b,
    /// Unaligned wrapper over another [`TrivialType`].
    ///
    /// [`TrivialType`]: crate::trivial_type::TrivialType
    ///
    /// Encoded as follows:
    /// * Recursive metadata of a contained type
    Unaligned,
    /// Address of a contract.
    ///
    /// Internally `u128` with `8` byte alignment
    Address = 128,
    /// Balance of a token.
    ///
    /// Internally `u128` with `8` byte alignment
    Balance,
}

impl IoTypeMetadataKind {
    // TODO: Implement `TryFrom` once it is available in const environment
    /// Try to create an instance from its `u8` representation
    #[inline]
    pub const fn try_from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
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
            13 => Self::Struct0,
            14 => Self::Struct1,
            15 => Self::Struct2,
            16 => Self::Struct3,
            17 => Self::Struct4,
            18 => Self::Struct5,
            19 => Self::Struct6,
            20 => Self::Struct7,
            21 => Self::Struct8,
            22 => Self::Struct9,
            23 => Self::Struct10,
            24 => Self::TupleStruct,
            25 => Self::TupleStruct1,
            26 => Self::TupleStruct2,
            27 => Self::TupleStruct3,
            28 => Self::TupleStruct4,
            29 => Self::TupleStruct5,
            30 => Self::TupleStruct6,
            31 => Self::TupleStruct7,
            32 => Self::TupleStruct8,
            33 => Self::TupleStruct9,
            34 => Self::TupleStruct10,
            35 => Self::Enum,
            36 => Self::Enum1,
            37 => Self::Enum2,
            38 => Self::Enum3,
            39 => Self::Enum4,
            40 => Self::Enum5,
            41 => Self::Enum6,
            42 => Self::Enum7,
            43 => Self::Enum8,
            44 => Self::Enum9,
            45 => Self::Enum10,
            46 => Self::EnumNoFields,
            47 => Self::EnumNoFields1,
            48 => Self::EnumNoFields2,
            49 => Self::EnumNoFields3,
            50 => Self::EnumNoFields4,
            51 => Self::EnumNoFields5,
            52 => Self::EnumNoFields6,
            53 => Self::EnumNoFields7,
            54 => Self::EnumNoFields8,
            55 => Self::EnumNoFields9,
            56 => Self::EnumNoFields10,
            57 => Self::Array8b,
            58 => Self::Array16b,
            59 => Self::Array32b,
            60 => Self::ArrayU8x8,
            61 => Self::ArrayU8x16,
            62 => Self::ArrayU8x32,
            63 => Self::ArrayU8x64,
            64 => Self::ArrayU8x128,
            65 => Self::ArrayU8x256,
            66 => Self::ArrayU8x512,
            67 => Self::ArrayU8x1024,
            68 => Self::ArrayU8x2028,
            69 => Self::ArrayU8x4096,
            70 => Self::VariableBytes8b,
            71 => Self::VariableBytes16b,
            72 => Self::VariableBytes32b,
            73 => Self::VariableBytes0,
            74 => Self::VariableBytes512,
            75 => Self::VariableBytes1024,
            76 => Self::VariableBytes2028,
            77 => Self::VariableBytes4096,
            78 => Self::VariableBytes8192,
            79 => Self::VariableBytes16384,
            80 => Self::VariableBytes32768,
            81 => Self::VariableBytes65536,
            82 => Self::VariableBytes131072,
            83 => Self::VariableBytes262144,
            84 => Self::VariableBytes524288,
            85 => Self::VariableBytes1048576,
            86 => Self::VariableElements8b,
            87 => Self::VariableElements16b,
            88 => Self::VariableElements32b,
            89 => Self::VariableElements0,
            90 => Self::FixedCapacityBytes8b,
            91 => Self::FixedCapacityBytes16b,
            92 => Self::FixedCapacityString8b,
            93 => Self::FixedCapacityString16b,
            94 => Self::Unaligned,
            128 => Self::Address,
            129 => Self::Balance,
            _ => {
                return None;
            }
        })
    }

    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Produce compact metadata.
    ///
    /// Compact metadata retains the shape, but throws some details. Specifically, the following
    /// transformations are applied to metadata:
    /// * Struct names, enum names and enum variant names are removed (replaced with zero bytes
    ///   names)
    /// * Structs and enum variants are turned into tuple variants (removing field names)
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
