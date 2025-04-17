#[cfg(not(feature = "std"))]
extern crate alloc;

use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use subspace_core_primitives::pieces::{Piece, RawRecord, Record};
use subspace_core_primitives::segments::ArchivedHistorySegment;
use subspace_erasure_coding::ErasureCoding;
use subspace_kzg::Scalar;

/// Reconstructor-related instantiation error
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReconstructorError {
    /// Segment size is not bigger than record size
    #[error("Error during data shards reconstruction: {0}")]
    DataShardsReconstruction(String),
    /// Incorrect piece position provided.
    #[error("Incorrect piece position provided.")]
    IncorrectPiecePosition,
}

/// Reconstructor helps to retrieve blocks from archived pieces.
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

        // Scratch buffer to avoid re-allocation
        let mut tmp_shards_scalars =
            Vec::<Option<Scalar>>::with_capacity(ArchivedHistorySegment::NUM_PIECES);
        // Iterate over the chunks of `ScalarBytes::SAFE_BYTES` bytes of all records
        for record_offset in 0..RawRecord::NUM_CHUNKS {
            // Collect chunks of each record at the same offset
            for maybe_piece in input_pieces.iter() {
                let maybe_scalar = maybe_piece
                    .as_ref()
                    .map(|piece| {
                        piece
                            .record()
                            .get(record_offset)
                            .expect("Statically guaranteed to exist in a piece; qed")
                    })
                    .map(Scalar::try_from)
                    .transpose()
                    .map_err(ReconstructorError::DataShardsReconstruction)?;

                tmp_shards_scalars.push(maybe_scalar);
            }

            self.erasure_coding
                .recover(&tmp_shards_scalars)
                .map_err(ReconstructorError::DataShardsReconstruction)?
                .into_iter()
                .zip(reconstructed_pieces.iter_mut().map(|piece| {
                    piece
                        .record_mut()
                        .get_mut(record_offset)
                        .expect("Statically guaranteed to exist in a piece; qed")
                }))
                .for_each(|(source_scalar, segment_data)| {
                    segment_data.copy_from_slice(&source_scalar.to_bytes());
                });

            tmp_shards_scalars.clear();
        }

        let record_commitments = {
            #[cfg(not(feature = "parallel"))]
            let iter = reconstructed_pieces.iter_mut().zip(input_pieces);
            #[cfg(feature = "parallel")]
            let iter = reconstructed_pieces.par_iter_mut().zip_eq(input_pieces);

            iter.map(|(piece, maybe_input_piece)| {
                let record_commitment = if let Some(input_piece) = maybe_input_piece {
                    **input_piece.commitment()
                } else {
                    let scalars = {
                        let mut scalars =
                            Vec::with_capacity(piece.record().len().next_power_of_two());

                        for record_chunk in piece.record().iter() {
                            scalars.push(
                                Scalar::try_from(record_chunk)
                                    .map_err(ReconstructorError::DataShardsReconstruction)?,
                            );
                        }

                        scalars
                    };

                    // TODO: Think about committing to source and parity chunks separately, then
                    //  creating a separate commitment for both and retaining a proof. This way it would
                    //  be possible to verify pieces without re-doing erasure coding. Same note exists
                    //  in other files.
                    let parity_scalars = self.erasure_coding.extend(&scalars).expect(
                        "Erasure coding instance is deliberately configured to support this input; qed",
                    );

                    let chunks = scalars
                        .into_iter()
                        .zip(parity_scalars)
                        .flat_map(|(a, b)| [a, b])
                        .map(|chunk| chunk.to_bytes())
                        .collect::<Vec<_>>();

                    // TODO: Reuse allocation or remove parallel processing if it is fast enough as is
                    let record_merkle_tree =
                        BalancedHashedMerkleTree::<{ Record::NUM_S_BUCKETS.ilog2() }>::new_boxed(
                            chunks
                                .as_slice()
                                .try_into()
                                .expect("Statically guaranteed to have correct length; qed"),
                        );

                    record_merkle_tree.root()
                };

                piece.commitment_mut().copy_from_slice(&record_commitment);

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
        let pieces = self.reconstruct_shards(segment_pieces)?;

        let piece = Piece::from(&pieces[piece_position]);

        Ok(piece.to_shared())
    }
}
