//! Merkle Trees used by the protocol.
//!
//! Every tree hashes its inner nodes with its own key, so a node of one tree can't be interpreted
//! as a node of another tree. Trees with a number of leaves that isn't known at compile time also
//! commit to it.

mod hashers;
#[cfg(test)]
mod tests;

use crate::address::Address;
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::balanced::BalancedMerkleTree;
use ab_merkle_tree::blake3::Blake3MerkleHasher;
use ab_merkle_tree::mmr::MerkleMountainRange;
use ab_merkle_tree::sparse::SparseMerkleTree;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use hashers::{Blake3SizedMerkleHasher, Blake3StaticMerkleHasher};

type SegmentRootsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"segment-roots-merkle-tree") }>;
type OwnSegmentsMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"own-segments-merkle-tree") }>;
type ShardOwnSegmentsMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"shard-own-segments-merkle-tree") }>;
type LeafShardBodyMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"leaf-shard-body-merkle-tree") }>;
type TransactionsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"transactions-merkle-tree") }>;
type IntermediateShardBodyMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"intermediate-shard-body-merkle-tree") }>;
type LeafShardsSegmentsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"leaf-shards-segments-merkle-tree") }>;
type ChildLeafShardBlocksMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"child-leaf-shard-blocks-merkle-tree") }>;
type ChildIntermediateShardBlocksMerkleHasher = Blake3SizedMerkleHasher<
    { ab_blake3::const_hash(b"child-intermediate-shard-blocks-merkle-tree") },
>;
type BeaconChainBodyMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"beacon-chain-body-merkle-tree") }>;
type IntermediateShardsSegmentsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"intermediate-shards-segments-merkle-tree") }>;
type BeaconChainHeaderMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"beacon-chain-header-merkle-tree") }>;
type IntermediateShardHeaderMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"intermediate-shard-header-merkle-tree") }>;
type LeafShardHeaderMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"leaf-shard-header-merkle-tree") }>;
type SuperSegmentMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"super-segment-merkle-tree") }>;
type StateOwnersSparseMerkleHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"state-owners-sparse-merkle-tree") }>;
type StateOwnerContractsSparseMerkleHasher = Blake3StaticMerkleHasher<
    { ab_blake3::const_hash(b"state-owner-contracts-sparse-merkle-tree") },
>;
type BlockMerkleMountainRangeHasher =
    Blake3StaticMerkleHasher<{ ab_blake3::const_hash(b"block-merkle-mountain-range") }>;

const {
    let keys = [
        Blake3MerkleHasher::KEY,
        SegmentRootsMerkleHasher::BASE_KEY,
        OwnSegmentsMerkleHasher::KEY,
        ShardOwnSegmentsMerkleHasher::KEY,
        LeafShardBodyMerkleHasher::KEY,
        TransactionsMerkleHasher::BASE_KEY,
        IntermediateShardBodyMerkleHasher::KEY,
        LeafShardsSegmentsMerkleHasher::BASE_KEY,
        ChildLeafShardBlocksMerkleHasher::BASE_KEY,
        ChildIntermediateShardBlocksMerkleHasher::BASE_KEY,
        BeaconChainBodyMerkleHasher::KEY,
        IntermediateShardsSegmentsMerkleHasher::BASE_KEY,
        BeaconChainHeaderMerkleHasher::KEY,
        IntermediateShardHeaderMerkleHasher::KEY,
        LeafShardHeaderMerkleHasher::KEY,
        SuperSegmentMerkleHasher::BASE_KEY,
        StateOwnersSparseMerkleHasher::KEY,
        StateOwnerContractsSparseMerkleHasher::KEY,
        BlockMerkleMountainRangeHasher::KEY,
    ];

    let mut i = 0;
    while i < keys.len() {
        let mut j = i + 1;
        while j < keys.len() {
            assert!(keys[i] != keys[j], "Keys of Merkle Trees must be unique");
            j += 1;
        }
        i += 1;
    }
}

/// Number of bits in [`Address`], which is the number of levels of state sparse Merkle Trees
const ADDRESS_BITS: u8 = Address::SIZE as u8 * u8::BITS as u8;

/// Segment roots of own segments of a shard
pub type SegmentRootsMerkleTree = UnbalancedMerkleTree<SegmentRootsMerkleHasher>;
/// Own segments of a shard
pub type OwnSegmentsMerkleTree<'a> = BalancedMerkleTree<'a, 2, OwnSegmentsMerkleHasher>;
/// Own segments of a leaf shard
pub type ShardOwnSegmentsMerkleTree<'a> = BalancedMerkleTree<'a, 2, ShardOwnSegmentsMerkleHasher>;
/// Leaf shard block body
pub type LeafShardBodyMerkleTree<'a> = BalancedMerkleTree<'a, 2, LeafShardBodyMerkleHasher>;
/// Transactions of a leaf shard block
pub type TransactionsMerkleTree = UnbalancedMerkleTree<TransactionsMerkleHasher>;
/// Intermediate shard block body
pub type IntermediateShardBodyMerkleTree = UnbalancedMerkleTree<IntermediateShardBodyMerkleHasher>;
/// Own segments of child leaf shard blocks of an intermediate shard block
pub type LeafShardsSegmentsMerkleTree = UnbalancedMerkleTree<LeafShardsSegmentsMerkleHasher>;
/// Child leaf shard block roots of an intermediate shard block
pub type ChildLeafShardBlocksMerkleTree = UnbalancedMerkleTree<ChildLeafShardBlocksMerkleHasher>;
/// Child intermediate shard block roots of a beacon chain block
pub type ChildIntermediateShardBlocksMerkleTree =
    UnbalancedMerkleTree<ChildIntermediateShardBlocksMerkleHasher>;
/// Beacon chain block body
pub type BeaconChainBodyMerkleTree<'a> = BalancedMerkleTree<'a, 4, BeaconChainBodyMerkleHasher>;
/// Segments of child intermediate shard blocks of a beacon chain block
pub type IntermediateShardsSegmentsMerkleTree =
    UnbalancedMerkleTree<IntermediateShardsSegmentsMerkleHasher>;
/// Beacon chain block header fields, its root is the block root
pub type BeaconChainHeaderMerkleTree = UnbalancedMerkleTree<BeaconChainHeaderMerkleHasher>;
/// Intermediate shard block header fields, its root is the block root
pub type IntermediateShardHeaderMerkleTree =
    UnbalancedMerkleTree<IntermediateShardHeaderMerkleHasher>;
/// Leaf shard block header fields, its root is the block root
pub type LeafShardHeaderMerkleTree = UnbalancedMerkleTree<LeafShardHeaderMerkleHasher>;
/// Segment roots with their positions in a super segment
pub type SuperSegmentMerkleTree = UnbalancedMerkleTree<SuperSegmentMerkleHasher>;
/// Global state: state roots of owners by owner address
pub type StateOwnersSparseMerkleTree =
    SparseMerkleTree<ADDRESS_BITS, StateOwnersSparseMerkleHasher>;
/// State of one owner: hashes of contents of slots by the address of the managing contract
pub type StateOwnerContractsSparseMerkleTree =
    SparseMerkleTree<ADDRESS_BITS, StateOwnerContractsSparseMerkleHasher>;
/// Beacon chain block roots.
///
/// NOTE: `u32` is smaller than `BlockNumber`'s internal `u64` but will be sufficient for a long
/// time and substantially decrease the size of the in-memory data structure.
pub type BlockMerkleMountainRange =
    MerkleMountainRange<{ u64::from(u32::MAX) }, BlockMerkleMountainRangeHasher>;
