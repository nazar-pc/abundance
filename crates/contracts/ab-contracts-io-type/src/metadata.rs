mod compact;
mod recommended_capacity;
mod type_name;

use crate::metadata::compact::compact_metadata;
use crate::metadata::recommended_capacity::recommended_capacity;
use crate::metadata::type_name::type_name;
use core::ptr;

/// Max capacity for metadata bytes used in fixed size buffers
pub const MAX_METADATA_CAPACITY: usize = 8192;

/// Concatenates metadata sources.
///
/// Returns both a scratch memory and number of bytes in it that correspond to metadata
pub const fn concat_metadata_sources(sources: &[&[u8]]) -> ([u8; MAX_METADATA_CAPACITY], usize) {
    let mut metadata_scratch = [0u8; MAX_METADATA_CAPACITY];
    // TODO: Use `as_mut_slice` once stabilized: https://github.com/rust-lang/rust/issues/133333
    let mut remainder: &mut [u8] = &mut metadata_scratch;

    // For loops are not yet usable in const environment
    let mut i = 0;
    while i < sources.len() {
        let source = sources[i];
        let target;
        (target, remainder) = remainder.split_at_mut(source.len());

        // TODO: Switch to `copy_from_slice` once stable:
        //  https://github.com/rust-lang/rust/issues/131415
        // The same as `target.copy_from_slice(&source);`, but it doesn't work in const environment
        // yet
        // SAFETY: Size is correct due to slicing above, pointers are created from valid independent
        // slices of equal length
        unsafe {
            ptr::copy_nonoverlapping(source.as_ptr(), target.as_mut_ptr(), source.len());
        }
        i += 1;
    }

    let remainder_len = remainder.len();
    let size = metadata_scratch.len() - remainder_len;
    (metadata_scratch, size)
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
    /// `bool`
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
    /// Similar to [`Self::Struct`], but for exactly `11` struct fields and thus skips number of
    /// fields after struct name
    Struct11,
    /// Similar to [`Self::Struct`], but for exactly `12` struct fields and thus skips number of
    /// fields after struct name
    Struct12,
    /// Similar to [`Self::Struct`], but for exactly `13` struct fields and thus skips number of
    /// fields after struct name
    Struct13,
    /// Similar to [`Self::Struct`], but for exactly `14` struct fields and thus skips number of
    /// fields after struct name
    Struct14,
    /// Similar to [`Self::Struct`], but for exactly `15` struct fields and thus skips number of
    /// fields after struct name
    Struct15,
    /// Similar to [`Self::Struct`], but for exactly `16` struct fields and thus skips number of
    /// fields after struct name
    Struct16,
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
    /// Similar to [`Self::TupleStruct`], but for exactly `11` struct fields and thus skips number
    /// of fields after struct name
    TupleStruct11,
    /// Similar to [`Self::TupleStruct`], but for exactly `12` struct fields and thus skips number
    /// of fields after struct name
    TupleStruct12,
    /// Similar to [`Self::TupleStruct`], but for exactly `13` struct fields and thus skips number
    /// of fields after struct name
    TupleStruct13,
    /// Similar to [`Self::TupleStruct`], but for exactly `14` struct fields and thus skips number
    /// of fields after struct name
    TupleStruct14,
    /// Similar to [`Self::TupleStruct`], but for exactly `15` struct fields and thus skips number
    /// of fields after struct name
    TupleStruct15,
    /// Similar to [`Self::TupleStruct`], but for exactly `16` struct fields and thus skips number
    /// of fields after struct name
    TupleStruct16,
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
    /// Similar to [`Self::Enum`], but for exactly `11` enum variants and thus skips number of
    /// variants after enum name
    Enum11,
    /// Similar to [`Self::Enum`], but for exactly `12` enum variants and thus skips number of
    /// variants after enum name
    Enum12,
    /// Similar to [`Self::Enum`], but for exactly `13` enum variants and thus skips number of
    /// variants after enum name
    Enum13,
    /// Similar to [`Self::Enum`], but for exactly `14` enum variants and thus skips number of
    /// variants after enum name
    Enum14,
    /// Similar to [`Self::Enum`], but for exactly `15` enum variants and thus skips number of
    /// variants after enum name
    Enum15,
    /// Similar to [`Self::Enum`], but for exactly `16` enum variants and thus skips number of
    /// variants after enum name
    Enum16,
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
    /// Similar to [`Self::EnumNoFields`], but for exactly `11` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields11,
    /// Similar to [`Self::EnumNoFields`], but for exactly `12` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields12,
    /// Similar to [`Self::EnumNoFields`], but for exactly `13` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields13,
    /// Similar to [`Self::EnumNoFields`], but for exactly `14` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields14,
    /// Similar to [`Self::EnumNoFields`], but for exactly `15` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields15,
    /// Similar to [`Self::EnumNoFields`], but for exactly `16` enum variants and thus skips number
    /// of variants after enum name
    EnumNoFields16,
    /// Array `[T; N]` with up to 2^8 elements.
    ///
    /// Arrays with up to 2^8 encoded as follows:
    /// * 1 byte number of elements
    /// * Recursive metadata of contained type
    Array8b,
    /// Array `[T; N]` with up to 2^16 elements.
    ///
    /// Arrays with up to 2^16 encoded as follows:
    /// * 2 bytes number of elements (little-endian)
    /// * Recursive metadata of contained type
    Array16b,
    /// Array `[T; N]` with up to 2^32 elements.
    ///
    /// Arrays with up to 2^32 encoded as follows:
    /// * 4 bytes number of elements (little-endian)
    /// * Recursive metadata of contained type
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
    /// Variable bytes with up to 2^8 encoded as follows:
    /// * 1 byte recommended allocation in bytes
    VariableBytes8b,
    /// Variable bytes with up to 2^16 bytes recommended allocation.
    ///
    /// Variable bytes with up to 2^16 encoded as follows:
    /// * 2 bytes recommended allocation in bytes (little-endian)
    VariableBytes16b,
    /// Variable bytes with up to 2^32 bytes recommended allocation.
    ///
    /// Variable bytes with up to 2^8 encoded as follows:
    /// * 4 bytes recommended allocation in bytes (little-endian)
    VariableBytes32b,
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
    /// Compact alias [`VariableBytes<2097152>`](crate::variable_bytes::VariableBytes)
    VariableBytes2097152,
    /// Compact alias [`VariableBytes<4194304>`](crate::variable_bytes::VariableBytes)
    VariableBytes4194304,
    /// Compact alias [`VariableBytes<8388608>`](crate::variable_bytes::VariableBytes)
    VariableBytes8388608,
    /// Compact alias [`VariableBytes<16777216>`](crate::variable_bytes::VariableBytes)
    VariableBytes16777216,
}

impl IoTypeMetadataKind {
    // TODO: Implement `TryFrom` once it is available in const environment
    /// Try to create an instance from its `u8` representation
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
            24 => Self::Struct11,
            25 => Self::Struct12,
            26 => Self::Struct13,
            27 => Self::Struct14,
            28 => Self::Struct15,
            29 => Self::Struct16,
            30 => Self::TupleStruct,
            31 => Self::TupleStruct1,
            32 => Self::TupleStruct2,
            33 => Self::TupleStruct3,
            34 => Self::TupleStruct4,
            35 => Self::TupleStruct5,
            36 => Self::TupleStruct6,
            37 => Self::TupleStruct7,
            38 => Self::TupleStruct8,
            39 => Self::TupleStruct9,
            40 => Self::TupleStruct10,
            41 => Self::TupleStruct11,
            42 => Self::TupleStruct12,
            43 => Self::TupleStruct13,
            44 => Self::TupleStruct14,
            45 => Self::TupleStruct15,
            46 => Self::TupleStruct16,
            47 => Self::Enum,
            48 => Self::Enum1,
            49 => Self::Enum2,
            50 => Self::Enum3,
            51 => Self::Enum4,
            52 => Self::Enum5,
            53 => Self::Enum6,
            54 => Self::Enum7,
            55 => Self::Enum8,
            56 => Self::Enum9,
            57 => Self::Enum10,
            58 => Self::Enum11,
            59 => Self::Enum12,
            60 => Self::Enum13,
            61 => Self::Enum14,
            62 => Self::Enum15,
            63 => Self::Enum16,
            64 => Self::EnumNoFields,
            65 => Self::EnumNoFields1,
            66 => Self::EnumNoFields2,
            67 => Self::EnumNoFields3,
            68 => Self::EnumNoFields4,
            69 => Self::EnumNoFields5,
            70 => Self::EnumNoFields6,
            71 => Self::EnumNoFields7,
            72 => Self::EnumNoFields8,
            73 => Self::EnumNoFields9,
            74 => Self::EnumNoFields10,
            75 => Self::EnumNoFields11,
            76 => Self::EnumNoFields12,
            77 => Self::EnumNoFields13,
            78 => Self::EnumNoFields14,
            79 => Self::EnumNoFields15,
            80 => Self::EnumNoFields16,
            81 => Self::Array8b,
            82 => Self::Array16b,
            83 => Self::Array32b,
            84 => Self::ArrayU8x8,
            85 => Self::ArrayU8x16,
            86 => Self::ArrayU8x32,
            87 => Self::ArrayU8x64,
            88 => Self::ArrayU8x128,
            89 => Self::ArrayU8x256,
            90 => Self::ArrayU8x512,
            91 => Self::ArrayU8x1024,
            92 => Self::ArrayU8x2028,
            93 => Self::ArrayU8x4096,
            94 => Self::VariableBytes8b,
            95 => Self::VariableBytes16b,
            96 => Self::VariableBytes32b,
            97 => Self::VariableBytes512,
            98 => Self::VariableBytes1024,
            99 => Self::VariableBytes2028,
            100 => Self::VariableBytes4096,
            101 => Self::VariableBytes8192,
            102 => Self::VariableBytes16384,
            103 => Self::VariableBytes32768,
            104 => Self::VariableBytes65536,
            105 => Self::VariableBytes131072,
            106 => Self::VariableBytes262144,
            107 => Self::VariableBytes524288,
            108 => Self::VariableBytes1048576,
            109 => Self::VariableBytes2097152,
            110 => Self::VariableBytes4194304,
            111 => Self::VariableBytes8388608,
            112 => Self::VariableBytes16777216,
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
    pub const fn compact<'i, 'o>(
        input: &'i [u8],
        output: &'o mut [u8],
    ) -> Option<(&'i [u8], &'o mut [u8])> {
        compact_metadata(input, output)
    }

    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Decode type name
    pub const fn type_name(metadata: &[u8]) -> Option<&str> {
        type_name(metadata)
    }

    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Decode type, return its recommended capacity that should be allocated by the host.
    ///
    /// If actual data is larger, it will be passed down to the guest as it is, if smaller than host
    /// should allocate recommended capacity for guest anyway.
    ///
    /// Returns recommended capacity and whatever slice of bytes from `input` that is left after
    /// type decoding.
    pub const fn recommended_capacity(metadata: &[u8]) -> Option<(u32, &[u8])> {
        recommended_capacity(metadata)
    }
}
