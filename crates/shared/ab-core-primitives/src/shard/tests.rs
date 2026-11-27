use crate::hashes::Blake3Hash;
use crate::segments::HistorySize;
use crate::shard::{NumShards, ShardIndex, ShardKind};
use crate::solutions::{ShardCommitmentHash, ShardMembershipEntropy};
use core::num::NonZeroU16;

fn num_shards(intermediate_shards: u16, leaf_shards_per_intermediate_shard: u16) -> NumShards {
    NumShards::new(
        NonZeroU16::new(intermediate_shards).unwrap(),
        NonZeroU16::new(leaf_shards_per_intermediate_shard).unwrap(),
    )
    .unwrap()
}

fn leaf_shard_index(intermediate_shard: u32, position: u32) -> ShardIndex {
    ShardIndex::new(intermediate_shard | (position << 10)).unwrap()
}

#[test]
fn iter_leaf_shards() {
    assert_eq!(
        num_shards(1, 1).iter_leaf_shards().collect::<Vec<_>>(),
        [leaf_shard_index(1, 1)]
    );
    assert_eq!(
        num_shards(2, 3).iter_leaf_shards().collect::<Vec<_>>(),
        [
            leaf_shard_index(1, 1),
            leaf_shard_index(1, 2),
            leaf_shard_index(1, 3),
            leaf_shard_index(2, 1),
            leaf_shard_index(2, 2),
            leaf_shard_index(2, 3),
        ]
    );

    // The last leaf shards of the largest intermediate shard and the largest number of leaf shards
    for (num_shards, last_leaf_shard_index) in [
        (num_shards(1023, 1), leaf_shard_index(1023, 1)),
        (num_shards(1, 1023), leaf_shard_index(1, 1023)),
    ] {
        let leaf_shards = num_shards.iter_leaf_shards().collect::<Vec<_>>();
        assert_eq!(
            leaf_shards.len(),
            usize::try_from(num_shards.leaf_shards().get()).unwrap()
        );
        assert!(
            leaf_shards
                .iter()
                .all(|shard_index| shard_index.shard_kind() == Some(ShardKind::LeafShard))
        );
        assert_eq!(leaf_shards.last(), Some(&last_leaf_shard_index));
    }
}

#[test]
fn derived_shard_indices_are_leaf_shards() {
    // With a single leaf shard it is always that shard
    assert_eq!(
        num_shards(1, 1).derive_shard_index(
            &Blake3Hash::default(),
            &ShardCommitmentHash::default(),
            &ShardMembershipEntropy::default(),
            HistorySize::ONE,
        ),
        leaf_shard_index(1, 1)
    );

    let num_shards = num_shards(2, 3);

    for public_key_hash in 0..16 {
        let public_key_hash = Blake3Hash::new([public_key_hash; _]);
        let shard_index = num_shards.derive_shard_index(
            &public_key_hash,
            &ShardCommitmentHash::default(),
            &ShardMembershipEntropy::default(),
            HistorySize::ONE,
        );
        assert_eq!(
            shard_index.shard_kind(),
            Some(ShardKind::LeafShard),
            "{shard_index:?}"
        );
        let (combined_shard_index, _shard_commitment_index) = num_shards
            .derive_shard_index_and_shard_commitment_index(
                &public_key_hash,
                &ShardCommitmentHash::default(),
                &ShardMembershipEntropy::default(),
                HistorySize::ONE,
            );
        assert_eq!(combined_shard_index, shard_index);
    }
}
