use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
    OwnedIntermediateShardHeader, OwnedIntermediateShardHeaderError, OwnedLeafShardHeader,
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
use crate::shard::{NumShards, ShardIndex};
use crate::solutions::{Solution, SolutionRange};
use ab_aligned_buffer::OwnedAlignedBuffer;
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
    // As many child shard blocks as the headers can have
    let child_shard_blocks = vec![BlockRoot::default(); usize::from(u16::MAX)];
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
            &child_shard_blocks,
            &consensus_parameters,
        )
        .unwrap()
        .with_seal(BlockHeaderSeal::Ed25519(&seal()));
        let buffer = into_buffer(header);
        assert_eq!(
            buffer.capacity(),
            OwnedBeaconChainHeader::max_allocation_for(&child_shard_blocks)
        );
        assert_eq!(buffer.len() == buffer.capacity(), largest);
    }

    let header = OwnedIntermediateShardHeader::from_parts(
        &prefix(1),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
        &child_shard_blocks,
    )
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()));
    let buffer = into_buffer(header);
    assert_eq!(
        buffer.capacity(),
        OwnedIntermediateShardHeader::max_allocation_for(&child_shard_blocks)
    );
    assert_eq!(buffer.len(), buffer.capacity());

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
    let child_shard_blocks = [BlockRoot::default(); 2];
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
        &child_shard_blocks,
        &consensus_parameters,
        &mut buffer,
    )
    .unwrap();
    let owned_unsealed = OwnedBeaconChainHeader::from_parts(
        &prefix(0),
        &result(),
        &consensus_info(),
        &child_shard_blocks,
        &consensus_parameters,
    )
    .unwrap();
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
        &child_shard_blocks,
        &mut buffer,
    )
    .unwrap();
    let owned_unsealed = OwnedIntermediateShardHeader::from_parts(
        &prefix(1),
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
        &child_shard_blocks,
    )
    .unwrap();
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
fn from_parts_into_writes_nothing_on_errors() {
    // One more child shard block than headers can have. A buffer without space for a header
    // would need 4 GiB of initialized memory, so the other error is not checked here.
    let child_shard_blocks = vec![BlockRoot::default(); usize::from(u16::MAX) + 1];
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
    let bytes_before = [1, 2, 3];

    let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
    assert!(matches!(
        OwnedBeaconChainHeader::from_parts_into(
            &prefix(0),
            &result(),
            &consensus_info(),
            &child_shard_blocks,
            &consensus_parameters,
            &mut buffer,
        ),
        Err(OwnedBeaconChainHeaderError::TooManyChildShardBlocks { actual })
            if actual == child_shard_blocks.len()
    ));
    assert_eq!(buffer.as_slice(), bytes_before);

    let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
    assert!(matches!(
        OwnedIntermediateShardHeader::from_parts_into(
            &prefix(1),
            &result(),
            &consensus_info(),
            &beacon_chain_info(),
            &child_shard_blocks,
            &mut buffer,
        ),
        Err(OwnedIntermediateShardHeaderError::TooManyChildShardBlocks { actual })
            if actual == child_shard_blocks.len()
    ));
    assert_eq!(buffer.as_slice(), bytes_before);
}
