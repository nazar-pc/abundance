//! Block body primitives

use crate::block::align_to_and_ensure_zero_padding;
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
pub fn compute_segments_root(segment_roots: &[SegmentRoot]) -> Blake3Hash {
    // TODO: Keyed hash
    let root = UnbalancedHashedMerkleTree::compute_root_only::<{ u32::MAX as usize }, _, _>(
        segment_roots.iter().map(|segment_root| {
            // Hash the root again so we can prove it, otherwise segments' root is indistinguishable
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
    pub block_header: IntermediateShardBlockHeader<'a>,
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
    /// Segment roots produced by child shard
    pub child_segment_roots: &'a [SegmentRoot],
}

impl IntermediateShardBlockInfo<'_> {
    /// Compute the root of the intermediate shard block info
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        const MAX_N: usize = 3;
        let leaves: [_; MAX_N] = [
            self.block_header.hash().into(),
            compute_segments_root(self.own_segment_roots),
            compute_segments_root(self.child_segment_roots),
        ];
        let root = UnbalancedHashedMerkleTree::compute_root_only::<MAX_N, _, _>(leaves)
            .expect("List is not empty; qed");

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
        //   * block header
        //   * concatenated own segment roots
        //   * concatenated child segment roots
        //   * padding to 8-bytes boundary (if needed)

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

            let _segment_roots = remainder.split_off(
                ..(num_own_segment_roots + num_child_segment_roots) * SegmentRoot::SIZE,
            )?;

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)?;
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
    ) -> impl ExactSizeIterator<Item = IntermediateShardBlockInfo<'a>> + TrustedLen + 'a {
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
            let block_header;
            (block_header, remainder) = IntermediateShardBlockHeader::try_from_bytes(remainder)
                .expect("Checked in constructor; qed");

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

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)
                .expect("Checked in constructor; qed");

            IntermediateShardBlockInfo {
                block_header,
                own_segment_roots,
                child_segment_roots,
            }
        })
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
    /// Proof of time checkpoints from after future proof of time of the parent block to current
    /// block's future proof of time (inclusive)
    pub pot_checkpoints: &'a [PotCheckpoints],
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
    /// Intermediate shard blocks
    pub intermediate_shard_blocks: IntermediateShardBlocksInfo<'a>,
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
        // * concatenated PoT checkpoints
        // * concatenated own segment roots
        // * intermediate shard blocks: IntermediateShardBlocksInfo

        let num_pot_checkpoints = bytes.split_off(..size_of::<u16>())?;
        // SAFETY: All bit patterns are valid
        let num_pot_checkpoints =
            *unsafe { <u32 as TrivialType>::from_bytes(num_pot_checkpoints) }? as usize;

        if num_pot_checkpoints == 0 {
            return None;
        }

        let num_own_segment_roots = bytes.split_off(..size_of::<u8>())?;
        let num_own_segment_roots = usize::from(num_own_segment_roots[0]);

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

        Some((
            Self {
                pot_checkpoints,
                own_segment_roots,
                intermediate_shard_blocks,
            },
            remainder,
        ))
    }

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        const MAX_N: usize = 3;
        let leaves: [_; MAX_N] = [
            Blake3Hash::new(
                blake3::hash(PotCheckpoints::bytes_from_slice(self.pot_checkpoints).as_flattened())
                    .into(),
            ),
            compute_segments_root(self.own_segment_roots),
            self.intermediate_shard_blocks.root(),
        ];

        let root = UnbalancedHashedMerkleTree::compute_root_only::<MAX_N, _, _>(leaves)
            .expect("List is not empty; qed");

        Blake3Hash::new(root)
    }
}

/// Information about leaf shard block
#[derive(Debug, Copy, Clone)]
pub struct LeafShardBlockInfo<'a> {
    /// Block header that corresponds to an intermediate shard
    pub block_header: LeafShardBlockHeader<'a>,
    /// Segment roots produced by this shard
    pub own_segment_roots: &'a [SegmentRoot],
}

impl LeafShardBlockInfo<'_> {
    /// Compute the root of the leaf shard block info
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root = BalancedHashedMerkleTree::compute_root_only(&[
            **self.block_header.hash(),
            *compute_segments_root(self.own_segment_roots),
        ]);

        Blake3Hash::new(root)
    }
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
        //   * concatenated own segment roots
        //   * padding to 8-bytes boundary (if needed)

        let num_blocks = bytes.split_off(..size_of::<u16>())?;
        let num_blocks = usize::from(u16::from_le_bytes([num_blocks[0], num_blocks[1]]));

        let bytes_start = bytes;

        let mut counts = bytes.split_off(..num_blocks * size_of::<u8>())?;

        let mut remainder = align_to_and_ensure_zero_padding::<u64>(bytes)?;

        for _ in 0..num_blocks {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            (_, remainder) = LeafShardBlockHeader::try_from_bytes(remainder)?;

            let _own_segment_roots =
                remainder.split_off(..num_own_segment_roots * SegmentRoot::SIZE)?;

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)?;
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
    pub fn iter(&self) -> impl ExactSizeIterator<Item = LeafShardBlockInfo<'a>> + TrustedLen + 'a {
        // SAFETY: Checked in constructor
        let (mut counts, mut remainder) = unsafe {
            self.bytes
                .split_at_unchecked(self.num_blocks * size_of::<u8>())
        };

        (0..self.num_blocks).map(move |_| {
            let num_own_segment_roots = usize::from(counts[0]);
            counts = &counts[1..];

            // TODO: Unchecked method would have been helpful here
            let block_header;
            (block_header, remainder) = LeafShardBlockHeader::try_from_bytes(remainder)
                .expect("Checked in constructor; qed");

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

            remainder = align_to_and_ensure_zero_padding::<u64>(remainder)
                .expect("Checked in constructor; qed");

            LeafShardBlockInfo {
                block_header,
                own_segment_roots,
            }
        })
    }

    /// Compute the root of the leaf shard blocks info.
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
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Transaction<'a>> + TrustedLen + 'a {
        let mut remainder = self.bytes;

        (0..self.num_transactions).map(move |_| {
            // SAFETY: Checked in constructor
            let transaction = unsafe { Transaction::from_bytes_unchecked(remainder) };

            remainder = &remainder[transaction.encoded_size()..];
            remainder = align_to_and_ensure_zero_padding::<u128>(remainder)
                .expect("Checked in constructor; qed");

            transaction
        })
    }

    /// Compute the root of the leaf shard blocks info.
    ///
    /// Returns default value for an empty collection of shard blocks.
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        let root =
            UnbalancedHashedMerkleTree::compute_root_only::<{ u16::MAX as usize + 1 }, _, _>(
                self.iter().map(|transaction| {
                    // Hash the hash again so we can prove it, otherwise transactions' root is
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

    /// Compute block body root
    #[inline]
    pub fn root(&self) -> Blake3Hash {
        const MAX_N: usize = 3;
        let leaves: [_; MAX_N] = [
            compute_segments_root(self.own_segment_roots),
            self.leaf_shard_blocks.root(),
            self.transactions.root(),
        ];

        let root = UnbalancedHashedMerkleTree::compute_root_only::<MAX_N, _, _>(leaves)
            .expect("List is not empty; qed");

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
                let (block_header, remainder) = BeaconChainBlockBody::try_from_bytes(bytes)?;
                Some((Self::BeaconChain(block_header), remainder))
            }
            ShardKind::IntermediateShard => {
                let (block_header, remainder) = IntermediateShardBlockBody::try_from_bytes(bytes)?;
                Some((Self::IntermediateShard(block_header), remainder))
            }
            ShardKind::LeafShard => {
                let (block_header, remainder) = LeafShardBlockBody::try_from_bytes(bytes)?;
                Some((Self::LeafShard(block_header), remainder))
            }
            ShardKind::Phantom | ShardKind::Invalid => {
                // Blocks for such shards do not exist
                None
            }
        }
    }

    /// Compute block body hash.
    ///
    /// Block body hash is actually a Merkle Tree Root. The leaves are derived from individual
    /// fields this enum in the declaration order.
    #[inline]
    pub fn hash(&self) -> Blake3Hash {
        match self {
            Self::BeaconChain(body) => body.root(),
            Self::IntermediateShard(body) => body.root(),
            Self::LeafShard(body) => body.root(),
        }
    }
}
