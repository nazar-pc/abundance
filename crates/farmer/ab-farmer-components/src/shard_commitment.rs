//! Utilities related to shard commitments

use ab_blake3::{BLOCK_LEN, MAX_SIMD_DEGREE};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::segments::{HistorySize, SegmentIndex};
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::{
    ShardCommitmentHash, ShardMembershipEntropy, SolutionShardCommitment,
};
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::blake3::UnbalancedMerkleTree;
use blake3::Hasher;
use parking_lot::RwLock;
use schnellru::{ByLength, LruMap};
use std::mem::MaybeUninit;
use transparent_wrapper::TransparentWrapper;
use triomphe::Arc;

const SHARD_COMMITMENTS_ROOT_CACHE_SIZE: ByLength = ByLength::new(32);

/// Number of shard commitment leaves read from the XOF stream at once.
///
/// BLAKE3 computes up to `MAX_SIMD_DEGREE` whole output blocks of the XOF stream in parallel with
/// SIMD, but recomputes a whole block for every read shorter than a block. Reading this many leaves
/// at once keeps SIMD saturated with a small buffer on the stack instead of all leaves on the heap.
const LEAVES_PER_XOF_READ: usize = MAX_SIMD_DEGREE * BLOCK_LEN / ShardCommitmentHash::SIZE;

const _: () = {
    assert!((LEAVES_PER_XOF_READ * ShardCommitmentHash::SIZE).is_multiple_of(BLOCK_LEN));
    assert!(SolutionShardCommitment::NUM_LEAVES.is_multiple_of(LEAVES_PER_XOF_READ));
};

#[derive(Debug)]
struct Inner {
    shard_commitments_seed: Blake3Hash,
    lru: LruMap<HistorySize, ShardCommitmentHash, ByLength>,
}

/// Cache for shard commitments roots to avoid recomputing them repeatedly
#[derive(Debug, Clone)]
pub struct ShardCommitmentsRootsCache {
    inner: Arc<RwLock<Inner>>,
}

impl ShardCommitmentsRootsCache {
    /// Create a new instance
    pub fn new(shard_commitments_seed: Blake3Hash) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                shard_commitments_seed,
                lru: LruMap::new(SHARD_COMMITMENTS_ROOT_CACHE_SIZE),
            })),
        }
    }

    /// Seed used during instantiation
    pub fn shard_commitments_seed(&self) -> Blake3Hash {
        self.inner.read().shard_commitments_seed
    }

    /// Get root for a specified history size.
    ///
    /// Root will be recomputed unless already known.
    pub fn get(&self, history_size: HistorySize) -> ShardCommitmentHash {
        if let Some(root) = self.inner.read().lru.peek(&history_size).copied() {
            return root;
        }

        let inner = &mut *self.inner.write();
        // NOTE: See https://github.com/koute/schnellru/issues/7 for an explanation of the
        // `Option` return type
        *inner
            .lru
            .get_or_insert(history_size, || {
                derive_shard_commitments_root(&inner.shard_commitments_seed, history_size)
            })
            .expect("Not limited by memory; qed")
    }
}

/// Shard commitment leaves derived from the seed and history size.
///
/// Yields exactly [`SolutionShardCommitment::NUM_LEAVES`] leaves, which are consecutive chunks of
/// the XOF stream.
#[inline(always)]
fn shard_commitment_leaves(
    shard_commitments_seed: &Blake3Hash,
    history_size: HistorySize,
) -> impl Iterator<Item = [u8; ShardCommitmentHash::SIZE]> {
    let mut stream = {
        let mut hasher = Hasher::new_keyed(shard_commitments_seed);
        hasher.update(SegmentIndex::from(history_size).as_bytes());
        hasher.finalize_xof()
    };

    gen move {
        for _ in 0..SolutionShardCommitment::NUM_LEAVES / LEAVES_PER_XOF_READ {
            let mut leaves = [[0; ShardCommitmentHash::SIZE]; LEAVES_PER_XOF_READ];
            stream.fill(leaves.as_flattened_mut());
            for leaf in leaves {
                yield leaf;
            }
        }
    }
}

/// Derive shard commitments root from the seed and history size
pub fn derive_shard_commitments_root(
    shard_commitments_seed: &Blake3Hash,
    history_size: HistorySize,
) -> ShardCommitmentHash {
    let leaves = shard_commitment_leaves(shard_commitments_seed, history_size);

    // NOTE: Using unbalanced implementation since balanced implementation requires allocation of
    // leaves
    let root = UnbalancedMerkleTree::compute_root_only_dynamic_len::<
        { SolutionShardCommitment::NUM_LEAVES as u64 },
        _,
        _,
    >(leaves)
    .expect("List of leaves is not empty; qed");

    ShardCommitmentHash::new(root)
}

/// Derive solution shard commitment
pub fn derive_solution_shard_commitment(
    public_key_hash: &Blake3Hash,
    shard_commitments_seed: &Blake3Hash,
    shard_commitments_root: &ShardCommitmentHash,
    history_size: HistorySize,
    shard_membership_entropy: &ShardMembershipEntropy,
    num_shards: NumShards,
) -> SolutionShardCommitment {
    let leaf_index = num_shards.derive_shard_commitment_index(
        public_key_hash,
        shard_commitments_root,
        shard_membership_entropy,
        history_size,
    ) as usize;

    let mut leaf = [0; _];
    let leaves = shard_commitment_leaves(shard_commitments_seed, history_size)
        .enumerate()
        .map(|(index, bytes)| {
            if index == leaf_index {
                leaf = bytes;
            }

            bytes
        });

    let mut proof = [MaybeUninit::uninit(); _];
    // NOTE: Using unbalanced implementation since balanced implementation requires an allocation
    // and uses a lot more RAM
    let (_root, computed_proof) = UnbalancedMerkleTree::compute_root_and_proof_dynamic_len_in::<
        { SolutionShardCommitment::NUM_LEAVES as u64 },
        _,
        _,
    >(leaves, leaf_index, &mut proof)
    .expect("Index is always within the list of leaves; qed");
    debug_assert_eq!(computed_proof.len(), proof.len());

    // SAFETY: The iterator yields exactly `NUM_LEAVES` leaves, which is a power of two, so the tree
    // is full and the proof of any leaf has exactly `NUM_LEAVES.ilog2()` elements, which is the
    // length of the array
    let proof = unsafe { MaybeUninit::array_assume_init(proof) };

    SolutionShardCommitment {
        root: *shard_commitments_root,
        proof: <[ShardCommitmentHash; _]>::wrap(proof),
        leaf: ShardCommitmentHash::new(leaf),
    }
}
