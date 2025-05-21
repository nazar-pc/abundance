//! Block body primitives

#[cfg(feature = "alloc")]
pub mod owned;

use crate::block::align_to_and_ensure_zero_padding;
#[cfg(feature = "alloc")]
use crate::block::body::owned::{
    OwnedBeaconChainBlockBody, OwnedBeaconChainBlockBodyError, OwnedBlockBody, OwnedBlockBodyError,
    OwnedIntermediateShardBlockBody, OwnedIntermediateShardBlockBodyError, OwnedLeafShardBlockBody,
    OwnedLeafShardBlockBodyError,
};
use crate::block::header::{IntermediateShardBlockHeader, LeafShardBlockHeader};
use crate::hashes::Blake3Hash;
use crate::pot::PotCheckpoints;
use crate::segments::SegmentRoot;
use crate::shard::ShardKind;
use crate::transaction::Transaction;
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::balanced_hashed::BalancedHashedMerkleTree;
use ab_merkle_tree::unbalanced_hashed::UnbalancedHashedMerkleTree;
use core::iter::TrustedLen;
use core::slice;
use derive_more::From;

/// Calculates a Merkle Tree root for a provided list of segment roots
#[inline]
pub fn compute_segments_root<Item, Iter>(segment_roots: Iter) -> Blake3Hash
where
    Item: AsRef<[u8]>,
    Iter: IntoIterator<Item = Item>,
{
    // TODO: This is a workaround for https://github.com/rust-lang/rust/issues/139866 that
    //  allows the code to compile. Constant 16 is hardcoded here and in `if` branch below
    //  for compilation to succeed
    const _: () = {
        assert!(u32::MAX == 4294967295);
    };
    // TODO: Keyed hash
    let root = UnbalancedHashedMerkleTree::compute_root_only::<4294967295, _, _>(
        segment_roots.into_iter().map(|segment_root| {
            // Hash the root again so we can prove it, otherwise segments root is indistinguishable
            // from individual segment roots and can be used to confuse verifier
            blake3::hash(segment_root.as_ref())
        }),
    );

    Blake3Hash::new(root.unwrap_or_default())
}

/// Information about intermediate shard block
#[derive(Debug, Copy, Clone)]
pub struct IntermediateShardBlockInfo<'a> {
    /// Block header that corresponds to an intermediate shard
    pub header: IntermediateShardBlockHeader<'a>,
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
            **self.header.root(),
            *compute_segments_root(self.own_segment_roots),
            *compute_segments_root(self.child_segment_roots),
        ];

        let root = UnbalancedHashedMerkleTree::compute_root_only::<MAX_N, _, _>(leaves)
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
        // * padding to 8-bytes boundary (if needed)
        // * for each block:
        //   * block header: IntermediateShardBlockHeader
        //   * padding to 8-bytes boundary (if needed)
        //   * segment roots proof (if there is at least one segment root)
        //   * concatenated own segment roots
        //   * concatenated child segment roots

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = usize::from(u16::from_le_bytes([num_blocks[1], num_blocks[2]]));

        let bytes_start = bytes;

        let mut counts = bytes.split_off(..num_blocks * (size_of::<u8>() + size_of::<u16>()))?;

        let mut remainder = align_to_and_ensure_zero_padding::<u64>(bytes)?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            let num_child_segment_roots = usize::from(u16::from_le_bytes([counts[1], counts[2]]));
            counts = &counts[3..];

            (_, remainder) = IntermediateShardBlockHeader::try_from_bytes(remainder)?;

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)?;

            if num_own_segment_roots + num_child_segment_roots > 0 {
                let _segment_roots_proof = remainder.split_off(..SegmentRoot::SIZE)?;
            }

            let _segment_roots = remainder.split_off(
                ..(num_own_segment_roots + num_child_segment_roots) * SegmentRoot::SIZE,
            )?;
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
            (header, remainder) = IntermediateShardBlockHeader::try_from_bytes(remainder)
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

    /// Compute the segments root of the intermediate shard blocks info.
    ///
    /// Returns default value for an empty collection of segment roots.
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

    /// Compute the headers root of the intermediate shard blocks info.
    ///
    /// Returns default value for an empty collection of shard blocks.
    #[inline]
    pub fn headers_root(&self) -> Blake3Hash {
        let root =
            UnbalancedHashedMerkleTree::compute_root_only::<{ u16::MAX as usize + 1 }, _, _>(
                // TODO: Keyed hash
                self.iter().map(|shard_block_info| {
                    // Hash the root again so we can prove it, otherwise headers root is
                    // indistinguishable from individual block roots and can be used to confuse
                    // verifier

                    blake3::hash(shard_block_info.header.root().as_ref())
                }),
            )
            .unwrap_or_default();

        Blake3Hash::new(root)
    }

    /// Compute the root of the intermediate shard blocks info.
    ///
    /// Returns default value for an empty collection of shard blocks.
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root =
            UnbalancedHashedMerkleTree::compute_root_only::<{ u16::MAX as usize + 1 }, _, _>(
                self.iter().map(|shard_block_info| shard_block_info.root()),
            )
            .unwrap_or_default();

        Blake3Hash::new(root)
    }
}

/// Block body that corresponds to the beacon chain
#[derive(Debug, Copy, Clone)]
pub struct BeaconChainBlockBody<'a> {
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
    /// Intermediate shard blocks
    pub intermediate_shard_blocks: IntermediateShardBlocksInfo<'a>,
    /// Proof of time checkpoints from after future proof of time of the parent block to current
    /// block's future proof of time (inclusive)
    pub pot_checkpoints: &'a [PotCheckpoints],
}

impl<'a> BeaconChainBlockBody<'a> {
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
        // * concatenated own segment roots
        // * intermediate shard blocks: IntermediateShardBlocksInfo
        // * concatenated PoT checkpoints

        let num_pot_checkpoints = bytes.split_off(..size_of::<u16>())?;
        // SAFETY: All bit patterns are valid
        let num_pot_checkpoints =
            *unsafe { <u32 as TrivialType>::from_bytes(num_pot_checkpoints) }? as usize;

        if num_pot_checkpoints == 0 {
            return None;
        }

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
        // SAFETY: Valid pointer and size, no alignment requirements
        let own_segment_roots = unsafe {
            slice::from_raw_parts(
                own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                num_own_segment_roots,
            )
        };
        let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

        let (intermediate_shard_blocks, remainder) =
            IntermediateShardBlocksInfo::try_from_bytes(bytes)?;

        let pot_checkpoints = bytes.split_off(..num_pot_checkpoints * PotCheckpoints::SIZE)?;
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
                pot_checkpoints,
                own_segment_roots,
                intermediate_shard_blocks,
            },
            remainder,
        ))
    }

    /// Check block body's internal consistency
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        self.intermediate_shard_blocks
            .iter()
            .all(|intermediate_shard_block| {
                let Some(&segment_roots_proof) = intermediate_shard_block.segment_roots_proof
                else {
                    return true;
                };

                BalancedHashedMerkleTree::<2>::verify(
                    &intermediate_shard_block.header.result.body_root,
                    &[segment_roots_proof],
                    0,
                    BalancedHashedMerkleTree::compute_root_only(&[
                        *compute_segments_root(intermediate_shard_block.own_segment_roots),
                        *compute_segments_root(intermediate_shard_block.child_segment_roots),
                    ]),
                )
            })
    }

    /// Create an owned version of this body
    #[inline(always)]
    #[cfg(feature = "alloc")]
    pub fn to_owned(self) -> Result<OwnedBeaconChainBlockBody, OwnedBeaconChainBlockBodyError> {
        OwnedBeaconChainBlockBody::from_body(self)
    }

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = BalancedHashedMerkleTree::compute_root_only(&[
            *compute_segments_root(self.own_segment_roots),
            *self.intermediate_shard_blocks.segments_root(),
            *self.intermediate_shard_blocks.headers_root(),
            blake3::hash(PotCheckpoints::bytes_from_slice(self.pot_checkpoints).as_flattened())
                .into(),
        ]);

        Blake3Hash::new(root)
    }
}

/// Information about leaf shard block
#[derive(Debug, Copy, Clone)]
pub struct LeafShardBlockInfo<'a> {
    /// Block header that corresponds to an intermediate shard
    pub header: LeafShardBlockHeader<'a>,
    /// Segment roots proof if there are segment roots in the corresponding block
    pub segment_roots_proof: Option<&'a [u8; 32]>,
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
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
        // * padding to 8-bytes boundary (if needed)
        // * for each block:
        //   * block header
        //   * padding to 8-bytes boundary (if needed)
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

            (_, remainder) = LeafShardBlockHeader::try_from_bytes(remainder)?;

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)?;

            if num_own_segment_roots > 0 {
                let _segment_roots_proof = remainder.split_off(..SegmentRoot::SIZE)?;
            }

            let _own_segment_roots =
                remainder.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
        }

        Some((
            Self {
                num_blocks,
                bytes: &bytes_start[..bytes_start.len() - remainder.len()],
            },
            remainder,
        ))
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
            (header, remainder) = LeafShardBlockHeader::try_from_bytes(remainder)
                .expect("Already checked in constructor; qed");

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)
                .expect("Already checked in constructor; qed");

            let segment_roots_proof = if num_own_segment_roots > 0 {
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

            LeafShardBlockInfo {
                header,
                segment_roots_proof,
                own_segment_roots,
            }
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

    /// Compute the segments root of the leaf shard blocks info.
    ///
    /// Returns default value for an empty collection of segment roots.
    #[inline]
    pub fn segments_root(&self) -> Blake3Hash {
        compute_segments_root(
            self.iter()
                .flat_map(|shard_block_info| shard_block_info.own_segment_roots),
        )
    }

    /// Compute the headers root of the leaf shard blocks info.
    ///
    /// Returns default value for an empty collection of shard blocks.
    #[inline]
    pub fn headers_root(&self) -> Blake3Hash {
        let root =
            UnbalancedHashedMerkleTree::compute_root_only::<{ u16::MAX as usize + 1 }, _, _>(
                self.iter().map(|shard_block_info| {
                    // Hash the root again so we can prove it, otherwise headers root is
                    // indistinguishable from individual block roots and can be used to confuse
                    // verifier

                    blake3::hash(shard_block_info.header.root().as_ref())
                }),
            )
            .unwrap_or_default();

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
        // * padding to 16-bytes boundary (if needed)
        // * for each transaction
        //   * transaction: Transaction
        //   * padding to 16-bytes boundary (if needed)

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

    /// Compute the root of the leaf shard blocks info.
    ///
    /// Returns default value for an empty collection of shard blocks.
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root =
            UnbalancedHashedMerkleTree::compute_root_only::<{ u16::MAX as usize + 1 }, _, _>(
                self.iter().map(|transaction| {
                    // Hash the hash again so we can prove it, otherwise transactions root is
                    // indistinguishable from individual transaction roots and can be used to
                    // confuse verifier
                    blake3::hash(transaction.hash().as_ref())
                }),
            )
            .unwrap_or_default();

        Blake3Hash::new(root)
    }
}

/// Block body that corresponds to an intermediate shard
#[derive(Debug, Copy, Clone)]
pub struct IntermediateShardBlockBody<'a> {
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
    /// Leaf shard blocks
    pub leaf_shard_blocks: LeafShardBlocksInfo<'a>,
    /// User transactions
    pub transactions: Transactions<'a>,
}

impl<'a> IntermediateShardBlockBody<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of own segment roots: u8
        // * concatenated own segment roots
        // * leaf shard blocks: LeafShardBlocksInfo
        // * transactions: Transactions

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
        // SAFETY: Valid pointer and size, no alignment requirements
        let own_segment_roots = unsafe {
            slice::from_raw_parts(
                own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                num_own_segment_roots,
            )
        };
        let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

        let (leaf_shard_blocks, remainder) = LeafShardBlocksInfo::try_from_bytes(bytes)?;

        let (transactions, remainder) = Transactions::try_from_bytes(remainder)?;

        Some((
            Self {
                own_segment_roots,
                leaf_shard_blocks,
                transactions,
            },
            remainder,
        ))
    }

    /// Check block body's internal consistency
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        self.leaf_shard_blocks.iter().all(|leaf_shard_block| {
            let Some(&segment_roots_proof) = leaf_shard_block.segment_roots_proof else {
                return true;
            };

            BalancedHashedMerkleTree::<2>::verify(
                &leaf_shard_block.header.result.body_root,
                &[segment_roots_proof],
                0,
                *compute_segments_root(leaf_shard_block.own_segment_roots),
            )
        })
    }

    /// Proof for segment roots included in the body
    #[inline]
    pub fn segment_roots_proof(&self) -> [u8; 32] {
        // Merkle Tree is recursive. First two leafs (own and leaf shards record roots) are one
        // subtree, the second subtree is the proof needed to verify them both.
        BalancedHashedMerkleTree::compute_root_only(&[
            *self.leaf_shard_blocks.headers_root(),
            *self.transactions.root(),
        ])
    }

    /// Create an owned version of this body
    #[inline(always)]
    #[cfg(feature = "alloc")]
    pub fn to_owned(
        self,
    ) -> Result<OwnedIntermediateShardBlockBody, OwnedIntermediateShardBlockBodyError> {
        OwnedIntermediateShardBlockBody::from_body(self)
    }

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = BalancedHashedMerkleTree::compute_root_only(&[
            *compute_segments_root(self.own_segment_roots),
            *self.leaf_shard_blocks.segments_root(),
            *self.leaf_shard_blocks.headers_root(),
            *self.transactions.root(),
        ]);

        Blake3Hash::new(root)
    }
}

/// Block body that corresponds to a leaf shard
#[derive(Debug, Copy, Clone)]
pub struct LeafShardBlockBody<'a> {
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
    /// User transactions
    pub transactions: Transactions<'a>,
}

impl<'a> LeafShardBlockBody<'a> {
    /// Create an instance from provided bytes.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success.
    #[inline]
    pub fn try_from_bytes(mut bytes: &'a [u8]) -> Option<(Self, &'a [u8])> {
        // The layout here is as follows:
        // * number of own segment roots: u8
        // * concatenated own segment roots
        // * transactions: Transactions

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

        let own_segment_roots = bytes.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;
        // SAFETY: Valid pointer and size, no alignment requirements
        let own_segment_roots = unsafe {
            slice::from_raw_parts(
                own_segment_roots.as_ptr().cast::<[u8; SegmentRoot::SIZE]>(),
                num_own_segment_roots,
            )
        };
        let own_segment_roots = SegmentRoot::slice_from_repr(own_segment_roots);

        let (transactions, remainder) = Transactions::try_from_bytes(bytes)?;

        Some((
            Self {
                own_segment_roots,
                transactions,
            },
            remainder,
        ))
    }

    /// Check block body's internal consistency
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        // Nothing to check here
        true
    }

    /// Proof for segment roots included in the body
    #[inline]
    pub fn segment_roots_proof(&self) -> [u8; 32] {
        *self.transactions.root()
    }

    /// Create an owned version of this body
    #[inline(always)]
    #[cfg(feature = "alloc")]
    pub fn to_owned(self) -> Result<OwnedLeafShardBlockBody, OwnedLeafShardBlockBodyError> {
        OwnedLeafShardBlockBody::from_body(self)
    }

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = BalancedHashedMerkleTree::compute_root_only(&[
            *compute_segments_root(self.own_segment_roots),
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
    BeaconChain(BeaconChainBlockBody<'a>),
    /// Block body corresponds to an intermediate shard
    IntermediateShard(IntermediateShardBlockBody<'a>),
    /// Block body corresponds to a leaf shard
    LeafShard(LeafShardBlockBody<'a>),
}

impl<'a> BlockBody<'a> {
    /// Try to create a new instance from provided bytes for provided shard index.
    ///
    /// `bytes` do not need to be aligned.
    ///
    /// Returns an instance and remaining bytes on success, `None` if too few bytes were given,
    /// bytes are not properly aligned or input is otherwise invalid.
    #[inline]
    pub fn try_from_bytes(bytes: &'a [u8], shard_kind: ShardKind) -> Option<(Self, &'a [u8])> {
        match shard_kind {
            ShardKind::BeaconChain => {
                let (body, remainder) = BeaconChainBlockBody::try_from_bytes(bytes)?;
                Some((Self::BeaconChain(body), remainder))
            }
            ShardKind::IntermediateShard => {
                let (body, remainder) = IntermediateShardBlockBody::try_from_bytes(bytes)?;
                Some((Self::IntermediateShard(body), remainder))
            }
            ShardKind::LeafShard => {
                let (body, remainder) = LeafShardBlockBody::try_from_bytes(bytes)?;
                Some((Self::LeafShard(body), remainder))
            }
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                None
            }
        }
    }

    /// Check block body's internal consistency
    #[inline]
    pub fn is_internally_consistent(&self) -> bool {
        match self {
            Self::BeaconChain(body) => body.is_internally_consistent(),
            Self::IntermediateShard(body) => body.is_internally_consistent(),
            Self::LeafShard(body) => body.is_internally_consistent(),
        }
    }

    /// Create an owned version of this body
    #[inline(always)]
    #[cfg(feature = "alloc")]
    pub fn to_owned(self) -> Result<OwnedBlockBody, OwnedBlockBodyError> {
        OwnedBlockBody::from_body(self)
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
