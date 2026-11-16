use ab_aligned_buffer::alignment::Align4096;
use ab_direct_io_file::{DirectIoFile, OwnedPages};
use ab_farmer_components::ReadAtSync;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Write;
use tempfile::tempdir;

/// Whole pages of bytes, as files used with direct I/O are
fn test_bytes() -> Vec<u8> {
    (0..OwnedPages::ALIGN * 3)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect()
}

/// Reads at various offsets, reusing the buffer, then reads out of bounds
fn check_reader<Reader>(reader: &Reader, bytes: &[u8], out_of_bounds_kind: io::ErrorKind)
where
    Reader: ReadAtSync,
{
    let mut buffer = OwnedPages::EMPTY;
    for (offset, len) in [
        (0_u32, 1_u32),
        (5, 500),
        (4000, 200),
        (4096, 4096),
        (9000, 1000),
    ] {
        let contents = reader.read_at(buffer, len, u64::from(offset)).unwrap();
        assert_eq!(
            contents.as_slice(),
            &bytes[offset as usize..][..len as usize]
        );
        buffer = contents.into_empty();
    }

    // Large enough allocation is reused
    let allocation = buffer.as_byte_ptr();
    let contents = reader.read_at(buffer, 100, 10).unwrap();
    assert_eq!(contents.as_slice(), &bytes[10..110]);
    let buffer = contents.into_empty::<Align4096>();
    assert_eq!(buffer.as_byte_ptr(), allocation);

    let contents = reader.read_at(buffer, 0, 12_288).unwrap();
    assert!(contents.is_empty());
    let buffer = contents.into_empty();

    // Buffer is returned on error
    let capacity = buffer.capacity();
    let error = reader.read_at(buffer, 2, 12_287).unwrap_err();
    assert_eq!(io::Error::from(error.error).kind(), out_of_bounds_kind);
    assert_eq!(error.buffer.capacity(), capacity);

    // Offset overflow
    let error = reader
        .offset(u64::MAX)
        .read_at(error.buffer, 1, 1)
        .unwrap_err();
    assert_eq!(
        io::Error::from(error.error).kind(),
        io::ErrorKind::InvalidInput
    );

    // Offset is added to reads
    let contents = reader.offset(1000).read_at(error.buffer, 10, 20).unwrap();
    assert_eq!(contents.as_slice(), &bytes[1020..1030]);
}

#[test]
fn read_at_slice() {
    let bytes = test_bytes();

    check_reader(&bytes.as_slice(), &bytes, io::ErrorKind::InvalidInput);
    check_reader(&bytes, &bytes, io::ErrorKind::InvalidInput);
}

#[test]
// TODO: Windows shims are incomplete under Miri: https://github.com/rust-lang/miri/issues/3482
#[cfg_attr(all(miri, windows), ignore)]
fn read_at_file() {
    let bytes = test_bytes();
    let tempdir = tempdir().unwrap();
    let path = tempdir.path().join("file.bin");
    File::create(&path).unwrap().write_all(&bytes).unwrap();

    check_reader(
        &File::open(&path).unwrap(),
        &bytes,
        io::ErrorKind::UnexpectedEof,
    );

    let mut options = OpenOptions::new();
    options.read(true);
    check_reader(
        &DirectIoFile::open(options, &path).unwrap(),
        &bytes,
        io::ErrorKind::UnexpectedEof,
    );
}
