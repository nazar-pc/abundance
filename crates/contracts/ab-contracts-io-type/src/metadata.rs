use core::ptr;

/// Max capacity for metadata bytes used in fixed size buffers
pub const MAX_METADATA_CAPACITY: usize = 8192;

/// Concatenates metadata sources.
///
/// Returns both a scratch memory and number of bytes in it that correspond to metadata
pub const fn concat_metadata_sources(sources: &[&[u8]]) -> ([u8; MAX_METADATA_CAPACITY], usize) {
    let mut metadata_scratch = [0u8; MAX_METADATA_CAPACITY];
    // Just a way to convert above array into slice, `as_mut_slice` is not yet
    // stable in const environment
    let (_, mut remainder) = metadata_scratch.split_at_mut(0);

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
/// This metadata is sufficient to fully reconstruct hierarchy of the type in order to generate
/// language bindings, auto-generate UI forms, etc.
///
/// [`TrivialType::METADATA`]: crate::trivial_type::TrivialType::METADATA
/// [`IoType::METADATA`]: crate::IoType::METADATA
// TODO: Function that generates compact metadata
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
