use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedBeaconChainHeaderError,
    OwnedIntermediateShardHeader, OwnedIntermediateShardHeaderError, OwnedLeafShardHeader,
    OwnedLeafShardHeaderError,
};
use crate::block::header::{
    BeaconChainHeader, BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo,
    BlockHeaderConsensusParameters, BlockHeaderConsistencyError, BlockHeaderEd25519Seal,
    BlockHeaderFixedConsensusParameters, BlockHeaderPotParametersChange, BlockHeaderPrefix,
    BlockHeaderResult, BlockHeaderSeal, GenericBlockHeader, IntermediateShardHeader,
    LeafShardHeader,
};
use crate::block::{BlockNumber, BlockRoot, BlockTimestamp};
use crate::ed25519::{Ed25519PublicKey, Ed25519Signature};
use crate::hashes::Blake3Hash;
use crate::pot::{PotOutput, PotParametersChange, SlotNumber};
use crate::segments::SuperSegmentRoot;
use crate::shard::{NumShards, NumShardsUnchecked, RealShardKind, ShardIndex, ShardKind};
use crate::solutions::{Solution, SolutionRange};
use ab_aligned_buffer::OwnedAlignedBuffer;
use ab_io_type::trivial_type::TrivialType;
use std::assert_matches;
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

fn consensus_parameters() -> BlockHeaderConsensusParameters<'static> {
    BlockHeaderConsensusParameters {
        fixed_parameters: BlockHeaderFixedConsensusParameters {
            solution_range: SolutionRange::MAX,
            slot_iterations: NonZeroU32::MIN,
            num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
        },
        super_segment_root: None,
        next_solution_range: None,
        pot_parameters_change: None,
    }
}

/// Parts that are not consistent with a header of `shard_kind` with `prefix` and
/// [`consensus_info()`], each with the error it is rejected with
fn inconsistent_parts(
    prefix: BlockHeaderPrefix,
    shard_kind: RealShardKind,
) -> Vec<(
    BlockHeaderPrefix,
    BlockHeaderConsensusInfo,
    BlockHeaderConsistencyError,
)> {
    let mut parts = Vec::new();

    parts.push((
        BlockHeaderPrefix {
            padding_0: [0, 0, 0, 1],
            ..prefix
        },
        consensus_info(),
        BlockHeaderConsistencyError::NonZeroPrefixPadding,
    ));

    let mut out_of_range_shard_index = ShardIndex::BEACON_CHAIN;
    *out_of_range_shard_index.as_bytes_mut() = (ShardIndex::MAX_SHARD_INDEX + 1).to_le_bytes();
    let shard_indices = [
        ShardIndex::BEACON_CHAIN,
        ShardIndex::new(1).unwrap(),
        ShardIndex::new(1 | (1 << 10)).unwrap(),
        // Phantom shard
        ShardIndex::new(1 << 10).unwrap(),
        out_of_range_shard_index,
    ];
    for shard_index in shard_indices {
        if shard_index.shard_kind().and_then(ShardKind::to_real) == Some(shard_kind) {
            continue;
        }
        parts.push((
            BlockHeaderPrefix {
                shard_index,
                ..prefix
            },
            consensus_info(),
            BlockHeaderConsistencyError::InvalidShardIndex {
                shard_index,
                shard_kind,
            },
        ));
    }

    let mut consensus_info = consensus_info();
    consensus_info.solution.padding = [1, 0, 0, 0];
    parts.push((
        prefix,
        consensus_info,
        BlockHeaderConsistencyError::NonZeroSolutionPadding,
    ));

    parts
}

/// `header` bytes with the prefix and consensus info replaced
fn with_shared_parts(
    header: &[u8],
    prefix: &BlockHeaderPrefix,
    consensus_info: &BlockHeaderConsensusInfo,
) -> OwnedAlignedBuffer {
    let consensus_info_offset = (BlockHeaderPrefix::SIZE + BlockHeaderResult::SIZE) as usize;
    let bytes = [
        prefix.as_bytes(),
        &header[BlockHeaderPrefix::SIZE as usize..consensus_info_offset],
        consensus_info.as_bytes(),
        &header[consensus_info_offset + BlockHeaderConsensusInfo::SIZE as usize..],
    ]
    .concat();
    OwnedAlignedBuffer::from_bytes(&bytes)
}

/// Check that `buffer` contains `bytes_before` and then `header`, and that the header parses
fn check_written_header<Header>(buffer: &OwnedAlignedBuffer, bytes_before: &[u8], header: &Header)
where
    Header: GenericOwnedBlockHeader,
{
    let (written_bytes_before, header_bytes) = buffer.as_slice().split_at(bytes_before.len());
    assert_eq!(written_bytes_before, bytes_before);
    assert!(
        header_bytes == header.buffer().as_slice(),
        "Header must be written after the bytes before it"
    );

    let (parsed_header, remainder) =
        BlockHeader::try_from_bytes(header_bytes, Header::SHARD_KIND).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(*parsed_header.root(), *header.header().root());
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
        .unwrap()
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
    .unwrap()
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
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()));
    let buffer = into_buffer(header);
    assert_eq!(buffer.capacity(), OwnedLeafShardHeader::MAX_ALLOCATION);
    assert_eq!(buffer.len(), buffer.capacity());
}

#[test]
fn inconsistent_parts_are_rejected_by_from_parts_and_parser() {
    let seal = seal();
    let seal = BlockHeaderSeal::Ed25519(&seal);
    let child_shard_blocks_root = Blake3Hash::default();

    let prefix = prefix(0);
    let header = OwnedBeaconChainHeader::from_parts(
        &prefix,
        &result(),
        &consensus_info(),
        &child_shard_blocks_root,
        &consensus_parameters(),
    )
    .unwrap()
    .with_seal(seal);
    let header = header.buffer().as_slice();
    let bytes = with_shared_parts(header, &prefix, &consensus_info());
    assert!(BeaconChainHeader::try_from_bytes(bytes.as_slice()).is_some());
    for (prefix, consensus_info, error) in inconsistent_parts(prefix, RealShardKind::BeaconChain) {
        // Sealing parses the header, so inconsistent parts must be rejected before it
        let maybe_header = OwnedBeaconChainHeader::from_parts(
            &prefix,
            &result(),
            &consensus_info,
            &child_shard_blocks_root,
            &consensus_parameters(),
        )
        .map(|unsealed| unsealed.with_seal(seal));
        assert_eq!(maybe_header.err(), Some(error));

        let bytes = with_shared_parts(header, &prefix, &consensus_info);
        assert!(BeaconChainHeader::try_from_bytes(bytes.as_slice()).is_none());
    }

    let prefix = self::prefix(1);
    let header = OwnedIntermediateShardHeader::from_parts(
        &prefix,
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
        &child_shard_blocks_root,
    )
    .unwrap()
    .with_seal(seal);
    let header = header.buffer().as_slice();
    let bytes = with_shared_parts(header, &prefix, &consensus_info());
    assert!(IntermediateShardHeader::try_from_bytes(bytes.as_slice()).is_some());
    for (prefix, consensus_info, error) in
        inconsistent_parts(prefix, RealShardKind::IntermediateShard)
    {
        let maybe_header = OwnedIntermediateShardHeader::from_parts(
            &prefix,
            &result(),
            &consensus_info,
            &beacon_chain_info(),
            &child_shard_blocks_root,
        )
        .map(|unsealed| unsealed.with_seal(seal));
        assert_eq!(maybe_header.err(), Some(error));

        let bytes = with_shared_parts(header, &prefix, &consensus_info);
        assert!(IntermediateShardHeader::try_from_bytes(bytes.as_slice()).is_none());
    }

    let prefix = self::prefix(1 | (1 << 10));
    let header = OwnedLeafShardHeader::from_parts(
        &prefix,
        &result(),
        &consensus_info(),
        &beacon_chain_info(),
    )
    .unwrap()
    .with_seal(seal);
    let header = header.buffer().as_slice();
    let bytes = with_shared_parts(header, &prefix, &consensus_info());
    assert!(LeafShardHeader::try_from_bytes(bytes.as_slice()).is_some());
    for (prefix, consensus_info, error) in inconsistent_parts(prefix, RealShardKind::LeafShard) {
        let maybe_header = OwnedLeafShardHeader::from_parts(
            &prefix,
            &result(),
            &consensus_info,
            &beacon_chain_info(),
        )
        .map(|unsealed| unsealed.with_seal(seal));
        assert_eq!(maybe_header.err(), Some(error));

        let bytes = with_shared_parts(header, &prefix, &consensus_info);
        assert!(LeafShardHeader::try_from_bytes(bytes.as_slice()).is_none());
    }
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
    )
    .unwrap();
    assert_eq!(unsealed.pre_seal_hash(), owned_unsealed.pre_seal_hash());
    unsealed.with_seal(seal);
    let header = owned_unsealed.with_seal(seal);
    assert_eq!(buffer.as_slice()[..3], bytes_before);
    assert_eq!(buffer.as_slice()[3..], *header.buffer().as_slice());
}

#[test]
fn from_parts_into_rejects_inconsistent_parts() {
    let child_shard_blocks_root = Blake3Hash::default();
    // Nothing must be written after these bytes on error
    let bytes_before = [1, 2, 3];

    for (prefix, consensus_info, error) in inconsistent_parts(prefix(0), RealShardKind::BeaconChain)
    {
        let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
        assert_matches!(
            OwnedBeaconChainHeader::from_parts_into(
                &prefix,
                &result(),
                &consensus_info,
                &child_shard_blocks_root,
                &consensus_parameters(),
                &mut buffer,
            ),
            Err(OwnedBeaconChainHeaderError::InvalidParts { error: actual }) if actual == error
        );
        assert_eq!(buffer.as_slice(), bytes_before);
    }

    for (prefix, consensus_info, error) in
        inconsistent_parts(prefix(1), RealShardKind::IntermediateShard)
    {
        let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
        assert_matches!(
            OwnedIntermediateShardHeader::from_parts_into(
                &prefix,
                &result(),
                &consensus_info,
                &beacon_chain_info(),
                &child_shard_blocks_root,
                &mut buffer,
            ),
            Err(OwnedIntermediateShardHeaderError::InvalidParts { error: actual })
                if actual == error
        );
        assert_eq!(buffer.as_slice(), bytes_before);
    }

    for (prefix, consensus_info, error) in
        inconsistent_parts(prefix(1 | (1 << 10)), RealShardKind::LeafShard)
    {
        let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
        assert_matches!(
            OwnedLeafShardHeader::from_parts_into(
                &prefix,
                &result(),
                &consensus_info,
                &beacon_chain_info(),
                &mut buffer,
            ),
            Err(OwnedLeafShardHeaderError::InvalidParts { error: actual }) if actual == error
        );
        assert_eq!(buffer.as_slice(), bytes_before);
    }
}

#[test]
fn from_parts_into_writes_headers_that_parse() {
    let child_shard_blocks_root = Blake3Hash::new([1; _]);
    let super_segment_root = SuperSegmentRoot::from([1; _]);
    let pot_parameters_change = BlockHeaderPotParametersChange::from(PotParametersChange {
        slot: SlotNumber::ZERO,
        slot_iterations: NonZeroU32::MIN,
        entropy: Blake3Hash::default(),
    });
    let largest_consensus_parameters = BlockHeaderConsensusParameters {
        super_segment_root: Some(&super_segment_root),
        next_solution_range: Some(SolutionRange::MAX),
        pot_parameters_change: Some(&pot_parameters_change),
        ..consensus_parameters()
    };
    let seal = seal();
    let seal = BlockHeaderSeal::Ed25519(&seal);

    // Headers are written after 8-byte aligned numbers of bytes, which the parser requires
    for num_bytes_before in [0, 8, 16] {
        let bytes_before = vec![1; num_bytes_before];

        // Beacon chain headers have different sizes depending on consensus parameters
        for consensus_parameters in [consensus_parameters(), largest_consensus_parameters] {
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
            unsealed.with_seal(seal);
            let header = OwnedBeaconChainHeader::from_parts(
                &prefix(0),
                &result(),
                &consensus_info(),
                &child_shard_blocks_root,
                &consensus_parameters,
            )
            .unwrap()
            .with_seal(seal);
            check_written_header(&buffer, &bytes_before, &header);
        }

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
        unsealed.with_seal(seal);
        let header = OwnedIntermediateShardHeader::from_parts(
            &prefix(1),
            &result(),
            &consensus_info(),
            &beacon_chain_info(),
            &child_shard_blocks_root,
        )
        .unwrap()
        .with_seal(seal);
        check_written_header(&buffer, &bytes_before, &header);

        let mut buffer = OwnedAlignedBuffer::from_bytes(&bytes_before);
        let unsealed = OwnedLeafShardHeader::from_parts_into(
            &prefix(1 | (1 << 10)),
            &result(),
            &consensus_info(),
            &beacon_chain_info(),
            &mut buffer,
        )
        .unwrap();
        unsealed.with_seal(seal);
        let header = OwnedLeafShardHeader::from_parts(
            &prefix(1 | (1 << 10)),
            &result(),
            &consensus_info(),
            &beacon_chain_info(),
        )
        .unwrap()
        .with_seal(seal);
        check_written_header(&buffer, &bytes_before, &header);
    }
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
        .unwrap()
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
        .unwrap()
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
