use crate::{DirectIoFile, MAX_READ_SIZE};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{RngCore, SeedableRng};
use std::fs;
use std::fs::OpenOptions;
use tempfile::tempdir;

#[test]
fn read_write_small() {
    read_write_inner::<15000>(&[
        (0_usize, 512_usize),
        (0_usize, 4096_usize),
        (0, 500),
        (0, 4000),
        (5, 50),
        (12, 500),
        (96, 4000),
        (4000, 96),
        (10000, 5),
    ])
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
    ])
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

    // TODO: Not supported under Miri: https://github.com/rust-lang/miri/issues/4464
    if !cfg!(miri) {
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
