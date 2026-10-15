//! Tests of parsing `TrivialType` values from bytes.
//!
//! Parsing must reject bytes that are not a valid value of the type. Reading such a value is
//! undefined behavior, which Miri detects, so these tests are also run under Miri.

use ab_io_type::IoType;
use ab_io_type::bool::Bool;
use ab_io_type::fixed_capacity_elements::FixedCapacityElements;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use ab_io_type::variable_bytes::VariableBytes;
use ab_io_type::variable_elements::VariableElements;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct Header {
    a: u8,
    b: [u8; 3],
    c: u32,
    d: Unaligned<u64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct Tuple(u16, [u8; 2]);

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct Empty;

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(transparent)]
struct Newtype(u32);

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct Nested {
    header: Header,
    newtype: Newtype,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(u8)]
enum Ctx {
    A,
    B,
    C,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
struct WithCtx {
    value: u8,
    ctx: Ctx,
    rest: [u8; 2],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(u8)]
enum Msg {
    A([u8; 3]),
    B(u8, Unaligned<u16>),
    C { flag: Bool, ctx: Ctx, rest: u8 },
}

/// Number of times the parsing step of [`Counted`] was called
static COUNTED_PARSES: AtomicUsize = AtomicUsize::new(0);

/// Zero-sized type that counts how many times its parsing step is called
#[derive(Debug, Copy, Clone)]
struct Counted;

// SAFETY: Zero-sized, so there is no padding, no uninitialized byte and no interior mutability, and
// the parsing step returns a reference to the beginning of `bytes` and all of `bytes` after it
unsafe impl TrivialType for Counted {
    const METADATA: &[u8] = <() as TrivialType>::METADATA;

    fn try_from_bytes_unaligned(bytes: &[u8]) -> Option<(&Unaligned<Self>, &[u8])> {
        COUNTED_PARSES.fetch_add(1, Ordering::Relaxed);
        // SAFETY: `Unaligned<Self>` is zero-sized with alignment 1, so any pointer into `bytes` is
        // a valid reference to it
        let value = unsafe { &*bytes.as_ptr().cast::<Unaligned<Self>>() };
        Some((value, bytes))
    }
}

/// Bytes aligned to 8 bytes
#[derive(Debug, PartialEq, Eq)]
#[repr(C, align(8))]
struct Aligned<const N: usize>([u8; N]);

/// Parse the beginning of `bytes` with the parsing step, returns a copy of the value and the
/// remaining bytes
fn parse<T>(bytes: &[u8]) -> Option<(T, &[u8])>
where
    T: TrivialType,
{
    let (value, remainder) = T::try_from_bytes_unaligned(bytes)?;
    Some((value.as_inner(), remainder))
}

#[test]
fn integers() {
    let value = u32::from_le_bytes([1, 2, 3, 4]);
    assert_eq!(parse::<u32>(&[1, 2, 3, 4]), Some((value, &[][..])));
    assert_eq!(parse::<u32>(&[1, 2, 3, 4, 5]), Some((value, &[5][..])));
    assert_eq!(parse::<u32>(&[1, 2, 3]), None);
    assert_eq!(parse::<u32>(&[]), None);

    assert_eq!(parse::<i8>(&[0xff]), Some((-1, &[][..])));
    assert_eq!(parse::<i128>(&[0xff; 16]), Some((-1, &[][..])));
    assert_eq!(parse::<i128>(&[0xff; 15]), None);

    // Zero-sized, nothing is consumed
    assert_eq!(parse::<()>(&[]), Some(((), &[][..])));
    assert_eq!(parse::<()>(&[1]), Some(((), &[1][..])));
}

#[test]
fn arrays() {
    assert_eq!(
        parse::<[u16; 3]>(&[1, 0, 2, 0, 3, 0, 4]),
        Some(([1, 2, 3], &[4][..]))
    );
    assert_eq!(parse::<[u16; 3]>(&[1, 0, 2, 0, 3]), None);
    assert_eq!(
        parse::<[[u8; 2]; 2]>(&[1, 2, 3, 4]),
        Some(([[1, 2], [3, 4]], &[][..]))
    );
    assert_eq!(parse::<[u8; 0]>(&[1]), Some(([], &[1][..])));
    assert_eq!(parse::<[(); 3]>(&[]), Some(([(); 3], &[][..])));

    // Every element is checked
    assert_eq!(
        parse::<[Ctx; 3]>(&[0, 1, 2]),
        Some(([Ctx::A, Ctx::B, Ctx::C], &[][..]))
    );
    assert_eq!(parse::<[Ctx; 3]>(&[0, 1, 3]), None);
    assert_eq!(parse::<[Ctx; 3]>(&[3, 1, 2]), None);
}

#[test]
fn zero_sized_elements() {
    assert!(parse::<[Counted; 0]>(&[1]).is_some());
    assert_eq!(COUNTED_PARSES.load(Ordering::Relaxed), 0);

    // Elements don't consume bytes, so one of them is checked instead of all of them
    assert!(parse::<[Counted; 1_000_000]>(&[1]).is_some());
    assert_eq!(COUNTED_PARSES.load(Ordering::Relaxed), 1);
    assert!(parse::<FixedCapacityElements<Counted, 1_000_000>>(&[0; 4]).is_some());
    assert_eq!(COUNTED_PARSES.load(Ordering::Relaxed), 2);
}

#[test]
fn structs() {
    let header = Header {
        a: 1,
        b: [2, 3, 4],
        c: 5,
        d: Unaligned::new(6),
    };
    let mut bytes = header.as_bytes().to_vec();
    assert_eq!(parse::<Header>(&bytes), Some((header, &[][..])));
    assert_eq!(parse::<Header>(bytes.split_last().unwrap().1), None);
    bytes.push(9);
    assert_eq!(parse::<Header>(&bytes), Some((header, &[9][..])));

    assert_eq!(
        parse::<Tuple>(&[1, 0, 2, 3, 4]),
        Some((Tuple(1, [2, 3]), &[4][..]))
    );
    assert_eq!(parse::<Empty>(&[]), Some((Empty, &[][..])));
    assert_eq!(parse::<Empty>(&[1]), Some((Empty, &[1][..])));
    assert_eq!(parse::<Newtype>(&[1, 0, 0, 0]), Some((Newtype(1), &[][..])));
    assert_eq!(parse::<Newtype>(&[1, 0, 0]), None);

    let nested = Nested {
        header,
        newtype: Newtype(7),
    };
    assert_eq!(parse::<Nested>(nested.as_bytes()), Some((nested, &[][..])));
}

#[test]
fn struct_with_enum_field() {
    let with_ctx = WithCtx {
        value: 1,
        ctx: Ctx::C,
        rest: [3, 4],
    };
    assert_eq!(parse::<WithCtx>(&[1, 2, 3, 4]), Some((with_ctx, &[][..])));
    assert_eq!(parse::<WithCtx>(&[1, 3, 3, 4]), None);
}

#[test]
fn fieldless_enum() {
    assert_eq!(parse::<Ctx>(&[0]), Some((Ctx::A, &[][..])));
    assert_eq!(parse::<Ctx>(&[1]), Some((Ctx::B, &[][..])));
    assert_eq!(parse::<Ctx>(&[2, 7]), Some((Ctx::C, &[7][..])));
    assert_eq!(parse::<Ctx>(&[]), None);
}

/// Bytes that are not a discriminant of any variant are rejected, a cast to the enum would create
/// an invalid value
#[test]
fn invalid_discriminant() {
    assert!(Ctx::try_from_bytes_unaligned(&[3]).is_none());
    assert!(Ctx::try_from_bytes_unaligned(&[7]).is_none());
    assert!(Ctx::try_from_bytes_unaligned(&[255]).is_none());
    assert!(Msg::try_from_bytes_unaligned(&[3, 0, 0, 0]).is_none());
    assert!(Msg::try_from_bytes_unaligned(&[255, 0, 0, 0]).is_none());
}

#[test]
fn data_enum() {
    assert_eq!(
        parse::<Msg>(&[0, 1, 2, 3]),
        Some((Msg::A([1, 2, 3]), &[][..]))
    );
    assert_eq!(
        parse::<Msg>(&[1, 5, 6, 7, 8]),
        Some((
            Msg::B(5, Unaligned::new(u16::from_le_bytes([6, 7]))),
            &[8][..]
        ))
    );
    assert_eq!(
        parse::<Msg>(&[2, 1, 2, 9]),
        Some((
            Msg::C {
                flag: Bool::new(true),
                ctx: Ctx::C,
                rest: 9
            },
            &[][..]
        ))
    );
    assert_eq!(parse::<Msg>(&[2, 1, 2]), None);

    // An invalid field of the selected variant is rejected, the same bytes are accepted for a
    // variant whose fields accept them
    assert_eq!(parse::<Msg>(&[2, 0, 3, 9]), None);
    assert_eq!(
        parse::<Msg>(&[0, 0, 3, 9]),
        Some((Msg::A([0, 3, 9]), &[][..]))
    );
}

/// A constant named like a local of the generated parsing step doesn't change what it returns
#[test]
fn constant_named_like_a_local() {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
    #[repr(u8)]
    enum Single {
        Only,
    }

    // Matches every value of `Single`, a binding with this name would be a constant pattern
    #[expect(
        non_upper_case_globals,
        reason = "Named like a local of the generated step"
    )]
    const value: &Unaligned<Single> = &Unaligned::new(Single::Only);

    let bytes = [0, 1];
    let (parsed, remainder) = Single::try_from_bytes_unaligned(&bytes).unwrap();
    assert_eq!(
        core::ptr::from_ref(parsed).cast::<u8>(),
        <[u8]>::as_ptr(&bytes)
    );
    assert!(!core::ptr::eq(parsed, value));
    assert_eq!(remainder, &[1]);
}

#[test]
fn unaligned() {
    assert_eq!(
        parse::<Unaligned<u64>>(&[1, 0, 0, 0, 0, 0, 0, 0, 2]),
        Some((Unaligned::new(1), &[2][..]))
    );
    assert_eq!(parse::<Unaligned<u64>>(&[1, 0, 0, 0, 0, 0, 0]), None);
    assert_eq!(
        parse::<Unaligned<Ctx>>(&[1]),
        Some((Unaligned::new(Ctx::B), &[][..]))
    );
    assert!(Unaligned::<Ctx>::try_from_bytes_unaligned(&[3]).is_none());
    assert!(Unaligned::<Msg>::try_from_bytes_unaligned(&[2, 0, 3, 9]).is_none());
}

#[test]
fn aligned_references() {
    let aligned = Aligned([1, 0, 0, 0, 0, 0, 0, 0, 2]);
    let (value, remainder) = u64::try_from_bytes(&aligned.0).unwrap();
    assert_eq!(*value, 1);
    assert_eq!(remainder, &[2]);
    assert_eq!(u64::try_from_bytes(&aligned.0[..7]), None);

    // Misaligned bytes are rejected, unless they are read as a copy
    let (_first, misaligned) = aligned.0.split_first().unwrap();
    assert_eq!(u64::try_from_bytes(misaligned), None);
    assert_eq!(u64::try_from_bytes_exact(misaligned), None);
    assert_eq!(
        u64::try_read_unaligned(misaligned),
        Some((2 << 56, &[][..]))
    );
    assert_eq!(u64::try_read_unaligned_exact(misaligned), Some(2 << 56));

    // Types with alignment 1 are never misaligned
    assert_eq!(
        <[u8; 2]>::try_from_bytes(misaligned),
        Some((&[0, 0], &misaligned[2..]))
    );
}

#[test]
fn exact() {
    let aligned = Aligned([1, 0, 0, 0, 0, 0, 0, 0, 2]);
    assert_eq!(u64::try_from_bytes_exact(&aligned.0[..8]), Some(&1));
    assert_eq!(u64::try_from_bytes_exact(&aligned.0), None);
    assert_eq!(u64::try_from_bytes_exact(&aligned.0[..7]), None);
    assert_eq!(u64::try_read_unaligned_exact(&aligned.0[..8]), Some(1));
    assert_eq!(u64::try_read_unaligned_exact(&aligned.0), None);
    assert_eq!(u64::try_read_unaligned_exact(&aligned.0[..7]), None);

    // Zero-sized types are parsed from empty input
    assert_eq!(<()>::try_from_bytes_exact(&[]), Some(&()));
    assert_eq!(<()>::try_from_bytes_exact(&[1]), None);
    assert_eq!(Empty::try_read_unaligned_exact(&[]), Some(Empty));
}

#[test]
fn exclusive_references() {
    let mut aligned = Aligned([0; 10]);
    let (value, remainder) = u64::try_from_bytes_mut(&mut aligned.0).unwrap();
    *value = 0x0102;
    // Both parts stay usable, interleaved writes don't invalidate each other
    remainder.fill(7);
    *value += 1;
    remainder[0] = 8;
    assert_eq!(aligned, Aligned([3, 1, 0, 0, 0, 0, 0, 0, 8, 7]));

    // Misaligned and short bytes are rejected
    let (_first, misaligned) = aligned.0.split_first_mut().unwrap();
    assert!(u64::try_from_bytes_mut(misaligned).is_none());
    assert!(u64::try_from_bytes_mut(&mut aligned.0[..7]).is_none());

    // Valid values written through the reference are parsed back
    let mut bytes = [0, 0, 0, 0];
    let (with_ctx, remainder) = WithCtx::try_from_bytes_mut(&mut bytes).unwrap();
    assert!(remainder.is_empty());
    with_ctx.ctx = Ctx::C;
    with_ctx.rest = [4, 5];
    assert_eq!(bytes, [0, 2, 4, 5]);
}

/// Every parsing method rejects bytes that are not a valid value
#[test]
fn invalid_values_are_rejected() {
    assert_eq!(Ctx::try_from_bytes(&[3]), None);
    assert_eq!(Ctx::try_from_bytes_exact(&[3]), None);
    assert!(Ctx::try_from_bytes_mut(&mut [3]).is_none());
    assert_eq!(Ctx::try_read_unaligned(&[3]), None);
    assert_eq!(Ctx::try_read_unaligned_exact(&[3]), None);

    assert_eq!(WithCtx::try_from_bytes(&[0, 3, 0, 0]), None);
    assert_eq!(WithCtx::try_from_bytes_exact(&[0, 3, 0, 0]), None);
    assert!(WithCtx::try_from_bytes_mut(&mut [0, 3, 0, 0]).is_none());
    assert_eq!(WithCtx::try_read_unaligned(&[0, 3, 0, 0]), None);
    assert_eq!(WithCtx::try_read_unaligned_exact(&[0, 3, 0, 0]), None);
}

/// The unsafe methods that skip parsing only require the bytes of the value itself to be a valid
/// value, bytes after it are not cast to `Self` (Miri checks this with
/// `-Zmiri-recursive-validation`)
#[test]
fn unchecked_methods_ignore_bytes_after_the_value() {
    let mut bytes = [1, 7];
    // SAFETY: The first byte is a valid value of `Ctx`
    assert_eq!(unsafe { Ctx::from_bytes(&bytes) }, Some(&Ctx::B));
    // SAFETY: The first byte is a valid value of `Ctx`
    let value = unsafe { Ctx::from_bytes_mut(&mut bytes) };
    assert_eq!(value, Some(&mut Ctx::B));
    // SAFETY: The first byte is a valid value of `Ctx`
    assert_eq!(unsafe { Ctx::read_unaligned(&bytes) }, Some(Ctx::B));
}

#[test]
fn read_from_variable_bytes() {
    // Aligned for `u32`, so that both aligned and unaligned reads happen
    let buffer = [
        u32::from_le_bytes([1, 2, 3, 4]),
        u32::from_le_bytes([5, 6, 7, 8]),
    ];
    let buffer = buffer.as_bytes();

    {
        let size = 4;
        let bytes = VariableBytes::<0>::from_buffer(&buffer[..4], &size).unwrap();
        assert_eq!(
            u32::try_read_unaligned(bytes.get_initialized()),
            Some((u32::from_le_bytes([1, 2, 3, 4]), &[][..]))
        );
    }

    {
        let size = 4;
        let bytes = VariableBytes::<0>::from_buffer(&buffer[1..5], &size).unwrap();
        assert_eq!(
            u32::try_read_unaligned(bytes.get_initialized()),
            Some((u32::from_le_bytes([2, 3, 4, 5]), &[][..]))
        );
    }

    // Not enough bytes
    {
        let size = 3;
        let bytes = VariableBytes::<0>::from_buffer(&buffer[..3], &size).unwrap();
        assert_eq!(u32::try_read_unaligned(bytes.get_initialized()), None);
    }

    // Not a valid value
    {
        let size = 1;
        let bytes = VariableBytes::<0>::from_buffer(&[7], &size).unwrap();
        assert_eq!(Ctx::try_read_unaligned(bytes.get_initialized()), None);
    }
}

#[test]
fn plain_try_from_ptr() {
    let mut value = 5u64;
    let mut ptr = NonNull::from_mut(&mut value);
    let mut size = 8;
    for capacity in [8, 16] {
        // SAFETY: Aligned, 8 initialized bytes that are not accessed otherwise
        let parsed = unsafe { u64::try_from_ptr(&ptr, &size, capacity) };
        assert_eq!(parsed.as_deref(), Some(&5));
    }
    {
        // SAFETY: Aligned, 8 initialized bytes that are not accessed otherwise
        let mut parsed = unsafe { u64::try_from_mut_ptr(&mut ptr, &mut size, 8) }.unwrap();
        *parsed = 6;
    }
    assert_eq!(value, 6);

    let mut values = [5u64, 6];
    let mut ptr = NonNull::from_mut(&mut values).cast::<u64>();
    for size in [0, 7, 9, 16] {
        // SAFETY: Aligned, 16 initialized bytes that are not accessed otherwise
        assert!(unsafe { u64::try_from_ptr(&ptr, &size, 16) }.is_none());
    }
    // Capacity is smaller than the size
    // SAFETY: Aligned, size exceeds capacity
    assert!(unsafe { u64::try_from_ptr(&ptr, &8, 7) }.is_none());
    let mut size = 8;
    // SAFETY: Aligned, valid for reads and writes of 7 bytes that are not accessed otherwise, size
    // exceeds capacity
    assert!(unsafe { u64::try_from_mut_ptr(&mut ptr, &mut size, 7) }.is_none());

    // Size `0` over uninitialized memory is rejected without reading it
    let mut uninit = MaybeUninit::<u64>::uninit();
    let mut ptr = NonNull::from_mut(&mut uninit).cast::<u64>();
    let mut size = 0;
    // SAFETY: Aligned, valid for reads and writes of 8 bytes, none of which are initialized
    assert!(unsafe { u64::try_from_mut_ptr(&mut ptr, &mut size, 8) }.is_none());
    // SAFETY: Aligned, size `0`
    assert!(unsafe { u64::try_from_ptr(&ptr, &size, 8) }.is_none());

    // Invalid contents are rejected
    let mut byte = 3u8;
    let mut ptr = NonNull::from_mut(&mut byte).cast::<Ctx>();
    let mut size = 1;
    // SAFETY: Alignment 1, one initialized byte that is not accessed otherwise
    assert!(unsafe { Ctx::try_from_ptr(&ptr, &size, 1) }.is_none());
    // SAFETY: Alignment 1, one initialized byte that is not accessed otherwise
    assert!(unsafe { Ctx::try_from_mut_ptr(&mut ptr, &mut size, 1) }.is_none());
    byte = 2;
    let ptr = NonNull::from_mut(&mut byte).cast::<Ctx>();
    {
        // SAFETY: Alignment 1, one initialized byte that is not accessed otherwise
        let parsed = unsafe { Ctx::try_from_ptr(&ptr, &1, 1) };
        assert_eq!(parsed.as_deref(), Some(&Ctx::C));
    }
}

#[test]
fn maybe_data_try_from_ptr() {
    // Size `0` over uninitialized memory is empty
    let mut uninit = MaybeUninit::<Ctx>::uninit();
    let mut ptr = NonNull::from_mut(&mut uninit).cast::<Ctx>();
    let mut size = 0;
    {
        // SAFETY: Alignment 1, valid for reads and writes of one byte, size `0`
        let mut maybe_data =
            unsafe { MaybeData::<Ctx>::try_from_mut_ptr(&mut ptr, &mut size, 1) }.unwrap();
        assert_eq!(maybe_data.get(), None);
        maybe_data.replace(Ctx::B);
    }
    assert_eq!(size, 1);
    {
        // SAFETY: Alignment 1, one byte initialized by `replace()` above
        let maybe_data = unsafe { MaybeData::<Ctx>::try_from_ptr(&ptr, &size, 1) }.unwrap();
        assert_eq!(maybe_data.get(), Some(&Ctx::B));
    }

    // Exclusive access requires capacity for the data even if it is missing
    let mut size = 0;
    // SAFETY: Alignment 1, capacity `0`
    assert!(unsafe { MaybeData::<Ctx>::try_from_mut_ptr(&mut ptr, &mut size, 0) }.is_none());

    // Invalid contents are rejected
    let mut byte = 7u8;
    let mut ptr = NonNull::from_mut(&mut byte).cast::<Ctx>();
    let mut size = 1;
    // SAFETY: Alignment 1, one initialized byte that is not accessed otherwise
    assert!(unsafe { MaybeData::<Ctx>::try_from_ptr(&ptr, &size, 1) }.is_none());
    // SAFETY: Alignment 1, one initialized byte that is not accessed otherwise
    assert!(unsafe { MaybeData::<Ctx>::try_from_mut_ptr(&mut ptr, &mut size, 1) }.is_none());

    // Sizes other than `0` and the size of data are rejected
    let mut value = 0u64;
    let mut ptr = NonNull::from_mut(&mut value);
    let mut size = 3;
    // SAFETY: Aligned, 8 initialized bytes that are not accessed otherwise
    assert!(unsafe { MaybeData::<u64>::try_from_ptr(&ptr, &size, 8) }.is_none());
    // SAFETY: Aligned, 8 initialized bytes that are not accessed otherwise
    assert!(unsafe { MaybeData::<u64>::try_from_mut_ptr(&mut ptr, &mut size, 8) }.is_none());
    // SAFETY: Aligned, size exceeds capacity
    assert!(unsafe { MaybeData::<u64>::try_from_ptr(&ptr, &8, 4) }.is_none());
    // Sizes above the size of data are rejected even with enough capacity
    let mut values = [0u64; 2];
    let mut ptr = NonNull::from_mut(&mut values).cast::<u64>();
    let mut size = 16;
    // SAFETY: Aligned, 16 initialized bytes that are not accessed otherwise
    assert!(unsafe { MaybeData::<u64>::try_from_ptr(&ptr, &size, 16) }.is_none());
    // SAFETY: Aligned, 16 initialized bytes that are not accessed otherwise
    assert!(unsafe { MaybeData::<u64>::try_from_mut_ptr(&mut ptr, &mut size, 16) }.is_none());
}

#[test]
fn maybe_data_shared_from_ref_none_arguments() {
    // Arguments of `MaybeData::from_ref(None)`: a dangling pointer, size `0` and capacity of the
    // data, nothing is readable
    let view = MaybeData::<u64>::from_ref(None);
    let ptr = view.as_ptr();
    let size = view.size();
    let capacity = view.capacity();
    assert_eq!(ptr, NonNull::dangling());
    assert_eq!((size, capacity), (0, 8));
    // SAFETY: Aligned, size `0`
    let parsed = unsafe { MaybeData::<u64>::try_from_ptr(&ptr, &size, capacity) }.unwrap();
    assert_eq!(parsed.get(), None);

    // Size `0` with capacity `0` is accepted, capacity between `0` and the size of data is not
    // SAFETY: Aligned, size `0`
    let parsed = unsafe { MaybeData::<u64>::try_from_ptr(&ptr, &0, 0) }.unwrap();
    assert_eq!(parsed.get(), None);
    for capacity in 1..8 {
        // SAFETY: Aligned, size `0`
        assert!(unsafe { MaybeData::<u64>::try_from_ptr(&ptr, &0, capacity) }.is_none());
    }
    // The size of data with capacity `0` is rejected without reading
    // SAFETY: Aligned, size exceeds capacity
    assert!(unsafe { MaybeData::<u64>::try_from_ptr(&ptr, &8, 0) }.is_none());
}

#[test]
fn variable_elements_try_from_ptr() {
    let mut buffer = [0u8, 1, 2, 1];
    let mut ptr = NonNull::from_mut(&mut buffer).cast::<Ctx>();
    let mut size = 4;
    {
        // SAFETY: Alignment 1, 4 initialized bytes that are not accessed otherwise
        let elements =
            unsafe { VariableElements::<Ctx>::try_from_mut_ptr(&mut ptr, &mut size, 4) }.unwrap();
        assert_eq!(
            elements.get_initialized(),
            &[Ctx::A, Ctx::B, Ctx::C, Ctx::B]
        );
    }
    {
        // SAFETY: Alignment 1, 4 initialized bytes that are not accessed otherwise
        let elements = unsafe { VariableElements::<Ctx>::try_from_ptr(&ptr, &size, 8) }.unwrap();
        assert_eq!(
            elements.get_initialized(),
            &[Ctx::A, Ctx::B, Ctx::C, Ctx::B]
        );
    }
    // SAFETY: Alignment 1, size exceeds capacity
    assert!(unsafe { VariableElements::<Ctx>::try_from_ptr(&ptr, &size, 3) }.is_none());
    // SAFETY: Alignment 1, size exceeds capacity
    assert!(unsafe { VariableElements::<Ctx>::try_from_mut_ptr(&mut ptr, &mut size, 3) }.is_none());

    // An invalid element in the middle or at the end is rejected
    for invalid in [[0, 3, 2, 1], [0, 1, 2, 7]] {
        buffer = invalid;
        let mut ptr = NonNull::from_mut(&mut buffer).cast::<Ctx>();
        // SAFETY: Alignment 1, 4 initialized bytes that are not accessed otherwise
        assert!(unsafe { VariableElements::<Ctx>::try_from_ptr(&ptr, &size, 4) }.is_none());
        // SAFETY: Alignment 1, 4 initialized bytes that are not accessed otherwise
        let parsed = unsafe { VariableElements::<Ctx>::try_from_mut_ptr(&mut ptr, &mut size, 4) };
        assert!(parsed.is_none());
    }

    // The size must be a multiple of the element size
    let mut words = [0u32; 2];
    let mut ptr = NonNull::from_mut(&mut words).cast::<u32>();
    let mut size = 6;
    // SAFETY: Aligned, 8 initialized bytes that are not accessed otherwise
    assert!(unsafe { VariableElements::<u32>::try_from_ptr(&ptr, &size, 8) }.is_none());
    // SAFETY: Aligned, 8 initialized bytes that are not accessed otherwise
    assert!(unsafe { VariableElements::<u32>::try_from_mut_ptr(&mut ptr, &mut size, 8) }.is_none());

    // Size `0` over uninitialized memory is empty
    let mut uninit = [MaybeUninit::<u32>::uninit(); 2];
    let mut ptr = NonNull::from_mut(&mut uninit).cast::<u32>();
    let mut size = 0;
    {
        // SAFETY: Aligned, valid for reads and writes of 8 bytes, size `0`
        let mut elements =
            unsafe { VariableElements::<u32>::try_from_mut_ptr(&mut ptr, &mut size, 8) }.unwrap();
        assert!(elements.get_initialized().is_empty());
        assert!(elements.append(&[7]));
    }
    assert_eq!(size, 4);
    // SAFETY: Aligned, 4 bytes initialized by `append()` above
    let elements = unsafe { VariableElements::<u32>::try_from_ptr(&ptr, &size, 8) }.unwrap();
    assert_eq!(elements.get_initialized(), &[7]);
}
