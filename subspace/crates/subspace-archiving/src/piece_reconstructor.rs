#[cfg(not(feature = "std"))]
extern crate alloc;

use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use subspace_core_primitives::pieces::{Piece, Record};
use subspace_core_primitives::segments::ArchivedHistorySegment;
use subspace_erasure_coding::{ErasureCoding, RecoveryShardState};

/// Reconstructor-related instantiation error
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReconstructorError {
    // TODO: Should be a better type than a string
    /// Segment size is not bigger than record size
    #[error("Error during data shards reconstruction: {0}")]
    DataShardsReconstruction(String),
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
        let mut reconstructed_pieces = ArchivedHistorySegment::default();

        // TODO: This will need to be simplified once pieces are no longer interleaved
        {
            let (source, parity) = input_pieces
                .iter()
                .zip(reconstructed_pieces.iter_mut())
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
                )
                .array_chunks::<2>()
                .map(|[a, b]| (a, b))
                .unzip::<_, _, Vec<_>, Vec<_>>();
            self.erasure_coding
                .recover(source.into_iter(), parity.into_iter())
                .map_err(ReconstructorError::DataShardsReconstruction)?;
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
                            .extend(piece.record().iter(), parity_chunks.iter_mut())
                            .expect(
                                "Erasure coding instance is deliberately configured to \
                                support this input; qed",
                            );

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

                Ok(record_commitment)
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
