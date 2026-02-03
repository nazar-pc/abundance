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
use ab_blake3::{BLOCK_LEN, single_block_hash};
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use ab_merkle_tree::balanced::BalancedMerkleTree;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
#[cfg(feature = "alloc")]
use core::iter;
use core::iter::TrustedLen;
use core::{array, cmp, fmt, slice};
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
pub fn compute_segments_root<'a, Iter>(segment_roots: Iter) -> Blake3Hash
where
    Iter: IntoIterator<Item = &'a SegmentRoot>,
{
    // TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that allows
    //  the code to compile. Constant 4294967295 is hardcoded here and below for compilation to
    //  succeed.
    #[expect(clippy::assertions_on_constants, reason = "Intentional documentation")]
    #[expect(clippy::eq_op, reason = "Intentional documentation")]
    const {
        assert!(u32::MAX == 4294967295);
    }
    // TODO: Keyed hash
    let root = UnbalancedMerkleTree::compute_root_only::<4294967295, _, _>(
        segment_roots.into_iter().map(|segment_root| {
            // Hash the root again so we can prove it, otherwise root of segments is
            // indistinguishable from individual segment roots and can be used to confuse verifier
            single_block_hash(segment_root.as_ref())
                .expect("Less than a single block worth of bytes; qed")
        }),
    );

    Blake3Hash::new(root.unwrap_or_default())
}

/// Own segments produced by a shard
#[derive(Debug, Copy, Clone)]
pub struct OwnSegments<'a> {
    /// Local segment index of the first own segment root
    pub first_local_segment_index: LocalSegmentIndex,
    /// Segment roots produced by a shard
    pub segment_roots: &'a [SegmentRoot],
}

impl OwnSegments<'_> {
    /// Compute the root of own segments
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        // TODO: Keyed hash
        let root = BalancedMerkleTree::compute_root_only(&[
            single_block_hash(self.first_local_segment_index.as_bytes())
                .expect("Less than a single block worth of bytes; qed"),
            *compute_segments_root(self.segment_roots),
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
        // TODO: Keyed hash
        let root = BalancedMerkleTree::compute_root_only(&[
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
            *compute_segments_root(self.segment_roots),
        ]);

        Blake3Hash::new(root)
    }
}

/// Information about intermediate shard block
#[derive(Debug, Clone)]
pub struct IntermediateShardBlockInfo<'a> {
    /// A block header that corresponds to an intermediate shard
    pub header: IntermediateShardHeader<'a>,
    /// Segment roots proof if there are segment roots in the corresponding block
    pub segment_roots_proof: Option<&'a [u8; 32]>,
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
    /// Segment roots produced by child shard
    pub child_segment_roots: &'a [SegmentRoot],
}

impl IntermediateShardBlockInfo<'_> {
    /// Compute the root of the intermediate shard block info
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        // TODO: Keyed hash
        const MAX_N: usize = 3;
        let leaves: [_; MAX_N] = [
            ***self.header.root(),
            *compute_segments_root(self.own_segment_roots),
            *compute_segments_root(self.child_segment_roots),
        ];

        let root = UnbalancedMerkleTree::compute_root_only::<{ MAX_N as u64 }, _, _>(leaves)
            .expect("The list is not empty; qed");

        Blake3Hash::new(root)
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
        //   * number of child segment roots: u16 as unaligned little-endian bytes
        // * padding to 8-bytes boundary with zeroes
        // * for each block:
        //   * block header: IntermediateShardHeader
        //   * padding to 8-bytes boundary with zeroes
        //   * segment roots proof (if there is at least one segment root)
        //   * concatenated own segment roots
        //   * concatenated child segment roots

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = usize::from(u16::from_le_bytes([num_blocks[0], num_blocks[1]]));

        let bytes_start = bytes;

        let mut counts = bytes.split_off(..num_blocks * (size_of::<u8>() + size_of::<u16>()))?;

        let mut remainder = align_to_and_ensure_zero_padding::<u64>(bytes)?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            let num_child_segment_roots = usize::from(u16::from_le_bytes([counts[1], counts[2]]));
            counts = &counts[3..];

            (_, remainder) = IntermediateShardHeader::try_from_bytes(remainder)?;

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)?;

            if num_own_segment_roots + num_child_segment_roots > 0 {
                // TODO: Verify this proof against segment roots
                let _segment_roots_proof = remainder.split_off(..SegmentRoot::SIZE)?;
                let _segment_roots = remainder.split_off(
                    ..(num_own_segment_roots + num_child_segment_roots) * SegmentRoot::SIZE,
                )?;
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

    /// Iterator over intermediate shard blocks in a collection
    #[inline]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = IntermediateShardBlockInfo<'a>> + TrustedLen + Clone + 'a
    {
        // SAFETY: Checked in constructor
        let (mut counts, mut remainder) = unsafe {
            self.bytes
                .split_at_unchecked(self.num_blocks * (size_of::<u8>() + size_of::<u16>()))
        };

        (0..self.num_blocks).map(move |_| {
            let num_own_segment_roots = usize::from(counts[0]);
            let num_child_segment_roots = usize::from(u16::from_le_bytes([counts[1], counts[2]]));
            counts = &counts[3..];

            // TODO: Unchecked method would have been helpful here
            let header;
            (header, remainder) = IntermediateShardHeader::try_from_bytes(remainder)
                .expect("Already checked in constructor; qed");

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)
                .expect("Already checked in constructor; qed");

            let segment_roots_proof = if num_own_segment_roots + num_child_segment_roots > 0 {
                let segment_roots_proof;
                // SAFETY: Checked in constructor
                (segment_roots_proof, remainder) =
                    unsafe { remainder.split_at_unchecked(SegmentRoot::SIZE) };
                // SAFETY: Valid pointer and size, no alignment requirements
                Some(unsafe {
                    segment_roots_proof
                        .as_ptr()
                        .cast::<[u8; SegmentRoot::SIZE]>()
                        .as_ref_unchecked()
                })
            } else {
                None
            };

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

            let child_segment_roots;
            // SAFETY: Checked in constructor
            (child_segment_roots, remainder) = unsafe {
                remainder.split_at_unchecked(num_child_segment_roots * SegmentRoot::SIZE)
            };
            // SAFETY: Valid pointer and size, no alignment requirements
            let child_segment_roots = unsafe {
                slice::from_raw_parts(
                    child_segment_roots
                        .as_ptr()
                        .cast::<[u8; SegmentRoot::SIZE]>(),
                    num_child_segment_roots,
                )
            };
            let child_segment_roots = SegmentRoot::slice_from_repr(child_segment_roots);

            IntermediateShardBlockInfo {
                header,
                segment_roots_proof,
                own_segment_roots,
                child_segment_roots,
            }
        })
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
    /// Returns the default value for an empty collection of segment roots.
    #[inline]
    pub fn segments_root(&self) -> Blake3Hash {
        compute_segments_root(
            self.iter()
                .flat_map(|shard_block_info| {
                    [
                        shard_block_info.own_segment_roots,
                        shard_block_info.child_segment_roots,
                    ]
                })
                .flatten(),
        )
    }

    /// Compute the root of headers of the intermediate shard blocks info.
    ///
    /// Returns the default value for an empty collection of shard blocks.
    #[inline]
    pub fn headers_root(&self) -> Blake3Hash {
        let root = UnbalancedMerkleTree::compute_root_only::<{ u16::MAX as u64 + 1 }, _, _>(
            // TODO: Keyed hash
            self.iter().map(|shard_block_info| {
                // Hash the root again so we can prove it, otherwise the root of headers is
                // indistinguishable from individual block roots and can be used to confuse
                // verifier
                single_block_hash(shard_block_info.header.root().as_ref())
                    .expect("Less than a single block worth of bytes; qed")
            }),
        )
        .unwrap_or_default();

        Blake3Hash::new(root)
    }

    /// Compute the root of the intermediate shard blocks info.
    ///
    /// Returns the default value for an empty collection of shard blocks.
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = UnbalancedMerkleTree::compute_root_only::<{ u16::MAX as u64 + 1 }, _, _>(
            self.iter().map(|shard_block_info| shard_block_info.root()),
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

        let num_pot_checkpoints = bytes.split_off(..size_of::<u32>())?;
        // SAFETY: All bit patterns are valid
        let num_pot_checkpoints =
            *unsafe { <u32 as TrivialType>::from_bytes(num_pot_checkpoints) }? as usize;

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index = bytes.split_off(..size_of::<LocalSegmentIndex>())?;
            // SAFETY: Unaligned and correct size
            let first_local_segment_index = unsafe {
                Unaligned::<LocalSegmentIndex>::from_bytes_unchecked(first_local_segment_index)
            }
            .as_inner();

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments {
                first_local_segment_index,
                segment_roots: own_segment_roots,
            })
        } else {
            None
        };

        let (intermediate_shard_blocks, mut remainder) =
            IntermediateShardBlocksInfo::try_from_bytes(bytes)?;

        let pot_checkpoints = remainder.split_off(..num_pot_checkpoints * PotCheckpoints::SIZE)?;
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
        self.intermediate_shard_blocks
            .iter()
            .all(|intermediate_shard_block| {
                let Some(&segment_roots_proof) = intermediate_shard_block.segment_roots_proof
                else {
                    return true;
                };

                BalancedMerkleTree::<2>::verify(
                    &intermediate_shard_block.header.result.body_root,
                    &[segment_roots_proof],
                    0,
                    BalancedMerkleTree::compute_root_only(&[
                        *compute_segments_root(intermediate_shard_block.own_segment_roots),
                        *compute_segments_root(intermediate_shard_block.child_segment_roots),
                    ]),
                )
            })
    }

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of PoT checkpoints: u32 as aligned little-endian bytes
        // * number of own segment roots: u8
        // * local segment index of the first segment root (if any): unaligned `LocalSegmentIndex`
        // * concatenated own segment roots
        // * intermediate shard blocks: IntermediateShardBlocksInfo
        // * concatenated PoT checkpoints

        let num_pot_checkpoints = bytes.split_off(..size_of::<u32>())?;
        // SAFETY: All bit patterns are valid
        let num_pot_checkpoints =
            *unsafe { <u32 as TrivialType>::from_bytes(num_pot_checkpoints) }? as usize;

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index = bytes.split_off(..size_of::<LocalSegmentIndex>())?;
            // SAFETY: Unaligned and correct size
            let first_local_segment_index = unsafe {
                Unaligned::<LocalSegmentIndex>::from_bytes_unchecked(first_local_segment_index)
            }
            .as_inner();

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments {
                first_local_segment_index,
                segment_roots: own_segment_roots,
            })
        } else {
            None
        };

        let (intermediate_shard_blocks, mut remainder) =
            IntermediateShardBlocksInfo::try_from_bytes(bytes)?;

        let pot_checkpoints = remainder.split_off(..num_pot_checkpoints * PotCheckpoints::SIZE)?;
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
            let first_local_segment_index = own_segments.first_local_segment_index;

            OwnedBeaconChainBody::new(
                own_segments.segment_roots.iter().copied().enumerate().map(
                    |(index, segment_root)| {
                        (
                            first_local_segment_index + LocalSegmentIndex::new(index as u64),
                            segment_root,
                        )
                    },
                ),
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
        // TODO: Keyed hash
        let root = BalancedMerkleTree::compute_root_only(&[
            *self
                .own_segments
                .as_ref()
                .map(OwnSegments::root)
                .unwrap_or_default(),
            *self.intermediate_shard_blocks.segments_root(),
            *self.intermediate_shard_blocks.headers_root(),
            blake3::hash(PotCheckpoints::bytes_from_slice(self.pot_checkpoints).as_flattened())
                .into(),
        ]);

        Blake3Hash::new(root)
    }
}

/// Information about leaf shard segments
#[derive(Debug, Copy, Clone)]
pub struct LeafShardOwnSegments<'a> {
    /// Segment roots proof
    pub segment_roots_proof: &'a [u8; 32],
    /// Segments produced by this shard
    pub own_segments: OwnSegments<'a>,
}

/// Information about leaf shard block container inside intermediate shard block body
#[derive(Debug, Clone)]
pub struct LeafShardBlockInfo<'a> {
    /// A block header that corresponds to an intermediate shard
    pub header: LeafShardHeader<'a>,
    /// Segments in the corresponding block
    pub segments: Option<LeafShardOwnSegments<'a>>,
}

/// Information about a collection of leaf shard blocks
#[derive(Debug, Copy, Clone)]
pub struct LeafShardBlocksInfo<'a> {
    num_blocks: usize,
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
        //   * local segment index of the first segment root (if any): `LocalSegmentIndex`
        //   * segment roots proof (if there is at least one segment root)
        //   * concatenated own segment roots

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = usize::from(u16::from_le_bytes([num_blocks[0], num_blocks[1]]));

        let bytes_start = bytes;

        let mut counts = bytes.split_off(..num_blocks * size_of::<u8>())?;

        let mut remainder = align_to_and_ensure_zero_padding::<u64>(bytes)?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            (_, remainder) = LeafShardHeader::try_from_bytes(remainder)?;

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)?;

            if num_own_segment_roots > 0 {
                let _first_local_segment_index =
                    remainder.split_off(..size_of::<LocalSegmentIndex>())?;
                let _segment_roots_proof = remainder.split_off(..SegmentRoot::SIZE)?;
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
        // The layout here is as follows:
        // * number of blocks: u16 as unaligned little-endian bytes
        // * for each block:
        //   * number of own segment roots: u8
        // * padding to 8-bytes boundary with zeroes
        // * for each block:
        //   * block header: LeafShardHeader
        //   * padding to 8-bytes boundary with zeroes
        //   * local segment index of the first segment root (if any): `LocalSegmentIndex`
        //   * segment roots proof (if there is at least one segment root)
        //   * concatenated own segment roots

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = usize::from(u16::from_le_bytes([num_blocks[0], num_blocks[1]]));

        let bytes_start = bytes;

        let mut counts = bytes.split_off(..num_blocks * size_of::<u8>())?;

        let mut remainder = align_to_and_ensure_zero_padding::<u64>(bytes)?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            (_, remainder) = LeafShardHeader::try_from_bytes(remainder)?;

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)?;

            if num_own_segment_roots > 0 {
                let _first_local_segment_index =
                    remainder.split_off(..size_of::<LocalSegmentIndex>())?;
                let _segment_roots_proof = remainder.split_off(..SegmentRoot::SIZE)?;
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
                                .segments
                                .as_ref()
                                .map(|segments| segments.own_segments.first_local_segment_index),
                        ));
                    }
                    cmp::Ordering::Equal => {
                        if last_leaf_shard_block_number >= leaf_shard_block.header.prefix.number {
                            return false;
                        }
                        if let Some(leaf_shard_segments) = &leaf_shard_block.segments {
                            if let Some(last_leaf_shard_first_local_segment_index) =
                                last_leaf_shard_first_local_segment_index
                                && last_leaf_shard_first_local_segment_index
                                    >= leaf_shard_segments.own_segments.first_local_segment_index
                            {
                                return false;
                            }

                            last_leaf_shard_info.replace((
                                shard_index,
                                leaf_shard_block.header.prefix.number,
                                Some(leaf_shard_segments.own_segments.first_local_segment_index),
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
                        .segments
                        .as_ref()
                        .map(|segments| segments.own_segments.first_local_segment_index),
                ));
            }

            let Some(segments) = leaf_shard_block.segments else {
                return true;
            };

            BalancedMerkleTree::<2>::verify(
                &leaf_shard_block.header.result.body_root,
                array::from_ref(segments.segment_roots_proof),
                0,
                *segments.own_segments.root(),
            )
        })
    }

    /// Iterator over leaf shard blocks in a collection
    #[inline]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = LeafShardBlockInfo<'a>> + TrustedLen + Clone + 'a {
        // SAFETY: Checked in constructor
        let (mut counts, mut remainder) = unsafe {
            self.bytes
                .split_at_unchecked(self.num_blocks * size_of::<u8>())
        };

        (0..self.num_blocks).map(move |_| {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            // TODO: Unchecked method would have been helpful here
            let header;
            (header, remainder) = LeafShardHeader::try_from_bytes(remainder)
                .expect("Already checked in constructor; qed");

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)
                .expect("Already checked in constructor; qed");

            let segments = if num_own_segment_roots > 0 {
                let first_local_segment_index;
                // SAFETY: Checked in constructor
                (first_local_segment_index, remainder) =
                    unsafe { remainder.split_at_unchecked(size_of::<LocalSegmentIndex>()) };
                // SAFETY: Correct alignment and size
                let first_local_segment_index =
                    *unsafe { LocalSegmentIndex::from_bytes_unchecked(first_local_segment_index) };

                let segment_roots_proof;
                // SAFETY: Checked in constructor
                (segment_roots_proof, remainder) =
                    unsafe { remainder.split_at_unchecked(SegmentRoot::SIZE) };
                // SAFETY: Valid pointer and size, no alignment requirements
                let segment_roots_proof = unsafe {
                    segment_roots_proof
                        .as_ptr()
                        .cast::<[u8; SegmentRoot::SIZE]>()
                        .as_ref_unchecked()
                };

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

                Some(LeafShardOwnSegments {
                    segment_roots_proof,
                    own_segments: OwnSegments {
                        first_local_segment_index,
                        segment_roots: own_segment_roots,
                    },
                })
            } else {
                None
            };

            LeafShardBlockInfo { header, segments }
        })
    }

    /// Number of leaf shard blocks
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.num_blocks
    }

    /// Returns `true` if there are no leaf shard blocks
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.num_blocks == 0
    }

    /// Compute the root of segments of the leaf shard blocks info.
    ///
    /// Returns the default value for an empty collection of segment roots.
    #[inline]
    pub fn segments_root(&self) -> Blake3Hash {
        let root = UnbalancedMerkleTree::compute_root_only::<{ u16::MAX as u64 + 1 }, _, _>(
            self.iter().map(|shard_block_info| {
                shard_block_info
                    .segments
                    .map(|own_segments| {
                        own_segments
                            .own_segments
                            .root_with_shard_index(shard_block_info.header.prefix.shard_index)
                    })
                    .unwrap_or_default()
            }),
        )
        .unwrap_or_default();

        Blake3Hash::new(root)
    }

    /// Compute the root of headers of the leaf shard blocks info.
    ///
    /// Returns the default value for an empty collection of shard blocks.
    #[inline]
    pub fn headers_root(&self) -> Blake3Hash {
        let root = UnbalancedMerkleTree::compute_root_only::<{ u16::MAX as u64 + 1 }, _, _>(
            self.iter().map(|shard_block_info| {
                // Hash the root again so we can prove it, otherwise root of headers is
                // indistinguishable from individual block roots and can be used to confuse
                // verifier
                single_block_hash(shard_block_info.header.root().as_ref())
                    .expect("Less than a single block worth of bytes; qed")
            }),
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
            let first_local_segment_index = bytes.split_off(..size_of::<LocalSegmentIndex>())?;
            // SAFETY: Unaligned and correct size
            let first_local_segment_index = unsafe {
                Unaligned::<LocalSegmentIndex>::from_bytes_unchecked(first_local_segment_index)
            }
            .as_inner();

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments {
                first_local_segment_index,
                segment_roots: own_segment_roots,
            })
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
        // The layout here is as follows:
        // * number of own segment roots: u8
        // * local segment index of the first segment root (if any): unaligned `LocalSegmentIndex`
        // * concatenated own segment roots
        // * leaf shard blocks: LeafShardBlocksInfo

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index = bytes.split_off(..size_of::<LocalSegmentIndex>())?;
            // SAFETY: Unaligned and correct size
            let first_local_segment_index = unsafe {
                Unaligned::<LocalSegmentIndex>::from_bytes_unchecked(first_local_segment_index)
            }
            .as_inner();

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments {
                first_local_segment_index,
                segment_roots: own_segment_roots,
            })
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
            let first_local_segment_index = own_segments.first_local_segment_index;

            OwnedIntermediateShardBody::new(
                own_segments.segment_roots.iter().copied().enumerate().map(
                    |(index, segment_root)| {
                        (
                            first_local_segment_index + LocalSegmentIndex::new(index as u64),
                            segment_root,
                        )
                    },
                ),
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
        // Explicit nested trees to emphasize that the proof size for segments is just one hash
        let root = BalancedMerkleTree::compute_root_only(&[
            BalancedMerkleTree::compute_root_only(&[
                *self
                    .own_segments
                    .as_ref()
                    .map(OwnSegments::root)
                    .unwrap_or_default(),
                *self.leaf_shard_blocks.segments_root(),
            ]),
            *self.leaf_shard_blocks.headers_root(),
        ]);

        Blake3Hash::new(root)
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

        let mut remainder = align_to_and_ensure_zero_padding::<u128>(bytes)?;
        let bytes_start = remainder;

        for _ in 0..num_transactions {
            (_, remainder) = Transaction::try_from_bytes(bytes)?;
            remainder = align_to_and_ensure_zero_padding::<u128>(remainder)?;
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
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Transaction<'a>> + TrustedLen + Clone + 'a {
        let mut remainder = self.bytes;

        (0..self.num_transactions).map(move |_| {
            // SAFETY: Checked in constructor
            let transaction = unsafe { Transaction::from_bytes_unchecked(remainder) };

            remainder = &remainder[transaction.encoded_size()..];
            remainder = align_to_and_ensure_zero_padding::<u128>(remainder)
                .expect("Already checked in constructor; qed");

            transaction
        })
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
        let root = UnbalancedMerkleTree::compute_root_only::<{ u16::MAX as u64 + 1 }, _, _>(
            self.iter().map(|transaction| {
                // Hash the hash again so we can prove it, otherwise root of transactions is
                // indistinguishable from individual transaction roots and can be used to
                // confuse verifier
                single_block_hash(transaction.hash().as_ref())
                    .expect("Less than a single block worth of bytes; qed")
            }),
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
            let first_local_segment_index = bytes.split_off(..size_of::<LocalSegmentIndex>())?;
            // SAFETY: Unaligned and correct size
            let first_local_segment_index = unsafe {
                Unaligned::<LocalSegmentIndex>::from_bytes_unchecked(first_local_segment_index)
            }
            .as_inner();

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments {
                first_local_segment_index,
                segment_roots: own_segment_roots,
            })
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

    /// The same as [`Self::try_from_bytes()`], but for trusted input that skips some consistency
    /// checks
    #[inline]
    pub fn try_from_bytes_unchecked(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of own segment roots: u8
        // * local segment index of the first segment root (if any): unaligned `LocalSegmentIndex`
        // * concatenated own segment roots
        // * transactions: Transactions

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segments = if num_own_segment_roots > 0 {
            let first_local_segment_index = bytes.split_off(..size_of::<LocalSegmentIndex>())?;
            // SAFETY: Unaligned and correct size
            let first_local_segment_index = unsafe {
                Unaligned::<LocalSegmentIndex>::from_bytes_unchecked(first_local_segment_index)
            }
            .as_inner();

            let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
            // SAFETY: Valid pointer and size, no alignment requirements
            let own_segment_roots = unsafe {
                slice::from_raw_parts(
                    own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                    num_own_segment_roots,
                )
            };
            let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

            Some(OwnSegments {
                first_local_segment_index,
                segment_roots: own_segment_roots,
            })
        } else {
            None
        };

        let (transactions, remainder) = Transactions::try_from_bytes(bytes)?;

        Some((
            Self {
                own_segments,
                transactions,
            },
            remainder,
        ))
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
            let first_local_segment_index = own_segments.first_local_segment_index;

            OwnedLeafShardBody::init(own_segments.segment_roots.iter().copied().enumerate().map(
                |(index, segment_root)| {
                    (
                        first_local_segment_index + LocalSegmentIndex::new(index as u64),
                        segment_root,
                    )
                },
            ))
            .expect("`self` is always a valid invariant; qed")
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
        let root = BalancedMerkleTree::compute_root_only(&[
            *self
                .own_segments
                .as_ref()
                .map(OwnSegments::root)
                .unwrap_or_default(),
            *self.transactions.root(),
        ]);

        Blake3Hash::new(root)
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
    /// `bytes` do not need to be aligned.
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
                let (body, remainder) = LeafShardBody::try_from_bytes_unchecked(bytes)?;
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
