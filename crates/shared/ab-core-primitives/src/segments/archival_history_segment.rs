use crate::pieces::{FlatPieces, InnerPiece, Piece, PiecePosition, Record};
use crate::segments::RecordedHistorySegment;
use derive_more::{Deref, DerefMut};
use std::ops::{Index, IndexMut};
use std::{array, mem};

/// Archived history segment after archiving is applied.
#[derive(Debug, Clone, Eq, PartialEq, Deref, DerefMut)]
#[repr(transparent)]
pub struct ArchivedHistorySegment(FlatPieces);

impl AsRef<[InnerPiece; Self::NUM_PIECES]> for ArchivedHistorySegment {
    #[inline(always)]
    fn as_ref(&self) -> &[InnerPiece; Self::NUM_PIECES] {
        self.0
            .as_ref()
            .try_into()
            .expect("Constructor always produces correct length; qed")
    }
}

impl AsMut<[InnerPiece; Self::NUM_PIECES]> for ArchivedHistorySegment {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [InnerPiece; Self::NUM_PIECES] {
        self.0
            .as_mut()
            .try_into()
            .expect("Constructor always produces correct length; qed")
    }
}

impl AsRef<[[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2]> for ArchivedHistorySegment {
    #[inline(always)]
    fn as_ref(&self) -> &[[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2] {
        const {
            assert!(
                RecordedHistorySegment::NUM_PIECES == RecordedHistorySegment::NUM_RAW_RECORDS * 2
            );
        }
        // SAFETY: The same size and layout
        unsafe {
            mem::transmute::<
                &[InnerPiece; Self::NUM_PIECES],
                &[[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2],
            >(self.as_ref())
        }
    }
}

impl AsMut<[[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2]> for ArchivedHistorySegment {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2] {
        const {
            assert!(
                RecordedHistorySegment::NUM_PIECES == RecordedHistorySegment::NUM_RAW_RECORDS * 2
            );
        }
        // SAFETY: The same size and layout
        unsafe {
            mem::transmute::<
                &mut [InnerPiece; Self::NUM_PIECES],
                &mut [[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2],
            >(self.as_mut())
        }
    }
}

impl Default for ArchivedHistorySegment {
    #[inline]
    fn default() -> Self {
        Self(FlatPieces::new(Self::NUM_PIECES))
    }
}

impl Index<PiecePosition> for ArchivedHistorySegment {
    type Output = InnerPiece;

    fn index(&self, index: PiecePosition) -> &Self::Output {
        // SAFETY: The size of the archived history segment is known and protected invariant
        unsafe { self.get_unchecked(usize::from(index)) }
    }
}

impl IndexMut<PiecePosition> for ArchivedHistorySegment {
    fn index_mut(&mut self, index: PiecePosition) -> &mut Self::Output {
        // SAFETY: The size of the archived history segment is known and protected invariant
        unsafe { self.get_unchecked_mut(usize::from(index)) }
    }
}

impl ArchivedHistorySegment {
    /// All records of this segment, split into source and parity halves
    #[inline(always)]
    pub fn split_records_mut(
        &mut self,
    ) -> (
        [&mut Record; RecordedHistorySegment::NUM_RAW_RECORDS],
        [&mut Record; RecordedHistorySegment::NUM_RAW_RECORDS],
    ) {
        let [source, parity]: &mut [[_; RecordedHistorySegment::NUM_RAW_RECORDS]; 2] =
            self.as_mut();
        let mut source = source.iter_mut().map(|piece| &mut piece.record);
        let mut parity = parity.iter_mut().map(|piece| &mut piece.record);

        (
            array::from_fn(|_| {
                source
                    .next()
                    .expect("Number of pieces matches the array size; qed")
            }),
            array::from_fn(|_| {
                parity
                    .next()
                    .expect("Number of pieces matches the array size; qed")
            }),
        )
    }

    /// Number of pieces in one segment of archived history.
    pub const NUM_PIECES: usize = RecordedHistorySegment::NUM_PIECES;
    /// Size of archived history segment in bytes.
    ///
    /// It includes erasure coded [`InnerPiece`]s (both source and parity) that are
    /// composed of [`crate::pieces::Record`]s together with corresponding roots and
    /// proofs.
    pub const SIZE: usize = Piece::SIZE * Self::NUM_PIECES;

    /// Ensure archived history segment contains cheaply cloneable shared data.
    ///
    /// Internally archived history segment uses CoW mechanism and can store either mutable owned
    /// data or data that is cheap to clone, calling this method will ensure further clones and
    /// returned pieces will not result in additional memory allocations.
    pub fn to_shared(self) -> Self {
        Self(self.0.to_shared())
    }
}
