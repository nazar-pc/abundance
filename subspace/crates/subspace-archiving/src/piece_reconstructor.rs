use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use alloc::boxed::Box;
use alloc::vec::Vec;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use subspace_core_primitives::pieces::{Piece, Record};
use subspace_core_primitives::segments::{ArchivedHistorySegment, RecordedHistorySegment};
use subspace_erasure_coding::{ErasureCoding, ErasureCodingError, RecoveryShardState};

/// Reconstructor-related instantiation error
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReconstructorError {
    /// Segment size is not bigger than record size
    #[error("Error during data shards reconstruction: {0}")]
    DataShardsReconstruction(#[from] ErasureCodingError),
    /// Not enough shards
    #[error("Not enough shards: {num_shards}")]
    NotEnoughShards { num_shards: usize },
    /// Incorrect piece position provided.
    #[error("Incorrect piece position provided.")]
    IncorrectPiecePosition,
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
                            output_piece
                                .record_mut()
                                .copy_from_slice(input_piece.record().as_slice());
                            RecoveryShardState::Present(input_piece.record().as_flattened())
                        }
                        None => RecoveryShardState::MissingRecover(
                            output_piece.record_mut().as_flattened_mut(),
                        ),
                    },
                );
            let parity = parity_input_pieces
                .iter()
                .zip(parity_reconstructed_pieces.iter_mut())
                .map(
                    |(maybe_input_piece, output_piece)| match maybe_input_piece {
                        Some(input_piece) => {
                            output_piece
                                .record_mut()
                                .copy_from_slice(input_piece.record().as_slice());
                            RecoveryShardState::Present(input_piece.record().as_flattened())
                        }
                        None => RecoveryShardState::MissingRecover(
                            output_piece.record_mut().as_flattened_mut(),
                        ),
                    },
                );
            self.erasure_coding.recover(source, parity)?;
        }

        let record_commitments = {
            #[cfg(not(feature = "parallel"))]
            let iter = reconstructed_pieces.iter_mut().zip(input_pieces);
            #[cfg(feature = "parallel")]
            let iter = reconstructed_pieces.par_iter_mut().zip_eq(input_pieces);

            iter.map(|(piece, maybe_input_piece)| {
                let (record_commitment, parity_chunks_root) = if let Some(input_piece) =
                    maybe_input_piece
                {
                    (
                        **input_piece.commitment(),
                        **input_piece.parity_chunks_root(),
                    )
                } else {
                    // TODO: Reuse allocations between iterations
                    let [source_chunks_root, parity_chunks_root] = {
                        let mut record_merkle_tree = Box::<
                            BalancedHashedMerkleTree<{ Record::NUM_CHUNKS.ilog2() }>,
                        >::new_uninit();

                        let source_chunks_root = BalancedHashedMerkleTree::new_in(
                            &mut record_merkle_tree,
                            piece.record(),
                        )
                        .root();

                        let mut parity_chunks = Record::new_boxed();

                        self.erasure_coding
                            .extend(piece.record().iter(), parity_chunks.iter_mut())?;

                        let parity_chunks_root = BalancedHashedMerkleTree::new_in(
                            &mut record_merkle_tree,
                            &parity_chunks,
                        )
                        .root();

                        [source_chunks_root, parity_chunks_root]
                    };

                    let record_commitment = BalancedHashedMerkleTree::<1>::new(&[
                        source_chunks_root,
                        parity_chunks_root,
                    ])
                    .root();

                    (record_commitment, parity_chunks_root)
                };

                piece.commitment_mut().copy_from_slice(&record_commitment);
                piece
                    .parity_chunks_root_mut()
                    .copy_from_slice(&parity_chunks_root);

                Ok::<_, ReconstructorError>(record_commitment)
            })
            .collect::<Result<Vec<_>, _>>()?
        };

        let segment_merkle_tree =
            BalancedHashedMerkleTree::<{ ArchivedHistorySegment::NUM_PIECES.ilog2() }>::new_boxed(
                record_commitments
                    .as_slice()
                    .try_into()
                    .expect("Statically guaranteed to have correct length; qed"),
            );

        reconstructed_pieces
            .iter_mut()
            .zip(segment_merkle_tree.all_proofs())
            .for_each(|(piece, record_witness)| {
                piece.witness_mut().copy_from_slice(&record_witness);
            });

        Ok(reconstructed_pieces)
    }

    /// Returns all the pieces for a segment using given set of pieces of a segment of the archived
    /// history (any half of all pieces are required to be present, the rest will be recovered
    /// automatically due to use of erasure coding if needed).
    pub fn reconstruct_segment(
        &self,
        segment_pieces: &[Option<Piece>],
    ) -> Result<ArchivedHistorySegment, ReconstructorError> {
        let pieces = self.reconstruct_shards(segment_pieces)?;

        Ok(pieces.to_shared())
    }

    /// Returns the missing piece for a segment using given set of pieces of a segment of the archived
    /// history (any half of all pieces are required to be present).
    pub fn reconstruct_piece(
        &self,
        segment_pieces: &[Option<Piece>],
        piece_position: usize,
    ) -> Result<Piece, ReconstructorError> {
        if piece_position >= ArchivedHistorySegment::NUM_PIECES {
            return Err(ReconstructorError::IncorrectPiecePosition);
        }

        // TODO: Early exit if position already exists and doesn't need reconstruction
        // TODO: It is now inefficient to recover all shards if only one piece is needed, especially
        //  source piece
        let pieces = self.reconstruct_shards(segment_pieces)?;

        let piece = Piece::from(&pieces[piece_position]);

        Ok(piece.to_shared())
    }
}
