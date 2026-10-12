//! Tests of `METADATA` of built-in types and of types with derived `TrivialType`.
//!
//! Every case checks exact metadata bytes, and that the metadata decodes back into the name, size
//! and alignment of the type, compacts into expected bytes that decode into the same size and
//! alignment, and that no truncated metadata decodes.

#![expect(dead_code, reason = "Types are only defined for their metadata")]

use ab_io_type::bool::Bool;
use ab_io_type::fixed_capacity_bytes::FixedCapacityBytes;
use ab_io_type::fixed_capacity_elements::FixedCapacityElements;
use ab_io_type::fixed_capacity_string::FixedCapacityString;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY};
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use ab_io_type::variable_elements::VariableElements;

/// Field names used by [`u8_struct()`]
const FIELD_NAMES: [&str; 12] = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"];
/// Variant names used by [`u8_enum()`]
const VARIANT_NAMES: [&str; 17] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
];

/// Declares a struct with named `u8` fields
macro_rules! named_struct {
    ($name:ident, [$( $field:ident ),*]) => {
        #[derive(Copy, Clone, TrivialType)]
        #[repr(C)]
        struct $name {
            $( $field: u8, )*
        }
    };
}

/// Declares a tuple struct with `u8` fields, one for each listed identifier
macro_rules! tuple_struct {
    ($name:ident, [$( $field:ident ),*]) => {
        #[derive(Copy, Clone, TrivialType)]
        #[repr(C)]
        struct $name( $( tuple_struct!(@u8 $field), )* );
    };
    (@u8 $field:ident) => {
        u8
    };
}

/// Declares an enum with variants without fields
macro_rules! fieldless_enum {
    ($name:ident, [$( $variant:ident ),*]) => {
        #[derive(Copy, Clone, TrivialType)]
        #[repr(u8)]
        enum $name {
            $( $variant, )*
        }
    };
}

/// Declares an enum with variants that have a single `value: u8` field
macro_rules! data_enum {
    ($name:ident, [$( $variant:ident ),*]) => {
        #[derive(Copy, Clone, TrivialType)]
        #[repr(u8)]
        enum $name {
            $( $variant { value: u8 }, )*
        }
    };
}

named_struct!(Point, [x, y]);

/// Expected metadata bytes, built piece by piece
#[derive(Debug)]
struct Expected(Vec<u8>);

impl Expected {
    /// Start with a metadata kind
    fn new(kind: IoTypeMetadataKind) -> Self {
        Self(vec![kind as u8])
    }

    /// Append a metadata kind
    fn kind(mut self, kind: IoTypeMetadataKind) -> Self {
        self.0.push(kind as u8);
        self
    }

    /// Append a name prefixed with its length
    fn name(mut self, name: &str) -> Self {
        self.0.push(u8::try_from(name.len()).unwrap());
        self.0.extend_from_slice(name.as_bytes());
        self
    }

    /// Append a single byte, like the number of fields or variants
    fn byte(mut self, byte: u8) -> Self {
        self.0.push(byte);
        self
    }

    /// Append a little-endian `u32`
    fn u32(mut self, value: u32) -> Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Append metadata of another type
    fn nested(mut self, nested: &Self) -> Self {
        self.0.extend_from_slice(&nested.0);
        self
    }
}

/// Compact metadata of a single type
fn compact(metadata: &[u8]) -> Vec<u8> {
    let mut compact = [0; MAX_METADATA_CAPACITY];
    let (remainder, compact_remainder) =
        IoTypeMetadataKind::compact(metadata, &mut compact).unwrap();
    assert!(remainder.is_empty());
    let compact_length = MAX_METADATA_CAPACITY - compact_remainder.len();

    compact[..compact_length].to_vec()
}

/// Check that metadata decodes into the expected name, capacity and alignment, compacts into the
/// expected bytes that decode into the same capacity and alignment, and that no truncated metadata
/// decodes
#[track_caller]
fn check_decoding(
    metadata: &[u8],
    name: &str,
    capacity: usize,
    alignment: usize,
    expected_compact: &Expected,
) {
    assert_eq!(
        IoTypeMetadataKind::type_name(metadata),
        Some(name.as_bytes())
    );

    let (type_details, remainder) = IoTypeMetadataKind::type_details(metadata).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(
        usize::try_from(type_details.recommended_capacity).unwrap(),
        capacity
    );
    assert_eq!(usize::from(type_details.alignment.get()), alignment);

    let compact_metadata = compact(metadata);
    assert_eq!(compact_metadata, expected_compact.0);

    // Compact metadata is valid metadata of the same shape
    let (compact_type_details, remainder) =
        IoTypeMetadataKind::type_details(&compact_metadata).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(
        compact_type_details.recommended_capacity,
        type_details.recommended_capacity
    );
    assert_eq!(compact_type_details.alignment, type_details.alignment);
    assert_eq!(compact(&compact_metadata), compact_metadata);

    let mut compact_buffer = [0; MAX_METADATA_CAPACITY];
    for length in 0..metadata.len() {
        let truncated = &metadata[..length];

        assert!(IoTypeMetadataKind::type_details(truncated).is_none());
        assert!(IoTypeMetadataKind::compact(truncated, &mut compact_buffer).is_none());
    }
}

/// Check that metadata doesn't decode
#[track_caller]
fn check_rejected(metadata: &[u8]) {
    assert!(IoTypeMetadataKind::type_details(metadata).is_none());
}

/// Check that metadata decodes completely into the given capacity and alignment
#[track_caller]
fn check_layout(metadata: &[u8], capacity: u32, alignment: u8) {
    let (type_details, remainder) = IoTypeMetadataKind::type_details(metadata).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(type_details.recommended_capacity, capacity);
    assert_eq!(type_details.alignment.get(), alignment);
}

/// Check `METADATA` of a [`TrivialType`] and its decoding, see [`check_decoding()`]
#[track_caller]
fn check_trivial_type<T>(name: &str, expected: &Expected, expected_compact: &Expected)
where
    T: TrivialType,
{
    assert_eq!(<T as TrivialType>::METADATA, expected.0.as_slice());
    assert_eq!(<T as ab_io_type::IoType>::METADATA, expected.0.as_slice());
    check_decoding(
        T::METADATA,
        name,
        size_of::<T>(),
        align_of::<T>(),
        expected_compact,
    );
}

/// Check `METADATA` of an [`IoType`](ab_io_type::IoType) and its decoding, see
/// [`check_decoding()`]
#[track_caller]
fn check_io_type<T>(
    name: &str,
    capacity: usize,
    alignment: usize,
    expected: &Expected,
    expected_compact: &Expected,
) where
    T: ab_io_type::IoType,
{
    assert_eq!(T::METADATA, expected.0.as_slice());
    check_decoding(T::METADATA, name, capacity, alignment, expected_compact);
}

/// Check that compact metadata of two types is the same
#[track_caller]
fn check_same_compact<A, B>()
where
    A: ab_io_type::IoType,
    B: ab_io_type::IoType,
{
    assert_eq!(compact(A::METADATA), compact(B::METADATA));
}

/// Expected metadata and compact metadata of [`Point`]
fn point() -> (Expected, Expected) {
    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("Point")
        .byte(2)
        .name("x")
        .kind(IoTypeMetadataKind::U8)
        .name("y")
        .kind(IoTypeMetadataKind::U8);
    let expected_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(2)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::U8);

    (expected, expected_compact)
}

/// Expected metadata and compact metadata of a struct declared with [`named_struct!`] or
/// [`tuple_struct!`] using the first `field_count` of [`FIELD_NAMES`]
fn u8_struct(name: &str, field_count: u8, named: bool) -> (Expected, Expected) {
    let mut expected = Expected::new(IoTypeMetadataKind::Struct)
        .name(name)
        .byte(field_count);
    let mut expected_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(field_count);

    for field_name in &FIELD_NAMES[..usize::from(field_count)] {
        // Fields of tuple structs have zero-length names
        expected = expected
            .name(if named { field_name } else { "" })
            .kind(IoTypeMetadataKind::U8);
        expected_compact = expected_compact.name("").kind(IoTypeMetadataKind::U8);
    }

    (expected, expected_compact)
}

/// Expected metadata and compact metadata of an enum declared with [`fieldless_enum!`] or
/// [`data_enum!`] using the first `variant_count` of [`VARIANT_NAMES`]
fn u8_enum(name: &str, variant_count: u8, with_fields: bool) -> (Expected, Expected) {
    let mut expected = Expected::new(IoTypeMetadataKind::Enum)
        .name(name)
        .byte(variant_count);
    let mut expected_compact = Expected::new(IoTypeMetadataKind::Enum)
        .name("")
        .byte(variant_count);

    for variant_name in &VARIANT_NAMES[..usize::from(variant_count)] {
        expected = expected.name(variant_name);
        expected_compact = expected_compact.name("");
        if with_fields {
            expected = expected.byte(1).name("value").kind(IoTypeMetadataKind::U8);
            expected_compact = expected_compact
                .byte(1)
                .name("")
                .kind(IoTypeMetadataKind::U8);
        } else {
            expected = expected.byte(0);
            expected_compact = expected_compact.byte(0);
        }
    }

    (expected, expected_compact)
}

/// Check `METADATA` of a struct declared with [`named_struct!`], see [`u8_struct()`]
#[track_caller]
fn check_named_struct<T>(name: &str, field_count: u8)
where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_struct(name, field_count, true);
    check_trivial_type::<T>(name, &expected, &expected_compact);
}

/// Check `METADATA` of a struct declared with [`tuple_struct!`], see [`u8_struct()`]
#[track_caller]
fn check_tuple_struct<T>(name: &str, field_count: u8)
where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_struct(name, field_count, false);
    check_trivial_type::<T>(name, &expected, &expected_compact);
}

/// Check `METADATA` of an enum declared with [`fieldless_enum!`], see [`u8_enum()`]
#[track_caller]
fn check_fieldless_enum<T>(name: &str, variant_count: u8)
where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_enum(name, variant_count, false);
    check_trivial_type::<T>(name, &expected, &expected_compact);
}

/// Check `METADATA` of an enum declared with [`data_enum!`], see [`u8_enum()`]
#[track_caller]
fn check_data_enum<T>(name: &str, variant_count: u8)
where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_enum(name, variant_count, true);
    check_trivial_type::<T>(name, &expected, &expected_compact);
}

#[test]
fn primitives() {
    let cases = [
        (<()>::METADATA, IoTypeMetadataKind::Unit, "()", 0, 1),
        (Bool::METADATA, IoTypeMetadataKind::Bool, "bool", 1, 1),
        (u8::METADATA, IoTypeMetadataKind::U8, "u8", 1, 1),
        (u16::METADATA, IoTypeMetadataKind::U16, "u16", 2, 2),
        (u32::METADATA, IoTypeMetadataKind::U32, "u32", 4, 4),
        (u64::METADATA, IoTypeMetadataKind::U64, "u64", 8, 8),
        (u128::METADATA, IoTypeMetadataKind::U128, "u128", 16, 16),
        (i8::METADATA, IoTypeMetadataKind::I8, "i8", 1, 1),
        (i16::METADATA, IoTypeMetadataKind::I16, "i16", 2, 2),
        (i32::METADATA, IoTypeMetadataKind::I32, "i32", 4, 4),
        (i64::METADATA, IoTypeMetadataKind::I64, "i64", 8, 8),
        (i128::METADATA, IoTypeMetadataKind::I128, "i128", 16, 16),
    ];

    for (metadata, kind, name, size, alignment) in cases {
        let expected = Expected::new(kind);

        assert_eq!(metadata, expected.0.as_slice());
        check_decoding(metadata, name, size, alignment, &expected);
    }
}

#[test]
fn arrays_of_u8() {
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(0)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 0]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(1)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 1]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(8)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 8]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(32)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 32]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(255)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 255]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(256)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 256]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(2048)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 2048]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(65_535)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 65_535]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(65_536)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 65_536]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(70_000)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 70_000]>("[T; N]", &expected, &expected);
}

#[test]
fn arrays_of_other_types() {
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(8)
        .kind(IoTypeMetadataKind::U16);
    check_trivial_type::<[u16; 8]>("[T; N]", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(300)
        .kind(IoTypeMetadataKind::U64);
    check_trivial_type::<[u64; 300]>("[T; N]", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(2)
        .kind(IoTypeMetadataKind::Array)
        .u32(8)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[[u8; 8]; 2]>("[T; N]", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(3)
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U32);
    check_trivial_type::<[Unaligned<u32>; 3]>("[T; N]", &expected, &expected);

    let (point, point_compact) = point();
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(4)
        .nested(&point);
    let expected_compact = Expected::new(IoTypeMetadataKind::Array)
        .u32(4)
        .nested(&point_compact);
    check_trivial_type::<[Point; 4]>("[T; N]", &expected, &expected_compact);
}

#[test]
fn unaligned() {
    let expected = Expected::new(IoTypeMetadataKind::Unaligned).kind(IoTypeMetadataKind::U64);
    check_trivial_type::<Unaligned<u64>>("Unaligned", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::Array)
        .u32(8)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<Unaligned<[u8; 8]>>("Unaligned", &expected, &expected);
}

#[test]
fn fixed_capacity_elements() {
    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct Wide {
        a: u64,
        b: u32,
        c: u32,
    }

    let name = "FixedCapacityElements";

    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(0)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<FixedCapacityBytes<0>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(10)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<FixedCapacityBytes<10>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(70_000)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<FixedCapacityBytes<70_000>>(name, &expected, &expected);
    // The largest size
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(u32::MAX - 4)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<FixedCapacityBytes<{ u32::MAX as usize - 4 }>>(name, &expected, &expected);

    // The length is extended to the alignment of elements, which is the alignment of the container
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::U16);
    check_trivial_type::<FixedCapacityElements<u16, 3>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::U32);
    check_trivial_type::<FixedCapacityElements<u32, 3>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::U64);
    check_trivial_type::<FixedCapacityElements<u64, 3>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(0)
        .kind(IoTypeMetadataKind::U128);
    check_trivial_type::<FixedCapacityElements<u128, 0>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::U128);
    check_trivial_type::<FixedCapacityElements<u128, 3>>(name, &expected, &expected);
    // The largest capacity, the size is `u32::MAX - 15`
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32((1 << 28) - 2)
        .kind(IoTypeMetadataKind::U128);
    check_trivial_type::<FixedCapacityElements<u128, { (1 << 28) - 2 }>>(
        name, &expected, &expected,
    );
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::Array)
        .u32(2)
        .kind(IoTypeMetadataKind::U64);
    check_trivial_type::<FixedCapacityElements<[u64; 2], 3>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U64);
    check_trivial_type::<FixedCapacityElements<Unaligned<u64>, 3>>(name, &expected, &expected);

    let (point, point_compact) = point();
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(5)
        .nested(&point);
    let expected_compact = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(5)
        .nested(&point_compact);
    check_trivial_type::<FixedCapacityElements<Point, 5>>(name, &expected, &expected_compact);

    let wide = Expected::new(IoTypeMetadataKind::Struct)
        .name("Wide")
        .byte(3)
        .name("a")
        .kind(IoTypeMetadataKind::U64)
        .name("b")
        .kind(IoTypeMetadataKind::U32)
        .name("c")
        .kind(IoTypeMetadataKind::U32);
    let wide_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(3)
        .name("")
        .kind(IoTypeMetadataKind::U64)
        .name("")
        .kind(IoTypeMetadataKind::U32)
        .name("")
        .kind(IoTypeMetadataKind::U32);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .nested(&wide);
    let expected_compact = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .nested(&wide_compact);
    check_trivial_type::<FixedCapacityElements<Wide, 3>>(name, &expected, &expected_compact);

    // Zero-sized elements only take space for the length
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::Unit);
    check_trivial_type::<FixedCapacityElements<(), 3>>(name, &expected, &expected);

    // Nested in other types
    let expected = Expected::new(IoTypeMetadataKind::Array)
        .u32(2)
        .kind(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::U64);
    check_trivial_type::<[FixedCapacityElements<u64, 3>; 2]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityElements)
        .u32(2)
        .kind(IoTypeMetadataKind::FixedCapacityElements)
        .u32(3)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<FixedCapacityElements<FixedCapacityBytes<3>, 2>>(
        name, &expected, &expected,
    );
}

#[test]
fn fixed_capacity_string() {
    let name = "FixedCapacityString";

    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityString).u32(0);
    check_trivial_type::<FixedCapacityString<0>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityString).u32(10);
    check_trivial_type::<FixedCapacityString<10>>(name, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityString).u32(70_000);
    check_trivial_type::<FixedCapacityString<70_000>>(name, &expected, &expected);
    // The largest size
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityString).u32(u32::MAX - 4);
    check_trivial_type::<FixedCapacityString<{ u32::MAX as usize - 4 }>>(
        name, &expected, &expected,
    );
}

#[test]
fn variable_elements() {
    let name = "VariableElements";

    let expected = Expected::new(IoTypeMetadataKind::VariableElements)
        .u32(0)
        .kind(IoTypeMetadataKind::U8);
    check_io_type::<VariableElements<u8>>(name, 0, 1, &expected, &expected);

    // Alignment is that of elements
    let expected = Expected::new(IoTypeMetadataKind::VariableElements)
        .u32(0)
        .kind(IoTypeMetadataKind::U128);
    check_io_type::<VariableElements<u128>>(name, 0, 16, &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::VariableElements)
        .u32(1)
        .kind(IoTypeMetadataKind::U128);
    check_io_type::<VariableElements<u128, 1>>(name, 16, 16, &expected, &expected);

    let (point, point_compact) = point();
    let expected = Expected::new(IoTypeMetadataKind::VariableElements)
        .u32(10)
        .nested(&point);
    let expected_compact = Expected::new(IoTypeMetadataKind::VariableElements)
        .u32(10)
        .nested(&point_compact);
    check_io_type::<VariableElements<Point, 10>>(name, 20, 1, &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::VariableElements)
        .u32(300)
        .kind(IoTypeMetadataKind::U32);
    check_io_type::<VariableElements<u32, 300>>(name, 1200, 4, &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::VariableElements)
        .u32(70_000)
        .kind(IoTypeMetadataKind::U16);
    check_io_type::<VariableElements<u16, 70_000>>(name, 140_000, 2, &expected, &expected);
}

#[test]
fn maybe_data() {
    // `MaybeData` has the same metadata as the type inside
    let (point, point_compact) = point();
    check_io_type::<MaybeData<Point>>("Point", 2, 1, &point, &point_compact);

    let expected = Expected::new(IoTypeMetadataKind::U64);
    check_io_type::<MaybeData<u64>>("u64", 8, 8, &expected, &expected);
}

#[test]
fn empty_structs() {
    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    #[expect(clippy::empty_structs_with_brackets, reason = "Tested")]
    struct Empty {}

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    #[expect(clippy::empty_structs_with_brackets, reason = "Tested")]
    struct EmptyTuple();

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct EmptyUnit;

    let expected_compact = Expected::new(IoTypeMetadataKind::Struct).name("").byte(0);

    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("Empty")
        .byte(0);
    check_trivial_type::<Empty>("Empty", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("EmptyTuple")
        .byte(0);
    check_trivial_type::<EmptyTuple>("EmptyTuple", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("EmptyUnit")
        .byte(0);
    check_trivial_type::<EmptyUnit>("EmptyUnit", &expected, &expected_compact);
}

#[test]
fn structs_with_named_fields() {
    named_struct!(Named1, [a]);
    named_struct!(Named2, [a, b]);
    named_struct!(Named3, [a, b, c]);
    named_struct!(Named4, [a, b, c, d]);
    named_struct!(Named5, [a, b, c, d, e]);
    named_struct!(Named6, [a, b, c, d, e, f]);
    named_struct!(Named7, [a, b, c, d, e, f, g]);
    named_struct!(Named8, [a, b, c, d, e, f, g, h]);
    named_struct!(Named9, [a, b, c, d, e, f, g, h, i]);
    named_struct!(Named10, [a, b, c, d, e, f, g, h, i, j]);
    named_struct!(Named11, [a, b, c, d, e, f, g, h, i, j, k]);
    named_struct!(Named12, [a, b, c, d, e, f, g, h, i, j, k, l]);

    check_named_struct::<Named1>("Named1", 1);
    check_named_struct::<Named2>("Named2", 2);
    check_named_struct::<Named3>("Named3", 3);
    check_named_struct::<Named4>("Named4", 4);
    check_named_struct::<Named5>("Named5", 5);
    check_named_struct::<Named6>("Named6", 6);
    check_named_struct::<Named7>("Named7", 7);
    check_named_struct::<Named8>("Named8", 8);
    check_named_struct::<Named9>("Named9", 9);
    check_named_struct::<Named10>("Named10", 10);
    check_named_struct::<Named11>("Named11", 11);
    check_named_struct::<Named12>("Named12", 12);
}

#[test]
fn tuple_structs() {
    tuple_struct!(Tuple1, [a]);
    tuple_struct!(Tuple2, [a, b]);
    tuple_struct!(Tuple3, [a, b, c]);
    tuple_struct!(Tuple4, [a, b, c, d]);
    tuple_struct!(Tuple5, [a, b, c, d, e]);
    tuple_struct!(Tuple6, [a, b, c, d, e, f]);
    tuple_struct!(Tuple7, [a, b, c, d, e, f, g]);
    tuple_struct!(Tuple8, [a, b, c, d, e, f, g, h]);
    tuple_struct!(Tuple9, [a, b, c, d, e, f, g, h, i]);
    tuple_struct!(Tuple10, [a, b, c, d, e, f, g, h, i, j]);
    tuple_struct!(Tuple11, [a, b, c, d, e, f, g, h, i, j, k]);
    tuple_struct!(Tuple12, [a, b, c, d, e, f, g, h, i, j, k, l]);

    check_tuple_struct::<Tuple1>("Tuple1", 1);
    check_tuple_struct::<Tuple2>("Tuple2", 2);
    check_tuple_struct::<Tuple3>("Tuple3", 3);
    check_tuple_struct::<Tuple4>("Tuple4", 4);
    check_tuple_struct::<Tuple5>("Tuple5", 5);
    check_tuple_struct::<Tuple6>("Tuple6", 6);
    check_tuple_struct::<Tuple7>("Tuple7", 7);
    check_tuple_struct::<Tuple8>("Tuple8", 8);
    check_tuple_struct::<Tuple9>("Tuple9", 9);
    check_tuple_struct::<Tuple10>("Tuple10", 10);
    check_tuple_struct::<Tuple11>("Tuple11", 11);
    check_tuple_struct::<Tuple12>("Tuple12", 12);
}

#[test]
fn named_and_tuple_forms_compact_equally() {
    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    #[expect(clippy::empty_structs_with_brackets, reason = "Tested")]
    struct EmptyNamed {}

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    #[expect(clippy::empty_structs_with_brackets, reason = "Tested")]
    struct EmptyTuple();

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct EmptyUnit;

    named_struct!(Named1, [a]);
    named_struct!(Named2, [a, b]);
    named_struct!(Named3, [a, b, c]);
    named_struct!(Named4, [a, b, c, d]);
    named_struct!(Named5, [a, b, c, d, e]);
    named_struct!(Named6, [a, b, c, d, e, f]);
    named_struct!(Named7, [a, b, c, d, e, f, g]);
    named_struct!(Named8, [a, b, c, d, e, f, g, h]);
    named_struct!(Named9, [a, b, c, d, e, f, g, h, i]);
    named_struct!(Named10, [a, b, c, d, e, f, g, h, i, j]);
    named_struct!(Named11, [a, b, c, d, e, f, g, h, i, j, k]);
    named_struct!(Named12, [a, b, c, d, e, f, g, h, i, j, k, l]);
    tuple_struct!(Tuple1, [a]);
    tuple_struct!(Tuple2, [a, b]);
    tuple_struct!(Tuple3, [a, b, c]);
    tuple_struct!(Tuple4, [a, b, c, d]);
    tuple_struct!(Tuple5, [a, b, c, d, e]);
    tuple_struct!(Tuple6, [a, b, c, d, e, f]);
    tuple_struct!(Tuple7, [a, b, c, d, e, f, g]);
    tuple_struct!(Tuple8, [a, b, c, d, e, f, g, h]);
    tuple_struct!(Tuple9, [a, b, c, d, e, f, g, h, i]);
    tuple_struct!(Tuple10, [a, b, c, d, e, f, g, h, i, j]);
    tuple_struct!(Tuple11, [a, b, c, d, e, f, g, h, i, j, k]);
    tuple_struct!(Tuple12, [a, b, c, d, e, f, g, h, i, j, k, l]);

    // Enums only support variants with named fields or without fields, the latter can be written
    // with braces, parentheses or neither
    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    enum EnumOfNamed {
        A { pair: Named2 },
        B { pair: Tuple2 },
        C { singles: [Named1; 2] },
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    enum EnumOfTuple {
        X { first: Tuple2 },
        Y { second: Named2 },
        Z { third: [Tuple1; 2] },
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    enum UnitVariants {
        A,
        B,
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    #[expect(clippy::empty_enum_variants_with_brackets, reason = "Tested")]
    enum BracedVariants {
        A {},
        B {},
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    #[expect(clippy::empty_enum_variants_with_brackets, reason = "Tested")]
    enum ParenthesizedVariants {
        A(),
        B(),
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    enum MixedUnitVariant {
        A { value: () },
        B,
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    #[expect(clippy::empty_enum_variants_with_brackets, reason = "Tested")]
    enum MixedBracedVariant {
        A { value: () },
        B {},
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    #[expect(clippy::empty_enum_variants_with_brackets, reason = "Tested")]
    enum MixedParenthesizedVariant {
        A { value: () },
        B(),
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct OuterNamed {
        empty: EmptyNamed,
        pair: Named2,
        many: [Named11; 2],
        choice: EnumOfNamed,
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct OuterTuple(EmptyUnit, Tuple2, [Tuple11; 2], EnumOfTuple);

    // Struct, enum, variant and field names are replaced with zero-length names, so only the number
    // of fields and their types remain
    check_same_compact::<EmptyNamed, EmptyTuple>();
    check_same_compact::<EmptyNamed, EmptyUnit>();
    check_same_compact::<Named1, Tuple1>();
    check_same_compact::<Named2, Tuple2>();
    check_same_compact::<Named3, Tuple3>();
    check_same_compact::<Named4, Tuple4>();
    check_same_compact::<Named5, Tuple5>();
    check_same_compact::<Named6, Tuple6>();
    check_same_compact::<Named7, Tuple7>();
    check_same_compact::<Named8, Tuple8>();
    check_same_compact::<Named9, Tuple9>();
    check_same_compact::<Named10, Tuple10>();
    check_same_compact::<Named11, Tuple11>();
    check_same_compact::<Named12, Tuple12>();
    assert_ne!(compact(Named2::METADATA), compact(Tuple3::METADATA));

    // Variants with braces or parentheses and no fields are the same as variants without fields
    check_same_compact::<UnitVariants, BracedVariants>();
    check_same_compact::<UnitVariants, ParenthesizedVariants>();
    check_same_compact::<MixedUnitVariant, MixedBracedVariant>();
    check_same_compact::<MixedUnitVariant, MixedParenthesizedVariant>();

    // Nested types
    check_same_compact::<[Named2; 3], [Tuple2; 3]>();
    check_same_compact::<[Named11; 300], [Tuple11; 300]>();
    check_same_compact::<Unaligned<Named2>, Unaligned<Tuple2>>();
    check_same_compact::<MaybeData<Named2>, MaybeData<Tuple2>>();
    check_same_compact::<VariableElements<Named2>, VariableElements<Tuple2>>();
    check_same_compact::<VariableElements<Named11, 300>, VariableElements<Tuple11, 300>>();
    check_same_compact::<FixedCapacityElements<Named2, 3>, FixedCapacityElements<Tuple2, 3>>();
    check_same_compact::<EnumOfNamed, EnumOfTuple>();
    check_same_compact::<OuterNamed, OuterTuple>();
}

#[test]
fn structs_with_different_field_types() {
    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct Primitives {
        a: u128,
        b: i128,
        c: u64,
        d: i64,
        e: u32,
        f: i32,
        g: u16,
        h: i16,
        i: u8,
        j: i8,
        k: Bool,
        l: (),
        padding: [u8; 1],
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct Mixed(u64, Unaligned<u32>, [u8; 4]);

    #[derive(Copy, Clone, TrivialType)]
    #[repr(transparent)]
    struct Transparent(u32);

    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("Primitives")
        .byte(13)
        .name("a")
        .kind(IoTypeMetadataKind::U128)
        .name("b")
        .kind(IoTypeMetadataKind::I128)
        .name("c")
        .kind(IoTypeMetadataKind::U64)
        .name("d")
        .kind(IoTypeMetadataKind::I64)
        .name("e")
        .kind(IoTypeMetadataKind::U32)
        .name("f")
        .kind(IoTypeMetadataKind::I32)
        .name("g")
        .kind(IoTypeMetadataKind::U16)
        .name("h")
        .kind(IoTypeMetadataKind::I16)
        .name("i")
        .kind(IoTypeMetadataKind::U8)
        .name("j")
        .kind(IoTypeMetadataKind::I8)
        .name("k")
        .kind(IoTypeMetadataKind::Bool)
        .name("l")
        .kind(IoTypeMetadataKind::Unit)
        .name("padding")
        .kind(IoTypeMetadataKind::Array)
        .u32(1)
        .kind(IoTypeMetadataKind::U8);
    let expected_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(13)
        .name("")
        .kind(IoTypeMetadataKind::U128)
        .name("")
        .kind(IoTypeMetadataKind::I128)
        .name("")
        .kind(IoTypeMetadataKind::U64)
        .name("")
        .kind(IoTypeMetadataKind::I64)
        .name("")
        .kind(IoTypeMetadataKind::U32)
        .name("")
        .kind(IoTypeMetadataKind::I32)
        .name("")
        .kind(IoTypeMetadataKind::U16)
        .name("")
        .kind(IoTypeMetadataKind::I16)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::I8)
        .name("")
        .kind(IoTypeMetadataKind::Bool)
        .name("")
        .kind(IoTypeMetadataKind::Unit)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(1)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<Primitives>("Primitives", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("Mixed")
        .byte(3)
        .name("")
        .kind(IoTypeMetadataKind::U64)
        .name("")
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U32)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(4)
        .kind(IoTypeMetadataKind::U8);
    let expected_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(3)
        .name("")
        .kind(IoTypeMetadataKind::U64)
        .name("")
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U32)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(4)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<Mixed>("Mixed", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("Transparent")
        .byte(1)
        .name("")
        .kind(IoTypeMetadataKind::U32);
    let expected_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(1)
        .name("")
        .kind(IoTypeMetadataKind::U32);
    check_trivial_type::<Transparent>("Transparent", &expected, &expected_compact);
}

#[test]
fn fieldless_enums() {
    fieldless_enum!(NoFields1, [A]);
    fieldless_enum!(NoFields2, [A, B]);
    fieldless_enum!(NoFields3, [A, B, C]);
    fieldless_enum!(NoFields4, [A, B, C, D]);
    fieldless_enum!(NoFields5, [A, B, C, D, E]);
    fieldless_enum!(NoFields6, [A, B, C, D, E, F]);
    fieldless_enum!(NoFields7, [A, B, C, D, E, F, G]);
    fieldless_enum!(NoFields8, [A, B, C, D, E, F, G, H]);
    fieldless_enum!(NoFields9, [A, B, C, D, E, F, G, H, I]);
    fieldless_enum!(NoFields10, [A, B, C, D, E, F, G, H, I, J]);
    fieldless_enum!(NoFields11, [A, B, C, D, E, F, G, H, I, J, K]);
    fieldless_enum!(NoFields12, [A, B, C, D, E, F, G, H, I, J, K, L]);
    fieldless_enum!(NoFields16, [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P]);
    fieldless_enum!(
        NoFields17,
        [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q]
    );

    check_fieldless_enum::<NoFields1>("NoFields1", 1);
    check_fieldless_enum::<NoFields2>("NoFields2", 2);
    check_fieldless_enum::<NoFields3>("NoFields3", 3);
    check_fieldless_enum::<NoFields4>("NoFields4", 4);
    check_fieldless_enum::<NoFields5>("NoFields5", 5);
    check_fieldless_enum::<NoFields6>("NoFields6", 6);
    check_fieldless_enum::<NoFields7>("NoFields7", 7);
    check_fieldless_enum::<NoFields8>("NoFields8", 8);
    check_fieldless_enum::<NoFields9>("NoFields9", 9);
    check_fieldless_enum::<NoFields10>("NoFields10", 10);
    check_fieldless_enum::<NoFields11>("NoFields11", 11);
    check_fieldless_enum::<NoFields12>("NoFields12", 12);
    check_fieldless_enum::<NoFields16>("NoFields16", 16);
    check_fieldless_enum::<NoFields17>("NoFields17", 17);
}

#[test]
fn enums_with_fields() {
    data_enum!(Data1, [A]);
    data_enum!(Data2, [A, B]);
    data_enum!(Data3, [A, B, C]);
    data_enum!(Data4, [A, B, C, D]);
    data_enum!(Data5, [A, B, C, D, E]);
    data_enum!(Data6, [A, B, C, D, E, F]);
    data_enum!(Data7, [A, B, C, D, E, F, G]);
    data_enum!(Data8, [A, B, C, D, E, F, G, H]);
    data_enum!(Data9, [A, B, C, D, E, F, G, H, I]);
    data_enum!(Data10, [A, B, C, D, E, F, G, H, I, J]);
    data_enum!(Data11, [A, B, C, D, E, F, G, H, I, J, K]);
    data_enum!(Data12, [A, B, C, D, E, F, G, H, I, J, K, L]);
    data_enum!(Data16, [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P]);
    data_enum!(Data17, [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q]);

    check_data_enum::<Data1>("Data1", 1);
    check_data_enum::<Data2>("Data2", 2);
    check_data_enum::<Data3>("Data3", 3);
    check_data_enum::<Data4>("Data4", 4);
    check_data_enum::<Data5>("Data5", 5);
    check_data_enum::<Data6>("Data6", 6);
    check_data_enum::<Data7>("Data7", 7);
    check_data_enum::<Data8>("Data8", 8);
    check_data_enum::<Data9>("Data9", 9);
    check_data_enum::<Data10>("Data10", 10);
    check_data_enum::<Data11>("Data11", 11);
    check_data_enum::<Data12>("Data12", 12);
    check_data_enum::<Data16>("Data16", 16);
    check_data_enum::<Data17>("Data17", 17);
}

#[test]
fn enums_with_different_variants() {
    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    enum Shape {
        Dot { point: Point },
        Line { from: u8, to: u8 },
        Wide { bytes: [u8; 2] },
    }

    // Variants without fields are only possible next to variants with zero-sized fields, since
    // all variants must have the same size
    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    enum MixedDataFirst {
        Full { value: () },
        Empty,
    }

    #[derive(Copy, Clone, TrivialType)]
    #[repr(u8)]
    enum MixedFieldlessFirst {
        Empty,
        Full { value: () },
        AlsoEmpty,
        AlsoFull { first: [u8; 0], second: () },
    }

    let (point, point_compact) = point();
    let expected = Expected::new(IoTypeMetadataKind::Enum)
        .name("Shape")
        .byte(3)
        .name("Dot")
        .byte(1)
        .name("point")
        .nested(&point)
        .name("Line")
        .byte(2)
        .name("from")
        .kind(IoTypeMetadataKind::U8)
        .name("to")
        .kind(IoTypeMetadataKind::U8)
        .name("Wide")
        .byte(1)
        .name("bytes")
        .kind(IoTypeMetadataKind::Array)
        .u32(2)
        .kind(IoTypeMetadataKind::U8);
    let expected_compact = Expected::new(IoTypeMetadataKind::Enum)
        .name("")
        .byte(3)
        .name("")
        .byte(1)
        .name("")
        .nested(&point_compact)
        .name("")
        .byte(2)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .byte(1)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(2)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<Shape>("Shape", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::Enum)
        .name("MixedDataFirst")
        .byte(2)
        .name("Full")
        .byte(1)
        .name("value")
        .kind(IoTypeMetadataKind::Unit)
        .name("Empty")
        .byte(0);
    let expected_compact = Expected::new(IoTypeMetadataKind::Enum)
        .name("")
        .byte(2)
        .name("")
        .byte(1)
        .name("")
        .kind(IoTypeMetadataKind::Unit)
        .name("")
        .byte(0);
    check_trivial_type::<MixedDataFirst>("MixedDataFirst", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::Enum)
        .name("MixedFieldlessFirst")
        .byte(4)
        .name("Empty")
        .byte(0)
        .name("Full")
        .byte(1)
        .name("value")
        .kind(IoTypeMetadataKind::Unit)
        .name("AlsoEmpty")
        .byte(0)
        .name("AlsoFull")
        .byte(2)
        .name("first")
        .kind(IoTypeMetadataKind::Array)
        .u32(0)
        .kind(IoTypeMetadataKind::U8)
        .name("second")
        .kind(IoTypeMetadataKind::Unit);
    let expected_compact = Expected::new(IoTypeMetadataKind::Enum)
        .name("")
        .byte(4)
        .name("")
        .byte(0)
        .name("")
        .byte(1)
        .name("")
        .kind(IoTypeMetadataKind::Unit)
        .name("")
        .byte(0)
        .name("")
        .byte(2)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(0)
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::Unit);
    check_trivial_type::<MixedFieldlessFirst>("MixedFieldlessFirst", &expected, &expected_compact);
}

#[test]
fn enums_without_variants_are_rejected() {
    // Enums without variants have no values and can't derive `TrivialType`, since `#[repr(u8)]`
    // requires variants, so their metadata is rejected
    let empty = Expected::new(IoTypeMetadataKind::Enum)
        .name("Empty")
        .byte(0);
    check_rejected(&empty.0);
    check_rejected(&Expected::new(IoTypeMetadataKind::Enum).name("").byte(0).0);
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Array)
            .u32(1)
            .nested(&empty)
            .0,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Struct)
            .name("Outer")
            .byte(1)
            .name("")
            .nested(&empty)
            .0,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Enum)
            .name("Outer")
            .byte(1)
            .name("A")
            .byte(1)
            .name("inner")
            .nested(&empty)
            .0,
    );
}

#[test]
fn fields_are_either_named_or_unnamed() {
    // Fields of tuple structs have zero-length names, structs that also have named fields are
    // rejected
    let named_first = Expected::new(IoTypeMetadataKind::Struct)
        .name("Mixed")
        .byte(2)
        .name("a")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::U8);
    let unnamed_first = Expected::new(IoTypeMetadataKind::Struct)
        .name("Mixed")
        .byte(2)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .name("b")
        .kind(IoTypeMetadataKind::U8);
    check_rejected(&named_first.0);
    check_rejected(&unnamed_first.0);
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Array)
            .u32(1)
            .nested(&named_first)
            .0,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Struct)
            .name("Outer")
            .byte(1)
            .name("inner")
            .nested(&unnamed_first)
            .0,
    );

    // Enum variants are encoded like structs, so each variant either has named or unnamed fields
    let tuple_variant = Expected::new(IoTypeMetadataKind::Enum)
        .name("Enum")
        .byte(2)
        .name("Named")
        .byte(1)
        .name("value")
        .kind(IoTypeMetadataKind::Array)
        .u32(2)
        .kind(IoTypeMetadataKind::U8)
        .name("Tuple")
        .byte(2)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::U8);
    let tuple_variant_compact = Expected::new(IoTypeMetadataKind::Enum)
        .name("")
        .byte(2)
        .name("")
        .byte(1)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(2)
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .byte(2)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::U8);
    check_decoding(&tuple_variant.0, "Enum", 3, 1, &tuple_variant_compact);

    let mixed_variant = Expected::new(IoTypeMetadataKind::Enum)
        .name("Enum")
        .byte(1)
        .name("Mixed")
        .byte(2)
        .name("a")
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .kind(IoTypeMetadataKind::U8);
    check_rejected(&mixed_variant.0);
}

#[test]
fn nested_types() {
    fieldless_enum!(Direction, [A, B]);
    data_enum!(Value, [A, B]);

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct Inner(Point, [u8; 8]);

    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct Outer {
        wide: Unaligned<u64>,
        points: [Point; 3],
        direction: Direction,
        value: Value,
        inner: Inner,
    }

    let (point, point_compact) = point();
    let (direction, direction_compact) = u8_enum("Direction", 2, false);
    let (value, value_compact) = u8_enum("Value", 2, true);
    let inner = Expected::new(IoTypeMetadataKind::Struct)
        .name("Inner")
        .byte(2)
        .name("")
        .nested(&point)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(8)
        .kind(IoTypeMetadataKind::U8);
    let inner_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(2)
        .name("")
        .nested(&point_compact)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(8)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<Inner>("Inner", &inner, &inner_compact);

    let expected = Expected::new(IoTypeMetadataKind::Struct)
        .name("Outer")
        .byte(5)
        .name("wide")
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U64)
        .name("points")
        .kind(IoTypeMetadataKind::Array)
        .u32(3)
        .nested(&point)
        .name("direction")
        .nested(&direction)
        .name("value")
        .nested(&value)
        .name("inner")
        .nested(&inner);
    let expected_compact = Expected::new(IoTypeMetadataKind::Struct)
        .name("")
        .byte(5)
        .name("")
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U64)
        .name("")
        .kind(IoTypeMetadataKind::Array)
        .u32(3)
        .nested(&point_compact)
        .name("")
        .nested(&direction_compact)
        .name("")
        .nested(&value_compact)
        .name("")
        .nested(&inner_compact);
    check_trivial_type::<Outer>("Outer", &expected, &expected_compact);
}

/// Metadata of `[u8; count]`
fn u8_array(count: u32) -> Expected {
    Expected::new(IoTypeMetadataKind::Array)
        .u32(count)
        .kind(IoTypeMetadataKind::U8)
}

/// Metadata of a tuple struct with a field of the given type followed by a `u8` field
fn pair_with_byte(first: &Expected) -> Expected {
    Expected::new(IoTypeMetadataKind::Struct)
        .name("Pair")
        .byte(2)
        .name("")
        .nested(first)
        .name("")
        .kind(IoTypeMetadataKind::U8)
}

/// Metadata of an enum with a single variant with a single field of the given type
fn enum_with(field: &Expected) -> Expected {
    Expected::new(IoTypeMetadataKind::Enum)
        .name("Enum")
        .byte(1)
        .name("A")
        .byte(1)
        .name("field")
        .nested(field)
}

#[test]
fn capacity_overflow_is_rejected() {
    // Crafted metadata with a capacity that doesn't fit into `u32` must be rejected instead of
    // overflowing

    // Number of elements times capacity of an element
    check_layout(&u8_array(u32::MAX).0, u32::MAX, 1);
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Array)
            .u32(1 << 31)
            .kind(IoTypeMetadataKind::U16)
            .0,
    );
    check_layout(
        &Expected::new(IoTypeMetadataKind::VariableElements)
            .u32(u32::MAX)
            .kind(IoTypeMetadataKind::U8)
            .0,
        u32::MAX,
        1,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::VariableElements)
            .u32(1 << 31)
            .kind(IoTypeMetadataKind::U16)
            .0,
    );

    // Capacity of elements plus the length
    check_layout(
        &Expected::new(IoTypeMetadataKind::FixedCapacityElements)
            .u32(u32::MAX - 4)
            .kind(IoTypeMetadataKind::U8)
            .0,
        u32::MAX,
        1,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::FixedCapacityElements)
            .u32(u32::MAX - 3)
            .kind(IoTypeMetadataKind::U8)
            .0,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::FixedCapacityElements)
            .u32(u32::MAX)
            .kind(IoTypeMetadataKind::U8)
            .0,
    );
    // The length takes 16 bytes with `u128` elements, so `(1 << 28) - 2` elements is the largest
    // capacity, see `fixed_capacity_elements()`
    check_rejected(
        &Expected::new(IoTypeMetadataKind::FixedCapacityElements)
            .u32((1 << 28) - 1)
            .kind(IoTypeMetadataKind::U128)
            .0,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::FixedCapacityElements)
            .u32(1 << 31)
            .kind(IoTypeMetadataKind::U16)
            .0,
    );
    check_layout(
        &Expected::new(IoTypeMetadataKind::FixedCapacityString)
            .u32(u32::MAX - 4)
            .0,
        u32::MAX,
        1,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::FixedCapacityString)
            .u32(u32::MAX - 3)
            .0,
    );

    // Sum of capacities of fields
    check_layout(&pair_with_byte(&u8_array(u32::MAX - 1)).0, u32::MAX, 1);
    check_rejected(&pair_with_byte(&u8_array(u32::MAX)).0);

    // Capacity of a variant plus the discriminant
    check_layout(&enum_with(&u8_array(u32::MAX - 1)).0, u32::MAX, 1);
    check_rejected(&enum_with(&u8_array(u32::MAX)).0);

    // Overflow in nested types
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Unaligned)
            .nested(&enum_with(&u8_array(u32::MAX)))
            .0,
    );
    check_rejected(
        &Expected::new(IoTypeMetadataKind::Array)
            .u32(2)
            .nested(&u8_array(u32::MAX))
            .0,
    );
}
