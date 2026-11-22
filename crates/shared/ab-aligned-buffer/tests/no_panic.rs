//! Calls methods that are checked by the `no-panic` feature with inputs that are not known at
//! compile time.
//!
//! Annotated methods are checked where they are inlined, so a panic that only happens for some
//! inputs is optimized out of call sites with constant inputs, like in most other tests, and goes
//! unnoticed. These tests only need to be built in release mode with the `no-panic` feature, a
//! debug build doesn't link with it.

#![cfg(feature = "no-panic")]

use ab_aligned_buffer::alignment::{Align1, Align8, Align16, Align4096, Alignment, AtLeast};
use ab_aligned_buffer::{AlignedBuffer, OwnedAlignedBuffer, SharedAlignedBuffer};
use std::hint::black_box;

fn owned<A>()
where
    A: Alignment,
{
    let _: Result<_, _> = black_box(OwnedAlignedBuffer::<A>::try_zeroed(black_box(64)));
    black_box(OwnedAlignedBuffer::<A>::default());
    let mut buffer =
        black_box(OwnedAlignedBuffer::<A>::try_with_capacity(black_box(64)).unwrap_or_default());

    let _: Result<_, _> = black_box(buffer.try_ensure_capacity(black_box(128)));
    let _: Result<_, _> = black_box(buffer.append(black_box(&[1_u8; 8])));
    let _: Result<_, _> = black_box(buffer.pad_to_alignment::<Align8>());
    black_box(buffer.as_slice());
    black_box(buffer.as_mut_slice());
    black_box(&*buffer);
    black_box(&mut *buffer);
    black_box(buffer.as_byte_ptr());
    black_box(buffer.as_mut_byte_ptr());
    black_box(buffer.spare_capacity_mut());
    black_box(buffer.is_empty());
    black_box(buffer.len());
    black_box(buffer.capacity());
    black_box(buffer.offset());
    black_box(buffer.allocation_alignment());
    black_box(AlignedBuffer::is_empty(&buffer));
    black_box(AlignedBuffer::len(&buffer));
    black_box(AlignedBuffer::offset(&buffer));
    black_box(AlignedBuffer::allocation_alignment(&buffer));
    black_box(AlignedBuffer::set_offset(&mut buffer, black_box(0)));
    black_box(buffer.set_offset(black_box(0)));
    buffer.truncate(black_box(16));
    let len = black_box(buffer.len());
    // SAFETY: Not larger than the current length
    unsafe {
        buffer.set_len(len);
    }

    let buffer = black_box(buffer.into_start_align::<Align1>());
    let buffer = black_box(buffer.try_into_start_align::<A>().unwrap_or_default());
    let buffer = black_box(buffer.into_empty::<A>());
    black_box(AlignedBuffer::into_shared(black_box(
        buffer.into_shared().into_owned(),
    )));
}

fn owned_u128<A>()
where
    A: AtLeast<Align16>,
{
    let mut buffer =
        black_box(OwnedAlignedBuffer::<A>::try_zeroed(black_box(64)).unwrap_or_default());

    black_box(buffer.as_u128_slice());
    black_box(buffer.as_ptr());
    black_box(buffer.as_mut_ptr());
}

fn shared<A>()
where
    A: Alignment,
{
    black_box(SharedAlignedBuffer::<A>::default());
    let mut buffer = black_box(
        OwnedAlignedBuffer::<A>::try_zeroed(black_box(64))
            .unwrap_or_default()
            .into_shared(),
    );

    black_box(buffer.as_slice());
    black_box(&*buffer);
    black_box(buffer.as_byte_ptr());
    black_box(buffer.is_empty());
    black_box(buffer.len());
    black_box(buffer.capacity());
    black_box(buffer.offset());
    black_box(buffer.allocation_alignment());
    black_box(AlignedBuffer::is_empty(&buffer));
    black_box(AlignedBuffer::len(&buffer));
    black_box(AlignedBuffer::offset(&buffer));
    black_box(AlignedBuffer::allocation_alignment(&buffer));
    black_box(AlignedBuffer::set_offset(&mut buffer.clone(), black_box(0)));
    black_box(buffer.get_mut());
    black_box(buffer.get_slice(black_box(1)..black_box(10)));
    black_box(buffer.split_at(black_box(8)));

    let mut other = black_box(buffer.clone());
    black_box(other.set_offset(black_box(0)));
    other.truncate(black_box(16));
    black_box(other.clone().into_start_align::<Align1>());
    let _: Result<_, _> = black_box(other.try_into_start_align::<A>());

    let mut reader = black_box(buffer.reader());
    black_box(reader.position());
    black_box(reader.remaining());
    let _: Result<_, _> = black_box(reader.read_to_alignment::<Align8>());
    let _: Result<_, _> = black_box(reader.read_bytes(black_box(4)));
    let _: Result<_, _> = black_box(reader.read_bytes_array::<4>());
    let _: Result<_, _> = black_box(reader.read_buffer::<Align1>(black_box(4)));
    let _: Result<_, _> = black_box(reader.read_trivial::<u64>());
    let _: Result<_, _> = black_box(reader.read_trivial_array::<u32, 2>());
    let _: Result<_, _> = black_box(reader.read_trivial_slice::<u16>(black_box(2)));

    black_box(AlignedBuffer::into_shared(black_box(buffer.clone())));
    let _: Result<_, _> = black_box(buffer.try_into_owned());
}

fn shared_u128<A>()
where
    A: AtLeast<Align16>,
{
    let buffer = black_box(
        OwnedAlignedBuffer::<A>::try_zeroed(black_box(64))
            .unwrap_or_default()
            .into_shared(),
    );

    black_box(buffer.as_ptr());
}

#[test]
fn owned_buffer() {
    owned::<Align1>();
    owned::<Align16>();
    owned::<Align4096>();
    owned_u128::<Align16>();
    owned_u128::<Align4096>();
}

#[test]
fn shared_buffer() {
    shared::<Align1>();
    shared::<Align16>();
    shared::<Align4096>();
    shared_u128::<Align16>();
    shared_u128::<Align4096>();
}
