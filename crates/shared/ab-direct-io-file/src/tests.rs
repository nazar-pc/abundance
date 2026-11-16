use crate::{DirectIoError, DirectIoFile, MAX_READ_SIZE, OwnedPages};
use ab_aligned_buffer::alignment::Align1;
use ab_aligned_buffer::{AlignedBuffer, AllocationError, OwnedAlignedBuffer};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use std::fs::OpenOptions;
use std::{fs, io};
use tempfile::tempdir;

#[test]
fn read_write_small() {
    read_write_inner::<15000>(&[
        (0usize, 1_usize),
        (0, 512),
        (0, 4096),
        (0, 500),
        (0, 4000),
        (5, 50),
        (12, 500),
        (96, 4000),
        (4000, 96),
        (10000, 5),
    ]);
}

#[test]
// TODO: Extremely slow under Miri: https://github.com/rust-lang/miri/issues/4463
#[cfg_attr(miri, ignore)]
fn read_write_large() {
    read_write_inner::<{ MAX_READ_SIZE * 5 }>(&[
        (0, MAX_READ_SIZE),
        (0, MAX_READ_SIZE * 2),
        (5, MAX_READ_SIZE - 5),
        (5, MAX_READ_SIZE * 2 - 5),
        (5, MAX_READ_SIZE),
        (5, MAX_READ_SIZE * 2),
        (MAX_READ_SIZE, MAX_READ_SIZE),
        (MAX_READ_SIZE, MAX_READ_SIZE * 2),
        (MAX_READ_SIZE + 5, MAX_READ_SIZE - 5),
        (MAX_READ_SIZE + 5, MAX_READ_SIZE * 2 - 5),
        (MAX_READ_SIZE + 5, MAX_READ_SIZE),
        (MAX_READ_SIZE + 5, MAX_READ_SIZE * 2),
    ]);
}

fn read_write_inner<const BUFFER_SIZE: usize>(offset_size_pairs: &[(usize, usize)]) {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let tempdir = tempdir().unwrap();
    let file_path = tempdir.as_ref().join("file.bin");
    let mut data = vec![0u8; BUFFER_SIZE];
    if cfg!(miri) {
        // TODO: This de-sugaring helps Miri to compute the thing faster:
        //  https://github.com/rust-lang/miri/issues/4463
        let data = data.as_mut_slice();
        let mut index = 0;
        let data_len = data.len();
        while index < data_len {
            data[index] = index as u8;
            index += 1;
        }
    } else {
        rng.fill_bytes(data.as_mut_slice());
    }
    fs::write(&file_path, &data).unwrap();

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    let file = DirectIoFile::open(options, &file_path).unwrap();

    let mut buffer = Vec::new();
    for &(offset, size) in offset_size_pairs {
        let data = &mut data[offset..][..size];
        buffer.resize(size, 0);
        // Read contents
        file.read_exact_at(buffer.as_mut_slice(), offset as u64)
            .unwrap_or_else(|error| panic!("Offset {offset}, size {size}: {error}"));

        // Ensure it is correct
        assert_eq!(data, buffer.as_slice(), "Offset {offset}, size {size}");

        // Update data with random contents and write
        if cfg!(miri) {
            // TODO: This de-sugaring helps Miri to compute the thing faster:
            //  https://github.com/rust-lang/miri/issues/4463
            let mut index = 0;
            let data_len = data.len();
            while index < data_len {
                data[index] = index as u8;
                index += 1;
            }
        } else {
            rng.fill_bytes(data);
        }
        file.write_all_at(data, offset as u64)
            .unwrap_or_else(|error| panic!("Offset {offset}, size {size}: {error}"));

        // Read contents again
        file.read_exact_at(buffer.as_mut_slice(), offset as u64)
            .unwrap_or_else(|error| panic!("Offset {offset}, size {size}: {error}"));

        // Ensure it is correct too
        assert_eq!(data, buffer.as_slice(), "Offset {offset}, size {size}");
    }
}

#[test]
fn other_operations() {
    let tempdir = tempdir().unwrap();
    let file_path = tempdir.as_ref().join("file.bin");

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    let file = DirectIoFile::open(options, &file_path).unwrap();

    assert_eq!(file.len().unwrap(), 0);
    assert!(file.is_empty().unwrap());

    // TODO: Not supported under Miri
    if !cfg!(all(miri, target_os = "macos")) {
        file.allocate(100).unwrap();
        assert_eq!(file.len().unwrap(), 100);
        assert!(!file.is_empty().unwrap());
    }

    file.set_len(50).unwrap();
    assert_eq!(file.len().unwrap(), 50);
    assert!(!file.is_empty().unwrap());

    file.set_len(150).unwrap();
    assert_eq!(file.len().unwrap(), 150);
}

/// File with `len` bytes, where every byte is derived from its offset, opened for direct I/O
fn file_with_bytes(len: usize) -> (tempfile::TempDir, DirectIoFile, Vec<u8>) {
    let tempdir = tempdir().unwrap();
    let file_path = tempdir.as_ref().join("file.bin");
    let bytes = (0..len)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    fs::write(&file_path, &bytes).unwrap();

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    let file = DirectIoFile::open(options, &file_path).unwrap();

    (tempdir, file, bytes)
}

/// Address of the beginning of the data region of the allocation of `buffer`
fn data_region<B>(buffer: &B) -> usize
where
    B: AlignedBuffer,
{
    buffer.as_ptr().addr() - buffer.offset() as usize
}

#[test]
fn read_at() {
    let page = OwnedPages::ALIGN as usize;
    let (_tempdir, file, bytes) = file_with_bytes(page * 3);

    let mut buffer = OwnedPages::EMPTY;
    for (offset, len) in [
        (0, 1),
        (0, page),
        (5, 50),
        (page - 1, 2),
        (page + 7, page),
        (page * 3 - 10, 10),
        (page * 2, page),
    ] {
        let read = file
            .read_at(buffer, len as u32, offset as u64)
            .unwrap_or_else(|error| panic!("Offset {offset}, len {len}: {error}"));
        assert_eq!(read.as_slice(), &bytes[offset..][..len], "Offset {offset}");
        assert_eq!(read.offset() as usize, offset % page, "Offset {offset}");
        assert!(data_region(&read).is_multiple_of(page), "Offset {offset}");
        assert!(read.allocation_alignment() >= OwnedPages::ALIGN);

        // The allocation is reused for the next read
        let allocation = data_region(&read);
        buffer = read.into_empty();
        assert_eq!(data_region(&buffer), allocation);
    }

    // Nothing to read
    let read = file.read_at(buffer, 0, 5).unwrap();
    assert!(read.is_empty());

    // Missing bytes, the buffer is returned
    let error = file
        .read_at(OwnedPages::EMPTY, 11, (page * 3 - 10) as u64)
        .unwrap_err();
    assert!(matches!(
        &error.error,
        DirectIoError::Io(error) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    // Too large for a buffer
    let error = file.read_at(error.buffer, u32::MAX, 1).unwrap_err();
    assert!(matches!(
        error.error,
        DirectIoError::AllocationFailed(AllocationError::CapacityOverflow)
    ));
}

#[test]
fn read_pages_at() {
    let page = OwnedPages::ALIGN as usize;
    let (_tempdir, file, bytes) = file_with_bytes(page * 3);

    let pages = file.read_pages_at(OwnedPages::EMPTY, 2, 1).unwrap();
    assert_eq!(pages.as_slice(), &bytes[page..][..page * 2]);
    let allocation = data_region(&pages);

    // Contents are replaced, the allocation is reused
    let pages = file.read_pages_at(pages, 1, 0).unwrap();
    assert_eq!(pages.as_slice(), &bytes[..page]);
    assert_eq!(data_region(&pages), allocation);

    // All pages must exist
    let error = file.read_pages_at(pages, 1, 3).unwrap_err();
    assert!(matches!(
        &error.error,
        DirectIoError::Io(error) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
    let error = file.read_pages_at(error.buffer, u32::MAX, 0).unwrap_err();
    assert!(matches!(
        error.error,
        DirectIoError::AllocationFailed(AllocationError::CapacityOverflow)
    ));
    let error = file.read_pages_at(error.buffer, 1, u64::MAX).unwrap_err();
    assert!(matches!(
        &error.error,
        DirectIoError::Io(error) if error.kind() == io::ErrorKind::InvalidInput
    ));
}

/// Buffer with `contents` at the same offset within a page as `offset`
fn buffer_at(contents: &[u8], offset: usize) -> OwnedAlignedBuffer<Align1> {
    let head = offset % OwnedPages::ALIGN as usize;
    let mut buffer = OwnedPages::with_capacity(u32::try_from(head + contents.len()).unwrap())
        .into_start_align::<Align1>();
    buffer.append(&vec![0; head]).unwrap();
    buffer.append(contents).unwrap();
    assert!(buffer.set_offset(u32::try_from(head).unwrap()));
    buffer
}

#[test]
fn write_at() {
    let page = OwnedPages::ALIGN as usize;
    let (_tempdir, file, mut bytes) = file_with_bytes(page * 3);

    for (offset, len) in [
        // Whole pages
        (page, page),
        (0, page * 2),
        // Unaligned beginning, end or both, within a page and across pages
        (5, page - 5),
        (page, 10),
        (page + 3, 10),
        (page - 3, page + 6),
        (page + 3, page * 2 - 3),
    ] {
        let contents = (0..len)
            .map(|index| (index % 239) as u8 ^ 0xff)
            .collect::<Vec<_>>();
        let buffer = file
            .write_at(buffer_at(&contents, offset), offset as u64)
            .unwrap_or_else(|error| panic!("Offset {offset}, len {len}: {error}"));
        assert_eq!(buffer.as_slice(), contents.as_slice());

        bytes[offset..][..len].copy_from_slice(&contents);
        let read = file
            .read_at(OwnedPages::EMPTY, bytes.len() as u32, 0)
            .unwrap();
        assert_eq!(
            read.as_slice(),
            bytes.as_slice(),
            "Offset {offset}, len {len}"
        );
    }

    // Buffer that was read can be written back at the same offset
    let read = file
        .read_at(OwnedPages::EMPTY, 100, page as u64 + 5)
        .unwrap();
    file.write_at(read, page as u64 + 5).unwrap();
    let read = file
        .read_at(OwnedPages::EMPTY, bytes.len() as u32, 0)
        .unwrap();
    assert_eq!(read.as_slice(), bytes.as_slice());

    // Contents must have the same offset within a page as the file offset
    let error = file
        .write_at(OwnedPages::from_bytes(&[1; 10]), 5)
        .unwrap_err();
    assert!(matches!(
        &error.error,
        DirectIoError::Io(error) if error.kind() == io::ErrorKind::InvalidInput
    ));
    assert_eq!(error.buffer.as_slice(), &[1; 10]);

    // Edge pages must exist in the file
    let error = file
        .write_at(buffer_at(&[1; 10], page * 3 - 2), (page * 3 - 2) as u64)
        .unwrap_err();
    assert!(matches!(
        &error.error,
        DirectIoError::Io(error) if error.kind() == io::ErrorKind::UnexpectedEof
    ));

    // Shared buffers can be written too
    let shared = OwnedPages::from_bytes(&bytes[..page]).into_shared();
    file.write_at(shared, 0).unwrap();
}
