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
/// This metadata is sufficient to fully reconstruct hierarchy of the type in order to generate
/// language bindings, auto-generate UI forms, etc.
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

    /// Produce compact metadata.
    ///
    /// Compact metadata retains the shape, but throws some of the details. Specifically following
    /// transformations are applied to metadata:
    /// * Struct names, enum names and enum variant names are removed (replaced with 0 bytes names)
    /// * Structs and enum variants are turned into tuple variants (removing field names)
    ///
    /// This is typically called by higher-level functions and doesn't need to be used directly.
    ///
    /// This function takes an `input` that starts with metadata defined in [`IoTypeMetadataKind`]
    /// and `output` where compact metadata must be written. Since input might have other data past
    /// the data structure to be processed, remainders of input and output are returned back to the
    /// caller.
    ///
    /// Unexpected metadata kind results in `None` being returned.
    pub const fn compact<'i, 'o>(
        input: &'i [u8],
        output: &'o mut [u8],
    ) -> Option<(&'i [u8], &'o mut [u8])> {
        compact_metadata(input, output)
    }
}

/// This macro is necessary to reduce boilerplate due to lack of `?` in const environment
macro_rules! forward_option {
    ($expr:expr) => {{
        let Some(result) = $expr else {
            return None;
        };
        result
    }};
}

const fn compact_metadata<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    let kind = forward_option!(IoTypeMetadataKind::try_from_u8(input[0]));

    match kind {
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
        | IoTypeMetadataKind::I128 => copy_n_bytes(input, output, 1),
        IoTypeMetadataKind::Struct => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, None, false)
        }
        IoTypeMetadataKind::Struct0 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(0), false)
        }
        IoTypeMetadataKind::Struct1 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct1 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(1), false)
        }
        IoTypeMetadataKind::Struct2 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct2 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(2), false)
        }
        IoTypeMetadataKind::Struct3 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct3 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(3), false)
        }
        IoTypeMetadataKind::Struct4 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct4 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(4), false)
        }
        IoTypeMetadataKind::Struct5 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct5 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(5), false)
        }
        IoTypeMetadataKind::Struct6 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct6 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(6), false)
        }
        IoTypeMetadataKind::Struct7 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct7 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(7), false)
        }
        IoTypeMetadataKind::Struct8 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct8 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(8), false)
        }
        IoTypeMetadataKind::Struct9 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct9 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(9), false)
        }
        IoTypeMetadataKind::Struct10 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct10 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(10), false)
        }
        IoTypeMetadataKind::Struct11 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct11 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(11), false)
        }
        IoTypeMetadataKind::Struct12 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct12 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(12), false)
        }
        IoTypeMetadataKind::Struct13 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct13 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(13), false)
        }
        IoTypeMetadataKind::Struct14 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct14 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(14), false)
        }
        IoTypeMetadataKind::Struct15 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct15 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(15), false)
        }
        IoTypeMetadataKind::Struct16 => {
            // Convert struct with field names to tuple struct
            output[0] = IoTypeMetadataKind::TupleStruct16 as u8;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            compact_struct(input, output, Some(16), false)
        }
        IoTypeMetadataKind::TupleStruct => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, None, true)
        }
        IoTypeMetadataKind::TupleStruct1 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(1), true)
        }
        IoTypeMetadataKind::TupleStruct2 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(2), true)
        }
        IoTypeMetadataKind::TupleStruct3 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(3), true)
        }
        IoTypeMetadataKind::TupleStruct4 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(4), true)
        }
        IoTypeMetadataKind::TupleStruct5 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(5), true)
        }
        IoTypeMetadataKind::TupleStruct6 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(6), true)
        }
        IoTypeMetadataKind::TupleStruct7 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(7), true)
        }
        IoTypeMetadataKind::TupleStruct8 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(8), true)
        }
        IoTypeMetadataKind::TupleStruct9 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(9), true)
        }
        IoTypeMetadataKind::TupleStruct10 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(10), true)
        }
        IoTypeMetadataKind::TupleStruct11 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(11), true)
        }
        IoTypeMetadataKind::TupleStruct12 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(12), true)
        }
        IoTypeMetadataKind::TupleStruct13 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(13), true)
        }
        IoTypeMetadataKind::TupleStruct14 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(14), true)
        }
        IoTypeMetadataKind::TupleStruct15 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(15), true)
        }
        IoTypeMetadataKind::TupleStruct16 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_struct(input, output, Some(16), true)
        }
        IoTypeMetadataKind::Enum => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, None, true)
        }
        IoTypeMetadataKind::Enum1 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(1), true)
        }
        IoTypeMetadataKind::Enum2 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(2), true)
        }
        IoTypeMetadataKind::Enum3 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(3), true)
        }
        IoTypeMetadataKind::Enum4 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(4), true)
        }
        IoTypeMetadataKind::Enum5 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(5), true)
        }
        IoTypeMetadataKind::Enum6 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(6), true)
        }
        IoTypeMetadataKind::Enum7 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(7), true)
        }
        IoTypeMetadataKind::Enum8 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(8), true)
        }
        IoTypeMetadataKind::Enum9 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(9), true)
        }
        IoTypeMetadataKind::Enum10 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(10), true)
        }
        IoTypeMetadataKind::Enum11 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(11), true)
        }
        IoTypeMetadataKind::Enum12 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(12), true)
        }
        IoTypeMetadataKind::Enum13 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(13), true)
        }
        IoTypeMetadataKind::Enum14 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(14), true)
        }
        IoTypeMetadataKind::Enum15 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(15), true)
        }
        IoTypeMetadataKind::Enum16 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(16), true)
        }
        IoTypeMetadataKind::EnumNoFields => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, None, false)
        }
        IoTypeMetadataKind::EnumNoFields1 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(1), false)
        }
        IoTypeMetadataKind::EnumNoFields2 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(2), false)
        }
        IoTypeMetadataKind::EnumNoFields3 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(3), false)
        }
        IoTypeMetadataKind::EnumNoFields4 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(4), false)
        }
        IoTypeMetadataKind::EnumNoFields5 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(5), false)
        }
        IoTypeMetadataKind::EnumNoFields6 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(6), false)
        }
        IoTypeMetadataKind::EnumNoFields7 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(7), false)
        }
        IoTypeMetadataKind::EnumNoFields8 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(8), false)
        }
        IoTypeMetadataKind::EnumNoFields9 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(9), false)
        }
        IoTypeMetadataKind::EnumNoFields10 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(10), false)
        }
        IoTypeMetadataKind::EnumNoFields11 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(11), false)
        }
        IoTypeMetadataKind::EnumNoFields12 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(12), false)
        }
        IoTypeMetadataKind::EnumNoFields13 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(13), false)
        }
        IoTypeMetadataKind::EnumNoFields14 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(14), false)
        }
        IoTypeMetadataKind::EnumNoFields15 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(15), false)
        }
        IoTypeMetadataKind::EnumNoFields16 => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));
            compact_enum(input, output, Some(16), false)
        }
        IoTypeMetadataKind::Array8b => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1 + 1));
            compact_metadata(input, output)
        }
        IoTypeMetadataKind::Array16b => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1 + 2));
            compact_metadata(input, output)
        }
        IoTypeMetadataKind::Array32b => {
            (input, output) = forward_option!(copy_n_bytes(input, output, 1 + 4));
            compact_metadata(input, output)
        }
        IoTypeMetadataKind::ArrayU8x8
        | IoTypeMetadataKind::ArrayU8x16
        | IoTypeMetadataKind::ArrayU8x32
        | IoTypeMetadataKind::ArrayU8x64
        | IoTypeMetadataKind::ArrayU8x128
        | IoTypeMetadataKind::ArrayU8x256
        | IoTypeMetadataKind::ArrayU8x512
        | IoTypeMetadataKind::ArrayU8x1024
        | IoTypeMetadataKind::ArrayU8x2028
        | IoTypeMetadataKind::ArrayU8x4096 => copy_n_bytes(input, output, 1),
        IoTypeMetadataKind::VariableBytes8b => copy_n_bytes(input, output, 1 + 1),
        IoTypeMetadataKind::VariableBytes16b => copy_n_bytes(input, output, 1 + 2),
        IoTypeMetadataKind::VariableBytes32b => copy_n_bytes(input, output, 1 + 4),
        IoTypeMetadataKind::VariableBytes512
        | IoTypeMetadataKind::VariableBytes1024
        | IoTypeMetadataKind::VariableBytes2028
        | IoTypeMetadataKind::VariableBytes4096
        | IoTypeMetadataKind::VariableBytes8192
        | IoTypeMetadataKind::VariableBytes16384
        | IoTypeMetadataKind::VariableBytes32768
        | IoTypeMetadataKind::VariableBytes65536
        | IoTypeMetadataKind::VariableBytes131072
        | IoTypeMetadataKind::VariableBytes262144
        | IoTypeMetadataKind::VariableBytes524288
        | IoTypeMetadataKind::VariableBytes1048576
        | IoTypeMetadataKind::VariableBytes2097152
        | IoTypeMetadataKind::VariableBytes4194304
        | IoTypeMetadataKind::VariableBytes8388608
        | IoTypeMetadataKind::VariableBytes16777216 => copy_n_bytes(input, output, 1),
    }
}

const fn compact_struct<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    arguments_count: Option<u8>,
    tuple: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    // Remove struct name
    let struct_name_length = input[0] as usize;
    output[0] = 0;
    (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
    input = forward_option!(skip_n_bytes(input, struct_name_length));

    let mut arguments_count = match arguments_count {
        Some(arguments_count) => arguments_count,
        None => {
            if input.is_empty() || output.is_empty() {
                return None;
            }

            let arguments_count = input[0];
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));

            arguments_count
        }
    };

    // Compact arguments
    while arguments_count > 0 {
        if input.is_empty() || output.is_empty() {
            return None;
        }

        // Remove field name if needed
        if !tuple {
            let field_name_length = input[0] as usize;
            input = forward_option!(skip_n_bytes(input, 1 + field_name_length));
        }

        // Compact argument's type
        (input, output) = forward_option!(compact_metadata(input, output));

        arguments_count -= 1;
    }

    Some((input, output))
}

const fn compact_enum<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    variant_count: Option<u8>,
    has_fields: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    // Remove enum name
    let enum_name_length = input[0] as usize;
    output[0] = 0;
    (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
    input = forward_option!(skip_n_bytes(input, enum_name_length));

    let mut variant_count = match variant_count {
        Some(variant_count) => variant_count,
        None => {
            if input.is_empty() || output.is_empty() {
                return None;
            }

            let variant_count = input[0];
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));

            variant_count
        }
    };

    // Compact enum variants
    while variant_count > 0 {
        if input.is_empty() || output.is_empty() {
            return None;
        }

        // Remove variant name
        let field_name_length = input[0] as usize;
        input = forward_option!(skip_n_bytes(input, 1 + field_name_length));

        if has_fields {
            // Compact variant as if it was a struct
            (input, output) = forward_option!(compact_struct(input, output, None, false));
        } else {
            // Compact variant as if it was a struct without fields
            (input, output) = forward_option!(compact_struct(input, output, Some(0), false));
        }

        variant_count -= 1;
    }

    Some((input, output))
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

/// Skips `n` bytes in input and output
const fn skip_n_bytes_io<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if n > input.len() || n > output.len() {
        return None;
    }

    // `&input[n..]` not supported in const yet
    input = input.split_at(n).1;
    // `&mut output[n..]` not supported in const yet
    output = output.split_at_mut(n).1;

    Some((input, output))
}
