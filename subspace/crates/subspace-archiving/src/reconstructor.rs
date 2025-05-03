use crate::archiver::{Segment, SegmentItem};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::pieces::Piece;
use ab_core_primitives::segments::{
    ArchivedHistorySegment, LastArchivedBlock, RecordedHistorySegment, SegmentHeader, SegmentIndex,
};
use ab_erasure_coding::{ErasureCoding, ErasureCodingError, RecoveryShardState};
use alloc::vec::Vec;
use core::mem;
use parity_scale_codec::Decode;

/// Reconstructor-related instantiation error
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReconstructorError {
    /// Error during data shards reconstruction
    #[error("Error during data shards reconstruction: {0}")]
    DataShardsReconstruction(#[from] ErasureCodingError),
    /// Not enough shards
    #[error("Not enough shards: {num_shards}")]
    NotEnoughShards { num_shards: usize },
    /// Segment size is not bigger than record size
    #[error("Error during segment decoding: {0}")]
    SegmentDecoding(parity_scale_codec::Error),
    /// Invalid padding
    #[error("Invalid padding")]
    InvalidPadding,
    /// Incorrect segment order, each next segment must have monotonically increasing segment index
    #[error(
        "Incorrect segment order, expected index {expected_segment_index}, actual \
        {actual_segment_index}"
    )]
    IncorrectSegmentOrder {
        expected_segment_index: SegmentIndex,
        actual_segment_index: SegmentIndex,
    },
}

/// Data structure that contains information reconstructed from given segment (potentially using
/// information from segments that were added previously)
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct ReconstructedContents {
    /// Segment header stored in a segment
    pub segment_header: Option<SegmentHeader>,
    /// Reconstructed encoded blocks with their block numbers
    pub blocks: Vec<(BlockNumber, Vec<u8>)>,
}

/// Reconstructor helps to retrieve blocks from archived pieces.
#[derive(Debug, Clone)]
pub struct Reconstructor {
    /// Erasure coding data structure
    erasure_coding: ErasureCoding,
    /// Index of last segment added to reconstructor
    last_segment_index: Option<SegmentIndex>,
    /// Partially reconstructed block waiting for more data
    partial_block: Option<Vec<u8>>,
}

impl Reconstructor {
    /// Create a new instance
    pub fn new(erasure_coding: ErasureCoding) -> Self {
        Self {
            erasure_coding,
            last_segment_index: None,
            partial_block: None,
        }
    }

    /// Given a set of pieces of a segment of the archived history (any half of all pieces are
    /// required to be present, the rest will be recovered automatically due to use of erasure
    /// coding if needed), reconstructs and returns the segment itself.
    ///
    /// Does not modify the internal state of the reconstructor.
    pub fn reconstruct_segment(
        &self,
        segment_pieces: &[Option<Piece>],
    ) -> Result<Segment, ReconstructorError> {
        if segment_pieces.len() < ArchivedHistorySegment::NUM_PIECES {
            return Err(ReconstructorError::NotEnoughShards {
                num_shards: segment_pieces.len(),
            });
        }
        let mut segment_data = RecordedHistorySegment::new_boxed();

        if !segment_pieces
            .iter()
            .zip(segment_data.iter_mut())
            .all(|(maybe_piece, record)| {
                if let Some(piece) = maybe_piece {
                    record.copy_from_slice(piece.record().as_slice());
                    true
                } else {
                    false
                }
            })
        {
            // If not all data pieces are available, need to reconstruct data shards using erasure
            // coding.

            let (source_segment_pieces, parity_segment_pieces) =
                segment_pieces.split_at(RecordedHistorySegment::NUM_RAW_RECORDS);
            let source = segment_data.iter_mut().zip(source_segment_pieces).map(
                |(output_record, maybe_source_piece)| match maybe_source_piece {
                    Some(input_piece) => {
                        output_record.copy_from_slice(input_piece.record().as_slice());
                        RecoveryShardState::Present(input_piece.record().as_flattened())
                    }
                    None => RecoveryShardState::MissingRecover(output_record.as_flattened_mut()),
                },
            );
            let parity =
                parity_segment_pieces
                    .iter()
                    .map(|maybe_source_piece| match maybe_source_piece {
                        Some(input_piece) => {
                            RecoveryShardState::Present(input_piece.record().as_flattened())
                        }
                        None => RecoveryShardState::MissingIgnore,
                    });
            self.erasure_coding.recover(source, parity)?;
        }

        let segment = Segment::decode(&mut AsRef::<[u8]>::as_ref(segment_data.as_ref()))
            .map_err(ReconstructorError::SegmentDecoding)?;

        Ok(segment)
    }

    /// Given a set of pieces of a segment of the archived history (any half of all pieces are
    /// required to be present, the rest will be recovered automatically due to use of erasure
    /// coding if needed), reconstructs and returns segment header and a list of encoded blocks with
    /// corresponding block numbers.
    ///
    /// It is possible to start with any segment, but when next segment is pushed, it needs to
    /// follow the previous one or else error will be returned.
    pub fn add_segment(
        &mut self,
        segment_pieces: &[Option<Piece>],
    ) -> Result<ReconstructedContents, ReconstructorError> {
        let segment = self.reconstruct_segment(segment_pieces)?;

        let mut reconstructed_contents = ReconstructedContents::default();
        let mut next_block_number = BlockNumber::ZERO;
        let mut partial_block = self.partial_block.take().unwrap_or_default();

        for segment_item in segment.items {
            match segment_item {
                SegmentItem::Padding => {
                    // Doesn't contain anything
                }
                SegmentItem::Block { bytes, .. } => {
                    if !partial_block.is_empty() {
                        reconstructed_contents
                            .blocks
                            .push((next_block_number, mem::take(&mut partial_block)));

                        next_block_number += BlockNumber::ONE;
                    }

                    reconstructed_contents
                        .blocks
                        .push((next_block_number, bytes));

                    next_block_number += BlockNumber::ONE;
                }
                SegmentItem::BlockStart { bytes, .. } => {
                    if !partial_block.is_empty() {
                        reconstructed_contents
                            .blocks
                            .push((next_block_number, mem::take(&mut partial_block)));

                        next_block_number += BlockNumber::ONE;
                    }

                    partial_block = bytes;
                }
                SegmentItem::BlockContinuation { bytes, .. } => {
                    if partial_block.is_empty() {
                        // This is continuation from previous segment, we don't have the beginning
                        // of the block to continue.
                        continue;
                    }

                    partial_block.extend_from_slice(&bytes);
                }
                SegmentItem::ParentSegmentHeader(segment_header) => {
                    let segment_index = segment_header.segment_index;

                    if let Some(last_segment_index) = self.last_segment_index {
                        if last_segment_index != segment_index {
                            return Err(ReconstructorError::IncorrectSegmentOrder {
                                expected_segment_index: last_segment_index + SegmentIndex::ONE,
                                actual_segment_index: segment_index + SegmentIndex::ONE,
                            });
                        }
                    }

                    self.last_segment_index
                        .replace(segment_index + SegmentIndex::ONE);

                    let LastArchivedBlock {
                        number,
                        archived_progress,
                        padding,
                    } = segment_header.last_archived_block;

                    if padding != [0; _] {
                        return Err(ReconstructorError::InvalidPadding);
                    }

                    reconstructed_contents
                        .segment_header
                        .replace(segment_header);

                    match archived_progress.partial() {
                        None => {
                            reconstructed_contents
                                .blocks
                                .push((next_block_number, mem::take(&mut partial_block)));

                            next_block_number = number + BlockNumber::ONE;
                        }
                        Some(_bytes) => {
                            next_block_number = number;

                            if partial_block.is_empty() {
                                // Will not be able to recover full block, bump right away.
                                next_block_number += BlockNumber::ONE;
                            }
                        }
                    }
                }
            }
        }

        if !partial_block.is_empty() {
            self.partial_block.replace(partial_block);
        }

        if self.last_segment_index.is_none() {
            self.last_segment_index.replace(SegmentIndex::ZERO);
        }

        Ok(reconstructed_contents)
    }
}
