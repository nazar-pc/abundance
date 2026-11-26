// Hashing millions of leaves takes too long under Miri
#![cfg(not(miri))]

use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::segments::{HistorySize, SegmentIndex};
use ab_core_primitives::shard::NumShards;
use ab_core_primitives::solutions::{
    ShardCommitmentHash, ShardMembershipEntropy, SolutionShardCommitment,
};
use ab_farmer_components::shard_commitment::{
    derive_shard_commitments_root, derive_solution_shard_commitment,
};
use ab_io_type::trivial_type::TrivialType;
use ab_merkle_tree::blake3::{BalancedMerkleTree, UnbalancedMerkleTree};
use blake3::{Hasher, OutputReader};
use std::iter;
use std::num::{NonZeroU16, NonZeroU64};
use transparent_wrapper::TransparentWrapper;

const SHARD_COMMITMENTS_SEED: Blake3Hash = Blake3Hash::new([1; _]);

fn reference_stream(history_size: HistorySize) -> OutputReader {
    let mut hasher = Hasher::new_keyed(&SHARD_COMMITMENTS_SEED);
    hasher.update(SegmentIndex::from(history_size).as_bytes());
    hasher.finalize_xof()
}

#[test]
fn shard_commitments_match_xof_stream() {
    let history_size = HistorySize::new(NonZeroU64::new(3).unwrap());

    let expected_root = {
        let mut stream = reference_stream(history_size);
        let leaves = iter::repeat_with(|| {
            let mut leaf = [0; ShardCommitmentHash::SIZE];
            stream.fill(&mut leaf);
            leaf
        })
        .take(SolutionShardCommitment::NUM_LEAVES);

        ShardCommitmentHash::new(
            UnbalancedMerkleTree::compute_root_only_dynamic_len::<
                { SolutionShardCommitment::NUM_LEAVES as u64 },
                _,
                _,
            >(leaves)
            .unwrap(),
        )
    };

    let shard_commitments_root =
        derive_shard_commitments_root(&SHARD_COMMITMENTS_SEED, history_size);
    assert_eq!(shard_commitments_root, expected_root);

    let shard_membership_entropy = ShardMembershipEntropy::default();
    let num_shards = NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap();
    for public_key_hash in [[0; _], [1; _], [2; _]].map(Blake3Hash::new) {
        let shard_commitment = derive_solution_shard_commitment(
            &public_key_hash,
            &SHARD_COMMITMENTS_SEED,
            &shard_commitments_root,
            history_size,
            &shard_membership_entropy,
            num_shards,
        );

        let leaf_index = num_shards.derive_shard_commitment_index(
            &public_key_hash,
            &shard_commitments_root,
            &shard_membership_entropy,
            history_size,
        );
        let expected_leaf = {
            let mut stream = reference_stream(history_size);
            stream.set_position(u64::from(leaf_index) * ShardCommitmentHash::SIZE as u64);
            let mut leaf = [0; ShardCommitmentHash::SIZE];
            stream.fill(&mut leaf);
            ShardCommitmentHash::new(leaf)
        };

        assert_eq!(shard_commitment.root, shard_commitments_root);
        assert_eq!(shard_commitment.leaf, expected_leaf);
        assert!(
            BalancedMerkleTree::<{ SolutionShardCommitment::NUM_LEAVES }>::verify(
                &expected_root,
                shard_commitment.proof.peel_ref(),
                leaf_index as usize,
                *expected_leaf,
            ),
            "Proof for leaf {leaf_index} doesn't verify"
        );
    }
}
