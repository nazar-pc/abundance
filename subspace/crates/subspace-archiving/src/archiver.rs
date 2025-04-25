use ab_erasure_coding::ErasureCoding;
use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cmp::Ordering;
use parity_scale_codec::{Compact, CompactLen, Decode, Encode, Input, Output};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use subspace_core_primitives::BlockNumber;
use subspace_core_primitives::hashes::Blake3Hash;
use subspace_core_primitives::objects::{BlockObject, BlockObjectMapping, GlobalObject};
use subspace_core_primitives::pieces::Record;
use subspace_core_primitives::segments::{
    ArchivedBlockProgress, ArchivedHistorySegment, LastArchivedBlock, RecordedHistorySegment,
    SegmentHeader, SegmentIndex, SegmentRoot,
};

const INITIAL_LAST_ARCHIVED_BLOCK: LastArchivedBlock = LastArchivedBlock {
    number: 0,
    // Special case for the genesis block.
    //
    // When we start archiving process with pre-genesis objects, we do not yet have any blocks
    // archived, but `LastArchivedBlock` is required for `SegmentHeader`s to be produced, so
    // `ArchivedBlockProgress::Partial(0)` indicates that we have archived 0 bytes of block `0` (see
    // field above), meaning we did not in fact archive actual blocks yet.
    archived_progress: ArchivedBlockProgress::Partial(0),
};

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
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Segment {
    // V0 of the segment data structure
    V0 {
        /// Segment items
        items: Vec<SegmentItem>,
    },
}

impl Default for Segment {
    fn default() -> Self {
        Segment::V0 { items: Vec::new() }
    }
}

impl Encode for Segment {
    fn size_hint(&self) -> usize {
        RecordedHistorySegment::SIZE
    }

    fn encode_to<O: Output + ?Sized>(&self, dest: &mut O) {
        match self {
            Segment::V0 { items } => {
                dest.push_byte(0);
                for item in items {
                    item.encode_to(dest);
                }
            }
        }
    }
}

impl Decode for Segment {
    fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let variant = input
            .read_byte()
            .map_err(|e| e.chain("Could not decode `Segment`, failed to read variant byte"))?;
        match variant {
            0 => {
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
                                "Source doesn't report remaining length, decoding not possible"
                                    .into(),
                            );
                        }
                    }

                    match SegmentItem::decode(input) {
                        Ok(item) => {
                            items.push(item);
                        }
                        Err(error) => {
                            return Err(error.chain("Could not decode `Segment::V0::items`"));
                        }
                    }
                }

                Ok(Segment::V0 { items })
            }
            _ => Err("Could not decode `Segment`, variant doesn't exist".into()),
        }
    }
}

impl Segment {
    fn push_item(&mut self, segment_item: SegmentItem) {
        let Self::V0 { items } = self;
        items.push(segment_item);
    }

    pub fn items(&self) -> &[SegmentItem] {
        match self {
            Segment::V0 { items } => items,
        }
    }

    pub(crate) fn items_mut(&mut self) -> &mut Vec<SegmentItem> {
        match self {
            Segment::V0 { items } => items,
        }
    }

    pub fn into_items(self) -> Vec<SegmentItem> {
        match self {
            Segment::V0 { items } => items,
        }
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
        bytes: Vec<u8>,
        /// This is a convenience implementation detail and will not be available on decoding
        #[doc(hidden)]
        #[codec(skip)]
        object_mapping: BlockObjectMapping,
    },
    /// Contains the beginning of the block inside, remainder will be found in subsequent segments
    #[codec(index = 2)]
    BlockStart {
        /// Block bytes
        bytes: Vec<u8>,
        /// This is a convenience implementation detail and will not be available on decoding
        #[doc(hidden)]
        #[codec(skip)]
        object_mapping: BlockObjectMapping,
    },
    /// Continuation of the partial block spilled over into the next segment
    #[codec(index = 3)]
    BlockContinuation {
        /// Block bytes
        bytes: Vec<u8>,
        /// This is a convenience implementation detail and will not be available on decoding
        #[doc(hidden)]
        #[codec(skip)]
        object_mapping: BlockObjectMapping,
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
    pub object_mapping: Vec<GlobalObject>,
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
    last_archived_block: LastArchivedBlock,
}

impl Archiver {
    /// Create a new instance
    pub fn new(erasure_coding: ErasureCoding) -> Self {
        Self {
            buffer: VecDeque::default(),
            erasure_coding,
            segment_index: SegmentIndex::ZERO,
            prev_segment_header_hash: Blake3Hash::default(),
            last_archived_block: INITIAL_LAST_ARCHIVED_BLOCK,
        }
    }

    /// Create a new instance of the archiver with initial state in case of restart.
    ///
    /// `block` corresponds to `last_archived_block` and will be processed according to its state.
    pub fn with_initial_state(
        erasure_coding: ErasureCoding,
        segment_header: SegmentHeader,
        encoded_block: &[u8],
        mut object_mapping: BlockObjectMapping,
    ) -> Result<Self, ArchiverInstantiationError> {
        let mut archiver = Self::new(erasure_coding);

        archiver.segment_index = segment_header.segment_index() + SegmentIndex::ONE;
        archiver.prev_segment_header_hash = segment_header.hash();
        archiver.last_archived_block = segment_header.last_archived_block();

        // The first thing in the buffer should be segment header
        archiver
            .buffer
            .push_back(SegmentItem::ParentSegmentHeader(segment_header));

        if let Some(archived_block_bytes) = archiver.last_archived_block.partial_archived() {
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
                    object_mapping
                        .objects_mut()
                        .retain_mut(|block_object: &mut BlockObject| {
                            if block_object.offset >= archived_block_bytes {
                                block_object.offset -= archived_block_bytes;
                                true
                            } else {
                                false
                            }
                        });
                    archiver.buffer.push_back(SegmentItem::BlockContinuation {
                        bytes: encoded_block[(archived_block_bytes as usize)..].to_vec(),
                        object_mapping,
                    });
                }
            }
        }

        Ok(archiver)
    }

    /// Get last archived block if there was any
    pub fn last_archived_block_number(&self) -> Option<BlockNumber> {
        if self.last_archived_block != INITIAL_LAST_ARCHIVED_BLOCK {
            Some(self.last_archived_block.number)
        } else {
            None
        }
    }

    /// Adds new block to internal buffer, potentially producing pieces, segment headers, and
    /// object mappings
    pub fn add_block(
        &mut self,
        bytes: Vec<u8>,
        object_mapping: BlockObjectMapping,
    ) -> ArchiveBlockOutcome {
        // Append new block to the buffer
        self.buffer.push_back(SegmentItem::Block {
            bytes,
            object_mapping,
        });

        let mut archived_segments = Vec::new();
        let mut object_mapping = Vec::new();

        // Add completed segments and their mappings for this block.
        while let Some(mut segment) = self.produce_segment() {
            // Produce any segment mappings that haven't already been produced.
            object_mapping.extend(Self::produce_object_mappings(
                self.segment_index,
                segment.items_mut().iter_mut(),
            ));
            archived_segments.push(self.produce_archived_segment(segment));
        }

        // Produce any next segment buffer mappings that haven't already been produced.
        object_mapping.extend(self.produce_next_segment_mappings());

        ArchiveBlockOutcome {
            archived_segments,
            object_mapping,
        }
    }

    /// Try to slice buffer contents into segments if there is enough data, producing one segment at
    /// a time
    fn produce_segment(&mut self) -> Option<Segment> {
        let mut segment = Segment::V0 {
            items: Vec::with_capacity(self.buffer.len()),
        };

        let mut last_archived_block = self.last_archived_block;

        let mut segment_size = segment.encoded_size();

        // `-2` because even the smallest segment item will take 2 bytes to encode, so it makes
        // sense to stop earlier here
        while segment_size < (RecordedHistorySegment::SIZE - 2) {
            let segment_item = match self.buffer.pop_front() {
                Some(segment_item) => segment_item,
                None => {
                    // Push all of the items back into the buffer, we don't have enough data yet
                    for segment_item in segment.into_items().into_iter().rev() {
                        self.buffer.push_front(segment_item);
                    }

                    return None;
                }
            };

            let segment_item_encoded_size = segment_item.encoded_size();
            segment_size += segment_item_encoded_size;

            // Check if there would be enough data collected with above segment item inserted
            if segment_size >= RecordedHistorySegment::SIZE {
                // Check if there is an excess of data that should be spilled over into the next
                // segment
                let spill_over = segment_size - RecordedHistorySegment::SIZE;

                // Due to compact vector length encoding in scale codec, spill over might happen to
                // be the same or even bigger than the inserted segment item bytes, in which case
                // last segment item insertion needs to be skipped to avoid out of range panic when
                // trying to cut segment item internal bytes.
                let inner_bytes_size = match &segment_item {
                    SegmentItem::Padding => {
                        unreachable!("Buffer never contains SegmentItem::Padding; qed");
                    }
                    SegmentItem::Block { bytes, .. } => bytes.len(),
                    SegmentItem::BlockStart { .. } => {
                        unreachable!("Buffer never contains SegmentItem::BlockStart; qed");
                    }
                    SegmentItem::BlockContinuation { bytes, .. } => bytes.len(),
                    SegmentItem::ParentSegmentHeader(_) => {
                        unreachable!(
                            "SegmentItem::SegmentHeader is always the first element in the buffer \
                            and fits into the segment; qed",
                        );
                    }
                };

                if spill_over > inner_bytes_size {
                    self.buffer.push_front(segment_item);
                    segment_size -= segment_item_encoded_size;
                    break;
                }
            }

            match &segment_item {
                SegmentItem::Padding => {
                    unreachable!("Buffer never contains SegmentItem::Padding; qed");
                }
                SegmentItem::Block { .. } => {
                    // Skip block number increase in case of the very first block
                    if last_archived_block != INITIAL_LAST_ARCHIVED_BLOCK {
                        // Increase archived block number and assume the whole block was
                        // archived
                        last_archived_block.number += 1;
                    }
                    last_archived_block.set_complete();
                }
                SegmentItem::BlockStart { .. } => {
                    unreachable!("Buffer never contains SegmentItem::BlockStart; qed");
                }
                SegmentItem::BlockContinuation { bytes, .. } => {
                    // Same block, but assume for now that the whole block was archived, but
                    // also store the number of bytes as opposed to `None`, we'll transform
                    // it into `None` if needed later
                    let archived_bytes = last_archived_block.partial_archived().expect(
                        "Block continuation implies that there are some bytes \
                            archived already; qed",
                    );
                    last_archived_block.set_partial_archived(
                        archived_bytes
                            + u32::try_from(bytes.len())
                                .expect("Blocks length is never bigger than u32; qed"),
                    );
                }
                SegmentItem::ParentSegmentHeader(_) => {
                    // We are not interested in segment header here
                }
            }

            segment.push_item(segment_item);
        }

        // Check if there is an excess of data that should be spilled over into the next segment
        let spill_over = segment_size
            .checked_sub(RecordedHistorySegment::SIZE)
            .unwrap_or_default();

        if spill_over > 0 {
            let items = segment.items_mut();
            let segment_item = items
                .pop()
                .expect("Segment over segment size always has at least one item; qed");

            let segment_item = match segment_item {
                SegmentItem::Padding => {
                    unreachable!("Buffer never contains SegmentItem::Padding; qed");
                }
                SegmentItem::Block {
                    mut bytes,
                    mut object_mapping,
                } => {
                    let split_point = bytes.len() - spill_over;
                    let continuation_bytes = bytes[split_point..].to_vec();

                    bytes.truncate(split_point);

                    let continuation_object_mapping = BlockObjectMapping::V0 {
                        objects: object_mapping
                            .objects_mut()
                            .extract_if(.., |block_object: &mut BlockObject| {
                                if block_object.offset >= split_point as u32 {
                                    block_object.offset -= split_point as u32;
                                    true
                                } else {
                                    false
                                }
                            })
                            .collect(),
                    };

                    // Update last archived block to include partial archiving info
                    last_archived_block.set_partial_archived(
                        u32::try_from(bytes.len())
                            .expect("Blocks length is never bigger than u32; qed"),
                    );

                    // Push continuation element back into the buffer where removed segment item was
                    self.buffer.push_front(SegmentItem::BlockContinuation {
                        bytes: continuation_bytes,
                        object_mapping: continuation_object_mapping,
                    });

                    SegmentItem::BlockStart {
                        bytes,
                        object_mapping,
                    }
                }
                SegmentItem::BlockStart { .. } => {
                    unreachable!("Buffer never contains SegmentItem::BlockStart; qed");
                }
                SegmentItem::BlockContinuation {
                    mut bytes,
                    mut object_mapping,
                } => {
                    let split_point = bytes.len() - spill_over;
                    let continuation_bytes = bytes[split_point..].to_vec();

                    bytes.truncate(split_point);

                    let continuation_object_mapping = BlockObjectMapping::V0 {
                        objects: object_mapping
                            .objects_mut()
                            .extract_if(.., |block_object: &mut BlockObject| {
                                if block_object.offset >= split_point as u32 {
                                    block_object.offset -= split_point as u32;
                                    true
                                } else {
                                    false
                                }
                            })
                            .collect(),
                    };

                    // Above code assumed that block was archived fully, now remove spilled-over
                    // bytes from the size
                    let archived_bytes = last_archived_block.partial_archived().expect(
                        "Block continuation implies that there are some bytes archived \
                        already; qed",
                    );
                    last_archived_block.set_partial_archived(
                        archived_bytes
                            - u32::try_from(spill_over)
                                .expect("Blocks length is never bigger than u32; qed"),
                    );

                    // Push continuation element back into the buffer where removed segment item was
                    self.buffer.push_front(SegmentItem::BlockContinuation {
                        bytes: continuation_bytes,
                        object_mapping: continuation_object_mapping,
                    });

                    SegmentItem::BlockContinuation {
                        bytes,
                        object_mapping,
                    }
                }
                SegmentItem::ParentSegmentHeader(_) => {
                    unreachable!(
                        "SegmentItem::SegmentHeader is always the first element in the buffer and \
                        fits into the segment; qed",
                    );
                }
            };

            // Push back shortened segment item
            items.push(segment_item);
        } else {
            // Above code added bytes length even though it was assumed that all continuation bytes
            // fit into the segment, now we need to tweak that
            last_archived_block.set_complete();
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
                    bytes,
                    object_mapping,
                }
                | SegmentItem::BlockStart {
                    bytes,
                    object_mapping,
                }
                | SegmentItem::BlockContinuation {
                    bytes,
                    object_mapping,
                } => {
                    for block_object in object_mapping.objects_mut().drain(..) {
                        // `+1` corresponds to `SegmentItem::X {}` enum variant encoding
                        let offset_in_segment = base_offset_in_segment
                            + 1
                            + Compact::compact_len(&(bytes.len() as u32))
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
        let segment_header = SegmentHeader::V0 {
            segment_index: self.segment_index,
            segment_root,
            prev_segment_header_hash: self.prev_segment_header_hash,
            last_archived_block: self.last_archived_block,
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
