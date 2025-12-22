//! Utilities related to shard commitments

use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::segments::HistorySize;
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::{
    ShardCommitmentHash, ShardMembershipEntropy, SolutionShardCommitment,
};
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use blake3::Hasher;
use parking_lot::RwLock;
use schnellru::{ByLength, LruMap};
use std::iter;
use std::mem::MaybeUninit;
use std::sync::Arc;

const SHARD_COMMITMENTS_ROOT_CACHE_SIZE: ByLength = ByLength::new(32);

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

/// Derive shard commitments root from the seed and history size
pub fn derive_shard_commitments_root(
    shard_commitments_seed: &Blake3Hash,
    history_size: HistorySize,
) -> ShardCommitmentHash {
    let mut stream = {
        let mut hasher = Hasher::new_keyed(shard_commitments_seed);
        hasher.update(&history_size.as_u64().to_le_bytes());
        hasher.finalize_xof()
    };

    let mut index = 0;
    let leaves = iter::from_fn(|| {
        if index < SolutionShardCommitment::NUM_LEAVES {
            let mut bytes = [0; ShardCommitmentHash::SIZE];
            stream.fill(&mut bytes);

            index += 1;

            Some(bytes)
        } else {
            None
        }
    });

    // NOTE: Using unbalanced implementation since balanced implementation requires allocation of
    // leaves
    const NUM_LEAVES_U64: u64 = SolutionShardCommitment::NUM_LEAVES as u64;
    let root = UnbalancedMerkleTree::compute_root_only::<NUM_LEAVES_U64, _, _>(leaves)
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
    let mut stream = {
        let mut hasher = Hasher::new_keyed(shard_commitments_seed);
        hasher.update(&history_size.as_u64().to_le_bytes());
        hasher.finalize_xof()
    };

    let leaf_index = num_shards.derive_shard_commitment_index(
        public_key_hash,
        shard_commitments_root,
        shard_membership_entropy,
        history_size,
    ) as usize;

    let mut leaf = [0; _];
    let mut index = 0;
    let leaves = iter::from_fn(|| {
        if index < SolutionShardCommitment::NUM_LEAVES {
            let mut bytes = [0; ShardCommitmentHash::SIZE];
            stream.fill(&mut bytes);

            if index == leaf_index {
                leaf = bytes;
            }

            index += 1;

            Some(bytes)
        } else {
            None
        }
    });

    const NUM_LEAVES_U64: u64 = SolutionShardCommitment::NUM_LEAVES as u64;
    let mut proof = [MaybeUninit::uninit(); _];
    // NOTE: Using unbalanced implementation since balanced implementation requires an allocation
    // and uses a lot more RAM
    let (_root, computed_proof) =
        UnbalancedMerkleTree::compute_root_and_proof_in::<NUM_LEAVES_U64, _, _>(
            leaves, leaf_index, &mut proof,
        )
        .expect("Index is always within the list of leaves; qed");
    debug_assert_eq!(computed_proof.len(), proof.len());

    // SAFETY: Checked above that it is fully initialized
    let proof = unsafe { MaybeUninit::array_assume_init(proof) };

    SolutionShardCommitment {
        root: *shard_commitments_root,
        proof: ShardCommitmentHash::array_from_repr(proof),
        leaf: ShardCommitmentHash::new(leaf),
    }
}
