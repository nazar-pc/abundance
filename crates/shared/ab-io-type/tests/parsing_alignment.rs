//! Tests of parsing values whose contents are not aligned for their types.
//!
//! Parsing methods for aligned memory use the aligned flavour of the parsing step, which reads with
//! aligned loads. That is only sound where the alignment is guaranteed: not inside `Unaligned`,
//! which uses the unaligned flavour, and not for the `u32` length of `FixedCapacityElements` with a
//! smaller alignment. Values are placed at addresses that are not aligned for their contents, and
//! Miri detects aligned loads of them, so these tests are also run under Miri. Both flavours must
//! accept the same values.

use ab_io_type::IoType;
use ab_io_type::fixed_capacity_bytes::FixedCapacityBytes;
use ab_io_type::fixed_capacity_elements::FixedCapacityElements;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use ab_io_type::variable_elements::VariableElements;
use core::any::type_name;
use core::ops::Range;
use core::ptr::NonNull;

/// Offsets into [`Buffer`] with every alignment up to 16 bytes
const OFFSETS: Range<usize> = 0..16;

/// `b` doesn't exceed `c`, which the `validate` function checks on the whole value
#[derive(Default, Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct Validated {
    a: u64,
    b: u32,
    c: u32,
}

impl Validated {
    const VALID: Self = Self { a: 1, b: 2, c: 3 };
    const INVALID: Self = Self { a: 1, b: 3, c: 2 };

    const fn is_valid(&self) -> bool {
        self.b <= self.c
    }
}

/// Contents with alignment 8 and 4 at odd offsets
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithUnaligned {
    tag: u8,
    validated: Unaligned<Validated>,
    elements: Unaligned<FixedCapacityElements<u32, 2>>,
}

/// Length prefixes at offsets 1 and 10, aligned for their containers, but not for `u32`
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithSmallAlignments {
    tag: u8,
    bytes: FixedCapacityBytes<3>,
    short: u16,
    shorts: FixedCapacityElements<u16, 1>,
}

/// Fields of both variants at offset 1
#[derive(Copy, Clone, TrivialType)]
#[expect(dead_code, reason = "Fields are only parsed")]
#[repr(u8)]
enum Tagged {
    Value(Unaligned<Validated>),
    Bytes(FixedCapacityBytes<12>),
}

/// The last fields of both variants at offset 8, aligned for them
#[derive(Copy, Clone, TrivialType)]
#[expect(dead_code, reason = "Fields are only parsed")]
#[repr(u8)]
enum TaggedAligned {
    Value([u8; 6], bool, Validated),
    Elements([u8; 7], FixedCapacityElements<u32, 3>),
}

/// A value with a `validate` function containing another one, and a length prefix
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct Outer {
    validated: Validated,
    elements: FixedCapacityElements<u32, 2>,
    padding: [u8; 4],
}

impl Outer {
    const fn is_valid(&self) -> bool {
        self.validated.a != 0
    }
}

/// Zero-sized with alignment 8 and a `validate` function, which gets a reference to the value
#[derive(Default, Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct ZeroSized {
    alignment: [u64; 0],
}

impl ZeroSized {
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "`validate` functions take a reference"
    )]
    const fn is_valid(&self) -> bool {
        true
    }
}

/// Memory aligned to 16 bytes, with room for two values of any type in these tests after any
/// offset in [`OFFSETS`]
#[repr(C, align(16))]
struct Buffer([u8; 128]);

impl Buffer {
    /// Buffer with `copies` copies of `value` after `offset` bytes
    fn new(offset: usize, value: &[u8], copies: usize) -> Self {
        let mut buffer = Self([0; 128]);
        let Self(bytes) = &mut buffer;
        for copy in bytes[offset..][..value.len() * copies].chunks_exact_mut(value.len()) {
            copy.copy_from_slice(value);
        }
        buffer
    }

    /// `len` bytes after `offset`
    fn bytes(&mut self, offset: usize, len: usize) -> &mut [u8] {
        let Self(bytes) = self;
        &mut bytes[offset..][..len]
    }
}

/// `bytes` with `patch` written at `offset`
fn patched(bytes: &[u8], offset: usize, patch: &[u8]) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes[offset..][..patch.len()].copy_from_slice(patch);
    bytes
}

/// Whether the parsing methods accept `value`, the bytes of a value of `T`, placed at `offset`.
///
/// Methods for aligned memory, including those of `MaybeData` and `VariableElements`, use the
/// aligned flavour of the parsing step and are called if `offset` is aligned for `T`, the methods
/// of `Unaligned<T>` use the unaligned flavour and are called at every offset. All of them must
/// agree.
fn parse_at<T>(value: &[u8], offset: usize) -> bool
where
    T: TrivialType,
    for<'a> MaybeData<'a, T>: IoType<PointerType = T>,
    for<'a> VariableElements<'a, T>: IoType<PointerType = T>,
    for<'a> MaybeData<'a, Unaligned<T>>: IoType<PointerType = Unaligned<T>>,
    for<'a> VariableElements<'a, Unaligned<T>>: IoType<PointerType = Unaligned<T>>,
{
    assert_eq!(value.len(), size_of::<T>());
    let len = value.len();
    let size = T::SIZE;
    let elements_size = 2 * size;
    let mut buffer = Buffer::new(offset, value, 2);
    let mut results = Vec::new();

    if offset.is_multiple_of(align_of::<T>()) {
        results.push(T::try_from_bytes(buffer.bytes(offset, len)).is_some());
        results.push(T::try_from_bytes_mut(buffer.bytes(offset, len)).is_some());

        let mut ptr = NonNull::from_mut(buffer.bytes(offset, len)).cast::<T>();
        let mut mut_size = size;
        // SAFETY: Aligned, the size and capacity are the size of `T`, that many initialized bytes
        // are not accessed otherwise
        unsafe {
            results.push(T::try_from_ptr(&ptr, &size, size).is_some());
            results.push(T::try_from_mut_ptr(&mut ptr, &mut mut_size, size).is_some());
            results.push(MaybeData::<T>::try_from_ptr(&ptr, &size, size).is_some());
            results.push(MaybeData::<T>::try_from_mut_ptr(&mut ptr, &mut mut_size, size).is_some());
        }

        let mut ptr = NonNull::from_mut(buffer.bytes(offset, 2 * len)).cast::<T>();
        let mut mut_size = elements_size;
        // SAFETY: Aligned, the size and capacity are the size of two values of `T`, that many
        // initialized bytes are not accessed otherwise
        unsafe {
            results.push(
                VariableElements::<T>::try_from_ptr(&ptr, &elements_size, elements_size).is_some(),
            );
            results.push(
                VariableElements::<T>::try_from_mut_ptr(&mut ptr, &mut mut_size, elements_size)
                    .is_some(),
            );
        }
    } else {
        assert!(T::try_from_bytes(buffer.bytes(offset, len)).is_none());
        assert!(T::try_from_bytes_mut(buffer.bytes(offset, len)).is_none());
    }

    results.push(Unaligned::<T>::try_from_bytes(buffer.bytes(offset, len)).is_some());
    results.push(Unaligned::<T>::try_from_bytes_mut(buffer.bytes(offset, len)).is_some());

    let ptr = NonNull::from_mut(buffer.bytes(offset, 2 * len)).cast::<Unaligned<T>>();
    // SAFETY: Alignment 1, the size and capacity are the size of one or two values of `T`, that
    // many initialized bytes are not accessed otherwise
    unsafe {
        results.push(Unaligned::<T>::try_from_ptr(&ptr, &size, size).is_some());
        results.push(MaybeData::<Unaligned<T>>::try_from_ptr(&ptr, &size, size).is_some());
        results.push(
            VariableElements::<Unaligned<T>>::try_from_ptr(&ptr, &elements_size, elements_size)
                .is_some(),
        );
    }

    let accepted = results.contains(&true);
    assert!(
        results.iter().all(|&result| result == accepted),
        "{} at offset {offset}: {results:?}",
        type_name::<T>()
    );
    accepted
}

/// Checks that the parsing methods accept each of `valid` and reject each of `invalid` at every
/// offset in [`OFFSETS`]
fn check<T>(valid: &[&[u8]], invalid: &[&[u8]])
where
    T: TrivialType,
    for<'a> MaybeData<'a, T>: IoType<PointerType = T>,
    for<'a> VariableElements<'a, T>: IoType<PointerType = T>,
    for<'a> MaybeData<'a, Unaligned<T>>: IoType<PointerType = Unaligned<T>>,
    for<'a> VariableElements<'a, Unaligned<T>>: IoType<PointerType = Unaligned<T>>,
{
    for offset in OFFSETS {
        for value in valid {
            assert!(parse_at::<T>(value, offset), "{}", type_name::<T>());
        }
        for value in invalid {
            assert!(!parse_at::<T>(value, offset), "{}", type_name::<T>());
        }
    }
}

#[test]
fn validate() {
    check::<Validated>(
        &[Validated::VALID.as_bytes()],
        &[Validated::INVALID.as_bytes()],
    );
}

#[test]
fn unaligned_fields() {
    let valid = WithUnaligned {
        tag: 7,
        validated: Unaligned::new(Validated::VALID),
        elements: Unaligned::new(FixedCapacityElements::try_from_slice(&[1, 2]).unwrap()),
    };
    let invalid = WithUnaligned {
        validated: Unaligned::new(Validated::INVALID),
        ..valid
    };
    check::<WithUnaligned>(
        &[valid.as_bytes()],
        &[
            invalid.as_bytes(),
            // The length exceeds the capacity
            &patched(valid.as_bytes(), 17, &3_u32.to_le_bytes()),
        ],
    );
}

#[test]
fn length_prefixes_with_small_alignment() {
    let valid = WithSmallAlignments {
        tag: 7,
        bytes: FixedCapacityBytes::try_from_slice(&[1, 2]).unwrap(),
        short: 3,
        shorts: FixedCapacityElements::try_from_slice(&[4]).unwrap(),
    };
    check::<WithSmallAlignments>(
        &[valid.as_bytes()],
        // Lengths exceed capacities
        &[
            &patched(valid.as_bytes(), 1, &4_u32.to_le_bytes()),
            &patched(valid.as_bytes(), 10, &2_u32.to_le_bytes()),
        ],
    );

    let bytes = FixedCapacityBytes::<3>::try_from_slice(&[1, 2]).unwrap();
    check::<FixedCapacityBytes<3>>(
        &[bytes.as_bytes()],
        &[&patched(bytes.as_bytes(), 0, &4_u32.to_le_bytes())],
    );
    let shorts = FixedCapacityElements::<u16, 1>::try_from_slice(&[4]).unwrap();
    check::<FixedCapacityElements<u16, 1>>(
        &[shorts.as_bytes()],
        &[&patched(shorts.as_bytes(), 0, &2_u32.to_le_bytes())],
    );
}

#[test]
fn enum_fields() {
    let validated = Tagged::Value(Unaligned::new(Validated::VALID));
    let bytes = Tagged::Bytes(FixedCapacityBytes::try_from_slice(&[1, 2, 3]).unwrap());
    check::<Tagged>(
        &[validated.as_bytes(), bytes.as_bytes()],
        &[
            Tagged::Value(Unaligned::new(Validated::INVALID)).as_bytes(),
            // The length exceeds the capacity
            &patched(bytes.as_bytes(), 1, &13_u32.to_le_bytes()),
            // Not a discriminant of any variant
            &patched(bytes.as_bytes(), 0, &[2]),
        ],
    );

    let validated = TaggedAligned::Value([1; 6], true, Validated::VALID);
    let elements =
        TaggedAligned::Elements([1; 7], FixedCapacityElements::try_from_slice(&[1]).unwrap());
    check::<TaggedAligned>(
        &[validated.as_bytes(), elements.as_bytes()],
        &[
            TaggedAligned::Value([1; 6], true, Validated::INVALID).as_bytes(),
            // Not a `bool`
            &patched(validated.as_bytes(), 7, &[2]),
            // The length exceeds the capacity
            &patched(elements.as_bytes(), 8, &4_u32.to_le_bytes()),
            // Not a discriminant of any variant
            &patched(elements.as_bytes(), 0, &[2]),
        ],
    );
}

#[test]
fn nested_values() {
    let valid = Outer {
        validated: Validated::VALID,
        elements: FixedCapacityElements::try_from_slice(&[1, 2]).unwrap(),
        padding: [0; 4],
    };
    check::<Outer>(
        &[valid.as_bytes()],
        &[
            // Rejected by the `validate` function of the field
            Outer {
                validated: Validated::INVALID,
                ..valid
            }
            .as_bytes(),
            // Rejected by the `validate` function of `Outer`
            Outer {
                validated: Validated {
                    a: 0,
                    ..Validated::VALID
                },
                ..valid
            }
            .as_bytes(),
            // The length exceeds the capacity
            &patched(valid.as_bytes(), 16, &3_u32.to_le_bytes()),
        ],
    );
}

#[test]
fn arrays() {
    check::<[Validated; 2]>(
        &[[Validated::VALID; 2].as_bytes()],
        &[[Validated::VALID, Validated::INVALID].as_bytes()],
    );
    check::<[Unaligned<Validated>; 2]>(
        &[[Unaligned::new(Validated::VALID); 2].as_bytes()],
        &[[
            Unaligned::new(Validated::VALID),
            Unaligned::new(Validated::INVALID),
        ]
        .as_bytes()],
    );
    let bytes = [FixedCapacityBytes::<3>::try_from_slice(&[1]).unwrap(); 2];
    check::<[FixedCapacityBytes<3>; 2]>(
        &[bytes.as_bytes()],
        // The length of the second element exceeds the capacity
        &[&patched(bytes.as_bytes(), 7, &4_u32.to_le_bytes())],
    );
}

#[test]
fn elements() {
    let elements = FixedCapacityElements::<u32, 2>::try_from_slice(&[1, 2]).unwrap();
    check::<FixedCapacityElements<u32, 2>>(
        &[elements.as_bytes()],
        &[&patched(elements.as_bytes(), 0, &3_u32.to_le_bytes())],
    );

    let elements =
        FixedCapacityElements::<Validated, 2>::try_from_slice(&[Validated::VALID]).unwrap();
    check::<FixedCapacityElements<Validated, 2>>(
        &[elements.as_bytes()],
        // Unused elements are checked too
        &[&patched(
            elements.as_bytes(),
            24,
            Validated::INVALID.as_bytes(),
        )],
    );

    let elements =
        FixedCapacityElements::<Unaligned<Validated>, 2>::try_from_slice(&[Unaligned::new(
            Validated::VALID,
        )])
        .unwrap();
    check::<FixedCapacityElements<Unaligned<Validated>, 2>>(
        &[elements.as_bytes()],
        &[&patched(
            elements.as_bytes(),
            20,
            Validated::INVALID.as_bytes(),
        )],
    );

    // Zero-sized elements are checked once rather than one by one
    let elements =
        FixedCapacityElements::<ZeroSized, 2>::try_from_slice(&[ZeroSized::default()]).unwrap();
    check::<FixedCapacityElements<ZeroSized, 2>>(
        &[elements.as_bytes()],
        &[&patched(elements.as_bytes(), 0, &3_u32.to_le_bytes())],
    );
}
