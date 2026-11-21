//! Data structures related to the owned version of [`BlockBody`]

// Slow under Miri and these features don't change what the tests exercise, so under Miri they only
// run with just `alloc` or with all features (like when testing the whole workspace)
#[cfg(all(
    test,
    any(
        not(miri),
        not(any(
            feature = "ed25519",
            feature = "parallel",
            feature = "scale-codec",
            feature = "serde"
        )),
        all(
            feature = "ed25519",
            feature = "parallel",
            feature = "scale-codec",
            feature = "serde"
        ),
    ),
))]
mod tests;

use crate::block::body::{
    BeaconChainBody, BlockBody, GenericBlockBody, IntermediateShardBlockCounts,
    IntermediateShardBlockInfo, IntermediateShardBody, LeafShardBlockInfo, LeafShardBody,
    OwnSegments,
};
use crate::block::header::LeafShardHeader;
use crate::block::header::owned::{OwnedIntermediateShardHeader, OwnedLeafShardHeader};
use crate::block::{IntermediateShardBlock, LeafShardBlock};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::{LocalSegmentIndex, SegmentRoot};
use crate::shard::{RealShardKind, ShardIndex};
use crate::transaction::Transaction;
use crate::transaction::owned::OwnedTransaction;
use ab_aligned_buffer::alignment::{Align8, Align16};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::fmt;
use core::iter::{Step, TrustedLen};
use derive_more::From;
use triomphe::Arc;
use yoke::{Yoke, Yokeable};

/// Generic owned block body
pub trait GenericOwnedBlockBody:
    Clone + fmt::Debug + Send + Sync + Into<OwnedBlockBody> + 'static
{
    /// Shard kind
    const SHARD_KIND: RealShardKind;

    /// Block body
    type Body<'a>: GenericBlockBody<'a>
    where
        Self: 'a;

    /// Create an owned body from a buffer
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer>;

    /// Inner buffer with block body contents
    fn buffer(&self) -> &SharedAlignedBuffer;

    /// Number of clones in memory
    fn ref_count(&self) -> usize;

    /// Get a regular block body out of the owned version
    fn body(&self) -> &Self::Body<'_>;
}

/// Transaction that can be written into the body.
///
/// Can't be implemented outside of this module, because block body builders rely on it appending
/// exactly one correctly encoded transaction to the buffer or leaving it unchanged.
pub impl(self) trait WritableBodyTransaction {
    /// Write this transaction into the body.
    ///
    /// Returns [`OwnedLeafShardBodyError::BlockBodyIsTooLarge`] and leaves the buffer unchanged if
    /// the body becomes too large.
    fn write_into(&self, buffer: &mut OwnedAlignedBuffer) -> Result<(), OwnedLeafShardBodyError>;
}

impl WritableBodyTransaction for Transaction<'_> {
    #[inline(always)]
    fn write_into(&self, buffer: &mut OwnedAlignedBuffer) -> Result<(), OwnedLeafShardBodyError> {
        buffer
            .append(self.bytes())
            .map_err(|_error| OwnedLeafShardBodyError::BlockBodyIsTooLarge)
    }
}

impl WritableBodyTransaction for &OwnedTransaction {
    #[inline(always)]
    fn write_into(&self, buffer: &mut OwnedAlignedBuffer) -> Result<(), OwnedLeafShardBodyError> {
        buffer
            .append(self.buffer().as_slice())
            .map_err(|_error| OwnedLeafShardBodyError::BlockBodyIsTooLarge)
    }
}

#[derive(Debug, Clone)]
struct TransactionBuilder {
    num_transactions_offset: usize,
    buffer: OwnedAlignedBuffer,
}

impl TransactionBuilder {
    fn new(num_transactions_offset: usize, buffer: OwnedAlignedBuffer) -> Self {
        Self {
            num_transactions_offset,
            buffer,
        }
    }

    /// Add transaction to the body.
    ///
    /// Returns [`OwnedLeafShardBodyError::BlockBodyIsTooLarge`] and leaves the body unchanged if it
    /// becomes too large.
    fn add_transaction<T>(&mut self, transaction: T) -> Result<(), OwnedLeafShardBodyError>
    where
        T: WritableBodyTransaction,
    {
        // The buffer is already aligned, because padding follows the number of transactions and
        // each transaction
        self.inc_transaction_count();

        let old_buffer_len = self.buffer.len();

        if let Err(error) = transaction.write_into(&mut self.buffer) {
            self.dec_transaction_count();
            return Err(error);
        }

        if self.buffer.pad_to_alignment::<Align16>().is_err() {
            self.dec_transaction_count();
            // SAFETY: Length was obtained from the same buffer before the last write
            unsafe {
                self.buffer.set_len(old_buffer_len);
            }
            return Err(OwnedLeafShardBodyError::BlockBodyIsTooLarge);
        }

        Ok(())
    }

    /// Finish building block body
    #[inline(always)]
    fn finish(self) -> OwnedAlignedBuffer {
        self.buffer
    }

    /// Increase the number of stored transactions
    #[inline(always)]
    fn inc_transaction_count(&mut self) {
        // SAFETY: Constructor ensures the offset is valid and has space for `u32` (but not
        // necessarily aligned)
        unsafe {
            let num_transactions_ptr = self
                .buffer
                .as_mut_ptr()
                .byte_add(self.num_transactions_offset)
                .cast::<u32>();
            let num_transactions = num_transactions_ptr.read_unaligned();
            let num_transactions = num_transactions.checked_add(1).expect(
                "Each transaction takes at least `TransactionHeader::SIZE` bytes of the buffer, \
                which can't be longer than `u32::MAX` bytes; qed",
            );
            num_transactions_ptr.write_unaligned(num_transactions);
        }
    }

    /// Decrease the number of stored transactions
    #[inline(always)]
    fn dec_transaction_count(&mut self) {
        // SAFETY: Constructor ensures the offset is valid and has space for `u32` (but not
        // necessarily aligned)
        unsafe {
            let num_transactions_ptr = self
                .buffer
                .as_mut_ptr()
                .byte_add(self.num_transactions_offset)
                .cast::<u32>();
            let num_transactions = num_transactions_ptr.read_unaligned();
            let num_transactions = num_transactions.saturating_sub(1);
            num_transactions_ptr.write_unaligned(num_transactions);
        }
    }
}

/// The record of a child shard block doesn't fit into the buffer
#[derive(Debug)]
struct RecordTooLarge;

/// Append the record of a leaf shard block as in an intermediate shard body, see
/// [`LeafShardBlocksInfo::try_from_bytes()`] for the layout.
///
/// The length of the buffer must be a multiple of 8 for the record to parse. Returns an error if
/// the record doesn't fit into the buffer, part of it might have been appended already.
///
/// [`LeafShardBlocksInfo::try_from_bytes()`]:
/// crate::block::body::LeafShardBlocksInfo::try_from_bytes
fn append_leaf_shard_block_record(
    buffer: &mut OwnedAlignedBuffer,
    header: &LeafShardHeader<'_>,
    transactions_root: &Blake3Hash,
    own_segments: Option<&OwnSegments<'_>>,
) -> Result<(), RecordTooLarge> {
    buffer
        .append(header.bytes())
        .map_err(|_error| RecordTooLarge)?;
    buffer
        .pad_to_alignment::<Align8>()
        .map_err(|_error| RecordTooLarge)?;

    buffer
        .append(transactions_root.as_bytes())
        .map_err(|_error| RecordTooLarge)?;
    if let Some(own_segments) = own_segments {
        buffer
            .append(own_segments.first_local_segment_index.as_bytes())
            .map_err(|_error| RecordTooLarge)?;
        buffer
            .append(SegmentRoot::slice_as_bytes(own_segments.segment_roots))
            .map_err(|_error| RecordTooLarge)?;
    }

    Ok(())
}

// TODO: Derive `Yokeable` for `IntermediateShardBlockInfo` directly once `syn` can parse `mut`
//  restrictions of its fields: https://github.com/dtolnay/syn/issues/1987
#[derive(Debug, Yokeable)]
struct YokeableIntermediateShardBlockInfo<'a>(IntermediateShardBlockInfo<'a>);

/// An owned version of [`IntermediateShardBlockInfo`].
///
/// It is well suited for sending over the network and keeping in memory independently of any block
/// until a beacon chain block includes it.
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBlockInfo {
    inner: Arc<Yoke<YokeableIntermediateShardBlockInfo<'static>, SharedAlignedBuffer>>,
}

impl OwnedIntermediateShardBlockInfo {
    /// Create an owned info of an intermediate shard block
    pub fn from_block(block: &IntermediateShardBlock<'_>) -> Self {
        let own_segments = block.body.own_segments();
        let leaf_shard_blocks = block.body.leaf_shard_blocks();

        // Counts with padding, the header with the largest seal and own segments, then for each
        // leaf shard block with segments its number of segment roots and segments, then padding
        let (num_leaf_shard_blocks_with_segments, capacity) = leaf_shard_blocks
            .leaf_shards_segments()
            .try_fold(
                (
                    0u16,
                    (u8::SIZE + 2 * u16::SIZE)
                        .next_multiple_of(u32::from(Alignment::of::<u64>().get()))
                        + OwnedIntermediateShardHeader::MAX_ALLOCATION
                        + own_segments.map_or_default(|own_segments| {
                            LocalSegmentIndex::SIZE
                                + u32::from(own_segments.num_segment_roots().get())
                                    * SegmentRoot::SIZE as u32
                        }),
                ),
                |(num_leaf_shard_blocks_with_segments, capacity),
                 (_position, _shard_index, own_segments)| {
                    Some((
                        num_leaf_shard_blocks_with_segments.checked_add(1)?,
                        capacity.checked_add(
                            u8::SIZE
                                + u16::SIZE
                                + ShardIndex::SIZE
                                + LocalSegmentIndex::SIZE
                                + u32::from(own_segments.num_segment_roots().get())
                                    * SegmentRoot::SIZE as u32,
                        )?,
                    ))
                },
            )
            .and_then(|(num_leaf_shard_blocks_with_segments, capacity)| {
                Some((
                    num_leaf_shard_blocks_with_segments,
                    capacity.checked_next_multiple_of(u32::from(Alignment::of::<u64>().get()))?,
                ))
            })
            .expect(
                "At most `u16::MAX` leaf shard blocks with at most `u8::MAX` segment roots each \
                take less than 1 GiB, which is below `OwnedAlignedBuffer::MAX_CAPACITY`; qed",
            );

        let mut buffer = OwnedAlignedBuffer::with_capacity(capacity);
        buffer
            .append(
                &IntermediateShardBlockCounts {
                    num_own_segment_roots: own_segments
                        .map_or_default(|own_segments| own_segments.num_segment_roots().get()),
                    num_leaf_shard_blocks: u16::try_from(leaf_shard_blocks.len())
                        .expect("A body stores the number of leaf shard blocks as `u16`; qed"),
                    num_leaf_shard_blocks_with_segments,
                }
                .to_bytes(),
            )
            .expect("Enough capacity was allocated above; qed");
        buffer
            .pad_to_alignment::<Align8>()
            .expect("Enough capacity was allocated above; qed");
        buffer
            .append(block.header.bytes())
            .expect("Enough capacity was allocated above; qed");
        if let Some(own_segments) = &own_segments {
            buffer
                .append(own_segments.first_local_segment_index.as_bytes())
                .expect("Enough capacity was allocated above; qed");
            buffer
                .append(SegmentRoot::slice_as_bytes(own_segments.segment_roots))
                .expect("Enough capacity was allocated above; qed");
        }
        for (_position, _shard_index, own_segments) in leaf_shard_blocks.leaf_shards_segments() {
            buffer
                .append(&[own_segments.num_segment_roots().get()])
                .expect("Enough capacity was allocated above; qed");
        }
        for (position, shard_index, own_segments) in leaf_shard_blocks.leaf_shards_segments() {
            buffer
                .append(&position.to_le_bytes())
                .expect("Enough capacity was allocated above; qed");
            buffer
                .append(shard_index.as_bytes())
                .expect("Enough capacity was allocated above; qed");
            buffer
                .append(own_segments.first_local_segment_index.as_bytes())
                .expect("Enough capacity was allocated above; qed");
            buffer
                .append(SegmentRoot::slice_as_bytes(own_segments.segment_roots))
                .expect("Enough capacity was allocated above; qed");
        }
        buffer
            .pad_to_alignment::<Align8>()
            .expect("Enough capacity was allocated above; qed");

        Self::from_buffer(buffer.into_shared()).expect(
            "Written the way the parser reads it from a block that passed the same checks; qed",
        )
    }

    /// Create an owned info from a buffer.
    ///
    /// The info is checked the same way as each info in a beacon chain body, its seal is not
    /// checked.
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((info, extra_bytes)) =
                IntermediateShardBlockInfo::try_from_standalone_bytes(buffer)
            else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(YokeableIntermediateShardBlockInfo(info))
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with info contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }

    /// Get [`IntermediateShardBlockInfo`] out of [`OwnedIntermediateShardBlockInfo`]
    #[inline(always)]
    pub fn info(&self) -> &IntermediateShardBlockInfo<'_> {
        let YokeableIntermediateShardBlockInfo(info) = self.inner.get();
        info
    }
}

// TODO: Derive `Yokeable` for `LeafShardBlockInfo` directly once `syn` can parse `mut` restrictions
//  of its fields: https://github.com/dtolnay/syn/issues/1987
#[derive(Debug, Yokeable)]
struct YokeableLeafShardBlockInfo<'a>(LeafShardBlockInfo<'a>);

/// An owned version of [`LeafShardBlockInfo`].
///
/// It is well suited for sending over the network and keeping in memory independently of any block
/// until an intermediate shard block includes it.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlockInfo {
    inner: Arc<Yoke<YokeableLeafShardBlockInfo<'static>, SharedAlignedBuffer>>,
}

impl OwnedLeafShardBlockInfo {
    /// Create an owned info of a leaf shard block
    pub fn from_block(block: &LeafShardBlock<'_>) -> Self {
        let own_segments = block.body.own_segments();

        // The number of own segment roots with padding, the header with the largest seal and
        // padding, the root of transactions and own segments
        let capacity = (u8::SIZE.next_multiple_of(u32::from(Alignment::of::<u64>().get()))
            + OwnedLeafShardHeader::MAX_ALLOCATION)
            .next_multiple_of(u32::from(Alignment::of::<u64>().get()))
            + <Blake3Hash as TrivialType>::SIZE
            + own_segments.map_or_default(|own_segments| {
                LocalSegmentIndex::SIZE
                    + u32::from(own_segments.num_segment_roots().get()) * SegmentRoot::SIZE as u32
            });

        let mut buffer = OwnedAlignedBuffer::with_capacity(capacity);
        buffer
            .append(&[
                own_segments.map_or_default(|own_segments| own_segments.num_segment_roots().get())
            ])
            .expect("Enough capacity was allocated above; qed");
        buffer
            .pad_to_alignment::<Align8>()
            .expect("Enough capacity was allocated above; qed");
        append_leaf_shard_block_record(
            &mut buffer,
            &block.header,
            &block.body.transactions().root(),
            own_segments.as_ref(),
        )
        .expect("Enough capacity was allocated above; qed");

        Self::from_buffer(buffer.into_shared()).expect(
            "Written the way the parser reads it from a block that passed the same checks; qed",
        )
    }

    /// Create an owned info from a buffer.
    ///
    /// The info is checked the same way as each info in an intermediate shard body, its seal is
    /// not checked.
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((info, extra_bytes)) = LeafShardBlockInfo::try_from_standalone_bytes(buffer)
            else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(YokeableLeafShardBlockInfo(info))
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with info contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }

    /// Get [`LeafShardBlockInfo`] out of [`OwnedLeafShardBlockInfo`]
    #[inline(always)]
    pub fn info(&self) -> &LeafShardBlockInfo<'_> {
        let YokeableLeafShardBlockInfo(info) = self.inner.get();
        info
    }
}

/// Errors for [`OwnedBeaconChainBody`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedBeaconChainBodyError {
    /// Too many PoT checkpoints
    #[error("Too many PoT checkpoints: {actual}")]
    TooManyPotCheckpoints {
        /// Actual number of PoT checkpoints
        actual: usize,
    },
    /// Too many own segment roots
    #[error("Too many own segment roots: {actual}")]
    TooManyOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Local segment index after the last own segment root doesn't fit into `u64`
    #[error("Own segment index overflow, first local segment index: {first_local_segment_index}")]
    OwnSegmentIndexOverflow {
        /// Local segment index of the first own segment root
        first_local_segment_index: LocalSegmentIndex,
    },
    /// Local segment indices of own segment roots are not consecutive
    #[error(
        "Own segment indices are not consecutive: {local_segment_index} after \
        {previous_local_segment_index}"
    )]
    NonConsecutiveOwnSegmentIndices {
        /// Local segment index of the previous own segment root
        previous_local_segment_index: LocalSegmentIndex,
        /// Local segment index that doesn't follow the previous one
        local_segment_index: LocalSegmentIndex,
    },
    /// Too many intermediate shard blocks
    #[error("Too many intermediate shard blocks: {actual}")]
    TooManyIntermediateShardBlocks {
        /// Actual number of intermediate shard blocks
        actual: usize,
    },
    /// Block body is too large
    #[error("Block body is too large")]
    BlockBodyIsTooLarge,
    /// Intermediate shard blocks are not in the order described in
    /// [`IntermediateShardBlocksInfo::try_from_bytes()`]
    ///
    /// [`IntermediateShardBlocksInfo::try_from_bytes()`]:
    /// crate::block::body::IntermediateShardBlocksInfo::try_from_bytes
    #[error("Intermediate shard blocks are inconsistent")]
    InconsistentIntermediateShardBlocks,
}

/// An owned version of [`BeaconChainBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedBeaconChainBody {
    inner: Arc<Yoke<BeaconChainBody<'static>, SharedAlignedBuffer>>,
}

impl GenericOwnedBlockBody for OwnedBeaconChainBody {
    const SHARD_KIND: RealShardKind = RealShardKind::BeaconChain;

    type Body<'a> = BeaconChainBody<'a>;

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
    }

    #[inline(always)]
    fn buffer(&self) -> &SharedAlignedBuffer {
        self.buffer()
    }

    #[inline(always)]
    fn ref_count(&self) -> usize {
        self.ref_count()
    }

    #[inline(always)]
    fn body(&self) -> &Self::Body<'_> {
        self.body()
    }
}

impl OwnedBeaconChainBody {
    /// Create a new instance
    pub fn new<'a, OS, ISB>(
        own_segments: OS,
        intermediate_shard_blocks: ISB,
        pot_checkpoints: &[PotCheckpoints],
    ) -> Result<Self, OwnedBeaconChainBodyError>
    where
        OS: TrustedLen<Item = (LocalSegmentIndex, SegmentRoot)>,
        // Infos come from parsed beacon chain bodies or from
        // `OwnedIntermediateShardBlockInfo::info()`
        ISB: TrustedLen<Item = IntermediateShardBlockInfo<'a>> + 'a,
    {
        let num_pot_checkpoints = pot_checkpoints.len();
        let num_pot_checkpoints = u32::try_from(num_pot_checkpoints).map_err(|_error| {
            OwnedBeaconChainBodyError::TooManyPotCheckpoints {
                actual: num_pot_checkpoints,
            }
        })?;
        let num_own_segment_roots = own_segments.size_hint().0;
        let num_own_segment_roots = u8::try_from(num_own_segment_roots).map_err(|_error| {
            OwnedBeaconChainBodyError::TooManyOwnSegmentRoots {
                actual: num_own_segment_roots,
            }
        })?;
        let num_blocks = intermediate_shard_blocks.size_hint().0;
        let num_blocks = u16::try_from(num_blocks).map_err(|_error| {
            OwnedBeaconChainBodyError::TooManyIntermediateShardBlocks { actual: num_blocks }
        })?;

        // A body can't hold more than `u32::MAX` bytes of PoT checkpoints
        let pot_checkpoints_size = num_pot_checkpoints
            .checked_mul(PotCheckpoints::SIZE as u32)
            .ok_or(OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;
        let fixed_size = (
            // Number of PoT checkpoints
            u32::SIZE
                // Number of own segment roots
                + u8::SIZE
                // Local segment index of the first own segment root
                + if num_own_segment_roots > 0 { LocalSegmentIndex::SIZE } else { 0 }
                // Own segment roots
                + u32::from(num_own_segment_roots) * SegmentRoot::SIZE as u32
                // Number of intermediate shard blocks
                + u16::SIZE
                // Numbers of own segment roots, leaf shard blocks and leaf shard blocks with
                // segments of each block
                + u32::from(num_blocks) * (u8::SIZE + 2 * u16::SIZE)
        )
            // Padding to 8-bytes boundary
            .next_multiple_of(u32::from(Alignment::of::<u64>().get()));
        // Everything except intermediate shard blocks is known upfront, so a body that doesn't fit
        // into a buffer even without them is rejected before anything is allocated or copied
        #[cfg_attr(
            target_pointer_width = "64",
            expect(
                clippy::absurd_extreme_comparisons,
                reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
            )
        )]
        let min_size = fixed_size
            .checked_add(pot_checkpoints_size)
            .filter(|&size| size <= OwnedAlignedBuffer::<Align16>::MAX_CAPACITY)
            .ok_or(OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;
        // This is only an estimate of intermediate shard blocks to get in the ballpark where
        // reallocation should not be necessary in many cases
        let intermediate_shard_blocks_size =
            u32::from(num_blocks) * OwnedIntermediateShardHeader::MAX_ALLOCATION * 2;
        // The estimate can make the capacity larger than any body can be
        #[cfg_attr(
            target_pointer_width = "64",
            expect(
                clippy::unnecessary_min_or_max,
                reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
            )
        )]
        let mut buffer = OwnedAlignedBuffer::with_capacity(
            min_size
                .saturating_add(intermediate_shard_blocks_size)
                .min(OwnedAlignedBuffer::<Align16>::MAX_CAPACITY),
        );

        buffer
            .append(&num_pot_checkpoints.to_le_bytes())
            .expect("Fixed size data structures that are guaranteed to fit; qed");

        buffer
            .append(&[num_own_segment_roots])
            .expect("Fixed size data structures that are guaranteed to fit; qed");
        let mut own_segments = own_segments.peekable();
        if let Some(&(first_local_segment_index, _own_segment_root)) = own_segments.peek() {
            // The parser rejects bodies where the index after the last segment root doesn't fit
            // into `u64`
            if LocalSegmentIndex::forward_checked(
                first_local_segment_index,
                usize::from(num_own_segment_roots),
            )
            .is_none()
            {
                return Err(OwnedBeaconChainBodyError::OwnSegmentIndexOverflow {
                    first_local_segment_index,
                });
            }
            buffer
                .append(first_local_segment_index.as_bytes())
                .expect("Checked size above; qed");
        }
        let mut previous_local_segment_index = None;
        for (local_segment_index, own_segment_root) in own_segments {
            // Only the first local segment index is stored, the rest must follow it one by one
            if let Some(previous_local_segment_index) = previous_local_segment_index
                && LocalSegmentIndex::forward_checked(previous_local_segment_index, 1)
                    != Some(local_segment_index)
            {
                return Err(OwnedBeaconChainBodyError::NonConsecutiveOwnSegmentIndices {
                    previous_local_segment_index,
                    local_segment_index,
                });
            }
            previous_local_segment_index = Some(local_segment_index);

            buffer
                .append(own_segment_root.as_ref())
                .expect("Checked size above; qed");
        }
        // TODO: Would be nice for `IntermediateShardBlocksInfo` to have API to write this by itself
        {
            buffer
                .append(&num_blocks.to_le_bytes())
                .expect("Fixed size data structures that are guaranteed to fit; qed");
            let mut segments_roots_num_cursor = buffer.len() as usize;
            for _ in 0..num_blocks {
                // Placeholder values for the number of own segment roots, number of leaf shard
                // blocks and number of leaf shard blocks with segments
                buffer
                    .append(&[0; IntermediateShardBlockCounts::SIZE])
                    .expect("Checked size above; qed");
            }
            buffer
                .pad_to_alignment::<Align8>()
                .expect("Checked size above; qed");
            for intermediate_shard_block in intermediate_shard_blocks {
                let counts = IntermediateShardBlockCounts {
                    num_own_segment_roots: intermediate_shard_block
                        .own_segments
                        .as_ref()
                        .map_or_default(|own_segments| own_segments.num_segment_roots().get()),
                    num_leaf_shard_blocks: intermediate_shard_block.num_leaf_shard_blocks(),
                    num_leaf_shard_blocks_with_segments: intermediate_shard_block
                        .num_leaf_shard_blocks_with_segments(),
                };
                buffer.as_mut_slice()[segments_roots_num_cursor..]
                    [..IntermediateShardBlockCounts::SIZE]
                    .copy_from_slice(&counts.to_bytes());
                segments_roots_num_cursor += IntermediateShardBlockCounts::SIZE;

                buffer
                    .append(intermediate_shard_block.header.bytes())
                    .map_err(|_error| OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;
                if let Some(own_segments) = &intermediate_shard_block.own_segments {
                    buffer
                        .append(own_segments.first_local_segment_index.as_bytes())
                        .map_err(|_error| OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;
                    buffer
                        .append(SegmentRoot::slice_as_bytes(own_segments.segment_roots))
                        .map_err(|_error| OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;
                }
                buffer
                    .append(intermediate_shard_block.leaf_shards_segments_bytes)
                    .map_err(|_error| OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;
                buffer
                    .pad_to_alignment::<Align8>()
                    .map_err(|_error| OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;
            }
        }

        buffer
            .append(PotCheckpoints::slice_as_bytes(pot_checkpoints))
            .map_err(|_error| OwnedBeaconChainBodyError::BlockBodyIsTooLarge)?;

        Self::from_buffer(buffer.into_shared()).map_err(|_buffer| {
            // Everything is written the way the parser reads it, and infos can only be created by
            // parsers that check them, so only the order of intermediate shard blocks can be
            // rejected
            OwnedBeaconChainBodyError::InconsistentIntermediateShardBlocks
        })
    }

    /// Create an owned body from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((body, extra_bytes)) = BeaconChainBody::try_from_bytes(buffer) else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(body)
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with block body contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }

    /// Number of clones in memory
    #[inline(always)]
    pub fn ref_count(&self) -> usize {
        Arc::count(&self.inner)
    }

    /// Get [`BeaconChainBody`] out of [`OwnedBeaconChainBody`]
    #[inline(always)]
    pub fn body(&self) -> &BeaconChainBody<'_> {
        self.inner.get()
    }
}

/// Errors for [`OwnedIntermediateShardBody`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedIntermediateShardBodyError {
    /// Too many own segment roots
    #[error("Too many own segment roots: {actual}")]
    TooManyOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Local segment index after the last own segment root doesn't fit into `u64`
    #[error("Own segment index overflow, first local segment index: {first_local_segment_index}")]
    OwnSegmentIndexOverflow {
        /// Local segment index of the first own segment root
        first_local_segment_index: LocalSegmentIndex,
    },
    /// Local segment indices of own segment roots are not consecutive
    #[error(
        "Own segment indices are not consecutive: {local_segment_index} after \
        {previous_local_segment_index}"
    )]
    NonConsecutiveOwnSegmentIndices {
        /// Local segment index of the previous own segment root
        previous_local_segment_index: LocalSegmentIndex,
        /// Local segment index that doesn't follow the previous one
        local_segment_index: LocalSegmentIndex,
    },
    /// Too many leaf shard blocks
    #[error("Too many leaf shard blocks: {actual}")]
    TooManyLeafShardBlocks {
        /// Actual number of leaf shard blocks
        actual: usize,
    },
    /// Leaf shard blocks are not in the order described in
    /// [`LeafShardBlocksInfo::try_from_bytes()`]
    ///
    /// [`LeafShardBlocksInfo::try_from_bytes()`]:
    /// crate::block::body::LeafShardBlocksInfo::try_from_bytes
    #[error("Leaf shard blocks are inconsistent")]
    InconsistentLeafShardBlocks,
}

/// An owned version of [`IntermediateShardBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedIntermediateShardBody {
    inner: Arc<Yoke<IntermediateShardBody<'static>, SharedAlignedBuffer>>,
}

impl GenericOwnedBlockBody for OwnedIntermediateShardBody {
    const SHARD_KIND: RealShardKind = RealShardKind::IntermediateShard;

    type Body<'a> = IntermediateShardBody<'a>;

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
    }

    #[inline(always)]
    fn buffer(&self) -> &SharedAlignedBuffer {
        self.buffer()
    }

    #[inline(always)]
    fn ref_count(&self) -> usize {
        self.ref_count()
    }

    #[inline(always)]
    fn body(&self) -> &Self::Body<'_> {
        self.body()
    }
}

impl OwnedIntermediateShardBody {
    /// Create a new instance
    pub fn new<'a, OS, LSB>(
        own_segments: OS,
        leaf_shard_blocks: LSB,
    ) -> Result<Self, OwnedIntermediateShardBodyError>
    where
        OS: TrustedLen<Item = (LocalSegmentIndex, SegmentRoot)>,
        LSB: TrustedLen<Item = LeafShardBlockInfo<'a>> + 'a,
    {
        let num_own_segment_roots = own_segments.size_hint().0;
        let num_own_segment_roots = u8::try_from(num_own_segment_roots).map_err(|_error| {
            OwnedIntermediateShardBodyError::TooManyOwnSegmentRoots {
                actual: num_own_segment_roots,
            }
        })?;
        let num_blocks = leaf_shard_blocks.size_hint().0;
        let num_blocks = u16::try_from(num_blocks).map_err(|_error| {
            OwnedIntermediateShardBodyError::TooManyLeafShardBlocks { actual: num_blocks }
        })?;

        let mut buffer = OwnedAlignedBuffer::with_capacity(
            (
                // Number of own segment roots
                u8::SIZE
                    // Local segment index of the first own segment root
                    + if num_own_segment_roots > 0 { LocalSegmentIndex::SIZE } else { 0 }
                    // Own segment roots
                    + u32::from(num_own_segment_roots) * SegmentRoot::SIZE as u32
                    // Number of leaf shard blocks
                    + u16::SIZE
                    // Numbers of own segment roots of each block
                    + u32::from(num_blocks) * u8::SIZE
            )
            // Padding to 8-bytes boundary
            .next_multiple_of(u32::from(Alignment::of::<u64>().get()))
                // This is only an estimate of leaf shard blocks to get in the ballpark where
                // reallocation should not be necessary
                + u32::from(num_blocks) * OwnedLeafShardHeader::MAX_ALLOCATION * 2,
        );

        buffer
            .append(&[num_own_segment_roots])
            .expect("Fixed size data structures that are guaranteed to fit; qed");
        let mut own_segments = own_segments.peekable();
        if let Some(&(first_local_segment_index, _own_segment_root)) = own_segments.peek() {
            // The parser rejects bodies where the index after the last segment root doesn't fit
            // into `u64`
            if LocalSegmentIndex::forward_checked(
                first_local_segment_index,
                usize::from(num_own_segment_roots),
            )
            .is_none()
            {
                return Err(OwnedIntermediateShardBodyError::OwnSegmentIndexOverflow {
                    first_local_segment_index,
                });
            }
            buffer
                .append(first_local_segment_index.as_bytes())
                .expect("Enough capacity was allocated above; qed");
        }
        let mut previous_local_segment_index = None;
        for (local_segment_index, own_segment_root) in own_segments {
            // Only the first local segment index is stored, the rest must follow it one by one
            if let Some(previous_local_segment_index) = previous_local_segment_index
                && LocalSegmentIndex::forward_checked(previous_local_segment_index, 1)
                    != Some(local_segment_index)
            {
                return Err(
                    OwnedIntermediateShardBodyError::NonConsecutiveOwnSegmentIndices {
                        previous_local_segment_index,
                        local_segment_index,
                    },
                );
            }
            previous_local_segment_index = Some(local_segment_index);

            buffer
                .append(own_segment_root.as_ref())
                .expect("Enough capacity was allocated above; qed");
        }
        // TODO: Would be nice for `LeafShardBlocksInfo` to have API to write this by itself
        {
            buffer
                .append(&num_blocks.to_le_bytes())
                .expect("Fixed size data structures that are guaranteed to fit; qed");
            let mut own_segments_roots_num_cursor = buffer.len() as usize;
            for _ in 0..num_blocks {
                buffer
                    .append(&[0])
                    .expect("Enough capacity was allocated above; qed");
            }
            buffer
                .pad_to_alignment::<Align8>()
                .expect("Enough capacity was allocated above; qed");
            for leaf_shard_block in leaf_shard_blocks {
                if let Some(own_segments) = &leaf_shard_block.own_segments {
                    buffer.as_mut_slice()[own_segments_roots_num_cursor] =
                        own_segments.num_segment_roots().get();
                }
                own_segments_roots_num_cursor += 1;

                append_leaf_shard_block_record(
                    &mut buffer,
                    &leaf_shard_block.header,
                    leaf_shard_block.transactions_root,
                    leaf_shard_block.own_segments.as_ref(),
                )
                .expect(
                    "Bodies with at most `u16::MAX` leaf shard blocks with at most `u8::MAX` \
                    segment roots each are smaller than 1 GiB, which fits into a buffer even where \
                    `usize` is 32-bit; qed",
                );
            }
        }

        Self::from_buffer(buffer.into_shared()).map_err(|_buffer| {
            // Everything is written the way the parser reads it, and infos can only be created by
            // parsers that check them, so only the order of leaf shard blocks can be rejected
            OwnedIntermediateShardBodyError::InconsistentLeafShardBlocks
        })
    }

    /// Create an owned body from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((body, extra_bytes)) = IntermediateShardBody::try_from_bytes(buffer) else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(body)
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with block body contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }

    /// Number of clones in memory
    #[inline(always)]
    pub fn ref_count(&self) -> usize {
        Arc::count(&self.inner)
    }

    /// Get [`IntermediateShardBody`] out of [`OwnedIntermediateShardBody`]
    #[inline(always)]
    pub fn body(&self) -> &IntermediateShardBody<'_> {
        self.inner.get()
    }
}

/// Errors for [`OwnedLeafShardBody`]
#[derive(Debug, thiserror::Error)]
pub enum OwnedLeafShardBodyError {
    /// Too many own segment roots
    #[error("Too many own segment roots: {actual}")]
    TooManyOwnSegmentRoots {
        /// Actual number of own segment roots
        actual: usize,
    },
    /// Local segment index after the last own segment root doesn't fit into `u64`
    #[error("Own segment index overflow, first local segment index: {first_local_segment_index}")]
    OwnSegmentIndexOverflow {
        /// Local segment index of the first own segment root
        first_local_segment_index: LocalSegmentIndex,
    },
    /// Local segment indices of own segment roots are not consecutive
    #[error(
        "Own segment indices are not consecutive: {local_segment_index} after \
        {previous_local_segment_index}"
    )]
    NonConsecutiveOwnSegmentIndices {
        /// Local segment index of the previous own segment root
        previous_local_segment_index: LocalSegmentIndex,
        /// Local segment index that doesn't follow the previous one
        local_segment_index: LocalSegmentIndex,
    },
    /// Block body is too large
    #[error("Block body is too large")]
    BlockBodyIsTooLarge,
}

/// An owned version of [`LeafShardBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBody {
    inner: Arc<Yoke<LeafShardBody<'static>, SharedAlignedBuffer>>,
}

impl GenericOwnedBlockBody for OwnedLeafShardBody {
    const SHARD_KIND: RealShardKind = RealShardKind::LeafShard;

    type Body<'a> = LeafShardBody<'a>;

    #[inline(always)]
    fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        Self::from_buffer(buffer)
    }

    #[inline(always)]
    fn buffer(&self) -> &SharedAlignedBuffer {
        self.buffer()
    }

    #[inline(always)]
    fn ref_count(&self) -> usize {
        self.ref_count()
    }

    #[inline(always)]
    fn body(&self) -> &Self::Body<'_> {
        self.body()
    }
}

impl OwnedLeafShardBody {
    /// Initialize building of [`OwnedLeafShardBody`]
    pub fn init<OS>(
        own_segments: OS,
    ) -> Result<OwnedLeafShardBlockBodyBuilder, OwnedLeafShardBodyError>
    where
        OS: TrustedLen<Item = (LocalSegmentIndex, SegmentRoot)>,
    {
        let num_own_segment_roots = own_segments.size_hint().0;
        let num_own_segment_roots = u8::try_from(num_own_segment_roots).map_err(|_error| {
            OwnedLeafShardBodyError::TooManyOwnSegmentRoots {
                actual: num_own_segment_roots,
            }
        })?;

        // Transactions are not known yet, the buffer grows when they are added
        let mut buffer = OwnedAlignedBuffer::with_capacity(
            (
                // Number of own segment roots
                u8::SIZE
                    // Local segment index of the first own segment root
                    + if num_own_segment_roots > 0 {
                        LocalSegmentIndex::SIZE
                    } else {
                        0
                    }
                    // Own segment roots
                    + u32::from(num_own_segment_roots) * SegmentRoot::SIZE as u32
                    // Number of transactions
                    + u32::SIZE
            )
                // Padding to 16-bytes boundary
                .next_multiple_of(u32::from(Alignment::MAX.get())),
        );

        buffer
            .append(&[num_own_segment_roots])
            .expect("Fixed size data structures that are guaranteed to fit; qed");
        let mut own_segments = own_segments.peekable();
        if let Some(&(first_local_segment_index, _own_segment_root)) = own_segments.peek() {
            // The parser rejects bodies where the index after the last segment root doesn't fit
            // into `u64`
            if LocalSegmentIndex::forward_checked(
                first_local_segment_index,
                usize::from(num_own_segment_roots),
            )
            .is_none()
            {
                return Err(OwnedLeafShardBodyError::OwnSegmentIndexOverflow {
                    first_local_segment_index,
                });
            }
            buffer
                .append(first_local_segment_index.as_bytes())
                .expect("Enough capacity was allocated above; qed");
        }
        let mut previous_local_segment_index = None;
        for (local_segment_index, own_segment_root) in own_segments {
            // Only the first local segment index is stored, the rest must follow it one by one
            if let Some(previous_local_segment_index) = previous_local_segment_index
                && LocalSegmentIndex::forward_checked(previous_local_segment_index, 1)
                    != Some(local_segment_index)
            {
                return Err(OwnedLeafShardBodyError::NonConsecutiveOwnSegmentIndices {
                    previous_local_segment_index,
                    local_segment_index,
                });
            }
            previous_local_segment_index = Some(local_segment_index);

            buffer
                .append(own_segment_root.as_ref())
                .expect("Enough capacity was allocated above; qed");
        }

        let num_transactions_offset = buffer.len() as usize;
        buffer
            .append(&0u32.to_le_bytes())
            .expect("Enough capacity was allocated above; qed");
        // The parser expects padding after the number of transactions even if there are none
        buffer
            .pad_to_alignment::<Align16>()
            .expect("Enough capacity was allocated above; qed");

        Ok(OwnedLeafShardBlockBodyBuilder {
            transaction_builder: TransactionBuilder::new(num_transactions_offset, buffer),
        })
    }

    /// Create an owned body from a buffer
    #[inline]
    pub fn from_buffer(buffer: SharedAlignedBuffer) -> Result<Self, SharedAlignedBuffer> {
        // TODO: Cloning is cheap, but will not be necessary if/when this is resolved:
        //  https://github.com/unicode-org/icu4x/issues/6665
        let inner = Yoke::try_attach_to_cart(buffer.clone(), |buffer| {
            let Some((body, extra_bytes)) = LeafShardBody::try_from_bytes(buffer) else {
                return Err(());
            };
            if !extra_bytes.is_empty() {
                return Err(());
            }

            Ok(body)
        })
        .map_err(move |()| buffer)?;

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Inner buffer with block body contents
    #[inline(always)]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        self.inner.backing_cart()
    }

    /// Number of clones in memory
    #[inline(always)]
    pub fn ref_count(&self) -> usize {
        Arc::count(&self.inner)
    }

    /// Get [`LeafShardBody`] out of [`OwnedLeafShardBody`]
    #[inline(always)]
    pub fn body(&self) -> &LeafShardBody<'_> {
        self.inner.get()
    }
}

/// Builder for [`OwnedLeafShardBody`] that allows to add more transactions
#[derive(Debug, Clone)]
pub struct OwnedLeafShardBlockBodyBuilder {
    transaction_builder: TransactionBuilder,
}

impl OwnedLeafShardBlockBodyBuilder {
    /// Add transaction to the body
    #[inline(always)]
    pub fn add_transaction<T>(&mut self, transaction: T) -> Result<(), OwnedLeafShardBodyError>
    where
        T: WritableBodyTransaction,
    {
        self.transaction_builder.add_transaction(transaction)?;

        Ok(())
    }

    /// Finish building block body
    pub fn finish(self) -> OwnedLeafShardBody {
        OwnedLeafShardBody::from_buffer(self.transaction_builder.finish().into_shared()).expect(
            "`init()` wrote valid own segments and a zero number of transactions, each added \
            transaction is an already parsed one followed by padding and increments that number, \
            and the buffer is 16-byte aligned and can't be longer than `u32::MAX` bytes; qed",
        )
    }
}

/// An owned version of [`BlockBody`].
///
/// It is correctly aligned in memory and well suited for sending and receiving over the network
/// efficiently or storing in memory or on disk.
#[derive(Debug, Clone, From)]
pub enum OwnedBlockBody {
    /// Block body corresponds to the beacon chain
    BeaconChain(OwnedBeaconChainBody),
    /// Block body corresponds to an intermediate shard
    IntermediateShard(OwnedIntermediateShardBody),
    /// Block body corresponds to a leaf shard
    LeafShard(OwnedLeafShardBody),
}

impl OwnedBlockBody {
    /// Create an owned body from a buffer
    #[inline]
    pub fn from_buffer(
        buffer: SharedAlignedBuffer,
        shard_kind: RealShardKind,
    ) -> Result<Self, SharedAlignedBuffer> {
        Ok(match shard_kind {
            RealShardKind::BeaconChain => {
                Self::BeaconChain(OwnedBeaconChainBody::from_buffer(buffer)?)
            }
            RealShardKind::IntermediateShard => {
                Self::IntermediateShard(OwnedIntermediateShardBody::from_buffer(buffer)?)
            }
            RealShardKind::LeafShard => Self::LeafShard(OwnedLeafShardBody::from_buffer(buffer)?),
        })
    }

    /// Inner buffer block body contents
    #[inline]
    pub fn buffer(&self) -> &SharedAlignedBuffer {
        match self {
            Self::BeaconChain(owned_body) => owned_body.buffer(),
            Self::IntermediateShard(owned_body) => owned_body.buffer(),
            Self::LeafShard(owned_body) => owned_body.buffer(),
        }
    }

    /// Number of clones in memory
    #[inline(always)]
    pub fn ref_count(&self) -> usize {
        match self {
            Self::BeaconChain(owned_body) => owned_body.ref_count(),
            Self::IntermediateShard(owned_body) => owned_body.ref_count(),
            Self::LeafShard(owned_body) => owned_body.ref_count(),
        }
    }

    /// Get [`BlockBody`] out of [`OwnedBlockBody`]
    #[inline]
    pub fn body(&self) -> BlockBody<'_> {
        match self {
            Self::BeaconChain(owned_body) => BlockBody::BeaconChain(*owned_body.body()),
            Self::IntermediateShard(owned_body) => BlockBody::IntermediateShard(*owned_body.body()),
            Self::LeafShard(owned_body) => BlockBody::LeafShard(*owned_body.body()),
        }
    }
}
