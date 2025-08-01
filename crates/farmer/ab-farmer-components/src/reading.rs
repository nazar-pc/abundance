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
use ab_core_primitives::pieces::{Piece, PieceOffset, Record, RecordChunk};
use ab_core_primitives::sectors::{SBucket, SectorId};
use ab_erasure_coding::{ErasureCoding, ErasureCodingError, RecoveryShardState};
use ab_proof_of_space::{Table, TableGenerator};
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use parity_scale_codec::Decode;
use rayon::prelude::*;
use std::mem::ManuallyDrop;
use std::simd::Simd;
use std::str::FromStr;
use std::{fmt, io};
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
    /// Missing proof of space proof.
    ///
    /// This is either hardware issue or if happens for everyone all the time an implementation
    /// bug.
    #[error("Missing PoS proof for s-bucket {s_bucket}")]
    MissingPosProof {
        /// S-bucket
        s_bucket: SBucket,
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
        match self {
            ReadingError::FailedToReadChunk { .. } => false,
            ReadingError::MissingPosProof { .. } => false,
            ReadingError::FailedToErasureDecodeRecord { .. } => false,
            ReadingError::WrongRecordSizeAfterDecoding { .. } => false,
            ReadingError::FailedToDecodeSectorContentsMap(_) => false,
            ReadingError::Io(_) => true,
            ReadingError::ChecksumMismatch => false,
        }
    }
}

/// Defines a mode of reading chunks in [`read_sector_record_chunks`].
///
/// Which option that is slower or faster depends on disk used, there is no one-size-fits-all here,
/// unfortunately.
#[derive(Debug, Copy, Clone)]
pub enum ReadSectorRecordChunksMode {
    /// Read individual chunks ([`RecordChunk::SIZE`] in size) concurrently, which results in lower
    /// total data transfer, but requires for SSD to support high concurrency and low latency
    ConcurrentChunks,
    /// Read the whole sector at once and extract chunks from in-memory buffer, which uses more
    /// memory, but only requires linear read speed from the disk to be decent
    WholeSector,
}

impl fmt::Display for ReadSectorRecordChunksMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConcurrentChunks => {
                write!(f, "ConcurrentChunks")
            }
            Self::WholeSector => {
                write!(f, "WholeSector")
            }
        }
    }
}

impl FromStr for ReadSectorRecordChunksMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ConcurrentChunks" => Ok(Self::ConcurrentChunks),
            "WholeSector" => Ok(Self::WholeSector),
            s => Err(format!("Can't parse {s} as `ReadSectorRecordChunksMode`")),
        }
    }
}

// TODO: Workaround for https://github.com/rust-lang/rust/issues/144690 that gets triggered on
//  `s_bucket_offsets` argument below
const _: () = {
    assert!(65536 == Record::NUM_S_BUCKETS);
};
/// Read sector record chunks, only plotted s-buckets are returned (in decoded form).
///
/// NOTE: This is an async function, but it also does CPU-intensive operation internally, while it
/// is not very long, make sure it is okay to do so in your context.
pub async fn read_sector_record_chunks<PosTable, S, A>(
    piece_offset: PieceOffset,
    pieces_in_sector: u16,
    // TODO: Workaround for https://github.com/rust-lang/rust/issues/144690
    // s_bucket_offsets: &[u32; Record::NUM_S_BUCKETS],
    s_bucket_offsets: &[u32; 65536],
    sector_contents_map: &SectorContentsMap,
    pos_table: &PosTable,
    sector: &ReadAt<S, A>,
    mode: ReadSectorRecordChunksMode,
) -> Result<Box<[Option<RecordChunk>; Record::NUM_S_BUCKETS]>, ReadingError>
where
    PosTable: Table,
    S: ReadAtSync,
    A: ReadAtAsync,
{
    let mut record_chunks = Box::<[Option<RecordChunk>; Record::NUM_S_BUCKETS]>::try_from(
        vec![None::<RecordChunk>; Record::NUM_S_BUCKETS].into_boxed_slice(),
    )
    .expect("Correct size; qed");

    let read_chunks_inputs = record_chunks
        .par_iter_mut()
        .zip(sector_contents_map.par_iter_record_chunk_to_plot(piece_offset))
        .zip(
            (u16::from(SBucket::ZERO)..=u16::from(SBucket::MAX))
                .into_par_iter()
                .map(SBucket::from)
                .zip(s_bucket_offsets.par_iter()),
        )
        .map(
            |((maybe_record_chunk, maybe_chunk_details), (s_bucket, &s_bucket_offset))| {
                let (chunk_offset, encoded_chunk_used) = maybe_chunk_details?;

                let chunk_location = chunk_offset as u64 + u64::from(s_bucket_offset);

                Some((
                    maybe_record_chunk,
                    chunk_location,
                    encoded_chunk_used,
                    s_bucket,
                ))
            },
        )
        .collect::<Vec<_>>();

    let sector_contents_map_size = SectorContentsMap::encoded_size(pieces_in_sector) as u64;
    let sector_bytes = match mode {
        ReadSectorRecordChunksMode::ConcurrentChunks => None,
        ReadSectorRecordChunksMode::WholeSector => {
            Some(vec![0u8; crate::sector::sector_size(pieces_in_sector)])
        }
    };
    match sector {
        ReadAt::Sync(sector) => {
            let sector_bytes = {
                if let Some(mut sector_bytes) = sector_bytes {
                    sector.read_at(&mut sector_bytes, 0)?;
                    Some(sector_bytes)
                } else {
                    None
                }
            };
            read_chunks_inputs.into_par_iter().flatten().try_for_each(
                |(maybe_record_chunk, chunk_location, encoded_chunk_used, s_bucket)| {
                    let mut record_chunk = [0; RecordChunk::SIZE];
                    if let Some(sector_bytes) = &sector_bytes {
                        record_chunk.copy_from_slice(
                            &sector_bytes[sector_contents_map_size as usize
                                + chunk_location as usize * RecordChunk::SIZE..]
                                [..RecordChunk::SIZE],
                        );
                    } else {
                        sector
                            .read_at(
                                &mut record_chunk,
                                sector_contents_map_size
                                    + chunk_location * RecordChunk::SIZE as u64,
                            )
                            .map_err(|error| ReadingError::FailedToReadChunk {
                                chunk_location,
                                error,
                            })?;
                    }

                    // Decode chunk if necessary
                    if encoded_chunk_used {
                        let proof = pos_table
                            .find_proof(s_bucket.into())
                            .ok_or(ReadingError::MissingPosProof { s_bucket })?;

                        record_chunk =
                            Simd::to_array(Simd::from(record_chunk) ^ Simd::from(*proof.hash()));
                    }

                    maybe_record_chunk.replace(RecordChunk::from(record_chunk));

                    Ok::<_, ReadingError>(())
                },
            )?;
        }
        ReadAt::Async(sector) => {
            let sector_bytes = &{
                if let Some(sector_bytes) = sector_bytes {
                    Some(sector.read_at(sector_bytes, 0).await?)
                } else {
                    None
                }
            };
            let processing_chunks = read_chunks_inputs
                .into_iter()
                .flatten()
                .map(
                    |(maybe_record_chunk, chunk_location, encoded_chunk_used, s_bucket)| async move {
                        let mut record_chunk = [0; RecordChunk::SIZE];
                        if let Some(sector_bytes) = &sector_bytes {
                            record_chunk.copy_from_slice(
                                &sector_bytes[sector_contents_map_size as usize
                                    + chunk_location as usize * RecordChunk::SIZE..]
                                    [..RecordChunk::SIZE],
                            );
                        } else {
                            record_chunk.copy_from_slice(
                                &sector
                                    .read_at(
                                        vec![0; RecordChunk::SIZE],
                                        sector_contents_map_size + chunk_location * RecordChunk::SIZE as u64,
                                    )
                                    .await
                                    .map_err(|error| ReadingError::FailedToReadChunk {
                                        chunk_location,
                                        error,
                                    })?
                            );
                        }


                        // Decode chunk if necessary
                        if encoded_chunk_used {
                            let proof = pos_table.find_proof(s_bucket.into())
                                .ok_or(ReadingError::MissingPosProof { s_bucket })?;

                            record_chunk = Simd::to_array(
                                Simd::from(record_chunk) ^ Simd::from(*proof.hash()),
                            );
                        }

                        maybe_record_chunk.replace(RecordChunk::from(record_chunk));

                        Ok::<_, ReadingError>(())
                    },
                )
                .collect::<FuturesUnordered<_>>()
                .filter_map(|result| async move {
                    result.err()
                });

            std::pin::pin!(processing_chunks)
                .next()
                .await
                .map_or(Ok(()), Err)?;
        }
    }

    Ok(record_chunks)
}

/// Given sector record chunks recover extended record chunks (both source and parity)
pub fn recover_extended_record_chunks(
    sector_record_chunks: &[Option<RecordChunk>; Record::NUM_S_BUCKETS],
    piece_offset: PieceOffset,
    erasure_coding: &ErasureCoding,
) -> Result<Box<[RecordChunk; Record::NUM_S_BUCKETS]>, ReadingError> {
    // Restore source record scalars

    let mut recovered_sector_record_chunks = vec![[0u8; RecordChunk::SIZE]; Record::NUM_S_BUCKETS];
    {
        let (source_sector_record_chunks, parity_sector_record_chunks) =
            sector_record_chunks.split_at(Record::NUM_CHUNKS);
        let (source_recovered_sector_record_chunks, parity_recovered_sector_record_chunks) =
            recovered_sector_record_chunks.split_at_mut(Record::NUM_CHUNKS);

        let source = source_sector_record_chunks
            .iter()
            .zip(source_recovered_sector_record_chunks.iter_mut())
            .map(
                |(maybe_input_chunk, output_chunk)| match maybe_input_chunk {
                    Some(input_chunk) => {
                        output_chunk.copy_from_slice(input_chunk.as_slice());
                        RecoveryShardState::Present(input_chunk.as_slice())
                    }
                    None => RecoveryShardState::MissingRecover(output_chunk.as_mut_slice()),
                },
            );
        let parity = parity_sector_record_chunks
            .iter()
            .zip(parity_recovered_sector_record_chunks.iter_mut())
            .map(
                |(maybe_input_chunk, output_chunk)| match maybe_input_chunk {
                    Some(input_chunk) => {
                        output_chunk.copy_from_slice(input_chunk.as_slice());
                        RecoveryShardState::Present(input_chunk.as_slice())
                    }
                    None => RecoveryShardState::MissingRecover(output_chunk.as_mut_slice()),
                },
            );
        erasure_coding.recover(source, parity).map_err(|error| {
            ReadingError::FailedToErasureDecodeRecord {
                piece_offset,
                error,
            }
        })?;
    }

    // Allocation in vector can be larger than contents, we need to make sure allocation is the same
    // as the contents, this should also contain fast path if allocation matches contents
    let record_chunks = recovered_sector_record_chunks
        .into_iter()
        .map(RecordChunk::from)
        .collect::<Box<_>>();
    let mut record_chunks = ManuallyDrop::new(record_chunks);
    // SAFETY: Original memory is not dropped, size of the data checked above
    let record_chunks = unsafe { Box::from_raw(record_chunks.as_mut_ptr() as *mut _) };

    Ok(record_chunks)
}

/// Given sector record chunks recover source record chunks in form of an iterator.
pub fn recover_source_record(
    sector_record_chunks: &[Option<RecordChunk>; Record::NUM_S_BUCKETS],
    piece_offset: PieceOffset,
    erasure_coding: &ErasureCoding,
) -> Result<Box<Record>, ReadingError> {
    // Restore source record scalars
    let mut recovered_record = Record::new_boxed();

    let (source_sector_record_chunks, parity_sector_record_chunks) =
        sector_record_chunks.split_at(Record::NUM_CHUNKS);
    let source = source_sector_record_chunks
        .iter()
        .zip(recovered_record.iter_mut())
        .map(
            |(maybe_input_chunk, output_chunk)| match maybe_input_chunk {
                Some(input_chunk) => {
                    output_chunk.copy_from_slice(input_chunk.as_slice());
                    RecoveryShardState::Present(input_chunk.as_slice())
                }
                None => RecoveryShardState::MissingRecover(output_chunk.as_mut_slice()),
            },
        );
    let parity =
        parity_sector_record_chunks
            .iter()
            .map(|maybe_input_chunk| match maybe_input_chunk {
                Some(input_chunk) => RecoveryShardState::Present(input_chunk.as_slice()),
                None => RecoveryShardState::MissingIgnore,
            });
    erasure_coding.recover(source, parity).map_err(|error| {
        ReadingError::FailedToErasureDecodeRecord {
            piece_offset,
            error,
        }
    })?;

    Ok(recovered_record)
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
    mode: ReadSectorRecordChunksMode,
    table_generator: &mut PosTable::Generator,
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

    let sector_record_chunks = read_sector_record_chunks(
        piece_offset,
        pieces_in_sector,
        &sector_metadata.s_bucket_offsets(),
        &sector_contents_map,
        &table_generator.generate(&sector_id.derive_evaluation_seed(piece_offset)),
        sector,
        mode,
    )
    .await?;
    // Restore source record scalars
    let record = recover_source_record(&sector_record_chunks, piece_offset, erasure_coding)?;

    let record_metadata = read_record_metadata(piece_offset, pieces_in_sector, sector).await?;

    let mut piece = Piece::default();

    piece.record_mut().copy_from_slice(record.as_slice());

    *piece.root_mut() = record_metadata.root;
    *piece.parity_chunks_root_mut() = record_metadata.parity_chunks_root;
    *piece.proof_mut() = record_metadata.proof;

    // Verify checksum
    let actual_checksum = Blake3Hash::from(blake3::hash(piece.as_ref()));
    if actual_checksum != record_metadata.piece_checksum {
        debug!(
            ?sector_id,
            %piece_offset,
            %actual_checksum,
            expected_checksum = %record_metadata.piece_checksum,
            "Hash doesn't match, plotted piece is corrupted"
        );

        return Err(ReadingError::ChecksumMismatch);
    }

    Ok(piece.to_shared())
}
