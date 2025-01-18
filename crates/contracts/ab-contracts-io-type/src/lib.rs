#![no_std]

pub mod maybe_data;
pub mod trivial_type;
pub mod utils;
pub mod variable_bytes;

use crate::trivial_type::TrivialType;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;

// Refuse to compile on lower than 32-bit platforms
static_assertions::const_assert!(size_of::<usize>() >= size_of::<u32>());

// Only support little-endian environments, in big-endian byte order will be different, and
// it'll not be possible to simply send bytes of data structures that implement `TrivialType` from host
// to guest environment
static_assertions::const_assert_eq!(u16::from_ne_bytes(1u16.to_le_bytes()), 1u16);

// Only support targets with expected alignment and refuse to compile on other targets
static_assertions::const_assert_eq!(align_of::<()>(), 1);
static_assertions::const_assert_eq!(align_of::<u8>(), 1);
static_assertions::const_assert_eq!(align_of::<u16>(), 2);
static_assertions::const_assert_eq!(align_of::<u32>(), 4);
static_assertions::const_assert_eq!(align_of::<u64>(), 8);
static_assertions::const_assert_eq!(align_of::<u128>(), 16);
static_assertions::const_assert_eq!(align_of::<i8>(), 1);
static_assertions::const_assert_eq!(align_of::<i16>(), 2);
static_assertions::const_assert_eq!(align_of::<i32>(), 4);
static_assertions::const_assert_eq!(align_of::<i64>(), 8);
static_assertions::const_assert_eq!(align_of::<i128>(), 16);

/// Metadata types contained in [`TrivialType::METADATA`] and [`IoType::METADATA`].
///
/// Metadata encoding consists of this enum variant treated as `u8` followed by optional metadata
/// encoding rules specific to metadata type variant (see variant's description).
///
/// This metadata is sufficient to fully reconstruct hierarchy of the type in order to generate
/// language bindings, auto-generate UI forms, etc.
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
    /// * Each enum variant as if it was a struct, see [`Self::Struct`] and [`Self::TupleStruct`]
    ///   for details, depending on whether variant has named fields or looks like a tuple
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
    /// This is helpful for host allocations that do not need to think about allocation alignment
    /// and can simply allocate all data structures at 4096 bytes alignment.
    ///
    /// Without this types metadata would have to store alignment alongside other details, but since
    /// alignment beyond 4096 bytes is unlikely to be used in practice, metadata can be simplified.
    // TODO: Reject alignment customizations instead, such that it can be inferred automatically
    pub const MAX_ALIGNMENT: usize = 4096;
    // TODO: Add more constants once above variants are extended with shortcuts for typical data
    //  types
}

// TODO: A way to point output types to input types in order to avoid unnecessary memory copy
//  (setting a pointer)
/// Trait that is used for types that are crossing host/guest boundary in smart contracts.
///
/// Crucially it is implemented for any type that implements [`TrivialType`] and for
/// [`VariableBytes`](crate::variable_bytes::VariableBytes).
///
/// # Safety
/// This trait is used for types with memory transmutation capabilities, it must not be relied on
/// with untrusted data. Serializing and deserializing of types that implement this trait is simply
/// casting of underlying memory, as the result all the types implementing this trait must not use
/// implicit padding, unions or anything similar that might make it unsound to access any bits of
/// the type.
///
/// Helper functions are provided to make casting to/from bytes a bit safer than it would otherwise,
/// but extra care is still needed.
///
/// **Do not implement this type explicitly!** Use `#[derive(TrivialType)]` instead, which will
/// ensure safety requirements are upheld, or use `VariableBytes` if more flexibility is needed.
///
/// In case of variable state size is needed, create a wrapper struct around `VariableBytes` and
/// implement traits on it by forwarding everything to inner implementation.
pub unsafe trait IoType {
    /// Data structure metadata in binary form, describing shape and types of the contents, see
    /// [`IoTypeMetadataKind`] for encoding details.
    const METADATA: &[u8];

    /// Pointer with trivial type that this `IoType` represents
    type PointerType: TrivialType;

    /// How many bytes are currently used to store data
    fn size(&self) -> u32;

    /// How many bytes are allocated right now
    fn capacity(&self) -> u32;

    /// Set number of used bytes
    ///
    /// # Safety
    /// `size` must be set to number of properly bytes
    unsafe fn set_size(&mut self, size: u32);

    /// Create a reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned and sufficient in size, but padding beyond the size of the
    /// type is allowed. Memory behind pointer must not be written to in the meantime either.
    ///
    /// Only `size` are guaranteed to be allocated for types that can store variable amount of
    /// data due to read-only nature of read-only access here.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
    // `impl Deref` is used to tie lifetime of returned value to inputs, but still treat it as a
    // shared reference for most practical purposes. While lifetime here is somewhat superficial due
    // to `Copy` nature of the value, it must be respected.
    unsafe fn from_ptr<'a>(
        ptr: &'a NonNull<Self::PointerType>,
        size: &'a u32,
        capacity: u32,
    ) -> impl Deref<Target = Self> + 'a;

    /// Create a mutable reference to a type, which is represented by provided memory.
    ///
    /// Memory must be correctly aligned and sufficient in size or else `None` will be returned, but
    /// padding beyond the size of the type is allowed. Memory behind pointer must not be read or
    /// written to in the meantime either.
    ///
    /// `size` indicates how many bytes are used within larger allocation for types that can
    /// store variable amount of data.
    ///
    /// # Safety
    /// Input bytes must be previously produced by taking underlying bytes of the same type.
    // `impl DerefMut` is used to tie lifetime of returned value to inputs, but still treat it as an
    // exclusive reference for most practical purposes. While lifetime here is somewhat superficial
    // due to `Copy` nature of the value, it must be respected.
    unsafe fn from_ptr_mut<'a>(
        ptr: &'a mut NonNull<Self::PointerType>,
        size: &'a mut u32,
        capacity: u32,
    ) -> impl DerefMut<Target = Self> + 'a;

    /// Get raw pointer to the underlying data with no checks
    ///
    /// # Safety
    /// While calling this function is technically safe, it and allows to ignore many of its
    /// invariants, so requires extra care. In particular no modifications must be done to the value
    /// while this returned pointer might be used and no changes must be done through returned
    /// pointer. Also, lifetimes are only superficial here and can be easily (and incorrectly)
    /// ignored by using `Copy`.
    unsafe fn as_ptr(&self) -> impl Deref<Target = NonNull<Self::PointerType>>;

    /// Get exclusive raw pointer to the underlying data with no checks
    ///
    /// # Safety
    /// While calling this function is technically safe, it and allows to ignore many of its
    /// invariants, so requires extra care. In particular the value's contents must not be read or
    /// written to while returned point might be used. Also, lifetimes are only superficial here and
    /// can be easily (and incorrectly) ignored by using `Copy`.
    unsafe fn as_mut_ptr(&mut self) -> impl DerefMut<Target = NonNull<Self::PointerType>>;
}

/// Marker trait, companion to [`IoType`] that indicates ability to store optional contents
pub trait IoTypeOptional: IoType {}
