use crate::disk_piece_cache::{DiskPieceCache, DiskPieceCacheError, PieceCacheOffset};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_direct_io_file::AlignedPage;
use rand::prelude::*;
use std::num::NonZeroU32;
use std::{assert_matches, fs};
use tempfile::tempdir;

#[test]
fn basic() {
    let path = tempdir().unwrap();
    {
        let disk_piece_cache =
            DiskPieceCache::open(path.as_ref(), NonZeroU32::new(2).unwrap(), None, None).unwrap();

        // Initially empty
        assert_eq!(
            disk_piece_cache
                .contents()
                .filter(|(_offset, maybe_piece_index)| maybe_piece_index.is_some())
                .count(),
            0
        );

        // Write first piece into cache
        {
            let offset = PieceCacheOffset(0);
            let piece_index = PieceIndex::ZERO;
            let piece = {
                let mut piece = Piece::default();
                rand::rng().fill(piece.as_mut());
                piece
            };

            assert_eq!(disk_piece_cache.read_piece_index(offset).unwrap(), None);
            assert!(disk_piece_cache.read_piece(offset).unwrap().is_none());

            disk_piece_cache
                .write_piece(offset, piece_index, &piece)
                .unwrap();

            assert_eq!(
                disk_piece_cache.read_piece_index(offset).unwrap(),
                Some(piece_index)
            );
            assert!(disk_piece_cache.read_piece(offset).unwrap().is_some());
        }

        // One piece stored
        assert_eq!(
            disk_piece_cache
                .contents()
                .filter(|(_offset, maybe_piece_index)| maybe_piece_index.is_some())
                .count(),
            1
        );

        // Write second piece into cache
        {
            let offset = PieceCacheOffset(1);
            let piece_index = PieceIndex::from(10);
            let piece = {
                let mut piece = Piece::default();
                rand::rng().fill(piece.as_mut());
                piece
            };

            assert_eq!(disk_piece_cache.read_piece_index(offset).unwrap(), None);
            assert!(disk_piece_cache.read_piece(offset).unwrap().is_none());

            disk_piece_cache
                .write_piece(offset, piece_index, &piece)
                .unwrap();

            assert_eq!(
                disk_piece_cache.read_piece_index(offset).unwrap(),
                Some(piece_index)
            );
            assert!(disk_piece_cache.read_piece(offset).unwrap().is_some());
        }

        // Two pieces stored
        assert_eq!(
            disk_piece_cache
                .contents()
                .filter(|(_offset, maybe_piece_index)| maybe_piece_index.is_some())
                .count(),
            2
        );

        // Writing beyond capacity fails
        assert_matches!(
            disk_piece_cache.write_piece(PieceCacheOffset(2), PieceIndex::ZERO, &Piece::default()),
            Err(DiskPieceCacheError::OffsetOutsideOfRange { .. })
        );

        // Override works
        {
            let offset = PieceCacheOffset(0);
            let piece_index = PieceIndex::from(13);
            let piece = {
                let mut piece = Piece::default();
                rand::rng().fill(piece.as_mut());
                piece
            };

            disk_piece_cache
                .write_piece(offset, piece_index, &piece)
                .unwrap();

            assert_eq!(
                disk_piece_cache.read_piece_index(offset).unwrap(),
                Some(piece_index)
            );
            assert!(disk_piece_cache.read_piece(offset).unwrap().is_some());
        }
    }

    // Reopening works
    {
        let disk_piece_cache =
            DiskPieceCache::open(path.as_ref(), NonZeroU32::new(2).unwrap(), None, None).unwrap();
        // Two pieces stored
        assert_eq!(
            disk_piece_cache
                .contents()
                .filter(|(_offset, maybe_piece_index)| maybe_piece_index.is_some())
                .count(),
            2
        );
    }

    // Wiping works
    {
        DiskPieceCache::wipe(path.as_ref()).unwrap();

        let disk_piece_cache =
            DiskPieceCache::open(path.as_ref(), NonZeroU32::new(2).unwrap(), None, None).unwrap();
        // Wiped successfully
        assert_eq!(
            disk_piece_cache
                .contents()
                .filter(|(_offset, maybe_piece_index)| maybe_piece_index.is_some())
                .count(),
            0
        );
    }
}

#[test]
fn element_layout() {
    let element_size = usize::try_from(DiskPieceCache::element_size()).unwrap();
    assert_eq!(element_size % AlignedPage::SIZE, 0);

    let path = tempdir().unwrap();
    let disk_piece_cache =
        DiskPieceCache::open(path.as_ref(), NonZeroU32::new(2).unwrap(), None, None).unwrap();

    let offset = PieceCacheOffset(1);
    let piece_index = PieceIndex::from(10);
    let piece = {
        let mut piece = Piece::default();
        rand::rng().fill(piece.as_mut());
        piece
    };
    disk_piece_cache
        .write_piece(offset, piece_index, &piece)
        .unwrap();
    let (read_piece_index, read_piece) = disk_piece_cache.read_piece(offset).unwrap().unwrap();
    assert_eq!(read_piece_index, piece_index);
    assert_eq!(read_piece.as_ref(), piece.as_ref());

    // Second element starts right after the first one and is padded with zeroes
    let file_contents = fs::read(path.path().join(DiskPieceCache::FILE_NAME)).unwrap();
    assert_eq!(file_contents.len(), 2 * element_size);
    let element = &file_contents[element_size..];
    let (piece_index_bytes, remaining_bytes) = element.split_at(PieceIndex::SIZE);
    let (piece_bytes, remaining_bytes) = remaining_bytes.split_at(Piece::SIZE);
    let (checksum, padding) = remaining_bytes.split_at(Blake3Hash::SIZE);
    assert_eq!(piece_index_bytes, piece_index.to_bytes());
    assert_eq!(piece_bytes, piece.as_ref());
    assert_eq!(
        checksum,
        blake3::hash(&element[..PieceIndex::SIZE + Piece::SIZE]).as_bytes()
    );
    assert!(!padding.is_empty());
    assert!(padding.iter().all(|&byte| byte == 0));
}
