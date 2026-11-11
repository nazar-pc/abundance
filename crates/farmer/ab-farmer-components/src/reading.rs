//! Reading utilities
//!
//! This module contains utilities for extracting data from plots/sectors created by functions in
//! [`plotting`](crate::plotting) module earlier. This is a relatively expensive operation and is
//! only used for cold storage purposes or when there is a need to prove a solution to consensus.

use crate::sector::{
    RecordMetadata, SectorContentsMap, SectorContentsMapFromBytesError, SectorMetadataChecksummed,
    sector_record_chunks_size,
};
use crate::{ReadAt, ReadAtAsync, ReadAtSync};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{ExtendedRecord, Piece, PieceOffset, Record, RecordChunk};
use ab_core_primitives::sectors::SectorId;
use ab_erasure_coding::{ErasureCoding, ErasureCodingError, ShardsPresent};
use ab_proof_of_space::{PosProofs, Table, TableGenerator};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use parity_scale_codec::Decode;
use rayon::prelude::*;
use std::io;
use std::simd::Simd;
use thiserror::Error;
use tracing::debug;

/// Errors that happen during reading
#[derive(Debug, Error)]
pub enum ReadingError {
    /// Failed to read chunk.
    ///
    /// This is an implementation bug, most likely due to mismatch between sector contents map and
    /// other farming parameters.
    #[error("Failed to read chunk at location {chunk_location}: {error}")]
    FailedToReadChunk {
        /// Chunk location
        chunk_location: u64,
        /// Low-level error
        error: io::Error,
    },
    /// PoS proofs don't match the sector contents map.
    ///
    /// Either the sector contents map has no entry for the record, or the record's chunks in it
    /// are not at exactly the s-buckets that have proofs. There is no entry when the piece offset
    /// is outside of the sector. Chunks at other s-buckets usually mean that the sector doesn't
    /// match its metadata, so proofs are created for a different sector ID than the one the sector
    /// was plotted with.
    #[error("PoS proofs don't match sector contents map for record at offset {piece_offset}")]
    PosProofsMismatch {
        /// Piece offset
        piece_offset: PieceOffset,
    },
    /// Failed to erasure-decode record
    #[error("Failed to erasure-decode record at offset {piece_offset}: {error}")]
    FailedToErasureDecodeRecord {
        /// Piece offset
        piece_offset: PieceOffset,
        /// Lower-level error
        error: ErasureCodingError,
    },
    /// Wrong record size after decoding
    #[error("Wrong record size after decoding: expected {expected}, actual {actual}")]
    WrongRecordSizeAfterDecoding {
        /// Expected size in bytes
        expected: usize,
        /// Actual size in bytes
        actual: usize,
    },
    /// Failed to decode sector contents map
    #[error("Failed to decode sector contents map: {0}")]
    FailedToDecodeSectorContentsMap(#[from] SectorContentsMapFromBytesError),
    /// I/O error occurred
    #[error("Reading I/O error: {0}")]
    Io(#[from] io::Error),
    /// Checksum mismatch
    #[error("Checksum mismatch")]
    ChecksumMismatch,
}

impl ReadingError {
    /// Whether this error is fatal and renders farm unusable
    pub fn is_fatal(&self) -> bool {
        #[expect(
            clippy::rest_pattern_accessible_field,
            reason = "Do not care about fields"
        )]
        match self {
            ReadingError::FailedToReadChunk { .. } => false,
            ReadingError::PosProofsMismatch { .. } => false,
            ReadingError::FailedToErasureDecodeRecord { .. } => false,
            ReadingError::WrongRecordSizeAfterDecoding { .. } => false,
            ReadingError::FailedToDecodeSectorContentsMap(_) => false,
            ReadingError::Io(_) => true,
            ReadingError::ChecksumMismatch => false,
        }
    }
}

/// Record chunks read from a sector.
///
/// Contents of chunks that are not present are unspecified.
#[derive(Debug)]
pub struct SectorRecordChunks {
    /// Source and parity chunks
    pub extended_record: Box<ExtendedRecord>,
    /// Which chunks are present
    pub present: ShardsPresent<{ Record::NUM_CHUNKS }>,
}

/// Read sector record chunks, only plotted s-buckets are marked as present (in decoded form).
///
/// Returns [`ReadingError::PosProofsMismatch`] if the sector contents map has no entry for
/// `piece_offset`, or if the record's chunks in it are not at exactly the s-buckets of
/// `pos_proofs`. Each chunk was encoded with the proof for its s-bucket, so chunks can't be
/// decoded correctly otherwise.
///
/// NOTE: This is an async function, but it also does CPU-intensive operation internally, while it
/// is not very long, make sure it is okay to do so in your context.
pub async fn read_sector_record_chunks<S, A>(
    piece_offset: PieceOffset,
    pieces_in_sector: u16,
    s_bucket_offsets: &[u32; Record::NUM_S_BUCKETS],
    sector_contents_map: &SectorContentsMap,
    pos_proofs: &PosProofs,
    sector: &ReadAt<S, A>,
) -> Result<SectorRecordChunks, ReadingError>
where
    S: ReadAtSync,
    A: ReadAtAsync,
{
    // Chunks are paired with proofs in s-bucket order below, which is only correct when the record
    // has chunks at exactly the s-buckets with proofs
    if sector_contents_map
        .iter_record_chunks_used()
        .get(usize::from(piece_offset))
        != Some(&pos_proofs.found_proofs)
    {
        return Err(ReadingError::PosProofsMismatch { piece_offset });
    }

    let mut extended_record = ExtendedRecord::new_boxed();
    let mut present = ShardsPresent::none();

    let read_chunks_inputs = extended_record
        .as_s_buckets_mut()
        .par_iter_mut()
        .zip(sector_contents_map.par_iter_record_chunk_to_plot(piece_offset))
        .zip(s_bucket_offsets.par_iter())
        .enumerate()
        .map(
            |(index, ((record_chunk, maybe_chunk_offset), &s_bucket_offset))| {
                let chunk_offset = maybe_chunk_offset?;

                let chunk_location = chunk_offset as u64 + u64::from(s_bucket_offset);

                Some((index, record_chunk, chunk_location))
            },
        )
        .flatten()
        .collect::<Vec<_>>();

    for &(index, _, _) in &read_chunks_inputs {
        if let Some(parity_index) = index.checked_sub(Record::NUM_CHUNKS) {
            present.parity.set(parity_index);
        } else {
            present.source.set(index);
        }
    }

    let sector_contents_map_size = SectorContentsMap::encoded_size(pieces_in_sector) as u64;
    match sector {
        ReadAt::Sync(sector) => {
            read_chunks_inputs
                .into_par_iter()
                .zip(&pos_proofs.proofs)
                .try_for_each(|((_index, output_chunk, chunk_location), pos_proof)| {
                    let mut record_chunk = [0; RecordChunk::SIZE];
                    sector
                        .read_at(
                            &mut record_chunk,
                            sector_contents_map_size + chunk_location * RecordChunk::SIZE as u64,
                        )
                        .map_err(|error| ReadingError::FailedToReadChunk {
                            chunk_location,
                            error,
                        })?;

                    // TODO: Use SIMD for hashing
                    record_chunk =
                        Simd::to_array(Simd::from(record_chunk) ^ Simd::from(*pos_proof.hash()));

                    *output_chunk = RecordChunk::from(record_chunk);

                    Ok::<_, ReadingError>(())
                })?;
        }
        ReadAt::Async(sector) => {
            let processing_chunks = read_chunks_inputs
                .into_iter()
                .zip(&pos_proofs.proofs)
                .map(
                    |((_index, output_chunk, chunk_location), pos_proof)| async move {
                        let mut record_chunk = [0; RecordChunk::SIZE];
                        record_chunk.copy_from_slice(
                            &sector
                                .read_at(
                                    vec![0; RecordChunk::SIZE],
                                    sector_contents_map_size
                                        + chunk_location * RecordChunk::SIZE as u64,
                                )
                                .await
                                .map_err(|error| ReadingError::FailedToReadChunk {
                                    chunk_location,
                                    error,
                                })?,
                        );

                        // TODO: Use SIMD for hashing
                        record_chunk = Simd::to_array(
                            Simd::from(record_chunk) ^ Simd::from(*pos_proof.hash()),
                        );

                        *output_chunk = RecordChunk::from(record_chunk);

                        Ok::<_, ReadingError>(())
                    },
                )
                .collect::<FuturesUnordered<_>>()
                .filter_map(|result| async move { result.err() });

            std::pin::pin!(processing_chunks)
                .next()
                .await
                .map_or(Ok(()), Err)?;
        }
    }

    Ok(SectorRecordChunks {
        extended_record,
        present,
    })
}

/// Given sector record chunks recover the source record in place
pub fn recover_source_record<'a>(
    sector_record_chunks: &'a mut SectorRecordChunks,
    piece_offset: PieceOffset,
    erasure_coding: &ErasureCoding,
) -> Result<&'a Record, ReadingError> {
    let SectorRecordChunks {
        extended_record,
        present,
    } = sector_record_chunks;
    let ExtendedRecord { source, parity } = &mut **extended_record;

    erasure_coding
        .recover_source(source, parity, present)
        .map_err(|error| ReadingError::FailedToErasureDecodeRecord {
            piece_offset,
            error,
        })?;

    Ok(source)
}

/// Read metadata (roots and proof) for record
pub(crate) async fn read_record_metadata<S, A>(
    piece_offset: PieceOffset,
    pieces_in_sector: u16,
    sector: &ReadAt<S, A>,
) -> Result<RecordMetadata, ReadingError>
where
    S: ReadAtSync,
    A: ReadAtAsync,
{
    let sector_metadata_start = SectorContentsMap::encoded_size(pieces_in_sector) as u64
        + sector_record_chunks_size(pieces_in_sector) as u64;
    // Move to the beginning of the root and proof we care about
    let record_metadata_offset =
        sector_metadata_start + RecordMetadata::encoded_size() as u64 * u64::from(piece_offset);

    let mut record_metadata_bytes = vec![0; RecordMetadata::encoded_size()];
    match sector {
        ReadAt::Sync(sector) => {
            sector.read_at(&mut record_metadata_bytes, record_metadata_offset)?;
        }
        ReadAt::Async(sector) => {
            record_metadata_bytes = sector
                .read_at(record_metadata_bytes, record_metadata_offset)
                .await?;
        }
    }
    let record_metadata = RecordMetadata::decode(&mut record_metadata_bytes.as_ref())
        .expect("Length is correct, contents doesn't have specific structure to it; qed");

    Ok(record_metadata)
}

/// Read piece from sector.
///
/// NOTE: Even though this function is async, proof of time table generation is expensive and should
/// be done in a dedicated thread where blocking is allowed.
pub async fn read_piece<PosTable, S, A>(
    piece_offset: PieceOffset,
    sector_id: &SectorId,
    sector_metadata: &SectorMetadataChecksummed,
    sector: &ReadAt<S, A>,
    erasure_coding: &ErasureCoding,
    table_generator: &PosTable::Generator,
) -> Result<Piece, ReadingError>
where
    PosTable: Table,
    S: ReadAtSync,
    A: ReadAtAsync,
{
    let pieces_in_sector = sector_metadata.pieces_in_sector;

    let sector_contents_map = {
        let mut sector_contents_map_bytes =
            vec![0; SectorContentsMap::encoded_size(pieces_in_sector)];
        match sector {
            ReadAt::Sync(sector) => {
                sector.read_at(&mut sector_contents_map_bytes, 0)?;
            }
            ReadAt::Async(sector) => {
                sector_contents_map_bytes = sector.read_at(sector_contents_map_bytes, 0).await?;
            }
        }

        SectorContentsMap::from_bytes(&sector_contents_map_bytes, pieces_in_sector)?
    };

    let mut sector_record_chunks = read_sector_record_chunks(
        piece_offset,
        pieces_in_sector,
        &sector_metadata.s_bucket_offsets(),
        &sector_contents_map,
        &table_generator.create_proofs(&sector_id.derive_evaluation_seed(piece_offset)),
        sector,
    )
    .await?;
    // Restore source record scalars
    let record = recover_source_record(&mut sector_record_chunks, piece_offset, erasure_coding)?;

    let RecordMetadata {
        piece_header,
        piece_checksum,
    } = read_record_metadata(piece_offset, pieces_in_sector, sector).await?;

    let piece = Piece::from_parts(&piece_header, record);

    // Verify checksum
    let actual_checksum = Blake3Hash::from(blake3::hash(piece.as_ref()));
    if actual_checksum != piece_checksum {
        debug!(
            ?sector_id,
            %piece_offset,
            %actual_checksum,
            expected_checksum = %piece_checksum,
            "Hash doesn't match, plotted piece is corrupted"
        );

        return Err(ReadingError::ChecksumMismatch);
    }

    Ok(piece)
}
