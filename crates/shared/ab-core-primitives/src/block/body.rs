//! Block body primitives

#[cfg(feature = "alloc")]
pub mod owned;

#[cfg(feature = "alloc")]
use crate::block::body::owned::{
    GenericOwnedBlockBody, OwnedBeaconChainBody, OwnedBlockBody, OwnedIntermediateShardBody,
    OwnedLeafShardBody,
};
use crate::block::header::{IntermediateShardHeader, LeafShardHeader};
use crate::block::{BlockNumber, align_to_and_ensure_zero_padding};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::{LocalSegmentIndex, SegmentRoot};
use crate::shard::{RealShardKind, ShardIndex};
use crate::transaction::Transaction;
use crate::trees::{
    BeaconChainBodyMerkleTree, ChildIntermediateShardBlocksMerkleTree,
    ChildLeafShardBlocksMerkleTree, IntermediateShardBodyMerkleTree,
    IntermediateShardsSegmentsMerkleTree, LeafShardBodyMerkleTree, LeafShardsSegmentsMerkleTree,
    OwnSegmentsMerkleTree, SegmentRootsMerkleTree, ShardOwnSegmentsMerkleTree,
    TransactionsMerkleTree,
};
use ab_blake3::{BLOCK_LEN, single_block_hash};
use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use core::iter::{Step, TrustedLen};
use core::num::NonZeroU8;
use core::{cmp, fmt, iter, slice};
use derive_more::From;
use yoke::Yokeable;

/// Generic block body
pub trait GenericBlockBody<'a>
where
    Self: Copy + fmt::Debug + Into<BlockBody<'a>> + Send + Sync,
{
    /// Shard kind
    const SHARD_KIND: RealShardKind;

    /// Owned block body
    #[cfg(feature = "alloc")]
    type Owned: GenericOwnedBlockBody<Body<'a> = Self>
    where
        Self: 'a;

    /// Turn into an owned version
    #[cfg(feature = "alloc")]
    fn to_owned(self) -> Self::Owned;

    /// Compute block body root
    fn root(&self) -> Blake3Hash;
}

/// Calculates a Merkle Tree root for a provided list of segment roots
#[inline]
pub fn compute_segments_root<'a, const MAX_SEGMENTS: u64, Iter>(segment_roots: Iter) -> Blake3Hash
where
    Iter: IntoIterator<Item = &'a SegmentRoot, IntoIter: TrustedLen>,
{
    let root = SegmentRootsMerkleTree::compute_root_only::<MAX_SEGMENTS, _, _>(
        segment_roots.into_iter().copied(),
    );

    Blake3Hash::new(root.unwrap_or_default())
}

/// Own segments produced by a shard.
///
/// They have between 1 and `u8::MAX` segment roots, which is how many a body can store. The local
/// segment index after the last segment root fits into `u64`, because that is where the next
/// segment of the shard starts. Local segment indices never get close to `u64::MAX` in practice.
#[derive(Debug, Copy, Clone)]
pub struct OwnSegments<'a> {
    /// Local segment index of the first own segment root
    pub mut(self) first_local_segment_index: LocalSegmentIndex,
    /// Segment roots produced by a shard
    pub mut(self) segment_roots: &'a [SegmentRoot],
}

impl<'a> OwnSegments<'a> {
    /// Create own segments from the local segment index of the first segment root and the roots.
    ///
    /// Returns `None` if there are no segment roots, more than `u8::MAX` of them, or the local
    /// segment index after the last one doesn't fit into `u64`.
    #[inline]
    pub fn new(
        first_local_segment_index: LocalSegmentIndex,
        segment_roots: &'a [SegmentRoot],
    ) -> Option<Self> {
        if segment_roots.is_empty()
            || segment_roots.len() > usize::from(u8::MAX)
            || LocalSegmentIndex::forward_checked(first_local_segment_index, segment_roots.len())
                .is_none()
        {
            return None;
        }

        Some(Self::new_unchecked(
            first_local_segment_index,
            segment_roots,
        ))
    }

    /// Create own segments without checking them.
    ///
    /// Iterators of child shard blocks use it for own segments that their parsers have already
    /// checked: layouts only allow between 1 and `u8::MAX` segment roots, and the parsers check
    /// that the local segment index after the last one fits into `u64`.
    #[inline(always)]
    const fn new_unchecked(
        first_local_segment_index: LocalSegmentIndex,
        segment_roots: &'a [SegmentRoot],
    ) -> Self {
        Self {
            first_local_segment_index,
            segment_roots,
        }
    }

    /// Number of segment roots
    #[inline(always)]
    pub fn num_segment_roots(&self) -> NonZeroU8 {
        u8::try_from(self.segment_roots.len())
            .ok()
            .and_then(NonZeroU8::new)
            .expect("Constructors ensure between 1 and `u8::MAX` segment roots; qed")
    }

    /// Iterator over segment roots with their local segment indices
    #[inline]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (LocalSegmentIndex, &'a SegmentRoot)> + TrustedLen + use<'a>
    {
        let first_local_segment_index = self.first_local_segment_index;

        self.segment_roots
            .iter()
            .enumerate()
            .map(move |(offset, segment_root)| {
                let local_segment_index =
                    LocalSegmentIndex::forward_checked(first_local_segment_index, offset).expect(
                        "`Self::new()` and parsers check that the index after the last root fits \
                        into `u64`; qed",
                    );

                (local_segment_index, segment_root)
            })
    }

    /// Compute the root of own segments
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = OwnSegmentsMerkleTree::compute_root_only(&[
            single_block_hash(self.first_local_segment_index.as_bytes())
                .expect("Less than a single block worth of bytes; qed"),
            *compute_segments_root::<{ u64::from(u8::MAX) }, _>(self.segment_roots),
        ]);

        Blake3Hash::new(root)
    }

    /// Compute the root of own segments while mixing in shard index.
    ///
    /// This method is useful for deriving roots on intermediate shard that can be verified by the
    /// beacon chain later (which will have just shard index available rather than the whole leaf
    /// shard block header).
    #[inline]
    pub fn root_with_shard_index(&self, shard_index: ShardIndex) -> Blake3Hash {
        let root = ShardOwnSegmentsMerkleTree::compute_root_only(&[
            {
                const {
                    assert!((ShardIndex::SIZE + LocalSegmentIndex::SIZE) as usize <= BLOCK_LEN);
                }
                let mut pair = [0u8; (ShardIndex::SIZE + LocalSegmentIndex::SIZE) as usize];
                pair[..ShardIndex::SIZE as usize].copy_from_slice(shard_index.as_bytes());
                pair[ShardIndex::SIZE as usize..]
                    .copy_from_slice(self.first_local_segment_index.as_bytes());

                single_block_hash(&pair).expect("Less than a single block worth of bytes; qed")
            },
            *compute_segments_root::<{ u64::from(u8::MAX) }, _>(self.segment_roots),
        ]);

        Blake3Hash::new(root)
    }
}

/// Compute the root of segments of leaf shard blocks.
///
/// Takes positions of leaf shard blocks with segments among `num_leaf_shard_blocks` blocks, their
/// shard indices and segments. Returns `None` if positions aren't strictly increasing or not
/// smaller than `num_leaf_shard_blocks`.
fn compute_leaf_shards_segments_root<'a, Iter>(
    num_leaf_shard_blocks: u16,
    leaf_shard_blocks_segments: Iter,
) -> Option<Blake3Hash>
where
    Iter: IntoIterator<Item = (u16, ShardIndex, OwnSegments<'a>)>,
{
    let mut leaves =
        leaf_shard_blocks_segments
            .into_iter()
            .map(|(position, shard_index, own_segments)| {
                (
                    u64::from(position),
                    *own_segments.root_with_shard_index(shard_index),
                )
            });

    if num_leaf_shard_blocks == 0 {
        return leaves.next().is_none().then_some(Blake3Hash::default());
    }

    LeafShardsSegmentsMerkleTree::compute_root_only_sparse::<{ u64::from(u16::MAX) }, _, _>(
        u64::from(num_leaf_shard_blocks),
        leaves,
    )
    .map(Blake3Hash::new)
}

/// Compute the body root of an intermediate shard block
fn intermediate_shard_body_root(
    own_segments: Option<&OwnSegments<'_>>,
    leaf_shards_segments_root: &Blake3Hash,
    leaf_shard_blocks_root: &Blake3Hash,
) -> Blake3Hash {
    Blake3Hash::new(IntermediateShardBodyMerkleTree::compute_root_only_array(&[
        *own_segments.map_or_default(OwnSegments::root),
        **leaf_shards_segments_root,
        **leaf_shard_blocks_root,
    ]))
}

/// Information about intermediate shard block
#[derive(Debug, Clone)]
#[expect(
    clippy::partial_pub_fields,
    reason = "Intentionally exposing immediately decoded fields above and hiding the rest of the implementation"
)]
pub struct IntermediateShardBlockInfo<'a> {
    /// A block header that corresponds to an intermediate shard
    pub header: IntermediateShardHeader<'a>,
    /// Segments in the corresponding block
    pub own_segments: Option<OwnSegments<'a>>,
    num_leaf_shard_blocks: u16,
    num_leaf_shard_blocks_with_segments: u16,
    leaf_shards_segments_bytes: &'a [u8],
}

impl<'a> IntermediateShardBlockInfo<'a> {
    /// Number of leaf shard blocks in the corresponding intermediate shard block
    #[inline(always)]
    pub const fn num_leaf_shard_blocks(&self) -> u16 {
        self.num_leaf_shard_blocks
    }

    /// Number of leaf shard blocks with segments in the corresponding intermediate shard block
    #[inline(always)]
    pub const fn num_leaf_shard_blocks_with_segments(&self) -> u16 {
        self.num_leaf_shard_blocks_with_segments
    }

    /// Segments of leaf shards in the corresponding intermediate shard block, with positions of
    /// their blocks among leaf shard blocks of the intermediate shard block
    #[inline]
    pub fn leaf_shards_segments(
        &self,
    ) -> impl ExactSizeIterator<Item = (u16, ShardIndex, OwnSegments<'a>)> + TrustedLen + use<'a>
    {
        // SAFETY: Checked in constructor
        let (counts, mut remainder) = unsafe {
            self.leaf_shards_segments_bytes.split_at_unchecked(
                usize::from(self.num_leaf_shard_blocks_with_segments) * size_of::<u8>(),
            )
        };
        counts.iter().map(move |&num_own_segment_roots| {
            let num_own_segment_roots = usize::from(num_own_segment_roots);

            let position;
            // SAFETY: Checked in constructor
            (position, remainder) = unsafe { remainder.split_at_unchecked(size_of::<u16>()) };
            let position = u16::from_le_bytes([position[0], position[1]]);

            let shard_index;
            // SAFETY: Checked in constructor
            (shard_index, remainder) =
                unsafe { remainder.split_at_unchecked(ShardIndex::SIZE as usize) };
            let shard_index = Unaligned::<ShardIndex>::from_bytes_exact(shard_index)
                .expect("Correct size; qed")
                .as_inner();

            let first_local_segment_index;
            // SAFETY: Checked in constructor
            (first_local_segment_index, remainder) =
                unsafe { remainder.split_at_unchecked(LocalSegmentIndex::SIZE as usize) };
            let first_local_segment_index =
                Unaligned::<LocalSegmentIndex>::from_bytes_exact(first_local_segment_index)
                    .expect("Correct size; qed")
                    .as_inner();

            let own_segment_roots;
            // SAFETY: Checked in constructor
            (own_segment_roots, remainder) =
                unsafe { remainder.split_at_unchecked(num_own_segment_roots * SegmentRoot::SIZE) };
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            (
                position,
                shard_index,
                OwnSegments::new_unchecked(first_local_segment_index, own_segment_roots),
            )
        })
    }

    /// Compute the root of segments of leaf shards in the corresponding intermediate shard block
    #[inline]
    fn leaf_shards_segments_root(&self) -> Blake3Hash {
        compute_leaf_shards_segments_root(self.num_leaf_shard_blocks, self.leaf_shards_segments())
            .expect("Positions are checked in constructor; qed")
    }

    /// Compute the body root of the corresponding intermediate shard block.
    ///
    /// It commits to the root of leaf shard blocks in the header, which is the same as in the body
    /// of a consistent intermediate shard block.
    #[inline]
    pub fn body_root(&self) -> Blake3Hash {
        intermediate_shard_body_root(
            self.own_segments.as_ref(),
            &self.leaf_shards_segments_root(),
            self.header.leaf_shard_blocks_root(),
        )
    }
}

/// Information about a collection of intermediate shard blocks
#[derive(Debug, Copy, Clone)]
pub struct IntermediateShardBlocksInfo<'a> {
    num_blocks: usize,
    bytes: &'a [u8],
}

impl<'a> IntermediateShardBlocksInfo<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of blocks: u16 as unaligned little-endian bytes
        // * for each block:
        //   * number of own segment roots: u8
        //   * number of leaf shard blocks: u16 as unaligned little-endian bytes
        //   * number of leaf shard blocks with segments: u16 as unaligned little-endian bytes
        // * padding to 8-bytes boundary with zeroes
        // * for each block:
        //   * block header: IntermediateShardHeader
        //   * local segment index of the first segment root (if any): unaligned `LocalSegmentIndex`
        //   * concatenated own segment roots
        //   * for each leaf shard block with segments:
        //     * number of own segment roots: NonZeroU8
        //   * for each leaf shard block with segments:
        //     * position among leaf shard blocks, strictly increasing: u16 as unaligned
        //       little-endian bytes
        //     * shard index: u32 as unaligned little-endian bytes
        //     * local segment index of the first segment root: unaligned `LocalSegmentIndex`
        //     * concatenated own segment roots
        //   * padding to 8-bytes boundary with zeroes

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = usize::from(u16::from_le_bytes([num_blocks[0], num_blocks[1]]));

        let bytes_start = bytes;

        let mut counts =
            bytes.split_off(..num_blocks * (size_of::<u8>() + 2 * size_of::<u16>()))?;

        let mut remainder = align_to_and_ensure_zero_padding(bytes, Alignment::of::<u64>())?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            let num_leaf_shard_blocks = u16::from_le_bytes([counts[1], counts[2]]);
            let num_leaf_shard_blocks_with_segments =
                usize::from(u16::from_le_bytes([counts[3], counts[4]]));
            counts = &counts[5..];

            (_, remainder) = IntermediateShardHeader::try_from_bytes(remainder)?;

            if num_own_segment_roots > 0 {
                let first_local_segment_index;
                (first_local_segment_index, remainder) =
                    Unaligned::<LocalSegmentIndex>::from_bytes(remainder)?;
                // The local segment index after the last segment root must fit into `u64`
                let _next_local_segment_index = LocalSegmentIndex::forward_checked(
                    first_local_segment_index.as_inner(),
                    num_own_segment_roots,
                )?;
                let _own_segment_roots =
                    remainder.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            }

            let leaf_shard_segment_roots_counts =
                remainder.split_off(..num_leaf_shard_blocks_with_segments * size_of::<u8>())?;

            // Positions of leaf shard blocks with segments must be strictly increasing and smaller
            // than the number of leaf shard blocks, the root of their segments relies on it
            let mut next_position = 0;
            for &num_own_segment_roots in leaf_shard_segment_roots_counts {
                let num_own_segment_roots = NonZeroU8::new(num_own_segment_roots)?;
                let position = remainder.split_off(..size_of::<u16>())?;
                let position = u16::from_le_bytes([position[0], position[1]]);
                if position < next_position || position >= num_leaf_shard_blocks {
                    return None;
                }
                next_position = position + 1;
                let _shard_index = remainder.split_off(..ShardIndex::SIZE as usize)?;
                let first_local_segment_index;
                (first_local_segment_index, remainder) =
                    Unaligned::<LocalSegmentIndex>::from_bytes(remainder)?;
                // The local segment index after the last segment root must fit into `u64`
                let _next_local_segment_index = LocalSegmentIndex::forward_checked(
                    first_local_segment_index.as_inner(),
                    usize::from(num_own_segment_roots.get()),
                )?;
                let _own_segment_roots = remainder
                    .split_off(..usize::from(num_own_segment_roots.get()) * SegmentRoot::SIZE)?;
            }

            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())?;
        }

        let info = Self {
            num_blocks,
            bytes: &bytes_start[..bytes_start.len() - remainder.len()],
        };

        if !info.is_internally_consistent() {
            return None;
        }

        Some((info, remainder))
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout is the same as in `Self::try_from_bytes()`

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = usize::from(u16::from_le_bytes([num_blocks[0], num_blocks[1]]));

        let bytes_start = bytes;

        let mut counts =
            bytes.split_off(..num_blocks * (size_of::<u8>() + 2 * size_of::<u16>()))?;

        let mut remainder = align_to_and_ensure_zero_padding(bytes, Alignment::of::<u64>())?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            let num_leaf_shard_blocks = u16::from_le_bytes([counts[1], counts[2]]);
            let num_leaf_shard_blocks_with_segments =
                usize::from(u16::from_le_bytes([counts[3], counts[4]]));
            counts = &counts[5..];

            (_, remainder) = IntermediateShardHeader::try_from_bytes(remainder)?;

            if num_own_segment_roots > 0 {
                let first_local_segment_index;
                (first_local_segment_index, remainder) =
                    Unaligned::<LocalSegmentIndex>::from_bytes(remainder)?;
                // The local segment index after the last segment root must fit into `u64`
                let _next_local_segment_index = LocalSegmentIndex::forward_checked(
                    first_local_segment_index.as_inner(),
                    num_own_segment_roots,
                )?;
                let _own_segment_roots =
                    remainder.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            }

            let leaf_shard_segment_roots_counts =
                remainder.split_off(..num_leaf_shard_blocks_with_segments * size_of::<u8>())?;

            // Positions of leaf shard blocks with segments must be strictly increasing and smaller
            // than the number of leaf shard blocks, the root of their segments relies on it
            let mut next_position = 0;
            for &num_own_segment_roots in leaf_shard_segment_roots_counts {
                let num_own_segment_roots = NonZeroU8::new(num_own_segment_roots)?;
                let position = remainder.split_off(..size_of::<u16>())?;
                let position = u16::from_le_bytes([position[0], position[1]]);
                if position < next_position || position >= num_leaf_shard_blocks {
                    return None;
                }
                next_position = position + 1;
                let _shard_index = remainder.split_off(..ShardIndex::SIZE as usize)?;
                let first_local_segment_index;
                (first_local_segment_index, remainder) =
                    Unaligned::<LocalSegmentIndex>::from_bytes(remainder)?;
                // The local segment index after the last segment root must fit into `u64`
                let _next_local_segment_index = LocalSegmentIndex::forward_checked(
                    first_local_segment_index.as_inner(),
                    usize::from(num_own_segment_roots.get()),
                )?;
                let _own_segment_roots = remainder
                    .split_off(..usize::from(num_own_segment_roots.get()) * SegmentRoot::SIZE)?;
            }

            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())?;
        }

        Some((
            Self {
                num_blocks,
                bytes: &bytes_start[..bytes_start.len() - remainder.len()],
            },
            remainder,
        ))
    }

    /// Check intermediate shard info's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        let mut last_intermediate_shard_info =
            None::<(ShardIndex, BlockNumber, Option<LocalSegmentIndex>)>;

        self.iter().all(|intermediate_shard_block| {
            let shard_index = intermediate_shard_block.header.prefix.shard_index;

            // Ensure increasing order of shard indices, block numbers and local segment indices
            if let Some((
                last_intermediate_shard_index,
                last_intermediate_shard_block_number,
                last_intermediate_shard_first_local_segment_index,
            )) = last_intermediate_shard_info
            {
                match last_intermediate_shard_index.cmp(&shard_index) {
                    cmp::Ordering::Less => {
                        last_intermediate_shard_info.replace((
                            shard_index,
                            intermediate_shard_block.header.prefix.number,
                            intermediate_shard_block
                                .own_segments
                                .as_ref()
                                .map(|own_segments| own_segments.first_local_segment_index),
                        ));
                    }
                    cmp::Ordering::Equal => {
                        if last_intermediate_shard_block_number
                            >= intermediate_shard_block.header.prefix.number
                        {
                            return false;
                        }
                        if let Some(own_segments) = &intermediate_shard_block.own_segments {
                            if let Some(last_intermediate_shard_first_local_segment_index) =
                                last_intermediate_shard_first_local_segment_index
                                && last_intermediate_shard_first_local_segment_index
                                    >= own_segments.first_local_segment_index
                            {
                                return false;
                            }

                            last_intermediate_shard_info.replace((
                                shard_index,
                                intermediate_shard_block.header.prefix.number,
                                Some(own_segments.first_local_segment_index),
                            ));
                        } else {
                            last_intermediate_shard_info.replace((
                                shard_index,
                                intermediate_shard_block.header.prefix.number,
                                last_intermediate_shard_first_local_segment_index,
                            ));
                        }
                    }
                    cmp::Ordering::Greater => {
                        return false;
                    }
                }
            } else {
                last_intermediate_shard_info.replace((
                    shard_index,
                    intermediate_shard_block.header.prefix.number,
                    intermediate_shard_block
                        .own_segments
                        .as_ref()
                        .map(|own_segments| own_segments.first_local_segment_index),
                ));
            }

            let mut last_leaf_shard_info = None::<(ShardIndex, LocalSegmentIndex)>;

            // Ensure increasing order of shard indices and local segment indices
            if !intermediate_shard_block.leaf_shards_segments().all(
                |(_position, leaf_shard_index, own_segments)| {
                    if !leaf_shard_index.is_child_of(shard_index) {
                        return false;
                    }

                    if let Some((
                        last_leaf_shard_index,
                        last_leaf_shard_first_local_segment_index,
                    )) = last_leaf_shard_info
                    {
                        match last_leaf_shard_index.cmp(&leaf_shard_index) {
                            cmp::Ordering::Less => {
                                // Expected
                            }
                            cmp::Ordering::Equal => {
                                if last_leaf_shard_first_local_segment_index
                                    >= own_segments.first_local_segment_index
                                {
                                    return false;
                                }
                            }
                            cmp::Ordering::Greater => {
                                return false;
                            }
                        }
                    }
                    last_leaf_shard_info
                        .replace((leaf_shard_index, own_segments.first_local_segment_index));

                    true
                },
            ) {
                return false;
            }

            // Also checks blocks without segments, positions and the number of leaf shard blocks,
            // the root of segments of leaf shards commits to them
            intermediate_shard_block.body_root() == intermediate_shard_block.header.result.body_root
        })
    }

    /// Iterator over intermediate shard blocks in a collection
    #[inline]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = IntermediateShardBlockInfo<'a>> + TrustedLen + use<'a> {
        // SAFETY: Checked in constructor
        let (mut counts, remainder) = unsafe {
            self.bytes
                .split_at_unchecked(self.num_blocks * (size_of::<u8>() + 2 * size_of::<u16>()))
        };
        let mut remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())
            .expect("Already checked in constructor; qed");

        iter::repeat_with(move || {
            let num_own_segment_roots = usize::from(counts[0]);
            let num_leaf_shard_blocks = u16::from_le_bytes([counts[1], counts[2]]);
            let num_leaf_shard_blocks_with_segments = u16::from_le_bytes([counts[3], counts[4]]);
            counts = &counts[5..];

            // TODO: Unchecked method would have been helpful here
            let header;
            (header, remainder) = IntermediateShardHeader::try_from_bytes(remainder)
                .expect("Already checked in constructor; qed");

            let own_segments = if num_own_segment_roots > 0 {
                let first_local_segment_index;
                // SAFETY: Checked in constructor
                (first_local_segment_index, remainder) =
                    unsafe { remainder.split_at_unchecked(size_of::<LocalSegmentIndex>()) };
                let first_local_segment_index =
                    Unaligned::<LocalSegmentIndex>::from_bytes_exact(first_local_segment_index)
                        .expect("Correct size; qed")
                        .as_inner();

                let own_segment_roots;
                // SAFETY: Checked in constructor
                (own_segment_roots, remainder) = unsafe {
                    remainder.split_at_unchecked(num_own_segment_roots * SegmentRoot::SIZE)
                };
                // SAFETY: Valid pointer and size, no alignment requirements
                let own_segment_roots = unsafe {
                    slice::from_raw_parts(
                        own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                        num_own_segment_roots,
                    )
                };
                let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

                Some(OwnSegments::new_unchecked(
                    first_local_segment_index,
                    own_segment_roots,
                ))
            } else {
                None
            };

            // SAFETY: Checked in constructor
            let leaf_shard_segment_roots_counts = unsafe {
                remainder.get_unchecked(
                    ..usize::from(num_leaf_shard_blocks_with_segments) * size_of::<u8>(),
                )
            };

            let leaf_shards_segments_bytes;
            // SAFETY: Checked in constructor
            (leaf_shards_segments_bytes, remainder) = unsafe {
                remainder.split_at_unchecked(
                    usize::from(num_leaf_shard_blocks_with_segments) * size_of::<u8>()
                        + usize::from(num_leaf_shard_blocks_with_segments)
                            * (size_of::<u16>()
                                + (ShardIndex::SIZE + LocalSegmentIndex::SIZE) as usize)
                        + leaf_shard_segment_roots_counts.iter().fold(
                            0usize,
                            |acc, &num_own_segment_roots| {
                                acc + usize::from(num_own_segment_roots) * SegmentRoot::SIZE
                            },
                        ),
                )
            };

            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())
                .expect("Already checked in constructor; qed");

            IntermediateShardBlockInfo {
                header,
                own_segments,
                num_leaf_shard_blocks,
                num_leaf_shard_blocks_with_segments,
                leaf_shards_segments_bytes,
            }
        })
        .take(self.num_blocks)
    }

    /// Number of intermediate shard blocks
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.num_blocks
    }

    /// Returns `true` if there are no intermediate shard blocks
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.num_blocks == 0
    }

    /// Compute the root of segments of the intermediate shard blocks info.
    ///
    /// Returns the default value for an empty collection of shard blocks.
    #[inline]
    pub fn segments_root(&self) -> Blake3Hash {
        let root = IntermediateShardsSegmentsMerkleTree::compute_root_only::<
            { u64::from(u16::MAX) },
            _,
            _,
        >(self.iter().map(|shard_block_info| {
            // The hasher doesn't depend on the number of leaves, so this is the inner node over the
            // first two leaves of the body tree of the intermediate shard block, which commits to
            // the same segments
            IntermediateShardBodyMerkleTree::compute_root_only_array(&[
                *shard_block_info
                    .own_segments
                    .as_ref()
                    .map_or_default(OwnSegments::root),
                *shard_block_info.leaf_shards_segments_root(),
            ])
        }))
        .unwrap_or_default();

        Blake3Hash::new(root)
    }

    /// Compute the root of child block roots, which the header stores.
    ///
    /// Returns the default value for an empty collection of shard blocks.
    #[inline]
    pub fn headers_root(&self) -> Blake3Hash {
        let root = ChildIntermediateShardBlocksMerkleTree::compute_root_only::<
            { u64::from(u16::MAX) },
            _,
            _,
        >(
            self.iter()
                .map(|shard_block_info| **shard_block_info.header.root()),
        )
        .unwrap_or_default();

        Blake3Hash::new(root)
    }
}

/// Block body that corresponds to the beacon chain
#[derive(Debug, Copy, Clone, Yokeable)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct BeaconChainBody<'a> {
    /// Segments produced by this shard
    own_segments: Option<OwnSegments<'a>>,
    /// Intermediate shard blocks
    intermediate_shard_blocks: IntermediateShardBlocksInfo<'a>,
    /// Proof of time checkpoints from after future proof of time of the parent block to the
    /// current block's future proof of time (inclusive)
    pot_checkpoints: &'a [PotCheckpoints],
}

impl<'a> GenericBlockBody<'a> for BeaconChainBody<'a> {
    const SHARD_KIND: RealShardKind = RealShardKind::BeaconChain;

    #[cfg(feature = "alloc")]
    type Owned = OwnedBeaconChainBody;

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }

    #[inline(always)]
    fn root(&self) -> Blake3Hash {
        self.root()
    }
}

impl<'a> BeaconChainBody<'a> {
    /// Create an instance from provided correctly aligned bytes.
    ///
    /// `bytes` should be 4-bytes aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of PoT checkpoints: u32 as aligned little-endian bytes
        // * number of own segment roots: u8
        // * local segment index of the first segment root (if any): unaligned `LocalSegmentIndex`
        // * concatenated own segment roots
        // * intermediate shard blocks: IntermediateShardBlocksInfo
        // * concatenated PoT checkpoints

        let num_pot_checkpoints;
        (num_pot_checkpoints, bytes) = u32::from_bytes(bytes)?;
        let num_pot_checkpoints = *num_pot_checkpoints as usize;

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index;
            (first_local_segment_index, bytes) = Unaligned::<LocalSegmentIndex>::from_bytes(bytes)?;

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments::new(
                first_local_segment_index.as_inner(),
                own_segment_roots,
            )?)
        } else {
            None
        };

        let (intermediate_shard_blocks, mut remainder) =
            IntermediateShardBlocksInfo::try_from_bytes(bytes)?;

        let pot_checkpoints =
            remainder.split_off(..num_pot_checkpoints.checked_mul(PotCheckpoints::SIZE)?)?;
        // SAFETY: Valid pointer and size, no alignment requirements
        let pot_checkpoints = unsafe {
            slice::from_raw_parts(
                pot_checkpoints
                    .as_ptr()
                    .cast::<[u8; PotCheckpoints::SIZE]>(),
                num_pot_checkpoints,
            )
        };
        let pot_checkpoints = PotCheckpoints::slice_from_bytes(pot_checkpoints);

        let body = Self {
            own_segments,
            intermediate_shard_blocks,
            pot_checkpoints,
        };

        if !body.is_internally_consistent() {
            return None;
        }

        Some((body, remainder))
    }

    /// Check block body's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        // Nothing to check here
        true
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout is the same as in `Self::try_from_bytes()`

        let num_pot_checkpoints;
        (num_pot_checkpoints, bytes) = u32::from_bytes(bytes)?;
        let num_pot_checkpoints = *num_pot_checkpoints as usize;

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index;
            (first_local_segment_index, bytes) = Unaligned::<LocalSegmentIndex>::from_bytes(bytes)?;

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments::new(
                first_local_segment_index.as_inner(),
                own_segment_roots,
            )?)
        } else {
            None
        };

        let (intermediate_shard_blocks, mut remainder) =
            IntermediateShardBlocksInfo::try_from_bytes_unchecked(bytes)?;

        let pot_checkpoints =
            remainder.split_off(..num_pot_checkpoints.checked_mul(PotCheckpoints::SIZE)?)?;
        // SAFETY: Valid pointer and size, no alignment requirements
        let pot_checkpoints = unsafe {
            slice::from_raw_parts(
                pot_checkpoints
                    .as_ptr()
                    .cast::<[u8; PotCheckpoints::SIZE]>(),
                num_pot_checkpoints,
            )
        };
        let pot_checkpoints = PotCheckpoints::slice_from_bytes(pot_checkpoints);

        Some((
            Self {
                own_segments,
                intermediate_shard_blocks,
                pot_checkpoints,
            },
            remainder,
        ))
    }

    /// Create an owned version of this body
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedBeaconChainBody {
        if let Some(own_segments) = self.own_segments {
            OwnedBeaconChainBody::new(
                own_segments
                    .iter()
                    .map(|(local_segment_index, &segment_root)| {
                        (local_segment_index, segment_root)
                    }),
                self.intermediate_shard_blocks.iter(),
                self.pot_checkpoints,
            )
            .expect("`self` is always a valid invariant; qed")
        } else {
            OwnedBeaconChainBody::new(
                iter::empty(),
                self.intermediate_shard_blocks.iter(),
                self.pot_checkpoints,
            )
            .expect("`self` is always a valid invariant; qed")
        }
    }

    /// Segment roots produced by this shard
    #[inline(always)]
    pub fn own_segments(&self) -> Option<OwnSegments<'a>> {
        self.own_segments
    }

    /// Intermediate shard blocks
    #[inline(always)]
    pub fn intermediate_shard_blocks(&self) -> &IntermediateShardBlocksInfo<'a> {
        &self.intermediate_shard_blocks
    }

    /// Proof of time checkpoints from after future proof of time of the parent block to the current
    /// block's future proof of time (inclusive)
    #[inline(always)]
    pub fn pot_checkpoints(&self) -> &'a [PotCheckpoints] {
        self.pot_checkpoints
    }

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = BeaconChainBodyMerkleTree::compute_root_only(&[
            *self.own_segments.as_ref().map_or_default(OwnSegments::root),
            *self.intermediate_shard_blocks.segments_root(),
            *self.intermediate_shard_blocks.headers_root(),
            blake3::hash(PotCheckpoints::bytes_from_slice(self.pot_checkpoints).as_flattened())
                .into(),
        ]);

        Blake3Hash::new(root)
    }
}

/// Compute the body root of a leaf shard block
fn leaf_shard_body_root(
    own_segments: Option<&OwnSegments<'_>>,
    transactions_root: &Blake3Hash,
) -> Blake3Hash {
    Blake3Hash::new(LeafShardBodyMerkleTree::compute_root_only(&[
        *own_segments.map_or_default(OwnSegments::root),
        **transactions_root,
    ]))
}

/// Information about the leaf shard block container inside the intermediate shard block body
#[derive(Debug, Clone)]
pub struct LeafShardBlockInfo<'a> {
    /// A block header that corresponds to a leaf shard
    pub header: LeafShardHeader<'a>,
    /// Root of transactions in the corresponding block
    pub transactions_root: &'a Blake3Hash,
    /// Segments in the corresponding block
    pub own_segments: Option<OwnSegments<'a>>,
}

/// Information about a collection of leaf shard blocks
#[derive(Debug, Copy, Clone)]
pub struct LeafShardBlocksInfo<'a> {
    num_blocks: u16,
    bytes: &'a [u8],
}

impl<'a> LeafShardBlocksInfo<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of blocks: u16 as unaligned little-endian bytes
        // * for each block:
        //   * number of own segment roots: u8
        // * padding to 8-bytes boundary with zeroes
        // * for each block:
        //   * block header: LeafShardHeader
        //   * padding to 8-bytes boundary with zeroes
        //   * root of transactions: `Blake3Hash`
        //   * local segment index of the first segment root (if any): `LocalSegmentIndex`
        //   * concatenated own segment roots

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = u16::from_le_bytes([num_blocks[0], num_blocks[1]]);

        let bytes_start = bytes;

        let mut counts = bytes.split_off(..usize::from(num_blocks) * size_of::<u8>())?;

        let mut remainder = align_to_and_ensure_zero_padding(bytes, Alignment::of::<u64>())?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            (_, remainder) = LeafShardHeader::try_from_bytes(remainder)?;

            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())?;

            (_, remainder) = Blake3Hash::from_bytes(remainder)?;

            if num_own_segment_roots > 0 {
                let first_local_segment_index;
                (first_local_segment_index, remainder) = LocalSegmentIndex::from_bytes(remainder)?;
                // The local segment index after the last segment root must fit into `u64`
                let _next_local_segment_index = LocalSegmentIndex::forward_checked(
                    *first_local_segment_index,
                    num_own_segment_roots,
                )?;
                let _own_segment_roots =
                    remainder.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            }
        }

        let info = Self {
            num_blocks,
            bytes: &bytes_start[..bytes_start.len() - remainder.len()],
        };

        if !info.is_internally_consistent() {
            return None;
        }

        Some((info, remainder))
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout is the same as in `Self::try_from_bytes()`

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = u16::from_le_bytes([num_blocks[0], num_blocks[1]]);

        let bytes_start = bytes;

        let mut counts = bytes.split_off(..usize::from(num_blocks) * size_of::<u8>())?;

        let mut remainder = align_to_and_ensure_zero_padding(bytes, Alignment::of::<u64>())?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            (_, remainder) = LeafShardHeader::try_from_bytes(remainder)?;

            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())?;

            (_, remainder) = Blake3Hash::from_bytes(remainder)?;

            if num_own_segment_roots > 0 {
                let first_local_segment_index;
                (first_local_segment_index, remainder) = LocalSegmentIndex::from_bytes(remainder)?;
                // The local segment index after the last segment root must fit into `u64`
                let _next_local_segment_index = LocalSegmentIndex::forward_checked(
                    *first_local_segment_index,
                    num_own_segment_roots,
                )?;
                let _own_segment_roots =
                    remainder.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            }
        }

        Some((
            Self {
                num_blocks,
                bytes: &bytes_start[..bytes_start.len() - remainder.len()],
            },
            remainder,
        ))
    }

    /// Check leaf shard info's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        let mut last_leaf_shard_info = None::<(ShardIndex, BlockNumber, Option<LocalSegmentIndex>)>;

        self.iter().all(|leaf_shard_block| {
            let shard_index = leaf_shard_block.header.prefix.shard_index;

            // Ensure increasing order of shard indices, block numbers and local segment indices
            if let Some((
                last_leaf_shard_index,
                last_leaf_shard_block_number,
                last_leaf_shard_first_local_segment_index,
            )) = last_leaf_shard_info
            {
                match last_leaf_shard_index.cmp(&shard_index) {
                    cmp::Ordering::Less => {
                        last_leaf_shard_info.replace((
                            shard_index,
                            leaf_shard_block.header.prefix.number,
                            leaf_shard_block
                                .own_segments
                                .as_ref()
                                .map(|own_segments| own_segments.first_local_segment_index),
                        ));
                    }
                    cmp::Ordering::Equal => {
                        if last_leaf_shard_block_number >= leaf_shard_block.header.prefix.number {
                            return false;
                        }
                        if let Some(own_segments) = &leaf_shard_block.own_segments {
                            if let Some(last_leaf_shard_first_local_segment_index) =
                                last_leaf_shard_first_local_segment_index
                                && last_leaf_shard_first_local_segment_index
                                    >= own_segments.first_local_segment_index
                            {
                                return false;
                            }

                            last_leaf_shard_info.replace((
                                shard_index,
                                leaf_shard_block.header.prefix.number,
                                Some(own_segments.first_local_segment_index),
                            ));
                        } else {
                            last_leaf_shard_info.replace((
                                shard_index,
                                leaf_shard_block.header.prefix.number,
                                last_leaf_shard_first_local_segment_index,
                            ));
                        }
                    }
                    cmp::Ordering::Greater => {
                        return false;
                    }
                }
            } else {
                last_leaf_shard_info.replace((
                    shard_index,
                    leaf_shard_block.header.prefix.number,
                    leaf_shard_block
                        .own_segments
                        .as_ref()
                        .map(|own_segments| own_segments.first_local_segment_index),
                ));
            }

            // Also checks blocks without segments, whose body root has a zero leaf for segments
            leaf_shard_body_root(
                leaf_shard_block.own_segments.as_ref(),
                leaf_shard_block.transactions_root,
            ) == leaf_shard_block.header.result.body_root
        })
    }

    /// Iterator over leaf shard blocks in a collection
    #[inline]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = LeafShardBlockInfo<'a>> + TrustedLen + use<'a> {
        // SAFETY: Checked in constructor
        let (mut counts, remainder) = unsafe {
            self.bytes
                .split_at_unchecked(usize::from(self.num_blocks) * size_of::<u8>())
        };
        let mut remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())
            .expect("Already checked in constructor; qed");

        iter::repeat_with(move || {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            // TODO: Unchecked method would have been helpful here
            let header;
            (header, remainder) = LeafShardHeader::try_from_bytes(remainder)
                .expect("Already checked in constructor; qed");

            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::of::<u64>())
                .expect("Already checked in constructor; qed");

            let transactions_root;
            // SAFETY: Checked in constructor
            (transactions_root, remainder) =
                unsafe { remainder.split_at_unchecked(Blake3Hash::SIZE) };
            // SAFETY: Correct alignment and size
            let transactions_root = unsafe { Blake3Hash::from_bytes_unchecked(transactions_root) };

            let own_segments = if num_own_segment_roots > 0 {
                let first_local_segment_index;
                // SAFETY: Checked in constructor
                (first_local_segment_index, remainder) =
                    unsafe { remainder.split_at_unchecked(LocalSegmentIndex::SIZE as usize) };
                // SAFETY: Correct alignment and size
                let first_local_segment_index =
                    *unsafe { LocalSegmentIndex::from_bytes_unchecked(first_local_segment_index) };

                let own_segment_roots;
                // SAFETY: Checked in constructor
                (own_segment_roots, remainder) = unsafe {
                    remainder.split_at_unchecked(num_own_segment_roots * SegmentRoot::SIZE)
                };
                // SAFETY: Valid pointer and size, no alignment requirements
                let own_segment_roots = unsafe {
                    slice::from_raw_parts(
                        own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                        num_own_segment_roots,
                    )
                };
                let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

                Some(OwnSegments::new_unchecked(
                    first_local_segment_index,
                    own_segment_roots,
                ))
            } else {
                None
            };

            LeafShardBlockInfo {
                header,
                transactions_root,
                own_segments,
            }
        })
        .take(usize::from(self.num_blocks))
    }

    /// Number of leaf shard blocks
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.num_blocks as usize
    }

    /// Returns `true` if there are no leaf shard blocks
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.num_blocks == 0
    }

    /// Compute the root of segments of the leaf shard blocks info.
    ///
    /// Returns the default value for an empty collection of shard blocks.
    #[inline]
    pub fn segments_root(&self) -> Blake3Hash {
        // Blocks come first, so positions never step past `u16::MAX`
        compute_leaf_shards_segments_root(
            self.num_blocks,
            self.iter()
                .zip(0..)
                .filter_map(|(shard_block_info, position)| {
                    shard_block_info.own_segments.map(|own_segments| {
                        (
                            position,
                            shard_block_info.header.prefix.shard_index,
                            own_segments,
                        )
                    })
                }),
        )
        .expect("Positions are strictly increasing and smaller than the number of blocks; qed")
    }

    /// Compute the root of child block roots, which the header stores.
    ///
    /// Returns the default value for an empty collection of shard blocks.
    #[inline]
    pub fn headers_root(&self) -> Blake3Hash {
        let root =
            ChildLeafShardBlocksMerkleTree::compute_root_only::<{ u64::from(u16::MAX) }, _, _>(
                self.iter()
                    .map(|shard_block_info| **shard_block_info.header.root()),
            )
            .unwrap_or_default();

        Blake3Hash::new(root)
    }
}

/// Block body that corresponds to an intermediate shard
#[derive(Debug, Copy, Clone, Yokeable)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct IntermediateShardBody<'a> {
    /// Segments produced by this shard
    own_segments: Option<OwnSegments<'a>>,
    /// Leaf shard blocks
    leaf_shard_blocks: LeafShardBlocksInfo<'a>,
}

impl<'a> GenericBlockBody<'a> for IntermediateShardBody<'a> {
    const SHARD_KIND: RealShardKind = RealShardKind::IntermediateShard;

    #[cfg(feature = "alloc")]
    type Owned = OwnedIntermediateShardBody;

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }

    #[inline(always)]
    fn root(&self) -> Blake3Hash {
        self.root()
    }
}

impl<'a> IntermediateShardBody<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of own segment roots: u8
        // * local segment index of the first segment root (if any): unaligned `LocalSegmentIndex`
        // * concatenated own segment roots
        // * leaf shard blocks: LeafShardBlocksInfo

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index;
            (first_local_segment_index, bytes) = Unaligned::<LocalSegmentIndex>::from_bytes(bytes)?;

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments::new(
                first_local_segment_index.as_inner(),
                own_segment_roots,
            )?)
        } else {
            None
        };

        let (leaf_shard_blocks, remainder) = LeafShardBlocksInfo::try_from_bytes(bytes)?;

        let body = Self {
            own_segments,
            leaf_shard_blocks,
        };

        if !body.is_internally_consistent() {
            return None;
        }

        Some((body, remainder))
    }

    /// Check block body's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        // Nothing to check here
        true
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout is the same as in `Self::try_from_bytes()`

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index;
            (first_local_segment_index, bytes) = Unaligned::<LocalSegmentIndex>::from_bytes(bytes)?;

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments::new(
                first_local_segment_index.as_inner(),
                own_segment_roots,
            )?)
        } else {
            None
        };

        let (leaf_shard_blocks, remainder) = LeafShardBlocksInfo::try_from_bytes_unchecked(bytes)?;

        Some((
            Self {
                own_segments,
                leaf_shard_blocks,
            },
            remainder,
        ))
    }

    /// Segment roots produced by this shard
    #[inline(always)]
    pub fn own_segments(&self) -> Option<OwnSegments<'a>> {
        self.own_segments
    }

    /// Leaf shard blocks
    #[inline(always)]
    pub fn leaf_shard_blocks(&self) -> &LeafShardBlocksInfo<'a> {
        &self.leaf_shard_blocks
    }

    /// Create an owned version of this body
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedIntermediateShardBody {
        if let Some(own_segments) = self.own_segments {
            OwnedIntermediateShardBody::new(
                own_segments
                    .iter()
                    .map(|(local_segment_index, &segment_root)| {
                        (local_segment_index, segment_root)
                    }),
                self.leaf_shard_blocks.iter(),
            )
            .expect("`self` is always a valid invariant; qed")
        } else {
            OwnedIntermediateShardBody::new(iter::empty(), self.leaf_shard_blocks.iter())
                .expect("`self` is always a valid invariant; qed")
        }
    }

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        intermediate_shard_body_root(
            self.own_segments.as_ref(),
            &self.leaf_shard_blocks.segments_root(),
            &self.leaf_shard_blocks.headers_root(),
        )
    }
}

/// Collection of transactions
#[derive(Debug, Copy, Clone)]
pub struct Transactions<'a> {
    num_transactions: usize,
    bytes: &'a [u8],
}

impl<'a> Transactions<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of transactions: u32 as unaligned little-endian bytes
        // * padding to 16-bytes boundary with zeroes
        // * for each transaction
        //   * transaction: Transaction
        //   * padding to 16-bytes boundary with zeroes

        let num_transactions = bytes.split_off(..size_of::<u32>())?;
        let num_transactions = u32::from_le_bytes([
            num_transactions[0],
            num_transactions[1],
            num_transactions[2],
            num_transactions[3],
        ]) as usize;

        let mut remainder = align_to_and_ensure_zero_padding(bytes, Alignment::MAX)?;
        let bytes_start = remainder;

        for _ in 0..num_transactions {
            (_, remainder) = Transaction::try_from_bytes(remainder)?;
            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::MAX)?;
        }

        Some((
            Self {
                num_transactions,
                bytes: &bytes_start[..bytes_start.len() - remainder.len()],
            },
            remainder,
        ))
    }

    /// Iterator over transactions in a collection
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Transaction<'a>> + TrustedLen + use<'a> {
        let mut remainder = self.bytes;

        iter::repeat_with(move || {
            // SAFETY: Checked in constructor
            let transaction = unsafe { Transaction::from_bytes_unchecked(remainder) };

            remainder = &remainder[transaction.encoded_size()..];
            remainder = align_to_and_ensure_zero_padding(remainder, Alignment::MAX)
                .expect("Already checked in constructor; qed");

            transaction
        })
        .take(self.num_transactions)
    }

    /// Number of transactions
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.num_transactions
    }

    /// Returns `true` if there are no transactions
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.num_transactions == 0
    }

    /// Compute the root of transactions.
    ///
    /// Returns the default value for an empty collection of transactions.
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = TransactionsMerkleTree::compute_root_only::<{ u64::from(u32::MAX) }, _, _>(
            self.iter().map(|transaction| **transaction.hash()),
        )
        .unwrap_or_default();

        Blake3Hash::new(root)
    }
}

/// Block body that corresponds to a leaf shard
#[derive(Debug, Copy, Clone, Yokeable)]
// Prevent creation of potentially broken invariants externally
#[non_exhaustive]
pub struct LeafShardBody<'a> {
    /// Segments produced by this shard
    own_segments: Option<OwnSegments<'a>>,
    /// User transactions
    transactions: Transactions<'a>,
}

impl<'a> GenericBlockBody<'a> for LeafShardBody<'a> {
    const SHARD_KIND: RealShardKind = RealShardKind::LeafShard;

    #[cfg(feature = "alloc")]
    type Owned = OwnedLeafShardBody;

    #[cfg(feature = "alloc")]
    #[inline(always)]
    fn to_owned(self) -> Self::Owned {
        self.to_owned()
    }

    #[inline(always)]
    fn root(&self) -> Blake3Hash {
        self.root()
    }
}

impl<'a> LeafShardBody<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of own segment roots: u8
        // * local segment index of the first segment root (if any): unaligned `LocalSegmentIndex`
        // * concatenated own segment roots
        // * transactions: Transactions

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index;
            (first_local_segment_index, bytes) = Unaligned::<LocalSegmentIndex>::from_bytes(bytes)?;

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments::new(
                first_local_segment_index.as_inner(),
                own_segment_roots,
            )?)
        } else {
            None
        };

        let (transactions, remainder) = Transactions::try_from_bytes(bytes)?;

        let body = Self {
            own_segments,
            transactions,
        };

        if !body.is_internally_consistent() {
            return None;
        }

        Some((body, remainder))
    }

    /// Check block body's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        // Nothing to check here
        true
    }

    /// Segment roots produced by this shard
    #[inline(always)]
    pub fn own_segments(&self) -> Option<OwnSegments<'a>> {
        self.own_segments
    }

    /// User transactions
    #[inline(always)]
    pub fn transactions(&self) -> &Transactions<'a> {
        &self.transactions
    }

    /// Create an owned version of this body
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedLeafShardBody {
        let mut builder = if let Some(own_segments) = self.own_segments {
            let own_segments = own_segments
                .iter()
                .map(|(local_segment_index, &segment_root)| (local_segment_index, segment_root));

            OwnedLeafShardBody::init(own_segments).expect("`self` is always a valid invariant; qed")
        } else {
            OwnedLeafShardBody::init(iter::empty())
                .expect("`self` is always a valid invariant; qed")
        };
        for transaction in self.transactions.iter() {
            builder
                .add_transaction(transaction)
                .expect("`self` is always a valid invariant; qed");
        }

        builder.finish()
    }

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        leaf_shard_body_root(self.own_segments.as_ref(), &self.transactions.root())
    }
}

/// Block body that together with [`BlockHeader`] form a [`Block`]
///
/// [`BlockHeader`]: crate::block::header::BlockHeader
/// [`Block`]: crate::block::Block
#[derive(Debug, Copy, Clone, From)]
pub enum BlockBody<'a> {
    /// Block body corresponds to the beacon chain
    BeaconChain(BeaconChainBody<'a>),
    /// Block body corresponds to an intermediate shard
    IntermediateShard(IntermediateShardBody<'a>),
    /// Block body corresponds to a leaf shard
    LeafShard(LeafShardBody<'a>),
}

impl<'a> BlockBody<'a> {
    /// Try to create a new instance from provided bytes for the provided shard index.
    ///
    /// `bytes` should be 4-bytes aligned for the beacon chain and do not need to be aligned for
    /// other shard kinds.
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8], shard_kind: RealShardKind) -> Option<(Self, &'a [u8])> {
        match shard_kind {
            RealShardKind::BeaconChain => {
                let (body, remainder) = BeaconChainBody::try_from_bytes(bytes)?;
                Some((Self::BeaconChain(body), remainder))
            }
            RealShardKind::IntermediateShard => {
                let (body, remainder) = IntermediateShardBody::try_from_bytes(bytes)?;
                Some((Self::IntermediateShard(body), remainder))
            }
            RealShardKind::LeafShard => {
                let (body, remainder) = LeafShardBody::try_from_bytes(bytes)?;
                Some((Self::LeafShard(body), remainder))
            }
        }
    }

    /// Check block body's internal consistency.
    ///
    /// This is usually not necessary to be called explicitly since internal consistency is checked
    /// by [`Self::try_from_bytes()`] internally.
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        match self {
            Self::BeaconChain(body) => body.is_internally_consistent(),
            Self::IntermediateShard(body) => body.is_internally_consistent(),
            Self::LeafShard(body) => body.is_internally_consistent(),
        }
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(
        bytes: &'a [u8],
        shard_kind: RealShardKind,
    ) -> Option<(Self, &'a [u8])> {
        match shard_kind {
            RealShardKind::BeaconChain => {
                let (body, remainder) = BeaconChainBody::try_from_bytes_unchecked(bytes)?;
                Some((Self::BeaconChain(body), remainder))
            }
            RealShardKind::IntermediateShard => {
                let (body, remainder) = IntermediateShardBody::try_from_bytes_unchecked(bytes)?;
                Some((Self::IntermediateShard(body), remainder))
            }
            RealShardKind::LeafShard => {
                // Leaf shard bodies have no consistency checks to skip
                let (body, remainder) = LeafShardBody::try_from_bytes(bytes)?;
                Some((Self::LeafShard(body), remainder))
            }
        }
    }

    /// Create an owned version of this body
    #[cfg(feature = "alloc")]
    #[inline(always)]
    pub fn to_owned(self) -> OwnedBlockBody {
        match self {
            Self::BeaconChain(body) => body.to_owned().into(),
            Self::IntermediateShard(body) => body.to_owned().into(),
            Self::LeafShard(body) => body.to_owned().into(),
        }
    }

    /// Compute block body root.
    ///
    /// Block body hash is actually a Merkle Tree Root. The leaves are derived from individual
    /// fields this enum in the declaration order.
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        match self {
            Self::BeaconChain(body) => body.root(),
            Self::IntermediateShard(body) => body.root(),
            Self::LeafShard(body) => body.root(),
        }
    }
}
