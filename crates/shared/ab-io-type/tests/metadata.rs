//! Tests of `METADATA` of built-in types and of types with derived `TrivialType`.
//!
//! Every case checks exact metadata bytes, and that the metadata decodes back into the name, size
//! and alignment of the type, compacts into expected bytes, and that no truncated metadata decodes.

#![expect(dead_code, reason = "Types are only defined for their metadata")]

use ab_io_type::bool::Bool;
use ab_io_type::fixed_capacity_bytes::{FixedCapacityBytesU8, FixedCapacityBytesU16};
use ab_io_type::fixed_capacity_string::{FixedCapacityStringU8, FixedCapacityStringU16};
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

    /// Append a little-endian `u16`
    fn u16(mut self, value: u16) -> Self {
        self.0.extend_from_slice(&value.to_le_bytes());
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
/// expected bytes, and that no truncated metadata decodes
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

    assert_eq!(compact(metadata), expected_compact.0);

    let mut compact_buffer = [0; MAX_METADATA_CAPACITY];
    for length in 0..metadata.len() {
        let truncated = &metadata[..length];

        assert!(IoTypeMetadataKind::type_details(truncated).is_none());
        assert!(IoTypeMetadataKind::compact(truncated, &mut compact_buffer).is_none());
    }
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
    let expected = Expected::new(IoTypeMetadataKind::Struct2)
        .name("Point")
        .name("x")
        .kind(IoTypeMetadataKind::U8)
        .name("y")
        .kind(IoTypeMetadataKind::U8);
    let expected_compact = Expected::new(IoTypeMetadataKind::TupleStruct2)
        .name("")
        .kind(IoTypeMetadataKind::U8)
        .kind(IoTypeMetadataKind::U8);

    (expected, expected_compact)
}

/// Expected metadata and compact metadata of a struct declared with [`named_struct!`] or
/// [`tuple_struct!`] using the first `field_count` of [`FIELD_NAMES`]
fn u8_struct(
    kind: IoTypeMetadataKind,
    compact_kind: IoTypeMetadataKind,
    name: &str,
    field_count: u8,
    named: bool,
) -> (Expected, Expected) {
    let mut expected = Expected::new(kind).name(name);
    let mut expected_compact = Expected::new(compact_kind).name("");
    if matches!(
        kind,
        IoTypeMetadataKind::Struct | IoTypeMetadataKind::TupleStruct
    ) {
        expected = expected.byte(field_count);
        expected_compact = expected_compact.byte(field_count);
    }

    for field_name in &FIELD_NAMES[..usize::from(field_count)] {
        if named {
            expected = expected.name(field_name);
        }
        expected = expected.kind(IoTypeMetadataKind::U8);
        expected_compact = expected_compact.kind(IoTypeMetadataKind::U8);
    }

    (expected, expected_compact)
}

/// Expected metadata and compact metadata of an enum declared with [`fieldless_enum!`] or
/// [`data_enum!`] using the first `variant_count` of [`VARIANT_NAMES`]
fn u8_enum(
    kind: IoTypeMetadataKind,
    name: &str,
    variant_count: u8,
    with_fields: bool,
) -> (Expected, Expected) {
    let mut expected = Expected::new(kind).name(name);
    let mut expected_compact = Expected::new(kind).name("");
    if matches!(
        kind,
        IoTypeMetadataKind::Enum | IoTypeMetadataKind::EnumNoFields
    ) {
        expected = expected.byte(variant_count);
        expected_compact = expected_compact.byte(variant_count);
    }

    for variant_name in &VARIANT_NAMES[..usize::from(variant_count)] {
        expected = expected.name(variant_name);
        expected_compact = expected_compact.name("");
        if with_fields {
            expected = expected.byte(1).name("value").kind(IoTypeMetadataKind::U8);
            expected_compact = expected_compact.byte(1).kind(IoTypeMetadataKind::U8);
        }
    }

    (expected, expected_compact)
}

/// Check `METADATA` of a struct declared with [`named_struct!`], see [`u8_struct()`]
#[track_caller]
fn check_named_struct<T>(
    name: &str,
    field_count: u8,
    kind: IoTypeMetadataKind,
    compact_kind: IoTypeMetadataKind,
) where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_struct(kind, compact_kind, name, field_count, true);
    check_trivial_type::<T>(name, &expected, &expected_compact);
}

/// Check `METADATA` of a struct declared with [`tuple_struct!`], see [`u8_struct()`]
#[track_caller]
fn check_tuple_struct<T>(name: &str, field_count: u8, kind: IoTypeMetadataKind)
where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_struct(kind, kind, name, field_count, false);
    check_trivial_type::<T>(name, &expected, &expected_compact);
}

/// Check `METADATA` of an enum declared with [`fieldless_enum!`], see [`u8_enum()`]
#[track_caller]
fn check_fieldless_enum<T>(name: &str, variant_count: u8, kind: IoTypeMetadataKind)
where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_enum(kind, name, variant_count, false);
    check_trivial_type::<T>(name, &expected, &expected_compact);
}

/// Check `METADATA` of an enum declared with [`data_enum!`], see [`u8_enum()`]
#[track_caller]
fn check_data_enum<T>(name: &str, variant_count: u8, kind: IoTypeMetadataKind)
where
    T: TrivialType,
{
    let (expected, expected_compact) = u8_enum(kind, name, variant_count, true);
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
    // Sizes with a dedicated metadata kind
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x8);
    check_trivial_type::<[u8; 8]>("[u8; 8]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x16);
    check_trivial_type::<[u8; 16]>("[u8; 16]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x32);
    check_trivial_type::<[u8; 32]>("[u8; 32]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x64);
    check_trivial_type::<[u8; 64]>("[u8; 64]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x128);
    check_trivial_type::<[u8; 128]>("[u8; 128]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x256);
    check_trivial_type::<[u8; 256]>("[u8; 256]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x512);
    check_trivial_type::<[u8; 512]>("[u8; 512]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x1024);
    check_trivial_type::<[u8; 1024]>("[u8; 1024]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::ArrayU8x4096);
    check_trivial_type::<[u8; 4096]>("[u8; 4096]", &expected, &expected);

    // Other sizes, with the number of elements in 1, 2 or 4 bytes
    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(0)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 0]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(1)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 1]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(7)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 7]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(255)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 255]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array16b)
        .u16(257)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 257]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array16b)
        .u16(65_535)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 65_535]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array32b)
        .u32(65_536)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 65_536]>("[T; N]", &expected, &expected);
    let expected = Expected::new(IoTypeMetadataKind::Array32b)
        .u32(70_000)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<[u8; 70_000]>("[T; N]", &expected, &expected);
}

#[test]
fn arrays_of_other_types() {
    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(8)
        .kind(IoTypeMetadataKind::U16);
    check_trivial_type::<[u16; 8]>("[T; N]", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Array16b)
        .u16(300)
        .kind(IoTypeMetadataKind::U64);
    check_trivial_type::<[u64; 300]>("[T; N]", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(2)
        .kind(IoTypeMetadataKind::ArrayU8x8);
    check_trivial_type::<[[u8; 8]; 2]>("[T; N]", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(3)
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U32);
    check_trivial_type::<[Unaligned<u32>; 3]>("[T; N]", &expected, &expected);

    let (point, point_compact) = point();
    let expected = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(4)
        .nested(&point);
    let expected_compact = Expected::new(IoTypeMetadataKind::Array8b)
        .byte(4)
        .nested(&point_compact);
    check_trivial_type::<[Point; 4]>("[T; N]", &expected, &expected_compact);
}

#[test]
fn unaligned() {
    let expected = Expected::new(IoTypeMetadataKind::Unaligned).kind(IoTypeMetadataKind::U64);
    check_trivial_type::<Unaligned<u64>>("Unaligned", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::Unaligned).kind(IoTypeMetadataKind::ArrayU8x8);
    check_trivial_type::<Unaligned<[u8; 8]>>("Unaligned", &expected, &expected);
}

#[test]
fn fixed_capacity_bytes_and_strings() {
    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityBytes8b).byte(10);
    check_trivial_type::<FixedCapacityBytesU8<10>>("FixedCapacityBytes", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityBytes16b).u16(300);
    check_trivial_type::<FixedCapacityBytesU16<300>>("FixedCapacityBytes", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityString8b).byte(10);
    check_trivial_type::<FixedCapacityStringU8<10>>("FixedCapacityString", &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::FixedCapacityString16b).u16(300);
    check_trivial_type::<FixedCapacityStringU16<300>>("FixedCapacityString", &expected, &expected);
}

#[test]
fn variable_elements() {
    let name = "VariableElements";

    let expected =
        Expected::new(IoTypeMetadataKind::VariableElements0).kind(IoTypeMetadataKind::U8);
    check_io_type::<VariableElements<u8>>(name, 0, 1, &expected, &expected);

    let (point, point_compact) = point();
    let expected = Expected::new(IoTypeMetadataKind::VariableElements8b)
        .byte(10)
        .nested(&point);
    let expected_compact = Expected::new(IoTypeMetadataKind::VariableElements8b)
        .byte(10)
        .nested(&point_compact);
    check_io_type::<VariableElements<Point, 10>>(name, 20, 1, &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::VariableElements16b)
        .u16(300)
        .kind(IoTypeMetadataKind::U32);
    check_io_type::<VariableElements<u32, 300>>(name, 1200, 4, &expected, &expected);

    let expected = Expected::new(IoTypeMetadataKind::VariableElements32b)
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

    let expected = Expected::new(IoTypeMetadataKind::TupleStruct)
        .name("Empty")
        .byte(0);
    let expected_compact = Expected::new(IoTypeMetadataKind::TupleStruct)
        .name("")
        .byte(0);
    check_trivial_type::<Empty>("Empty", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::TupleStruct)
        .name("EmptyTuple")
        .byte(0);
    let expected_compact = Expected::new(IoTypeMetadataKind::TupleStruct)
        .name("")
        .byte(0);
    check_trivial_type::<EmptyTuple>("EmptyTuple", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::TupleStruct)
        .name("EmptyUnit")
        .byte(0);
    let expected_compact = Expected::new(IoTypeMetadataKind::TupleStruct)
        .name("")
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

    check_named_struct::<Named1>(
        "Named1",
        1,
        IoTypeMetadataKind::Struct1,
        IoTypeMetadataKind::TupleStruct1,
    );
    check_named_struct::<Named2>(
        "Named2",
        2,
        IoTypeMetadataKind::Struct2,
        IoTypeMetadataKind::TupleStruct2,
    );
    check_named_struct::<Named3>(
        "Named3",
        3,
        IoTypeMetadataKind::Struct3,
        IoTypeMetadataKind::TupleStruct3,
    );
    check_named_struct::<Named4>(
        "Named4",
        4,
        IoTypeMetadataKind::Struct4,
        IoTypeMetadataKind::TupleStruct4,
    );
    check_named_struct::<Named5>(
        "Named5",
        5,
        IoTypeMetadataKind::Struct5,
        IoTypeMetadataKind::TupleStruct5,
    );
    check_named_struct::<Named6>(
        "Named6",
        6,
        IoTypeMetadataKind::Struct6,
        IoTypeMetadataKind::TupleStruct6,
    );
    check_named_struct::<Named7>(
        "Named7",
        7,
        IoTypeMetadataKind::Struct7,
        IoTypeMetadataKind::TupleStruct7,
    );
    check_named_struct::<Named8>(
        "Named8",
        8,
        IoTypeMetadataKind::Struct8,
        IoTypeMetadataKind::TupleStruct8,
    );
    check_named_struct::<Named9>(
        "Named9",
        9,
        IoTypeMetadataKind::Struct9,
        IoTypeMetadataKind::TupleStruct9,
    );
    check_named_struct::<Named10>(
        "Named10",
        10,
        IoTypeMetadataKind::Struct10,
        IoTypeMetadataKind::TupleStruct10,
    );
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

    check_tuple_struct::<Tuple1>("Tuple1", 1, IoTypeMetadataKind::TupleStruct1);
    check_tuple_struct::<Tuple2>("Tuple2", 2, IoTypeMetadataKind::TupleStruct2);
    check_tuple_struct::<Tuple3>("Tuple3", 3, IoTypeMetadataKind::TupleStruct3);
    check_tuple_struct::<Tuple4>("Tuple4", 4, IoTypeMetadataKind::TupleStruct4);
    check_tuple_struct::<Tuple5>("Tuple5", 5, IoTypeMetadataKind::TupleStruct5);
    check_tuple_struct::<Tuple6>("Tuple6", 6, IoTypeMetadataKind::TupleStruct6);
    check_tuple_struct::<Tuple7>("Tuple7", 7, IoTypeMetadataKind::TupleStruct7);
    check_tuple_struct::<Tuple8>("Tuple8", 8, IoTypeMetadataKind::TupleStruct8);
    check_tuple_struct::<Tuple9>("Tuple9", 9, IoTypeMetadataKind::TupleStruct9);
    check_tuple_struct::<Tuple10>("Tuple10", 10, IoTypeMetadataKind::TupleStruct10);
    // More than 10 fields need an explicit number of fields
    check_tuple_struct::<Tuple11>("Tuple11", 11, IoTypeMetadataKind::TupleStruct);
    check_tuple_struct::<Tuple12>("Tuple12", 12, IoTypeMetadataKind::TupleStruct);
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

    // Struct, enum, variant and field names are removed, so only the number of fields and their
    // types remain
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
    assert_ne!(compact(Named2::METADATA), compact(Tuple3::METADATA));

    // Variants with braces or parentheses and no fields are the same as variants without fields
    check_same_compact::<UnitVariants, BracedVariants>();
    check_same_compact::<UnitVariants, ParenthesizedVariants>();
    check_same_compact::<MixedUnitVariant, MixedBracedVariant>();
    check_same_compact::<MixedUnitVariant, MixedParenthesizedVariant>();

    // Nested types
    check_same_compact::<[Named2; 3], [Tuple2; 3]>();
    check_same_compact::<Unaligned<Named2>, Unaligned<Tuple2>>();
    check_same_compact::<MaybeData<Named2>, MaybeData<Tuple2>>();
    check_same_compact::<VariableElements<Named2>, VariableElements<Tuple2>>();
    check_same_compact::<EnumOfNamed, EnumOfTuple>();
}

#[test]
fn structs_with_different_field_types() {
    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct Mixed(u64, Unaligned<u32>, [u8; 4]);

    #[derive(Copy, Clone, TrivialType)]
    #[repr(transparent)]
    struct Transparent(u32);

    let expected = Expected::new(IoTypeMetadataKind::TupleStruct3)
        .name("Mixed")
        .kind(IoTypeMetadataKind::U64)
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U32)
        .kind(IoTypeMetadataKind::Array8b)
        .byte(4)
        .kind(IoTypeMetadataKind::U8);
    let expected_compact = Expected::new(IoTypeMetadataKind::TupleStruct3)
        .name("")
        .kind(IoTypeMetadataKind::U64)
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U32)
        .kind(IoTypeMetadataKind::Array8b)
        .byte(4)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<Mixed>("Mixed", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::TupleStruct1)
        .name("Transparent")
        .kind(IoTypeMetadataKind::U32);
    let expected_compact = Expected::new(IoTypeMetadataKind::TupleStruct1)
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
    fieldless_enum!(
        NoFields17,
        [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q]
    );

    check_fieldless_enum::<NoFields1>("NoFields1", 1, IoTypeMetadataKind::EnumNoFields1);
    check_fieldless_enum::<NoFields2>("NoFields2", 2, IoTypeMetadataKind::EnumNoFields2);
    check_fieldless_enum::<NoFields3>("NoFields3", 3, IoTypeMetadataKind::EnumNoFields3);
    check_fieldless_enum::<NoFields4>("NoFields4", 4, IoTypeMetadataKind::EnumNoFields4);
    check_fieldless_enum::<NoFields5>("NoFields5", 5, IoTypeMetadataKind::EnumNoFields5);
    check_fieldless_enum::<NoFields6>("NoFields6", 6, IoTypeMetadataKind::EnumNoFields6);
    check_fieldless_enum::<NoFields7>("NoFields7", 7, IoTypeMetadataKind::EnumNoFields7);
    check_fieldless_enum::<NoFields8>("NoFields8", 8, IoTypeMetadataKind::EnumNoFields8);
    check_fieldless_enum::<NoFields9>("NoFields9", 9, IoTypeMetadataKind::EnumNoFields9);
    check_fieldless_enum::<NoFields10>("NoFields10", 10, IoTypeMetadataKind::EnumNoFields10);
    // More than 10 variants need an explicit number of variants
    check_fieldless_enum::<NoFields17>("NoFields17", 17, IoTypeMetadataKind::EnumNoFields);
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
    data_enum!(Data17, [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q]);

    check_data_enum::<Data1>("Data1", 1, IoTypeMetadataKind::Enum1);
    check_data_enum::<Data2>("Data2", 2, IoTypeMetadataKind::Enum2);
    check_data_enum::<Data3>("Data3", 3, IoTypeMetadataKind::Enum3);
    check_data_enum::<Data4>("Data4", 4, IoTypeMetadataKind::Enum4);
    check_data_enum::<Data5>("Data5", 5, IoTypeMetadataKind::Enum5);
    check_data_enum::<Data6>("Data6", 6, IoTypeMetadataKind::Enum6);
    check_data_enum::<Data7>("Data7", 7, IoTypeMetadataKind::Enum7);
    check_data_enum::<Data8>("Data8", 8, IoTypeMetadataKind::Enum8);
    check_data_enum::<Data9>("Data9", 9, IoTypeMetadataKind::Enum9);
    check_data_enum::<Data10>("Data10", 10, IoTypeMetadataKind::Enum10);
    // More than 10 variants need an explicit number of variants
    check_data_enum::<Data17>("Data17", 17, IoTypeMetadataKind::Enum);
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

    let (point, point_compact) = point();
    let expected = Expected::new(IoTypeMetadataKind::Enum3)
        .name("Shape")
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
        .kind(IoTypeMetadataKind::Array8b)
        .byte(2)
        .kind(IoTypeMetadataKind::U8);
    let expected_compact = Expected::new(IoTypeMetadataKind::Enum3)
        .name("")
        .name("")
        .byte(1)
        .nested(&point_compact)
        .name("")
        .byte(2)
        .kind(IoTypeMetadataKind::U8)
        .kind(IoTypeMetadataKind::U8)
        .name("")
        .byte(1)
        .kind(IoTypeMetadataKind::Array8b)
        .byte(2)
        .kind(IoTypeMetadataKind::U8);
    check_trivial_type::<Shape>("Shape", &expected, &expected_compact);

    let expected = Expected::new(IoTypeMetadataKind::Enum2)
        .name("MixedDataFirst")
        .name("Full")
        .byte(1)
        .name("value")
        .kind(IoTypeMetadataKind::Unit)
        .name("Empty")
        .byte(0);
    let expected_compact = Expected::new(IoTypeMetadataKind::Enum2)
        .name("")
        .name("")
        .byte(1)
        .kind(IoTypeMetadataKind::Unit)
        .name("")
        .byte(0);
    check_trivial_type::<MixedDataFirst>("MixedDataFirst", &expected, &expected_compact);
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
    let (direction, direction_compact) =
        u8_enum(IoTypeMetadataKind::EnumNoFields2, "Direction", 2, false);
    let (value, value_compact) = u8_enum(IoTypeMetadataKind::Enum2, "Value", 2, true);
    let inner = Expected::new(IoTypeMetadataKind::TupleStruct2)
        .name("Inner")
        .nested(&point)
        .kind(IoTypeMetadataKind::ArrayU8x8);
    let inner_compact = Expected::new(IoTypeMetadataKind::TupleStruct2)
        .name("")
        .nested(&point_compact)
        .kind(IoTypeMetadataKind::ArrayU8x8);
    check_trivial_type::<Inner>("Inner", &inner, &inner_compact);

    let expected = Expected::new(IoTypeMetadataKind::Struct5)
        .name("Outer")
        .name("wide")
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U64)
        .name("points")
        .kind(IoTypeMetadataKind::Array8b)
        .byte(3)
        .nested(&point)
        .name("direction")
        .nested(&direction)
        .name("value")
        .nested(&value)
        .name("inner")
        .nested(&inner);
    let expected_compact = Expected::new(IoTypeMetadataKind::TupleStruct5)
        .name("")
        .kind(IoTypeMetadataKind::Unaligned)
        .kind(IoTypeMetadataKind::U64)
        .kind(IoTypeMetadataKind::Array8b)
        .byte(3)
        .nested(&point_compact)
        .nested(&direction_compact)
        .nested(&value_compact)
        .nested(&inner_compact);
    check_trivial_type::<Outer>("Outer", &expected, &expected_compact);
}
