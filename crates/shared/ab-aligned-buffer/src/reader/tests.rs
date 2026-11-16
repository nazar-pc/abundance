use crate::alignment::{Align1, Align4, Align8, Align16};
use crate::reader::ReadError;
use crate::{OwnedAlignedBuffer, SharedAlignedBuffer};
use alloc::vec::Vec;
use core::num::NonZeroU32;

/// Shared buffer with `len` bytes, where every byte is its index
fn buffer(len: u8) -> SharedAlignedBuffer<Align16> {
    OwnedAlignedBuffer::from_bytes(&(0..len).collect::<Vec<u8>>()).into_shared()
}

/// [`ReadError::NotEnoughBytes`] with `missing` bytes
fn not_enough_bytes(missing: u32) -> ReadError {
    ReadError::NotEnoughBytes {
        missing: NonZeroU32::new(missing).unwrap(),
    }
}

#[test]
fn reads_bytes_front_to_back() {
    let buffer = buffer(10);
    let mut reader = buffer.reader();

    assert_eq!(reader.position(), 0);
    assert_eq!(reader.read_bytes(3).unwrap(), &[0, 1, 2]);
    assert_eq!(reader.read_bytes_array::<2>().unwrap(), &[3, 4]);
    assert_eq!(reader.read_bytes(0).unwrap(), &[]);
    assert_eq!(reader.position(), 5);
    assert_eq!(reader.remaining(), &[5, 6, 7, 8, 9]);

    // Borrowed bytes outlive the reader
    let bytes = {
        let mut reader = reader;
        reader.read_bytes(5).unwrap()
    };
    assert_eq!(bytes, &[5, 6, 7, 8, 9]);
}

#[test]
fn read_to_alignment_is_counted_from_the_beginning() {
    let buffer = buffer(20);
    let mut reader = buffer.reader();

    // Already aligned
    assert_eq!(reader.read_to_alignment::<Align8>().unwrap(), &[]);
    reader.read_bytes(3).unwrap();
    assert_eq!(reader.read_to_alignment::<Align4>().unwrap(), &[3]);
    assert_eq!(reader.read_to_alignment::<Align8>().unwrap(), &[4, 5, 6, 7]);
    assert_eq!(reader.position(), 8);
    assert_eq!(reader.read_to_alignment::<Align1>().unwrap(), &[]);
}

#[test]
fn failed_read_leaves_reader_unchanged() {
    let buffer = buffer(10);
    let mut reader = buffer.reader();
    reader.read_bytes(3).unwrap();

    assert_eq!(reader.read_bytes(8), Err(not_enough_bytes(1)));
    assert_eq!(reader.read_bytes_array::<10>(), Err(not_enough_bytes(3)));
    assert_eq!(
        reader.read_buffer::<Align1>(8).map(|buffer| buffer.len()),
        Err(not_enough_bytes(1))
    );
    // Padding is not skipped implicitly
    assert_eq!(
        reader.read_buffer::<Align8>(4).map(|buffer| buffer.len()),
        Err(ReadError::Misaligned { alignment: 8 })
    );
    // Padding itself is required
    reader.read_bytes(6).unwrap();
    assert_eq!(
        reader.read_to_alignment::<Align16>(),
        Err(not_enough_bytes(6))
    );

    assert_eq!(reader.position(), 9);
    assert_eq!(reader.remaining(), &[9]);
}

#[test]
fn read_buffer_references_contents() {
    let buffer = buffer(40);
    let contents = buffer.as_slice().as_ptr_range();
    let mut reader = buffer.reader();
    reader.read_bytes(1).unwrap();

    reader.read_to_alignment::<Align16>().unwrap();
    let aligned_part = reader.read_buffer::<Align16>(20).unwrap();
    assert_eq!(aligned_part.as_slice(), &buffer[16..36]);
    assert!(contents.contains(&aligned_part.as_slice().as_ptr()));
    assert_eq!(reader.position(), 36);

    // Unaligned buffer right after it
    let unaligned_part = reader.read_buffer::<Align1>(3).unwrap();
    assert_eq!(unaligned_part.as_slice(), &[36, 37, 38]);

    // Empty buffer doesn't keep the allocation alive
    let empty = reader.read_buffer::<Align1>(0).unwrap();
    assert_eq!(reader.position(), 39);
    assert!(empty.is_empty());
    drop(aligned_part);
    drop(unaligned_part);
    buffer.try_into_owned().unwrap();
}

#[test]
fn alignment_is_checked_in_memory() {
    let buffer = buffer(40);
    // Contents start at an odd address
    let (_first, unaligned) = buffer.split_at(1).unwrap();
    let mut reader = unaligned.reader();

    // Position is aligned, the bytes in memory are not
    assert_eq!(
        reader.read_buffer::<Align4>(4).map(|buffer| buffer.len()),
        Err(ReadError::Misaligned { alignment: 4 })
    );
    assert_eq!(reader.position(), 0);
    // Aligned in memory after 3 bytes
    reader.read_bytes(3).unwrap();
    assert_eq!(
        reader.read_buffer::<Align4>(4).unwrap().as_slice(),
        &[4, 5, 6, 7]
    );
}
