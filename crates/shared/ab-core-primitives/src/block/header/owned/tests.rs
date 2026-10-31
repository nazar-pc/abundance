use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedIntermediateShardHeader,
    OwnedLeafShardHeader,
};
use crate::block::header::{
    BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters,
    BlockHeaderEd25519Seal, BlockHeaderFixedConsensusParameters, BlockHeaderPotParametersChange,
    BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
};
use crate::block::{BlockNumber, BlockRoot, BlockTimestamp};
use crate::ed25519::{Ed25519PublicKey, Ed25519Signature};
use crate::hashes::Blake3Hash;
use crate::pot::{PotOutput, PotParametersChange, SlotNumber};
use crate::segments::SuperSegmentRoot;
use crate::shard::{NumShards, NumShardsUnchecked, ShardIndex};
use crate::solutions::{Solution, SolutionRange};
use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_io_type::trivial_type::TrivialType;
use std::num::{NonZeroU16, NonZeroU32};

fn prefix(shard_index: u32) -> BlockHeaderPrefix {
    BlockHeaderPrefix {
        number: BlockNumber::ZERO,
        shard_index: ShardIndex::new(shard_index).unwrap(),
        padding_0: [0; _],
        timestamp: BlockTimestamp::default(),
        parent_root: BlockRoot::default(),
        mmr_root: Blake3Hash::default(),
    }
}

fn result() -> BlockHeaderResult {
    BlockHeaderResult {
        body_root: Blake3Hash::default(),
        state_root: Blake3Hash::default(),
    }
}

fn consensus_info() -> BlockHeaderConsensusInfo {
    BlockHeaderConsensusInfo {
        slot: SlotNumber::ZERO,
        proof_of_time: PotOutput::default(),
        future_proof_of_time: PotOutput::default(),
        // Its public key hash is that of the seal below
        solution: Solution::genesis_solution(),
    }
}

fn beacon_chain_info() -> BlockHeaderBeaconChainInfo {
    BlockHeaderBeaconChainInfo {
        number: BlockNumber::ZERO,
        root: BlockRoot::default(),
    }
}

fn seal() -> BlockHeaderEd25519Seal {
    BlockHeaderEd25519Seal {
        public_key: Ed25519PublicKey::default(),
        signature: Ed25519Signature::default(),
    }
}

/// The buffer of an owned header, which keeps the capacity it was allocated with unless it had to
/// grow
fn into_buffer<Header>(header: Header) -> OwnedAlignedBuffer
where
    Header: GenericOwnedBlockHeader,
{
    let buffer = header.buffer().clone();
    // The buffer is not shared anymore after this, so it is reused rather than copied
    drop(header);
    buffer.into_owned()
}

#[test]
fn max_allocation_is_enough_for_largest_headers() {
    let super_segment_root = SuperSegmentRoot::from([1; _]);
    let pot_parameters_change = BlockHeaderPotParametersChange::from(PotParametersChange {
        slot: SlotNumber::ZERO,
        slot_iterations: NonZeroU32::MIN,
        entropy: Blake3Hash::default(),
    });
    let fixed_parameters = BlockHeaderFixedConsensusParameters {
        solution_range: SolutionRange::MAX,
        slot_iterations: NonZeroU32::MIN,
        num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
    };
    // With all optional consensus parameters the header is as large as it can be, without them it
    // is smaller
    for (consensus_parameters, largest) in [
        (
            BlockHeaderConsensusParameters {
                fixed_parameters,
                super_segment_root: Some(&super_segment_root),
                next_solution_range: Some(SolutionRange::MAX),
                pot_parameters_change: Some(&pot_parameters_change),
            },
            true,
        ),
        (
            BlockHeaderConsensusParameters {
                fixed_parameters,
                super_segment_root: None,
                next_solution_range: None,
                pot_parameters_change: None,
            },
            false,
        ),
    ] {
        let header = OwnedBeaconChainHeader::from_parts(
            &prefix(0),
            &result(),
            &consensus_info(),
            &Blake3Hash::default(),
            &consensus_parameters,
        )
        .with_seal(BlockHeaderSeal::Ed25519(&seal()));
        let buffer = into_buffer(header);
        assert_eq!(buffer.capacity(), OwnedBeaconChainHeader::MAX_ALLOCATION);
        assert_eq!(buffer.len() == buffer.capacity(), largest);
    }
    assert_eq!(OwnedBeaconChainHeader::MAX_ALLOCATION, 2862);

    // Intermediate shard headers have a fixed size
    let header = OwnedIntermediateShardHeader::from_parts(
        &prefix(1),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
        &Blake3Hash::default(),
    )
    .with_seal(BlockHeaderSeal::Ed25519(&seal()));
    let buffer = into_buffer(header);
    assert_eq!(
        buffer.capacity(),
        OwnedIntermediateShardHeader::MAX_ALLOCATION
    );
    assert_eq!(buffer.len(), buffer.capacity());
    assert_eq!(OwnedIntermediateShardHeader::MAX_ALLOCATION, 2801);

    let header = OwnedLeafShardHeader::from_parts(
        &prefix(1 | (1 << 10)),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
    )
    .with_seal(BlockHeaderSeal::Ed25519(&seal()));
    let buffer = into_buffer(header);
    assert_eq!(buffer.capacity(), OwnedLeafShardHeader::MAX_ALLOCATION);
    assert_eq!(buffer.len(), buffer.capacity());
}

#[test]
fn from_parts_into_writes_headers_that_seal_like_owned_ones() {
    let child_shard_blocks_root = Blake3Hash::new([1; _]);
    let consensus_parameters = BlockHeaderConsensusParameters {
        fixed_parameters: BlockHeaderFixedConsensusParameters {
            solution_range: SolutionRange::MAX,
            slot_iterations: NonZeroU32::MIN,
            num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
        },
        super_segment_root: None,
        next_solution_range: None,
        pot_parameters_change: None,
    };
    let seal = seal();
    let seal = BlockHeaderSeal::Ed25519(&seal);
    // Headers are written after these bytes
    let bytes_before = [1, 2, 3];

    let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
    let unsealed = OwnedBeaconChainHeader::from_parts_into(
        &prefix(0),
        &result(),
        &consensus_info(),
        &child_shard_blocks_root,
        &consensus_parameters,
        &mut buffer,
    )
    .unwrap();
    let owned_unsealed = OwnedBeaconChainHeader::from_parts(
        &prefix(0),
        &result(),
        &consensus_info(),
        &child_shard_blocks_root,
        &consensus_parameters,
    );
    assert_eq!(unsealed.pre_seal_hash(), owned_unsealed.pre_seal_hash());
    unsealed.with_seal(seal);
    let header = owned_unsealed.with_seal(seal);
    assert_eq!(buffer.as_slice()[..3], bytes_before);
    assert_eq!(buffer.as_slice()[3..], *header.buffer().as_slice());

    let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
    let unsealed = OwnedIntermediateShardHeader::from_parts_into(
        &prefix(1),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
        &child_shard_blocks_root,
        &mut buffer,
    )
    .unwrap();
    let owned_unsealed = OwnedIntermediateShardHeader::from_parts(
        &prefix(1),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
        &child_shard_blocks_root,
    );
    assert_eq!(unsealed.pre_seal_hash(), owned_unsealed.pre_seal_hash());
    unsealed.with_seal(seal);
    let header = owned_unsealed.with_seal(seal);
    assert_eq!(buffer.as_slice()[..3], bytes_before);
    assert_eq!(buffer.as_slice()[3..], *header.buffer().as_slice());

    let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
    let unsealed = OwnedLeafShardHeader::from_parts_into(
        &prefix(1 | (1 << 10)),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
        &mut buffer,
    )
    .unwrap();
    let owned_unsealed = OwnedLeafShardHeader::from_parts(
        &prefix(1 | (1 << 10)),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
    );
    assert_eq!(unsealed.pre_seal_hash(), owned_unsealed.pre_seal_hash());
    unsealed.with_seal(seal);
    let header = owned_unsealed.with_seal(seal);
    assert_eq!(buffer.as_slice()[..3], bytes_before);
    assert_eq!(buffer.as_slice()[3..], *header.buffer().as_slice());
}

#[test]
fn header_roots_commit_to_child_block_roots() {
    let consensus_parameters = BlockHeaderConsensusParameters {
        fixed_parameters: BlockHeaderFixedConsensusParameters {
            solution_range: SolutionRange::MAX,
            slot_iterations: NonZeroU32::MIN,
            num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
        },
        super_segment_root: None,
        next_solution_range: None,
        pot_parameters_change: None,
    };
    let seal = seal();
    let seal = BlockHeaderSeal::Ed25519(&seal);
    // Headers that only differ in the root of child block roots
    let child_shard_blocks_roots = [Blake3Hash::default(), Blake3Hash::new([1; _])];

    let [header_a, header_b] = child_shard_blocks_roots.map(|intermediate_shard_blocks_root| {
        OwnedBeaconChainHeader::from_parts(
            &prefix(0),
            &result(),
            &consensus_info(),
            &intermediate_shard_blocks_root,
            &consensus_parameters,
        )
        .with_seal(seal)
    });
    for (header, root) in [&header_a, &header_b]
        .into_iter()
        .zip(&child_shard_blocks_roots)
    {
        assert_eq!(header.header().intermediate_shard_blocks_root(), root);
    }
    assert_ne!(*header_a.header().root(), *header_b.header().root());
    assert_ne!(
        header_a.header().pre_seal_hash(),
        header_b.header().pre_seal_hash()
    );

    let [header_a, header_b] = child_shard_blocks_roots.map(|leaf_shard_blocks_root| {
        OwnedIntermediateShardHeader::from_parts(
            &prefix(1),
            &result(),
            &consensus_info(),
            &beacon_chain_info(),
            &leaf_shard_blocks_root,
        )
        .with_seal(seal)
    });
    for (header, root) in [&header_a, &header_b]
        .into_iter()
        .zip(&child_shard_blocks_roots)
    {
        assert_eq!(header.header().leaf_shard_blocks_root(), root);
    }
    assert_ne!(*header_a.header().root(), *header_b.header().root());
    assert_ne!(
        header_a.header().pre_seal_hash(),
        header_b.header().pre_seal_hash()
    );
}

#[test]
fn fixed_consensus_parameters_from_unaligned_bytes() {
    let fixed_parameters = BlockHeaderFixedConsensusParameters {
        solution_range: SolutionRange::MAX,
        slot_iterations: NonZeroU32::MIN,
        num_shards: NumShards::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(3).unwrap())
            .unwrap(),
    };
    // Written like in headers, but after one byte of an aligned buffer, which puts the number of
    // shards at an odd address
    let bytes = [
        &[0][..],
        &fixed_parameters.solution_range.to_bytes(),
        &fixed_parameters.slot_iterations.get().to_le_bytes(),
        NumShardsUnchecked::from(fixed_parameters.num_shards).as_bytes(),
        &[1],
    ]
    .concat();
    let buffer = OwnedAlignedBuffer::from_bytes(&bytes);

    assert_eq!(
        BlockHeaderFixedConsensusParameters::try_from_bytes(&buffer.as_slice()[1..]),
        Some((fixed_parameters, &[1][..]))
    );
}
