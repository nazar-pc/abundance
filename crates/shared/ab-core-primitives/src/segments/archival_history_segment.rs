use crate::pieces::{FlatPieces, InnerPiece, Piece, PiecePosition, Record};
use crate::segments::RecordedHistorySegment;
use alloc::boxed::Box;
use array_reshape::Unflatten;
use std::array;
use std::ops::{Deref, DerefMut, Index, IndexMut};

/// Archived history segment after archiving is applied.
///
/// Dereferences to a fixed-size array of pieces, such that the number of pieces can't be changed.
#[derive(Debug, Clone, Eq, PartialEq)]
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
        let pieces: &[InnerPiece; Self::NUM_PIECES] = self.as_ref();
        pieces.unflatten_ref()
    }
}

impl AsMut<[[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2]> for ArchivedHistorySegment {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [[InnerPiece; RecordedHistorySegment::NUM_RAW_RECORDS]; 2] {
        let pieces: &mut [InnerPiece; Self::NUM_PIECES] = self.as_mut();
        pieces.unflatten_mut()
    }
}

impl Deref for ArchivedHistorySegment {
    type Target = [InnerPiece; Self::NUM_PIECES];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl DerefMut for ArchivedHistorySegment {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl Default for ArchivedHistorySegment {
    #[inline]
    fn default() -> Self {
        Self(FlatPieces::new(Self::NUM_PIECES))
    }
}

const {
    // Indexing with `PiecePosition` (a wrapper around `u8`) relies on every possible position
    // having a corresponding piece
    assert!(ArchivedHistorySegment::NUM_PIECES == usize::from(u8::MAX) + 1);
}

impl Index<PiecePosition> for ArchivedHistorySegment {
    type Output = InnerPiece;

    #[inline(always)]
    fn index(&self, index: PiecePosition) -> &Self::Output {
        self.get(usize::from(index)).expect(
            "`PiecePosition` wraps `u8` and there are `u8::MAX + 1` pieces, checked above; qed",
        )
    }
}

impl IndexMut<PiecePosition> for ArchivedHistorySegment {
    #[inline(always)]
    fn index_mut(&mut self, index: PiecePosition) -> &mut Self::Output {
        self.get_mut(usize::from(index)).expect(
            "`PiecePosition` wraps `u8` and there are `u8::MAX + 1` pieces, checked above; qed",
        )
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

    /// Iterator over all pieces, see [`FlatPieces::pieces()`] for details
    #[inline]
    pub fn pieces(&self) -> Box<dyn ExactSizeIterator<Item = Piece> + '_> {
        self.0.pieces()
    }

    /// Iterator over source pieces, see [`FlatPieces::source_pieces()`] for details
    #[inline]
    pub fn source_pieces(&self) -> impl ExactSizeIterator<Item = Piece> + '_ {
        self.0.source_pieces()
    }

    /// Iterator over parity pieces, see [`FlatPieces::parity_pieces()`] for details
    #[inline]
    pub fn parity_pieces(&self) -> impl ExactSizeIterator<Item = Piece> + '_ {
        self.0.parity_pieces()
    }

    /// Ensure archived history segment contains cheaply cloneable shared data.
    ///
    /// Internally archived history segment uses CoW mechanism and can store either mutable owned
    /// data or data that is cheap to clone, calling this method will ensure further clones and
    /// returned pieces will not result in additional memory allocations.
    pub fn to_shared(self) -> Self {
        Self(self.0.to_shared())
    }
}
