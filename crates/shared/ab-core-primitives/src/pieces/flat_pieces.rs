use crate::pieces::cow_bytes::CowBytes;
use crate::pieces::{Piece, PieceArray};
use crate::segments::RecordedHistorySegment;
use alloc::boxed::Box;
use bytes::{Bytes, BytesMut};
use core::ops::{Deref, DerefMut};
use core::{fmt, slice};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Flat representation of multiple pieces concatenated for more efficient for processing
#[derive(Clone, PartialEq, Eq)]
pub struct FlatPieces(CowBytes);

impl fmt::Debug for FlatPieces {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatPieces").finish_non_exhaustive()
    }
}

impl Deref for FlatPieces {
    type Target = [PieceArray];

    #[inline]
    fn deref(&self) -> &Self::Target {
        let bytes = self.0.as_ref();
        // SAFETY: Bytes slice has length of multiples of piece size and lifetimes of returned data
        // are preserved
        let pieces = unsafe {
            slice::from_raw_parts(
                bytes.as_ptr() as *const [u8; Piece::SIZE],
                bytes.len() / Piece::SIZE,
            )
        };
        PieceArray::slice_from_repr(pieces)
    }
}

impl DerefMut for FlatPieces {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        let bytes = self.0.as_mut();
        // SAFETY: Bytes slice has length of multiples of piece size and lifetimes of returned data
        // are preserved
        let pieces = unsafe {
            slice::from_raw_parts_mut(
                bytes.as_mut_ptr() as *mut [u8; Piece::SIZE],
                bytes.len() / Piece::SIZE,
            )
        };
        PieceArray::slice_mut_from_repr(pieces)
    }
}

impl FlatPieces {
    /// Allocate `FlatPieces` that will hold `piece_count` pieces filled with zeroes
    #[inline]
    pub fn new(piece_count: usize) -> Self {
        Self(CowBytes::Owned(BytesMut::zeroed(piece_count * Piece::SIZE)))
    }

    /// Iterate over all pieces.
    ///
    /// NOTE: Unless [`Self::to_shared`] was called first, iterator may have to allocate each piece
    /// from scratch, which is rarely a desired behavior.
    #[inline]
    pub fn pieces(&self) -> Box<dyn ExactSizeIterator<Item = Piece> + '_> {
        match &self.0 {
            CowBytes::Shared(bytes) => Box::new(
                bytes
                    .chunks_exact(Piece::SIZE)
                    .map(|slice| Piece(CowBytes::Shared(bytes.slice_ref(slice)))),
            ),
            CowBytes::Owned(bytes) => Box::new(
                bytes
                    .chunks_exact(Piece::SIZE)
                    .map(|slice| Piece(CowBytes::Shared(Bytes::copy_from_slice(slice)))),
            ),
        }
    }

    /// Iterator over source pieces (even indices)
    #[inline]
    pub fn source_pieces(&self) -> impl ExactSizeIterator<Item = Piece> + '_ {
        self.pieces().take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Iterator over source pieces (even indices)
    #[inline]
    pub fn source(&self) -> impl ExactSizeIterator<Item = &'_ PieceArray> + '_ {
        self.iter().take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable iterator over source pieces (even indices)
    #[inline]
    pub fn source_mut(&mut self) -> impl ExactSizeIterator<Item = &'_ mut PieceArray> + '_ {
        self.iter_mut()
            .take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Iterator over parity pieces (odd indices)
    #[inline]
    pub fn parity_pieces(&self) -> impl ExactSizeIterator<Item = Piece> + '_ {
        self.pieces().skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Iterator over parity pieces (odd indices)
    #[inline]
    pub fn parity(&self) -> impl ExactSizeIterator<Item = &'_ PieceArray> + '_ {
        self.iter().skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable iterator over parity pieces (odd indices)
    #[inline]
    pub fn parity_mut(&mut self) -> impl ExactSizeIterator<Item = &'_ mut PieceArray> + '_ {
        self.iter_mut()
            .skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Ensure flat pieces contains cheaply cloneable shared data.
    ///
    /// Internally flat pieces uses CoW mechanism and can store either mutable owned data or data
    /// that is cheap to clone, calling this method will ensure further clones and returned pieces
    /// will not result in additional memory allocations.
    pub fn to_shared(self) -> Self {
        Self(match self.0 {
            CowBytes::Shared(bytes) => CowBytes::Shared(bytes),
            CowBytes::Owned(bytes) => CowBytes::Shared(bytes.freeze()),
        })
    }
}

#[cfg(feature = "parallel")]
impl FlatPieces {
    /// Parallel iterator over source pieces (even indices)
    #[inline]
    pub fn par_source(&self) -> impl IndexedParallelIterator<Item = &'_ PieceArray> + '_ {
        self.par_iter()
            .take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable parallel iterator over source pieces (even indices)
    #[inline]
    pub fn par_source_mut(
        &mut self,
    ) -> impl IndexedParallelIterator<Item = &'_ mut PieceArray> + '_ {
        self.par_iter_mut()
            .take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Parallel iterator over parity pieces (odd indices)
    #[inline]
    pub fn par_parity(&self) -> impl IndexedParallelIterator<Item = &'_ PieceArray> + '_ {
        self.par_iter()
            .skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable parallel iterator over parity pieces (odd indices)
    #[inline]
    pub fn par_parity_mut(
        &mut self,
    ) -> impl IndexedParallelIterator<Item = &'_ mut PieceArray> + '_ {
        self.par_iter_mut()
            .skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }
}
