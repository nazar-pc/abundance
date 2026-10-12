//! Tests of the layout and `Debug` output of `FixedCapacityElements` and `FixedCapacityString`.
//!
//! The layout must have the alignment of elements and no padding, regardless of the alignment of
//! elements.

use ab_io_type::fixed_capacity_bytes::FixedCapacityBytes;
use ab_io_type::fixed_capacity_elements::FixedCapacityElements;
use ab_io_type::fixed_capacity_string::FixedCapacityString;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct Point(u16, u16, u16);

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct Wide {
    a: u64,
    b: u32,
    c: u32,
}

/// Containers can be fields of derived structs, which reject padding between fields
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Record {
    values: FixedCapacityElements<u64, 2>,
    name: FixedCapacityString<4>,
    tags: FixedCapacityBytes<4>,
}

/// Bytes of a container: `len` as little-endian `u32` followed by zero bytes up to `len_size`
/// bytes, then bytes of `elements`
fn bytes(len: u32, len_size: usize, elements: &[u8]) -> Vec<u8> {
    let mut bytes = len.to_le_bytes().to_vec();
    bytes.resize(len_size, 0);
    bytes.extend_from_slice(elements);
    bytes
}

/// Check the size and alignment of a container with `elements`, and that its bytes are `expected`,
/// unused elements must be filled with default elements
macro_rules! check_layout {
    ($element:ty, $capacity:expr, $elements:expr, $expected:expr $(,)?) => {{
        type Container = FixedCapacityElements<$element, $capacity>;
        let expected: &[u8] = &$expected;

        assert_eq!(size_of::<Container>(), expected.len());
        assert_eq!(Container::SIZE as usize, expected.len());
        assert_eq!(align_of::<Container>(), align_of::<$element>());

        let container = Container::try_from_slice(&$elements).unwrap();
        assert_eq!(container.as_bytes().as_slice(), expected);
        assert_eq!(container.get_elements(), &$elements);
    }};
}

#[test]
fn layout() {
    // Bytes, no padding after elements regardless of the capacity
    check_layout!(u8, 0, [], [0, 0, 0, 0]);
    check_layout!(u8, 1, [7], [1, 0, 0, 0, 7]);
    check_layout!(u8, 3, [7, 8], [2, 0, 0, 0, 7, 8, 0]);
    check_layout!(u8, 3, [], [0, 0, 0, 0, 0, 0, 0]);
    check_layout!(u16, 3, [7, 8], bytes(2, 4, &[7, 0, 8, 0, 0, 0]));
    check_layout!(u32, 2, [7], bytes(1, 4, &[7, 0, 0, 0, 0, 0, 0, 0]));
    // The length is extended to the alignment of elements
    check_layout!(u64, 0, [], bytes(0, 8, &[]));
    check_layout!(
        u64,
        2,
        [7],
        bytes(1, 8, &[7u64.to_le_bytes(), [0; 8]].concat())
    );
    check_layout!(
        u128,
        2,
        [7],
        bytes(1, 16, &[7u128.to_le_bytes(), [0; 16]].concat())
    );
    // Derived structs
    check_layout!(
        Rgb,
        2,
        [Rgb { r: 7, g: 8, b: 9 }],
        bytes(1, 4, &[7, 8, 9, 0, 0, 0])
    );
    check_layout!(
        Point,
        2,
        [Point(7, 8, 9), Point(10, 11, 12)],
        bytes(2, 4, &[7, 0, 8, 0, 9, 0, 10, 0, 11, 0, 12, 0])
    );
    check_layout!(
        Wide,
        2,
        [Wide { a: 7, b: 8, c: 9 }],
        bytes(
            1,
            8,
            &[
                &7u64.to_le_bytes()[..],
                &8u32.to_le_bytes(),
                &9u32.to_le_bytes(),
                &[0; 16],
            ]
            .concat()
        )
    );
    // Arrays and nested containers
    check_layout!([u16; 3], 1, [[7, 8, 9]], bytes(1, 4, &[7, 0, 8, 0, 9, 0]));
    let nested = FixedCapacityElements::<FixedCapacityBytes<1>, 2>::try_from_slice(&[
        FixedCapacityBytes::<1>::try_from_slice(&[7]).unwrap(),
    ])
    .unwrap();
    assert_eq!(align_of_val(&nested), 1);
    assert_eq!(
        nested.as_bytes().as_slice(),
        bytes(1, 4, &[bytes(1, 4, &[7]), bytes(0, 4, &[0])].concat())
    );
    // Zero-sized elements only take space for the length
    check_layout!((), 3, [(), ()], [2, 0, 0, 0]);
}

#[test]
fn derived_struct_field() {
    assert_eq!(size_of::<Record>(), 24 + 8 + 8);
    assert_eq!(align_of::<Record>(), 8);

    let record = Record {
        values: FixedCapacityElements::try_from_slice(&[7]).unwrap(),
        name: FixedCapacityString::try_from_str("ab").unwrap(),
        tags: FixedCapacityBytes::try_from_slice(&[1, 2, 3]).unwrap(),
    };
    let expected = [
        bytes(1, 8, &[7u64.to_le_bytes(), [0; 8]].concat()),
        bytes(2, 4, b"ab\0\0"),
        bytes(3, 4, &[1, 2, 3, 0]),
    ]
    .concat();
    assert_eq!(record.as_bytes().as_slice(), expected);
}

#[test]
fn string_layout() {
    // Same as bytes
    assert_eq!(size_of::<FixedCapacityString<3>>(), 7);
    assert_eq!(align_of::<FixedCapacityString<3>>(), 1);

    let string = FixedCapacityString::<3>::try_from_str("ab").unwrap();
    assert_eq!(string.as_bytes(), &[2, 0, 0, 0, b'a', b'b', 0]);
}

#[test]
fn size_limits() {
    // The largest sizes, including the length
    assert_eq!(
        FixedCapacityBytes::<{ u32::MAX as usize - 4 }>::SIZE,
        u32::MAX
    );
    assert_eq!(
        FixedCapacityString::<{ u32::MAX as usize - 4 }>::SIZE,
        u32::MAX
    );
    assert_eq!(
        FixedCapacityElements::<u128, { (1 << 28) - 2 }>::SIZE,
        u32::MAX - 15
    );
    // Zero-sized elements only need space for the length
    assert_eq!(FixedCapacityElements::<(), { u32::MAX as usize }>::SIZE, 4);
}

#[test]
fn debug() {
    // Only the length and stored elements are printed
    let bytes = FixedCapacityBytes::<4>::try_from_slice(&[1, 2]).unwrap();
    assert_eq!(
        format!("{bytes:?}"),
        "FixedCapacityElements { len: 2, elements: [1, 2] }"
    );

    let points = FixedCapacityElements::<Point, 4>::try_from_slice(&[Point(1, 2, 3)]).unwrap();
    assert_eq!(
        format!("{points:?}"),
        "FixedCapacityElements { len: 1, elements: [Point(1, 2, 3)] }"
    );

    let string = FixedCapacityString::<4>::try_from_str("ab").unwrap();
    assert_eq!(
        format!("{string:?}"),
        "FixedCapacityString { bytes: FixedCapacityElements { len: 2, elements: [97, 98] } }"
    );
}
