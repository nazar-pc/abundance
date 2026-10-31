//! Merkle Trees used by the protocol.
//!
//! Every tree hashes its inner nodes with its own key, so a node of one tree can't be interpreted
//! as a node of another tree. Trees with a known number of leaves also commit to it.

mod alias;
mod hashers;
#[cfg(test)]
mod tests;

use crate::address::Address;
use ab_blake3::{BLOCK_LEN, OUT_LEN};
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::MerkleHasher;
use ab_merkle_tree::balanced::BalancedMerkleTree;
use ab_merkle_tree::blake3::Blake3MerkleHasher;
use ab_merkle_tree::mmr::MerkleMountainRange;
use ab_merkle_tree::sparse::SparseMerkleTree;
use alias::UnbalancedMerkleTreeAlias;
use array_reshape::FlattenEach;
use hashers::{Blake3SizedMerkleHasher, Blake3StaticMerkleHasher};

type SegmentRootsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"segment-roots-merkle-tree") }>;
type OwnSegmentsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"own-segments-merkle-tree") }>;
type ShardOwnSegmentsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"shard-own-segments-merkle-tree") }>;
type LeafShardBodyMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"leaf-shard-body-merkle-tree") }>;
type TransactionsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"transactions-merkle-tree") }>;
type IntermediateShardBodyMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"intermediate-shard-body-merkle-tree") }>;
type IntermediateShardBodySegmentsMerkleHasher = Blake3SizedMerkleHasher<
    { ab_blake3::const_hash(b"intermediate-shard-body-segments-merkle-tree") },
>;
type LeafShardsSegmentsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"leaf-shards-segments-merkle-tree") }>;
type ChildLeafShardBlocksMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"child-leaf-shard-blocks-merkle-tree") }>;
type ChildIntermediateShardBlocksMerkleHasher = Blake3SizedMerkleHasher<
    { ab_blake3::const_hash(b"child-intermediate-shard-blocks-merkle-tree") },
>;
type BeaconChainBodyMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"beacon-chain-body-merkle-tree") }>;
type IntermediateShardsSegmentsMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"intermediate-shards-segments-merkle-tree") }>;
type IntermediateShardBlockSegmentsMerkleHasher = Blake3SizedMerkleHasher<
    { ab_blake3::const_hash(b"intermediate-shard-block-segments-merkle-tree") },
>;
type BeaconChainHeaderMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"beacon-chain-header-merkle-tree") }>;
type IntermediateShardHeaderMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"intermediate-shard-header-merkle-tree") }>;
type LeafShardHeaderMerkleHasher =
    Blake3SizedMerkleHasher<{ ab_blake3::const_hash(b"leaf-shard-header-merkle-tree") }>;
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
        OwnSegmentsMerkleHasher::BASE_KEY,
        ShardOwnSegmentsMerkleHasher::BASE_KEY,
        LeafShardBodyMerkleHasher::BASE_KEY,
        TransactionsMerkleHasher::BASE_KEY,
        IntermediateShardBodyMerkleHasher::BASE_KEY,
        IntermediateShardBodySegmentsMerkleHasher::BASE_KEY,
        LeafShardsSegmentsMerkleHasher::BASE_KEY,
        ChildLeafShardBlocksMerkleHasher::BASE_KEY,
        ChildIntermediateShardBlocksMerkleHasher::BASE_KEY,
        BeaconChainBodyMerkleHasher::BASE_KEY,
        IntermediateShardsSegmentsMerkleHasher::BASE_KEY,
        IntermediateShardBlockSegmentsMerkleHasher::BASE_KEY,
        BeaconChainHeaderMerkleHasher::BASE_KEY,
        IntermediateShardHeaderMerkleHasher::BASE_KEY,
        LeafShardHeaderMerkleHasher::BASE_KEY,
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
pub type SegmentRootsMerkleTree = <SegmentRootsMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Own segments: the first local segment index and the root of segment roots
pub type OwnSegmentsMerkleTree<'a> = BalancedMerkleTree<'a, 2, OwnSegmentsMerkleHasher>;
/// Own segments of a leaf shard: the shard index with the first local segment index and the root
/// of segment roots
pub type ShardOwnSegmentsMerkleTree<'a> = BalancedMerkleTree<'a, 2, ShardOwnSegmentsMerkleHasher>;
/// Leaf shard block body: own segments and transactions
pub type LeafShardBodyMerkleTree<'a> = BalancedMerkleTree<'a, 2, LeafShardBodyMerkleHasher>;
/// Transactions of a leaf shard block
pub type TransactionsMerkleTree = <TransactionsMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Intermediate shard block body: segments and child leaf shard blocks
pub type IntermediateShardBodyMerkleTree<'a> =
    BalancedMerkleTree<'a, 2, IntermediateShardBodyMerkleHasher>;
/// Segments of an intermediate shard block body: own segments and segments of leaf shards
pub type IntermediateShardBodySegmentsMerkleTree<'a> =
    BalancedMerkleTree<'a, 2, IntermediateShardBodySegmentsMerkleHasher>;
/// Own segments of child leaf shard blocks of an intermediate shard block
pub type LeafShardsSegmentsMerkleTree =
    <LeafShardsSegmentsMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Child leaf shard block roots of an intermediate shard block
pub type ChildLeafShardBlocksMerkleTree =
    <ChildLeafShardBlocksMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Child intermediate shard block roots of a beacon chain block
pub type ChildIntermediateShardBlocksMerkleTree =
    <ChildIntermediateShardBlocksMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Beacon chain block body: own segments, segments of intermediate shards, child intermediate
/// shard blocks and PoT checkpoints
pub type BeaconChainBodyMerkleTree<'a> = BalancedMerkleTree<'a, 4, BeaconChainBodyMerkleHasher>;
/// Segments of child intermediate shard blocks of a beacon chain block
pub type IntermediateShardsSegmentsMerkleTree =
    <IntermediateShardsSegmentsMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Segments of one intermediate shard block: its own segments and those of its leaf shards
pub type IntermediateShardBlockSegmentsMerkleTree =
    <IntermediateShardBlockSegmentsMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Beacon chain block header fields, its root is the block root
pub type BeaconChainHeaderMerkleTree =
    <BeaconChainHeaderMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Intermediate shard block header fields, its root is the block root
pub type IntermediateShardHeaderMerkleTree =
    <IntermediateShardHeaderMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Leaf shard block header fields, its root is the block root
pub type LeafShardHeaderMerkleTree =
    <LeafShardHeaderMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
/// Segment roots with their positions in a super segment
pub type SuperSegmentMerkleTree = <SuperSegmentMerkleHasher as UnbalancedMerkleTreeAlias>::Tree;
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

// `MerkleHasher` is implemented for each hasher separately, a generic implementation would need
// `<Self as MerkleHasher>::OUT_LEN` with generic `Self` in method signatures, which requires
// `macroless_generic_const_args` that this crate doesn't use
// TODO: Replace the macro with generic implementations once `macroless_generic_const_args` is
//  enabled in this crate:
// // `<Self as MerkleHasher>::OUT_LEN` doesn't unify with `OUT_LEN` in generic code, hence
// // reshaping
// impl<const KEY: [u8; KEY_LEN]> MerkleHasher for Blake3StaticMerkleHasher<KEY> {
//     const OUT_LEN: usize = OUT_LEN;
//
//     #[inline(always)]
//     fn hash_pair(
//         &self,
//         pair: &[[u8; <Self as MerkleHasher>::OUT_LEN]; 2],
//     ) -> [u8; <Self as MerkleHasher>::OUT_LEN] {
//         let hash =
//             ab_blake3::single_block_keyed_hash(self.node_key(), pair.flatten_ref::<BLOCK_LEN>());
//         // Reshaping inside `Option` results in fewer copies of the hash than after `expect()`
//         hash.map(|hash| [hash].flatten())
//             .expect("Exactly one block worth of data; qed")
//     }
//
//     #[inline(always)]
//     fn hash_pairs<const NUM_PAIRS: usize>(
//         &self,
//         pairs: &[[[u8; <Self as MerkleHasher>::OUT_LEN]; 2]; NUM_PAIRS],
//     ) -> [[u8; <Self as MerkleHasher>::OUT_LEN]; NUM_PAIRS] {
//         let pairs = pairs
//             .as_slice()
//             .flatten_each_ref::<BLOCK_LEN>()
//             .as_array::<NUM_PAIRS>()
//             .expect("Same number of pairs as in the input; qed");
//         let mut hashes = [[0; <Self as MerkleHasher>::OUT_LEN]; NUM_PAIRS];
//         ab_blake3::single_block_keyed_hash_many_exact(
//             self.node_key(),
//             pairs,
//             hashes.rechunk_mut(),
//         );
//         hashes
//     }
//
//     #[inline(always)]
//     fn hash_single_leaf(
//         &self,
//         leaf: &[u8; <Self as MerkleHasher>::OUT_LEN],
//     ) -> [u8; <Self as MerkleHasher>::OUT_LEN] {
//         [self.single_leaf_root(array::from_ref(leaf).flatten_ref::<OUT_LEN>())].flatten()
//     }
// }
//
// // Same body as above
// impl<const BASE_KEY: [u8; KEY_LEN]> MerkleHasher for Blake3SizedMerkleHasher<BASE_KEY> { ... }
macro_rules! impl_merkle_hasher {
    ($($hasher:ty),+ $(,)?) => {
        $(
            impl MerkleHasher for $hasher {
                const OUT_LEN: usize = OUT_LEN;

                #[inline(always)]
                fn hash_pair(&self, pair: &[[u8; OUT_LEN]; 2]) -> [u8; OUT_LEN] {
                    ab_blake3::single_block_keyed_hash(self.node_key(), pair.as_flattened())
                        .expect("Exactly one block worth of data; qed")
                }

                #[inline(always)]
                fn hash_pairs<const NUM_PAIRS: usize>(
                    &self,
                    pairs: &[[[u8; OUT_LEN]; 2]; NUM_PAIRS],
                ) -> [[u8; OUT_LEN]; NUM_PAIRS] {
                    let pairs = pairs
                        .as_slice()
                        .flatten_each_ref::<BLOCK_LEN>()
                        .as_array::<NUM_PAIRS>()
                        .expect("Same number of pairs as in the input; qed");
                    let mut hashes = [[0; OUT_LEN]; NUM_PAIRS];
                    ab_blake3::single_block_keyed_hash_many_exact(
                        self.node_key(),
                        pairs,
                        &mut hashes,
                    );
                    hashes
                }

                #[inline(always)]
                fn hash_single_leaf(&self, leaf: &[u8; OUT_LEN]) -> [u8; OUT_LEN] {
                    self.single_leaf_root(leaf)
                }
            }
        )+
    };
}

impl_merkle_hasher!(
    SegmentRootsMerkleHasher,
    OwnSegmentsMerkleHasher,
    ShardOwnSegmentsMerkleHasher,
    LeafShardBodyMerkleHasher,
    TransactionsMerkleHasher,
    IntermediateShardBodyMerkleHasher,
    IntermediateShardBodySegmentsMerkleHasher,
    LeafShardsSegmentsMerkleHasher,
    ChildLeafShardBlocksMerkleHasher,
    ChildIntermediateShardBlocksMerkleHasher,
    BeaconChainBodyMerkleHasher,
    IntermediateShardsSegmentsMerkleHasher,
    IntermediateShardBlockSegmentsMerkleHasher,
    BeaconChainHeaderMerkleHasher,
    IntermediateShardHeaderMerkleHasher,
    LeafShardHeaderMerkleHasher,
    SuperSegmentMerkleHasher,
    StateOwnersSparseMerkleHasher,
    StateOwnerContractsSparseMerkleHasher,
    BlockMerkleMountainRangeHasher,
);
