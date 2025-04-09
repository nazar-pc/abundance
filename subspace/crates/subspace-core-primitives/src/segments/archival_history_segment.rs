use crate::pieces::{FlatPieces, Piece};
use crate::segments::RecordedHistorySegment;
use derive_more::{Deref, DerefMut};

/// Archived history segment after archiving is applied.
#[derive(Debug, Clone, Eq, PartialEq, Deref, DerefMut)]
#[repr(transparent)]
pub struct ArchivedHistorySegment(FlatPieces);

impl Default for ArchivedHistorySegment {
    #[inline]
    fn default() -> Self {
        Self(FlatPieces::new(Self::NUM_PIECES))
    }
}

impl ArchivedHistorySegment {
    /// Number of pieces in one segment of archived history.
    pub const NUM_PIECES: usize = RecordedHistorySegment::NUM_PIECES;
    /// Size of archived history segment in bytes.
    ///
    /// It includes erasure coded [`crate::pieces::PieceArray`]s (both source and parity) that are
    /// composed of [`crate::pieces::Record`]s together with corresponding commitments and
    /// witnesses.
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
