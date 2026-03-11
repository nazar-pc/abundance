use ab_core_primitives::pieces::{
    Piece, PieceHeader, PiecePosition, Record, RecordChunksRoot, RecordProof, SegmentProof,
};
use ab_core_primitives::segments::{
    ArchivedHistorySegment, LocalSegmentIndex, RecordedHistorySegment, SegmentPosition,
    SegmentRoot, SuperSegmentIndex,
};
use ab_core_primitives::shard::ShardIndex;
use ab_erasure_coding::{ErasureCoding, ErasureCodingError, RecoveryShardState};
use ab_merkle_tree::balanced::BalancedMerkleTree;
use alloc::vec::Vec;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Reconstructor-related instantiation error
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReconstructorError {
    /// Segment size is not bigger than record size
    #[error("Error during data shards reconstruction: {0}")]
    DataShardsReconstruction(#[from] ErasureCodingError),
    /// Not enough shards
    #[error("Not enough shards: {num_shards}")]
    NotEnoughShards { num_shards: usize },
}

struct SharedPieceDetails {
    shard_index: ShardIndex,
    local_segment_index: LocalSegmentIndex,
    super_segment_index: SuperSegmentIndex,
    segment_position: SegmentPosition,
    segment_root: SegmentRoot,
    segment_proof: SegmentProof,
}

/// Piece reconstructor helps to reconstruct missing pieces.
#[derive(Debug, Clone)]
pub struct PiecesReconstructor {
    /// Erasure coding data structure
    erasure_coding: ErasureCoding,
}

impl PiecesReconstructor {
    /// Create a new instance
    pub fn new(erasure_coding: ErasureCoding) -> Self {
        Self { erasure_coding }
    }

    fn reconstruct_shards(
        &self,
        input_pieces: &[Option<Piece>],
    ) -> Result<ArchivedHistorySegment, ReconstructorError> {
        if input_pieces.len() < ArchivedHistorySegment::NUM_PIECES {
            return Err(ReconstructorError::NotEnoughShards {
                num_shards: input_pieces.len(),
            });
        }
        let mut reconstructed_pieces = ArchivedHistorySegment::default();

        // TODO: Fix up piece metadata
        let mut shared_piece_details = None;
        {
            let (source_input_pieces, parity_input_pieces) =
                input_pieces.split_at(RecordedHistorySegment::NUM_RAW_RECORDS);
            let (source_reconstructed_pieces, parity_reconstructed_pieces) =
                reconstructed_pieces.split_at_mut(RecordedHistorySegment::NUM_RAW_RECORDS);

            let source = source_input_pieces
                .iter()
                .zip(source_reconstructed_pieces)
                .map(
                    |(maybe_input_piece, output_piece)| match maybe_input_piece {
                        Some(input_piece) => {
                            if shared_piece_details.is_none() {
                                shared_piece_details.replace(SharedPieceDetails {
                                    shard_index: input_piece.header.shard_index.as_inner(),
                                    local_segment_index: input_piece
                                        .header
                                        .local_segment_index
                                        .as_inner(),
                                    super_segment_index: input_piece
                                        .header
                                        .super_segment_index
                                        .as_inner(),
                                    segment_position: input_piece
                                        .header
                                        .segment_position
                                        .as_inner(),
                                    segment_root: input_piece.header.segment_root,
                                    segment_proof: input_piece.header.segment_proof,
                                });
                            }
                            // Fancy way to insert value to avoid going through stack (if naive
                            // dereferencing is used) and potentially causing stack overflow as the
                            // result
                            output_piece.record.copy_from_slice(&*input_piece.record);
                            RecoveryShardState::Present(input_piece.record.as_flattened())
                        }
                        None => RecoveryShardState::MissingRecover(
                            output_piece.record.as_flattened_mut(),
                        ),
                    },
                );
            let parity = parity_input_pieces
                .iter()
                .zip(parity_reconstructed_pieces.iter_mut())
                .map(
                    |(maybe_input_piece, output_piece)| match maybe_input_piece {
                        Some(input_piece) => {
                            // Fancy way to insert value to avoid going through stack (if naive
                            // dereferencing is used) and potentially causing stack overflow as the
                            // result
                            output_piece.record.copy_from_slice(&*input_piece.record);
                            RecoveryShardState::Present(input_piece.record.as_flattened())
                        }
                        None => RecoveryShardState::MissingRecover(
                            output_piece.record.as_flattened_mut(),
                        ),
                    },
                );
            self.erasure_coding.recover(source, parity)?;
        }
        let SharedPieceDetails {
            shard_index,
            local_segment_index,
            super_segment_index,
            segment_position,
            segment_root,
            segment_proof,
        } = shared_piece_details.expect(
            "Sucessful recovery means there was at least one piece to fill this Option; qed",
        );

        let record_roots = {
            #[cfg(not(feature = "parallel"))]
            let iter = reconstructed_pieces.iter_mut().zip(input_pieces);
            #[cfg(feature = "parallel")]
            let iter = reconstructed_pieces.par_iter_mut().zip_eq(input_pieces);

            iter.map(|(piece, maybe_input_piece)| {
                let (record_root, parity_chunks_root) = if let Some(input_piece) = maybe_input_piece
                {
                    (
                        *input_piece.record_root(),
                        *input_piece.header.parity_chunks_root,
                    )
                } else {
                    // TODO: Reuse allocations between iterations
                    let [source_chunks_root, parity_chunks_root] = {
                        let mut parity_chunks = Record::new_boxed();

                        self.erasure_coding
                            .extend(piece.record.iter(), parity_chunks.iter_mut())?;

                        let source_chunks_root = *piece.record.source_chunks_root();
                        let parity_chunks_root =
                            BalancedMerkleTree::compute_root_only(&parity_chunks);

                        [source_chunks_root, parity_chunks_root]
                    };

                    let record_root =
                        BalancedMerkleTree::new(&[source_chunks_root, parity_chunks_root]).root();

                    (record_root, parity_chunks_root)
                };

                piece.header.parity_chunks_root = RecordChunksRoot::from(parity_chunks_root);

                Ok::<_, ReconstructorError>(record_root)
            })
            .collect::<Result<Vec<_>, _>>()?
        };

        let segment_merkle_tree =
            BalancedMerkleTree::<{ ArchivedHistorySegment::NUM_PIECES }>::new_boxed(
                record_roots
                    .as_slice()
                    .try_into()
                    .expect("Statically guaranteed to have correct length; qed"),
            );

        reconstructed_pieces
            .iter_mut()
            .zip(segment_merkle_tree.all_proofs())
            .for_each(|(piece, record_proof)| {
                piece.header = PieceHeader {
                    shard_index: shard_index.into(),
                    local_segment_index: local_segment_index.into(),
                    super_segment_index: super_segment_index.into(),
                    segment_position: segment_position.into(),
                    segment_root,
                    segment_proof,
                    parity_chunks_root: piece.header.parity_chunks_root,
                    record_proof: RecordProof::from(record_proof),
                };
            });

        Ok(reconstructed_pieces)
    }

    /// Returns all the pieces for a segment using a given set of pieces of a segment of the
    /// archived history.
    ///
    /// Any half of all pieces are required to be present, the rest will be recovered automatically
    /// due to use of erasure coding if needed.
    pub fn reconstruct_segment(
        &self,
        segment_pieces: &[Option<Piece>],
    ) -> Result<ArchivedHistorySegment, ReconstructorError> {
        let pieces = self.reconstruct_shards(segment_pieces)?;

        Ok(pieces.to_shared())
    }

    /// Returns the missing piece for a segment using given set of pieces of a segment of the
    /// archived history (any half of all pieces are required to be present).
    pub fn reconstruct_piece(
        &self,
        segment_pieces: &[Option<Piece>],
        piece_position: PiecePosition,
    ) -> Result<Piece, ReconstructorError> {
        // TODO: Early exit if position already exists and doesn't need reconstruction
        // TODO: It is now inefficient to recover all shards if only one piece is needed, especially
        //  source piece
        let pieces = self.reconstruct_shards(segment_pieces)?;

        let piece = Piece::from(&pieces[piece_position]);

        Ok(piece.to_shared())
    }
}
