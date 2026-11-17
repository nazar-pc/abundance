//! Elements of piece caches stored on disk.
//!
//! An element contains piece index, piece and checksum of both, followed by zero padding to whole
//! pages, such that elements can be written with direct I/O without read-modify-write. An element
//! that only contains zeroes is empty.

use ab_aligned_buffer::alignment::Align4096;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_direct_io_file::OwnedPages;
use std::ops::Range;
use tracing::debug;

/// Range of the piece within an element in bytes
const PIECE_RANGE: Range<u32> = {
    assert!(PieceIndex::SIZE + Piece::SIZE <= u32::MAX as usize);
    PieceIndex::SIZE as u32..(PieceIndex::SIZE + Piece::SIZE) as u32
};
/// Size of piece index, piece and checksum in bytes
const CONTENTS_SIZE: u32 = {
    let size = PieceIndex::SIZE + Piece::SIZE + Blake3Hash::SIZE;
    assert!(size <= u32::MAX as usize);
    size as u32
};
/// Size of a cache element in pages
pub(crate) const ELEMENT_PAGES: u32 = CONTENTS_SIZE.div_ceil(OwnedPages::ALIGN);
/// Size of a cache element in bytes, a whole number of pages
pub(crate) const ELEMENT_SIZE: u32 = ELEMENT_PAGES * OwnedPages::ALIGN;

/// Checksum of a cache element doesn't match its contents
#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("Checksum mismatch")]
pub(crate) struct ChecksumMismatch;

/// Encode an element with `piece` stored under `piece_index`
pub(crate) fn encode(piece_index: PieceIndex, piece: &Piece) -> OwnedPages {
    let mut element = OwnedPages::with_capacity(ELEMENT_SIZE);
    element
        .append(&piece_index.to_bytes())
        .expect("Capacity for the whole element is reserved above; qed");
    element
        .append(piece.as_ref())
        .expect("Capacity for the whole element is reserved above; qed");
    let checksum = blake3::hash(element.as_slice());
    element
        .append(checksum.as_bytes())
        .expect("Capacity for the whole element is reserved above; qed");
    element
        .pad_to_alignment::<Align4096>()
        .expect("Capacity for the whole element is reserved above; qed");
    element
}

/// Verify the checksum of an element and return the piece index stored in it, `None` means the
/// element is empty
pub(crate) fn verify_checksum(
    element: &[u8; ELEMENT_SIZE as usize],
) -> Result<Option<PieceIndex>, ChecksumMismatch> {
    let (piece_index_bytes, remaining_bytes) = element.split_at(PieceIndex::SIZE);
    let (piece_bytes, remaining_bytes) = remaining_bytes.split_at(Piece::SIZE);
    // The rest of the element after the checksum is zero padding
    let (expected_checksum, _padding) = remaining_bytes.split_at(Blake3Hash::SIZE);

    // Verify checksum
    let actual_checksum = {
        let mut hasher = blake3::Hasher::new();
        hasher.update(piece_index_bytes);
        hasher.update(piece_bytes);
        *hasher.finalize().as_bytes()
    };
    if actual_checksum != expected_checksum {
        if element.iter().all(|&byte| byte == 0) {
            return Ok(None);
        }

        debug!(
            actual_checksum = %hex::encode(actual_checksum),
            expected_checksum = %hex::encode(expected_checksum),
            "Hash doesn't match, corrupted piece in cache"
        );

        return Err(ChecksumMismatch);
    }

    let piece_index = PieceIndex::from_bytes(
        piece_index_bytes
            .try_into()
            .expect("Statically known to have correct size; qed"),
    );
    Ok(Some(piece_index))
}

/// Convert an element into the piece stored in it without copying, see [`verify_checksum()`] for
/// checking whether it contains one.
///
/// Returns `None` if the element is shorter than [`ELEMENT_SIZE`].
pub(crate) fn into_piece(element: OwnedPages) -> Option<Piece> {
    let piece = element.into_shared().get_slice(PIECE_RANGE)?;
    Piece::try_from(piece).ok()
}
