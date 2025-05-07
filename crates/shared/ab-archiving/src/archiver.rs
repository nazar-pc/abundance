use crate::objects::{BlockObject, GlobalObject};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::Record;
use ab_core_primitives::segments::{
    ArchivedBlockProgress, ArchivedHistorySegment, LastArchivedBlock, RecordedHistorySegment,
    SegmentHeader, SegmentIndex, SegmentRoot,
};
use ab_erasure_coding::ErasureCoding;
use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::num::NonZeroU32;
use core::ops::Deref;
use parity_scale_codec::{Decode, Encode, Input, Output};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

struct ArchivedHistorySegmentOutput<'a> {
    segment: &'a mut ArchivedHistorySegment,
    offset: usize,
}

impl Output for ArchivedHistorySegmentOutput<'_> {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let piece = self
                .segment
                .get_mut(self.offset / Record::SIZE)
                .expect("Encoding never exceeds the segment size; qed");
            let output = &mut piece.record_mut().as_flattened_mut()[self.offset % Record::SIZE..];
            let bytes_to_write = output.len().min(bytes.len());
            output[..bytes_to_write].copy_from_slice(&bytes[..bytes_to_write]);
            self.offset += bytes_to_write;
            bytes = &bytes[bytes_to_write..];
        }
    }
}

/// Segment represents a collection of items stored in archival history of the Subspace blockchain
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Segment {
    /// Segment items
    pub items: Vec<SegmentItem>,
}

impl Encode for Segment {
    #[inline(always)]
    fn size_hint(&self) -> usize {
        RecordedHistorySegment::SIZE
    }

    #[inline]
    fn encode_to<O: Output + ?Sized>(&self, dest: &mut O) {
        for item in &self.items {
            item.encode_to(dest);
        }
    }
}

impl Decode for Segment {
    #[inline]
    fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let mut items = Vec::new();
        loop {
            match input.remaining_len()? {
                Some(0) => {
                    break;
                }
                Some(_) => {
                    // Processing continues below
                }
                None => {
                    return Err(
                        "Source doesn't report remaining length, decoding not possible".into(),
                    );
                }
            }

            match SegmentItem::decode(input) {
                Ok(item) => {
                    items.push(item);
                }
                Err(error) => {
                    return Err(error.chain("Could not decode `Segment::items`"));
                }
            }
        }

        Ok(Self { items })
    }
}

/// Similar to `Vec<u8>`, but when encoded with SCALE codec uses fixed size length encoding (as
/// little-endian `u32`)
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockBytes(Vec<u8>);

impl Deref for BlockBytes {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<BlockBytes> for Vec<u8> {
    #[inline(always)]
    fn from(value: BlockBytes) -> Self {
        value.0
    }
}

impl Encode for BlockBytes {
    #[inline(always)]
    fn size_hint(&self) -> usize {
        size_of::<u32>() + self.0.len()
    }

    #[inline]
    fn encode_to<O: Output + ?Sized>(&self, dest: &mut O) {
        let length = u32::try_from(self.0.len())
            .expect("All constructors guarantee the size doesn't exceed `u32`; qed");

        length.encode_to(dest);
        dest.write(&self.0);
    }
}

impl Decode for BlockBytes {
    #[inline]
    fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let length = u32::decode(input)?;
        if length as usize > (RecordedHistorySegment::SIZE - size_of::<u32>()) {
            return Err("Segment item size is impossibly large".into());
        }
        // TODO: It is inefficient to zero it, but there is no API for it right now and actually
        //  implementation in `parity-scale-codec` itself is unsound:
        //  https://github.com/paritytech/parity-scale-codec/pull/605#discussion_r2076151291
        let mut bytes = vec![0; length as usize];
        input.read(&mut bytes)?;
        Ok(Self(bytes))
    }
}

impl BlockBytes {
    #[inline(always)]
    fn truncate(&mut self, size: usize) {
        self.0.truncate(size)
    }
}

/// Kinds of items that are contained within a segment
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum SegmentItem {
    /// Special dummy enum variant only used as an implementation detail for padding purposes
    #[codec(index = 0)]
    Padding,
    /// Contains full block inside
    #[codec(index = 1)]
    Block {
        /// Block bytes
        bytes: BlockBytes,
        /// This is a convenience implementation detail and will not be available on decoding
        #[doc(hidden)]
        #[codec(skip)]
        block_objects: Vec<BlockObject>,
    },
    /// Contains the beginning of the block inside, remainder will be found in subsequent segments
    #[codec(index = 2)]
    BlockStart {
        /// Block bytes
        bytes: BlockBytes,
        /// This is a convenience implementation detail and will not be available on decoding
        #[doc(hidden)]
        #[codec(skip)]
        block_objects: Vec<BlockObject>,
    },
    /// Continuation of the partial block spilled over into the next segment
    #[codec(index = 3)]
    BlockContinuation {
        /// Block bytes
        bytes: BlockBytes,
        /// This is a convenience implementation detail and will not be available on decoding
        #[doc(hidden)]
        #[codec(skip)]
        block_objects: Vec<BlockObject>,
    },
    /// Segment header of the parent
    #[codec(index = 4)]
    ParentSegmentHeader(SegmentHeader),
}

/// Newly archived segment as a combination of segment header and corresponding archived history
/// segment containing pieces
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NewArchivedSegment {
    /// Segment header
    pub segment_header: SegmentHeader,
    /// Segment of archived history containing pieces
    pub pieces: ArchivedHistorySegment,
}

/// The outcome of adding a block to the archiver.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ArchiveBlockOutcome {
    /// The new segments archived after adding the block.
    /// There can be zero or more segments created after each block.
    pub archived_segments: Vec<NewArchivedSegment>,

    /// The new object mappings for those segments.
    /// There can be zero or more mappings created after each block.
    pub global_objects: Vec<GlobalObject>,
}

/// Archiver instantiation error
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, thiserror::Error)]
pub enum ArchiverInstantiationError {
    /// Invalid last archived block, its size is the same as the encoded block
    /// (so it should have been completely archived, not partially archived)
    #[error("Invalid last archived block, its size {0} bytes is the same as the encoded block")]
    InvalidLastArchivedBlock(u32),
    /// Invalid block, its size is smaller than the already archived number of bytes
    #[error(
        "Invalid block, its size {block_bytes} bytes is smaller than the already archived block \
        {archived_block_bytes} bytes"
    )]
    InvalidBlockSmallSize {
        /// Full block size
        block_bytes: u32,
        /// Already archived portion of the block
        archived_block_bytes: u32,
    },
}

/// Block archiver for Subspace blockchain.
///
/// It takes new confirmed (at `K` depth) blocks and concatenates them into a buffer, buffer is
/// sliced into segments of [`RecordedHistorySegment::SIZE`] size, segments are sliced into source
/// records of [`Record::SIZE`], records are erasure coded, committed to, then roots with proofs are
/// appended and records become pieces that are returned alongside the corresponding segment header.
///
/// ## Panics
/// Panics when operating on blocks, whose length doesn't fit into u32 (should never be the case in
/// blockchain context anyway).
#[derive(Debug, Clone)]
pub struct Archiver {
    /// Buffer containing blocks and other buffered items that are pending to be included into the
    /// next segment
    buffer: VecDeque<SegmentItem>,
    /// Erasure coding data structure
    erasure_coding: ErasureCoding,
    /// An index of the current segment
    segment_index: SegmentIndex,
    /// Hash of the segment header of the previous segment
    prev_segment_header_hash: Blake3Hash,
    /// Last archived block
    last_archived_block: Option<LastArchivedBlock>,
}

impl Archiver {
    /// Create a new instance
    pub fn new(erasure_coding: ErasureCoding) -> Self {
        Self {
            buffer: VecDeque::default(),
            erasure_coding,
            segment_index: SegmentIndex::ZERO,
            prev_segment_header_hash: Blake3Hash::default(),
            last_archived_block: None,
        }
    }

    /// Create a new instance of the archiver with initial state in case of restart.
    ///
    /// `block` corresponds to `last_archived_block` and will be processed according to its state.
    pub fn with_initial_state(
        erasure_coding: ErasureCoding,
        segment_header: SegmentHeader,
        encoded_block: &[u8],
        mut block_objects: Vec<BlockObject>,
    ) -> Result<Self, ArchiverInstantiationError> {
        let mut archiver = Self::new(erasure_coding);

        archiver.segment_index = segment_header.segment_index() + SegmentIndex::ONE;
        archiver.prev_segment_header_hash = segment_header.hash();
        archiver.last_archived_block = Some(segment_header.last_archived_block);

        // The first thing in the buffer should be segment header
        archiver
            .buffer
            .push_back(SegmentItem::ParentSegmentHeader(segment_header));

        if let Some(archived_block_bytes) = archiver
            .last_archived_block
            .expect("Just inserted; qed")
            .partial_archived()
        {
            let archived_block_bytes = archived_block_bytes.get();
            let encoded_block_bytes = u32::try_from(encoded_block.len())
                .expect("Blocks length is never bigger than u32; qed");

            match encoded_block_bytes.cmp(&archived_block_bytes) {
                Ordering::Less => {
                    return Err(ArchiverInstantiationError::InvalidBlockSmallSize {
                        block_bytes: encoded_block_bytes,
                        archived_block_bytes,
                    });
                }
                Ordering::Equal => {
                    return Err(ArchiverInstantiationError::InvalidLastArchivedBlock(
                        encoded_block_bytes,
                    ));
                }
                Ordering::Greater => {
                    // Take part of the encoded block that wasn't archived yet and push to the
                    // buffer as a block continuation
                    block_objects.retain_mut(|block_object: &mut BlockObject| {
                        if block_object.offset >= archived_block_bytes {
                            block_object.offset -= archived_block_bytes;
                            true
                        } else {
                            false
                        }
                    });
                    archiver.buffer.push_back(SegmentItem::BlockContinuation {
                        bytes: BlockBytes(
                            encoded_block[(archived_block_bytes as usize)..].to_vec(),
                        ),
                        block_objects,
                    });
                }
            }
        }

        Ok(archiver)
    }

    /// Get last archived block if there was any
    pub fn last_archived_block_number(&self) -> Option<BlockNumber> {
        self.last_archived_block
            .map(|last_archived_block| last_archived_block.number())
    }

    /// Adds new block to internal buffer, potentially producing pieces, segment headers, and
    /// object mappings.
    ///
    /// Returns `None` if block is empty or larger than `u32::MAX`.
    pub fn add_block(
        &mut self,
        bytes: Vec<u8>,
        block_objects: Vec<BlockObject>,
    ) -> Option<ArchiveBlockOutcome> {
        if !(1..u32::MAX as usize).contains(&bytes.len()) {
            return None;
        }

        // Append new block to the buffer
        self.buffer.push_back(SegmentItem::Block {
            bytes: BlockBytes(bytes),
            block_objects,
        });

        let mut archived_segments = Vec::new();
        let mut object_mapping = Vec::new();

        // Add completed segments and their mappings for this block.
        while let Some(mut segment) = self.produce_segment() {
            // Produce any segment mappings that haven't already been produced.
            object_mapping.extend(Self::produce_object_mappings(
                self.segment_index,
                segment.items.iter_mut(),
            ));
            archived_segments.push(self.produce_archived_segment(segment));
        }

        // Produce any next segment buffer mappings that haven't already been produced.
        object_mapping.extend(self.produce_next_segment_mappings());

        Some(ArchiveBlockOutcome {
            archived_segments,
            global_objects: object_mapping,
        })
    }

    /// Try to slice buffer contents into segments if there is enough data, producing one segment at
    /// a time
    fn produce_segment(&mut self) -> Option<Segment> {
        let mut segment = Segment {
            items: Vec::with_capacity(self.buffer.len()),
        };

        let mut last_archived_block = self.last_archived_block;

        let mut segment_size = segment.encoded_size();

        // TODO: It is possible to simplify this whole loop to `if` in case "in progress" segment
        //  with precomputed size is stored somewhere already
        // 6 bytes is just large enough to encode a segment item (1 byte for enum variant, 4 bytes
        // for length and 1 for the actual data, while segment header item is never the last one)
        while RecordedHistorySegment::SIZE.saturating_sub(segment_size) >= 6 {
            let segment_item = match self.buffer.pop_front() {
                Some(segment_item) => segment_item,
                None => {
                    // Push all of the items back into the buffer, we don't have enough data yet
                    for segment_item in segment.items.into_iter().rev() {
                        self.buffer.push_front(segment_item);
                    }

                    return None;
                }
            };

            let segment_item_encoded_size = segment_item.encoded_size();
            segment_size += segment_item_encoded_size;

            // Check if there is an excess of data that should be spilled over into the next segment
            let spill_over = segment_size
                .checked_sub(RecordedHistorySegment::SIZE)
                .unwrap_or_default();

            let segment_item = match segment_item {
                SegmentItem::Padding => {
                    unreachable!("Buffer never contains SegmentItem::Padding; qed");
                }
                SegmentItem::Block {
                    mut bytes,
                    mut block_objects,
                } => {
                    let last_archived_block =
                        if let Some(last_archived_block) = &mut last_archived_block {
                            // Increase the archived block number and assume the whole block was
                            // archived (spill over checked below)
                            last_archived_block
                                .number
                                .replace(last_archived_block.number() + BlockNumber::ONE);
                            last_archived_block.set_complete();
                            last_archived_block
                        } else {
                            // Genesis block
                            last_archived_block.insert(LastArchivedBlock {
                                number: BlockNumber::ZERO.into(),
                                archived_progress: ArchivedBlockProgress::new_complete(),
                            })
                        };

                    if spill_over == 0 {
                        SegmentItem::Block {
                            bytes,
                            block_objects,
                        }
                    } else {
                        let split_point = bytes.len() - spill_over;

                        {
                            let continuation_bytes = bytes[split_point..].to_vec();
                            let continuation_block_objects = block_objects
                                .extract_if(.., |block_object: &mut BlockObject| {
                                    if block_object.offset >= split_point as u32 {
                                        block_object.offset -= split_point as u32;
                                        true
                                    } else {
                                        false
                                    }
                                })
                                .collect();

                            // Push a continuation element back into the buffer where the removed
                            // segment item was
                            self.buffer.push_front(SegmentItem::BlockContinuation {
                                bytes: BlockBytes(continuation_bytes),
                                block_objects: continuation_block_objects,
                            });
                        }

                        bytes.truncate(split_point);
                        // Update last archived block to include partial archiving info
                        let archived_bytes = u32::try_from(split_point)
                            .ok()
                            .and_then(NonZeroU32::new)
                            .expect(
                                "`::add_block()` method ensures block is not empty and doesn't \
                                exceed `u32::MAX`; qed",
                            );
                        last_archived_block.set_partial_archived(archived_bytes);

                        SegmentItem::BlockStart {
                            bytes,
                            block_objects,
                        }
                    }
                }
                SegmentItem::BlockStart { .. } => {
                    unreachable!("Buffer never contains SegmentItem::BlockStart; qed");
                }
                SegmentItem::BlockContinuation {
                    mut bytes,
                    mut block_objects,
                } => {
                    let last_archived_block = last_archived_block.as_mut().expect(
                        "Block continuation implies that there are some bytes archived \
                        already; qed",
                    );

                    let previously_archived_bytes = last_archived_block.partial_archived().expect(
                        "Block continuation implies that there are some bytes archived \
                        already; qed",
                    );

                    if spill_over == 0 {
                        last_archived_block.set_complete();

                        SegmentItem::BlockContinuation {
                            bytes,
                            block_objects,
                        }
                    } else {
                        let split_point = bytes.len() - spill_over;

                        {
                            let continuation_bytes = bytes[split_point..].to_vec();
                            let continuation_block_objects = block_objects
                                .extract_if(.., |block_object: &mut BlockObject| {
                                    if block_object.offset >= split_point as u32 {
                                        block_object.offset -= split_point as u32;
                                        true
                                    } else {
                                        false
                                    }
                                })
                                .collect();
                            // Push a continuation element back into the buffer where the removed
                            // segment item was
                            self.buffer.push_front(SegmentItem::BlockContinuation {
                                bytes: BlockBytes(continuation_bytes),
                                block_objects: continuation_block_objects,
                            });
                        }

                        bytes.truncate(split_point);
                        // Update last archived block to include partial archiving info
                        let archived_bytes = previously_archived_bytes.get()
                            + u32::try_from(split_point).expect(
                                "`::add_block()` method ensures block length doesn't \
                                    exceed `u32::MAX`; qed",
                            );
                        let archived_bytes = NonZeroU32::new(archived_bytes).expect(
                            "Spillover means non-zero length of the block was archived; qed",
                        );
                        last_archived_block.set_partial_archived(archived_bytes);

                        SegmentItem::BlockContinuation {
                            bytes,
                            block_objects,
                        }
                    }
                }
                SegmentItem::ParentSegmentHeader(parent_segment_header) => {
                    // We are not interested in segment header here
                    SegmentItem::ParentSegmentHeader(parent_segment_header)
                }
            };

            segment.items.push(segment_item);
        }

        self.last_archived_block = last_archived_block;

        Some(segment)
    }

    /// Produce object mappings for the buffered items for the next segment. Then remove the
    /// mappings in those items.
    ///
    /// Must only be called after all complete segments for a block have been produced. Before
    /// that, the buffer can contain a `BlockContinuation` which spans multiple segments.
    fn produce_next_segment_mappings(&mut self) -> Vec<GlobalObject> {
        Self::produce_object_mappings(self.segment_index, self.buffer.iter_mut())
    }

    /// Produce object mappings for `items` in `segment_index`. Then remove the mappings from those
    /// items.
    ///
    /// This method can be called on a `Segment`’s items, or on the `Archiver`'s internal buffer.
    fn produce_object_mappings<'a>(
        segment_index: SegmentIndex,
        items: impl Iterator<Item = &'a mut SegmentItem>,
    ) -> Vec<GlobalObject> {
        let source_piece_indexes =
            &segment_index.segment_piece_indexes()[..RecordedHistorySegment::NUM_RAW_RECORDS];

        let mut corrected_object_mapping = Vec::new();
        let mut base_offset_in_segment = Segment::default().encoded_size();
        for segment_item in items {
            match segment_item {
                SegmentItem::Padding => {
                    unreachable!(
                        "Segment during archiving never contains SegmentItem::Padding; qed"
                    );
                }
                SegmentItem::Block {
                    bytes: _,
                    block_objects,
                }
                | SegmentItem::BlockStart {
                    bytes: _,
                    block_objects,
                }
                | SegmentItem::BlockContinuation {
                    bytes: _,
                    block_objects,
                } => {
                    for block_object in block_objects.drain(..) {
                        // `+1` corresponds to `SegmentItem::X {}` enum variant encoding
                        let offset_in_segment = base_offset_in_segment
                            + 1
                            + u32::encoded_fixed_size().expect("Fixed size; qed")
                            + block_object.offset as usize;
                        let raw_piece_offset = (offset_in_segment % Record::SIZE)
                            .try_into()
                            .expect("Offset within piece should always fit in 32-bit integer; qed");
                        corrected_object_mapping.push(GlobalObject {
                            hash: block_object.hash,
                            piece_index: source_piece_indexes[offset_in_segment / Record::SIZE],
                            offset: raw_piece_offset,
                        });
                    }
                }
                SegmentItem::ParentSegmentHeader(_) => {
                    // Ignore, no object mappings here
                }
            }

            base_offset_in_segment += segment_item.encoded_size();
        }

        corrected_object_mapping
    }

    /// Take segment as an input, apply necessary transformations and produce archived segment
    fn produce_archived_segment(&mut self, segment: Segment) -> NewArchivedSegment {
        let mut pieces = {
            let mut pieces = ArchivedHistorySegment::default();

            segment.encode_to(&mut ArchivedHistorySegmentOutput {
                segment: &mut pieces,
                offset: 0,
            });
            // Segment is quite big and no longer necessary
            drop(segment);

            let (source_shards, parity_shards) =
                pieces.split_at_mut(RecordedHistorySegment::NUM_RAW_RECORDS);

            self.erasure_coding
                .extend(
                    source_shards.iter().map(|shard| shard.record()),
                    parity_shards.iter_mut().map(|shard| shard.record_mut()),
                )
                .expect("Statically correct parameters; qed");

            pieces
        };

        // Collect hashes to roots from all records
        let record_roots = {
            #[cfg(not(feature = "parallel"))]
            let source_pieces = pieces.iter_mut();
            #[cfg(feature = "parallel")]
            let source_pieces = pieces.par_iter_mut();

            // Here we build a tree of record chunks, with the first half being source chunks as
            // they are originally and the second half being parity chunks. While we build tree
            // threes here (for source chunks, parity chunks and combined for the whole record), it
            // could have been a single tree, and it would end up with the same root. Building them
            // separately requires less RAM and allows to capture parity chunks root more easily.
            let iter = source_pieces.map(|piece| {
                let [source_chunks_root, parity_chunks_root] = {
                    let mut parity_chunks = Record::new_boxed();

                    self.erasure_coding
                        .extend(piece.record().iter(), parity_chunks.iter_mut())
                        .expect(
                            "Erasure coding instance is deliberately configured to support this \
                            input; qed",
                        );

                    let source_chunks_root =
                        BalancedHashedMerkleTree::compute_root_only(piece.record());
                    let parity_chunks_root =
                        BalancedHashedMerkleTree::compute_root_only(&parity_chunks);

                    [source_chunks_root, parity_chunks_root]
                };

                let record_root = BalancedHashedMerkleTree::compute_root_only(&[
                    source_chunks_root,
                    parity_chunks_root,
                ]);

                piece.root_mut().copy_from_slice(&record_root);
                piece
                    .parity_chunks_root_mut()
                    .copy_from_slice(&parity_chunks_root);

                record_root
            });

            iter.collect::<Vec<_>>()
        };

        let segment_merkle_tree =
            BalancedHashedMerkleTree::<{ ArchivedHistorySegment::NUM_PIECES }>::new_boxed(
                record_roots
                    .as_slice()
                    .try_into()
                    .expect("Statically guaranteed to have correct length; qed"),
            );

        let segment_root = SegmentRoot::from(segment_merkle_tree.root());

        // Create proof for every record and write it to corresponding piece.
        pieces
            .iter_mut()
            .zip(segment_merkle_tree.all_proofs())
            .for_each(|(piece, record_proof)| {
                piece.proof_mut().copy_from_slice(&record_proof);
            });

        // Now produce segment header
        let segment_header = SegmentHeader {
            segment_index: self.segment_index.into(),
            segment_root,
            prev_segment_header_hash: self.prev_segment_header_hash,
            last_archived_block: self
                .last_archived_block
                .expect("Never empty by the time segment is produced; qed"),
        };

        // Update state
        self.segment_index += SegmentIndex::ONE;
        self.prev_segment_header_hash = segment_header.hash();

        // Add segment header to the beginning of the buffer to be the first thing included in the
        // next segment
        self.buffer
            .push_front(SegmentItem::ParentSegmentHeader(segment_header));

        NewArchivedSegment {
            segment_header,
            pieces: pieces.to_shared(),
        }
    }
}
