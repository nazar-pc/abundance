use crate::pieces::{InnerPiece, Piece};
use crate::segments::RecordedHistorySegment;
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::trivial_type::TrivialType;
use alloc::boxed::Box;
use core::fmt;
use core::ops::{Deref, DerefMut};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Buffer that is either owned and mutable or shared and cheap to clone
#[derive(Clone)]
enum CowBuffer {
    Owned(OwnedAlignedBuffer),
    Shared(SharedAlignedBuffer),
}

impl CowBuffer {
    #[inline(always)]
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Owned(buffer) => buffer.as_slice(),
            Self::Shared(buffer) => buffer.as_slice(),
        }
    }

    /// Shared buffer is copied into a new allocation first, unless it is the last reference to
    /// its allocation
    #[inline(always)]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        match self {
            Self::Owned(buffer) => buffer.as_mut_slice(),
            Self::Shared(buffer) => buffer.make_mut(),
        }
    }
}

/// Flat representation of multiple pieces concatenated for more efficient for processing
#[derive(Clone)]
pub struct FlatPieces(CowBuffer);

impl PartialEq for FlatPieces {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice() == other.0.as_slice()
    }
}

impl Eq for FlatPieces {}

impl fmt::Debug for FlatPieces {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatPieces").finish_non_exhaustive()
    }
}

impl Deref for FlatPieces {
    type Target = [InnerPiece];

    #[inline]
    fn deref(&self) -> &Self::Target {
        let bytes = self.0.as_slice();
        let (pieces, _) = InnerPiece::slice_from_bytes(bytes, bytes.len() / Piece::SIZE).expect(
            "`InnerPiece` has alignment of 1 and `bytes` are at least this many pieces long; qed",
        );
        pieces
    }
}

impl DerefMut for FlatPieces {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        let bytes = self.0.as_mut_slice();
        let num_pieces = bytes.len() / Piece::SIZE;
        let (pieces, _) = InnerPiece::slice_from_bytes_mut(bytes, num_pieces).expect(
            "`InnerPiece` has alignment of 1 and `bytes` are at least this many pieces long; qed",
        );
        pieces
    }
}

impl FlatPieces {
    /// Allocate `FlatPieces` that will hold `piece_count` pieces filled with zeroes.
    ///
    /// # Panics
    /// If pieces don't fit into [`OwnedAlignedBuffer::MAX_CAPACITY`]
    #[inline]
    pub fn new(piece_count: usize) -> Self {
        let size = piece_count
            .checked_mul(Piece::SIZE)
            .and_then(|size| u32::try_from(size).ok())
            .unwrap_or_else(|| panic!("{piece_count} pieces don't fit into a buffer"));

        Self(CowBuffer::Owned(OwnedAlignedBuffer::zeroed(size)))
    }

    /// Iterate over all pieces.
    ///
    /// NOTE: [`Piece`] returned by iterator is sharing memory with the original [`FlatPieces`]
    /// allocation, which is cheap, but may result in additional memory usage if the piece is kept
    /// around for a long time. But it only happens if [`Self::to_shared`] was called first,
    /// otherwise iterator may have to allocate each piece from scratch, which is rarely a
    /// desired behavior.
    #[inline]
    pub fn pieces(&self) -> Box<dyn ExactSizeIterator<Item = Piece> + '_> {
        match &self.0 {
            CowBuffer::Shared(buffer) => {
                Box::new((0..buffer.len()).step_by(Piece::SIZE).map(|start| {
                    Piece(
                        buffer
                            .get_slice(start..start + Piece::SIZE_U32)
                            .expect("Buffer contains whole pieces; qed"),
                    )
                }))
            }
            CowBuffer::Owned(buffer) => Box::new(
                buffer
                    .as_chunks::<{ Piece::SIZE }>()
                    .0
                    .iter()
                    .map(|bytes| Piece(SharedAlignedBuffer::from_bytes(bytes))),
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
    pub fn source(&self) -> impl ExactSizeIterator<Item = &'_ InnerPiece> + '_ {
        self.iter().take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable iterator over source pieces (even indices)
    #[inline]
    pub fn source_mut(&mut self) -> impl ExactSizeIterator<Item = &'_ mut InnerPiece> + '_ {
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
    pub fn parity(&self) -> impl ExactSizeIterator<Item = &'_ InnerPiece> + '_ {
        self.iter().skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable iterator over parity pieces (odd indices)
    #[inline]
    pub fn parity_mut(&mut self) -> impl ExactSizeIterator<Item = &'_ mut InnerPiece> + '_ {
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
            CowBuffer::Shared(buffer) => CowBuffer::Shared(buffer),
            CowBuffer::Owned(buffer) => CowBuffer::Shared(buffer.into_shared()),
        })
    }

    /// Parallel iterator over source pieces (even indices)
    #[inline]
    #[cfg(feature = "parallel")]
    pub fn par_source(&self) -> impl IndexedParallelIterator<Item = &'_ InnerPiece> + '_ {
        self.par_iter()
            .take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable parallel iterator over source pieces (even indices)
    #[inline]
    #[cfg(feature = "parallel")]
    pub fn par_source_mut(
        &mut self,
    ) -> impl IndexedParallelIterator<Item = &'_ mut InnerPiece> + '_ {
        self.par_iter_mut()
            .take(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Parallel iterator over parity pieces (odd indices)
    #[inline]
    #[cfg(feature = "parallel")]
    pub fn par_parity(&self) -> impl IndexedParallelIterator<Item = &'_ InnerPiece> + '_ {
        self.par_iter()
            .skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }

    /// Mutable parallel iterator over parity pieces (odd indices)
    #[inline]
    #[cfg(feature = "parallel")]
    pub fn par_parity_mut(
        &mut self,
    ) -> impl IndexedParallelIterator<Item = &'_ mut InnerPiece> + '_ {
        self.par_iter_mut()
            .skip(RecordedHistorySegment::NUM_RAW_RECORDS)
    }
}
