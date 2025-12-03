//! Sector-related data structures
//!
//! Sectors and corresponding metadata created by functions in [`plotting`](crate::plotting) module
//! have a specific structure, represented by data structured in this module.
//!
//! It is typically not needed to construct these data structures explicitly outside of this crate,
//! instead they will be returned as a result of certain operations (like plotting).

use ab_core_primitives::checksum::Blake3Checksummed;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{PieceOffset, Record, RecordChunksRoot, RecordProof, RecordRoot};
use ab_core_primitives::sectors::{SBucket, SectorIndex};
use ab_core_primitives::segments::{HistorySize, SegmentIndex};
use ab_io_type::trivial_type::TrivialType;
use parity_scale_codec::{Decode, Encode};
use rayon::prelude::*;
use std::ops::{Deref, DerefMut};
use thiserror::Error;
use tracing::debug;

/// Size of the part of the plot containing record chunks (s-buckets).
///
/// Total size of the plot can be computed with [`sector_size()`].
#[inline]
pub const fn sector_record_chunks_size(pieces_in_sector: u16) -> usize {
    pieces_in_sector as usize * Record::SIZE
}

/// Size of the part of the plot containing record metadata.
///
/// Total size of the plot can be computed with [`sector_size()`].
#[inline]
pub const fn sector_record_metadata_size(pieces_in_sector: u16) -> usize {
    pieces_in_sector as usize * RecordMetadata::encoded_size()
}

/// Exact sector plot size (sector contents map, record chunks, record metadata).
///
/// NOTE: Each sector also has corresponding fixed size metadata whose size can be obtained with
/// [`SectorMetadataChecksummed::encoded_size()`], size of the record chunks (s-buckets) with
/// [`sector_record_chunks_size()`] and size of record roots and proofs with
/// [`sector_record_metadata_size()`]. This function just combines those three together for
/// convenience.
#[inline]
pub const fn sector_size(pieces_in_sector: u16) -> usize {
    sector_record_chunks_size(pieces_in_sector)
        + sector_record_metadata_size(pieces_in_sector)
        + SectorContentsMap::encoded_size(pieces_in_sector)
        + Blake3Hash::SIZE
}

/// Metadata of the plotted sector
#[derive(Debug, Encode, Decode, Clone)]
pub struct SectorMetadata {
    /// Sector index
    pub sector_index: SectorIndex,
    /// Number of pieces stored in this sector
    pub pieces_in_sector: u16,
    /// S-bucket sizes in a sector
    pub s_bucket_sizes: Box<[u16; Record::NUM_S_BUCKETS]>,
    /// Size of the blockchain history at time of sector creation
    pub history_size: HistorySize,
}

impl SectorMetadata {
    /// Returns offsets of each s-bucket relatively to the beginning of the sector (in chunks)
    pub fn s_bucket_offsets(&self) -> Box<[u32; Record::NUM_S_BUCKETS]> {
        let s_bucket_offsets = self
            .s_bucket_sizes
            .iter()
            .map({
                let mut base_offset = 0;

                move |s_bucket_size| {
                    let offset = base_offset;
                    base_offset += u32::from(*s_bucket_size);
                    offset
                }
            })
            .collect::<Box<_>>();

        assert_eq!(s_bucket_offsets.len(), Record::NUM_S_BUCKETS);
        // SAFETY: Number of elements checked above
        unsafe {
            Box::from_raw(Box::into_raw(s_bucket_offsets).cast::<[u32; Record::NUM_S_BUCKETS]>())
        }
    }
}

/// Same as [`SectorMetadata`], but with checksums verified during SCALE encoding/decoding
#[derive(Debug, Clone, Encode, Decode)]
pub struct SectorMetadataChecksummed(Blake3Checksummed<SectorMetadata>);

impl From<SectorMetadata> for SectorMetadataChecksummed {
    #[inline]
    fn from(value: SectorMetadata) -> Self {
        Self(Blake3Checksummed(value))
    }
}

impl Deref for SectorMetadataChecksummed {
    type Target = SectorMetadata;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0.0
    }
}

impl DerefMut for SectorMetadataChecksummed {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.0
    }
}

impl SectorMetadataChecksummed {
    /// Size of encoded checksummed sector metadata.
    ///
    /// For sector plot size use [`sector_size()`].
    #[inline]
    pub fn encoded_size() -> usize {
        let default = SectorMetadataChecksummed::from(SectorMetadata {
            sector_index: SectorIndex::ZERO,
            pieces_in_sector: 0,
            // TODO: Should have been just `::new()`, but https://github.com/rust-lang/rust/issues/53827
            // SAFETY: Data structure filled with zeroes is a valid invariant
            s_bucket_sizes: unsafe { Box::new_zeroed().assume_init() },
            history_size: HistorySize::from(SegmentIndex::ZERO),
        });

        default.encoded_size()
    }
}

/// Root and proof corresponding to the same record
#[derive(Debug, Default, Clone, Encode, Decode)]
pub(crate) struct RecordMetadata {
    /// Record root
    pub(crate) root: RecordRoot,
    /// Parity chunks root
    pub(crate) parity_chunks_root: RecordChunksRoot,
    /// Record proof
    pub(crate) proof: RecordProof,
    /// Checksum (hash) of the whole piece
    pub(crate) piece_checksum: Blake3Hash,
}

impl RecordMetadata {
    pub(crate) const fn encoded_size() -> usize {
        RecordProof::SIZE + RecordRoot::SIZE + RecordChunksRoot::SIZE + Blake3Hash::SIZE
    }
}

/// Raw sector before it is transformed and written to plot, used during plotting
#[derive(Debug, Clone)]
pub(crate) struct RawSector {
    /// List of records, likely downloaded from the network
    pub(crate) records: Vec<Record>,
    /// Metadata (root and proof) corresponding to the same record
    pub(crate) metadata: Vec<RecordMetadata>,
}

impl RawSector {
    /// Create new raw sector with internal vectors being allocated and filled with default values
    pub(crate) fn new(pieces_in_sector: u16) -> Self {
        Self {
            records: Record::new_zero_vec(usize::from(pieces_in_sector)),
            metadata: vec![RecordMetadata::default(); usize::from(pieces_in_sector)],
        }
    }
}

/// S-buckets at which proofs were found.
///
/// S-buckets are grouped by 8, within each `u8` bits right to left (LSB) indicate the presence
/// of a proof for corresponding s-bucket, so that the whole array of bytes can be thought as a
/// large set of bits.
///
/// There will be at most [`Record::NUM_CHUNKS`] proofs produced/bits set to `1`.
pub type FoundProofs = [u8; Record::NUM_S_BUCKETS / u8::BITS as usize];

/// Error happening when trying to create [`SectorContentsMap`] from bytes
#[derive(Debug, Error, Copy, Clone, Eq, PartialEq)]
pub enum SectorContentsMapFromBytesError {
    /// Invalid bytes length
    #[error("Invalid bytes length, expected {expected}, actual {actual}")]
    InvalidBytesLength {
        /// Expected length
        expected: usize,
        /// Actual length
        actual: usize,
    },
    /// Checksum mismatch
    #[error("Checksum mismatch")]
    ChecksumMismatch,
}

/// Error happening when trying to encode [`SectorContentsMap`] into bytes
#[derive(Debug, Error, Copy, Clone, Eq, PartialEq)]
pub enum SectorContentsMapEncodeIntoError {
    /// Invalid bytes length
    #[error("Invalid bytes length, expected {expected}, actual {actual}")]
    InvalidBytesLength {
        /// Expected length
        expected: usize,
        /// Actual length
        actual: usize,
    },
}

/// Error happening when trying to create [`SectorContentsMap`] from bytes
#[derive(Debug, Error, Copy, Clone, Eq, PartialEq)]
pub enum SectorContentsMapIterationError {
    /// S-bucket provided is out of range
    #[error("S-bucket provided {provided} is out of range, max {max}")]
    SBucketOutOfRange {
        /// Provided s-bucket
        provided: usize,
        /// Max s-bucket
        max: usize,
    },
}

/// Map of sector contents.
///
/// Abstraction on top of bitfields that allow making sense of sector contents that contain
/// encoded (meaning erasure coded and encoded with existing PoSpace proof) chunks used at the same
/// time both in records (before writing to plot) and s-buckets (written into the plot) format
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SectorContentsMap {
    /// Bitfields for each record, each bit is `true` if record chunk at corresponding position was
    /// used
    record_chunks_used: Vec<FoundProofs>,
}

impl SectorContentsMap {
    /// Create new sector contents map initialized with zeroes to store data for `pieces_in_sector`
    /// records
    pub fn new(pieces_in_sector: u16) -> Self {
        Self {
            record_chunks_used: vec![[0; _]; usize::from(pieces_in_sector)],
        }
    }

    /// Reconstruct sector contents map from bytes.
    ///
    /// Returns error if length of the vector doesn't match [`Self::encoded_size()`] for
    /// `pieces_in_sector`.
    pub fn from_bytes(
        bytes: &[u8],
        pieces_in_sector: u16,
    ) -> Result<Self, SectorContentsMapFromBytesError> {
        if bytes.len() != Self::encoded_size(pieces_in_sector) {
            return Err(SectorContentsMapFromBytesError::InvalidBytesLength {
                expected: Self::encoded_size(pieces_in_sector),
                actual: bytes.len(),
            });
        }

        let (single_records_bit_arrays, expected_checksum) =
            bytes.split_at(bytes.len() - Blake3Hash::SIZE);
        // SAFETY: All bit patterns are valid
        let expected_checksum = unsafe {
            Blake3Hash::from_bytes(expected_checksum).expect("No alignment requirements; qed")
        };
        // Verify checksum
        let actual_checksum = Blake3Hash::from(blake3::hash(single_records_bit_arrays));
        if &actual_checksum != expected_checksum {
            debug!(
                %actual_checksum,
                %expected_checksum,
                "Hash doesn't match, corrupted bytes"
            );

            return Err(SectorContentsMapFromBytesError::ChecksumMismatch);
        }

        let mut record_chunks_used = vec![[0; _]; pieces_in_sector.into()];

        record_chunks_used
            .as_flattened_mut()
            .copy_from_slice(single_records_bit_arrays);

        Ok(Self { record_chunks_used })
    }

    /// Size of sector contents map when encoded and stored in the plot for specified number of
    /// pieces in sector
    pub const fn encoded_size(pieces_in_sector: u16) -> usize {
        size_of::<FoundProofs>() * pieces_in_sector as usize + Blake3Hash::SIZE
    }

    /// Encode internal contents into `output`
    pub fn encode_into(&self, output: &mut [u8]) -> Result<(), SectorContentsMapEncodeIntoError> {
        if output.len() != Self::encoded_size(self.record_chunks_used.len() as u16) {
            return Err(SectorContentsMapEncodeIntoError::InvalidBytesLength {
                expected: Self::encoded_size(self.record_chunks_used.len() as u16),
                actual: output.len(),
            });
        }

        let slice = self.record_chunks_used.as_flattened();
        // Write data and checksum
        output[..slice.len()].copy_from_slice(slice);
        output[slice.len()..].copy_from_slice(blake3::hash(slice).as_bytes());

        Ok(())
    }

    /// Iterate over individual record chunks (s-buckets) that were used
    pub fn iter_record_chunks_used(&self) -> &[FoundProofs] {
        &self.record_chunks_used
    }

    /// Iterate mutably over individual record chunks (s-buckets) that were used
    pub fn iter_record_chunks_used_mut(&mut self) -> &mut [FoundProofs] {
        &mut self.record_chunks_used
    }

    /// Returns sizes of each s-bucket
    pub fn s_bucket_sizes(&self) -> Box<[u16; Record::NUM_S_BUCKETS]> {
        // Rayon doesn't support iteration over custom types yet
        let s_bucket_sizes = (u16::from(SBucket::ZERO)..=u16::from(SBucket::MAX))
            .into_par_iter()
            .map(SBucket::from)
            .map(|s_bucket| {
                self.iter_s_bucket_piece_offsets(s_bucket)
                    .expect("S-bucket guaranteed to be in range; qed")
                    .count() as u16
            })
            .collect::<Box<_>>();

        assert_eq!(s_bucket_sizes.len(), Record::NUM_S_BUCKETS);

        // SAFETY: Number of elements checked above
        unsafe {
            Box::from_raw(Box::into_raw(s_bucket_sizes).cast::<[u16; Record::NUM_S_BUCKETS]>())
        }
    }

    /// Creates an iterator of `(s_bucket, chunk_location)`, where `s_bucket` is the position of the
    /// chunk in the erasure coded record and `chunk_location` is the offset of the chunk in the
    /// plot (across all s-buckets).
    pub fn iter_record_chunk_to_plot(
        &self,
        piece_offset: PieceOffset,
    ) -> impl Iterator<Item = (SBucket, usize)> + '_ {
        // Iterate over all s-buckets
        (SBucket::ZERO..=SBucket::MAX)
            // In each s-bucket map all records used
            .flat_map(|s_bucket| {
                self.iter_s_bucket_piece_offsets(s_bucket)
                    .expect("S-bucket guaranteed to be in range; qed")
                    .map(move |current_piece_offset| (s_bucket, current_piece_offset))
            })
            // We've got contents of all s-buckets in a flat iterator, enumerating them so it is
            // possible to find in the plot later if desired
            .enumerate()
            // Everything about the piece offset we care about
            .filter_map(move |(chunk_location, (s_bucket, current_piece_offset))| {
                // In case record for `piece_offset` is found, return necessary information
                (current_piece_offset == piece_offset).then_some((s_bucket, chunk_location))
            })
            // Tiny optimization in case we have found chunks for all records already
            .take(Record::NUM_CHUNKS)
    }

    /// Creates an iterator of `Option<chunk_offset>`, where each entry corresponds
    /// s-bucket/position of the chunk in the erasure coded record, `chunk_offset` is the offset of
    /// the chunk in the corresponding s-bucket.
    ///
    /// Similar to `Self::iter_record_chunk_to_plot()`, but runs in parallel, returns entries for
    /// all s-buckets and offsets are within corresponding s-buckets rather than the whole plot.
    pub fn par_iter_record_chunk_to_plot(
        &self,
        piece_offset: PieceOffset,
    ) -> impl IndexedParallelIterator<Item = Option<usize>> + '_ {
        let piece_offset = usize::from(piece_offset);
        (u16::from(SBucket::ZERO)..=u16::from(SBucket::MAX))
            .into_par_iter()
            .map(SBucket::from)
            // In each s-bucket map all records used
            .map(move |s_bucket| {
                let byte_offset = usize::from(s_bucket) / u8::BITS as usize;
                let bit_mask = 1 << (usize::from(s_bucket) % u8::BITS as usize);

                if self.record_chunks_used[piece_offset][byte_offset] & bit_mask == 0 {
                    return None;
                }

                // How many other record chunks we have in s-bucket before piece offset we care
                // about
                let chunk_offset = self
                    .record_chunks_used
                    .iter()
                    .take(piece_offset)
                    .filter(move |record_chunks_used| {
                        record_chunks_used[byte_offset] & bit_mask != 0
                    })
                    .count();

                Some(chunk_offset)
            })
    }

    /// Creates an iterator of piece offsets to which corresponding chunks belong.
    ///
    /// Returns error if `s_bucket` is outside of [`Record::NUM_S_BUCKETS`] range.
    pub fn iter_s_bucket_piece_offsets(
        &self,
        s_bucket: SBucket,
    ) -> Result<impl Iterator<Item = PieceOffset> + '_, SectorContentsMapIterationError> {
        let s_bucket = usize::from(s_bucket);

        if s_bucket >= Record::NUM_S_BUCKETS {
            return Err(SectorContentsMapIterationError::SBucketOutOfRange {
                provided: s_bucket,
                max: Record::NUM_S_BUCKETS,
            });
        }

        Ok((PieceOffset::ZERO..)
            .zip(&self.record_chunks_used)
            .filter_map(move |(piece_offset, record_chunks_used)| {
                let byte_offset = s_bucket / u8::BITS as usize;
                let bit_mask = 1 << (s_bucket % u8::BITS as usize);

                (record_chunks_used[byte_offset] & bit_mask != 0).then_some(piece_offset)
            }))
    }

    /// Iterate over chunks of s-bucket indicating if record chunk is used at corresponding
    /// position.
    ///
    /// ## Panics
    /// Panics if `s_bucket` is outside of [`Record::NUM_S_BUCKETS`] range.
    pub fn iter_s_bucket_used_record_chunks_used(
        &self,
        s_bucket: SBucket,
    ) -> Result<impl Iterator<Item = bool> + '_, SectorContentsMapIterationError> {
        let s_bucket = usize::from(s_bucket);

        if s_bucket >= Record::NUM_S_BUCKETS {
            return Err(SectorContentsMapIterationError::SBucketOutOfRange {
                provided: s_bucket,
                max: Record::NUM_S_BUCKETS,
            });
        }

        Ok(self
            .record_chunks_used
            .iter()
            .map(move |record_chunks_used| {
                let byte_offset = s_bucket / u8::BITS as usize;
                let bit_mask = 1 << (s_bucket % u8::BITS as usize);

                record_chunks_used[byte_offset] & bit_mask != 0
            }))
    }
}
