use crate::address::Address;
use crate::block::body::owned::{
    GenericOwnedBlockBody, OwnedIntermediateShardBlockInfo, OwnedLeafShardBlockInfo,
};
use crate::block::body::{IntermediateShardBlockInfo, LeafShardBlockInfo};
use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedIntermediateShardHeader,
};
use crate::block::header::{
    BlockHeader, BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo, BlockHeaderConsistencyError,
    BlockHeaderEd25519Seal, BlockHeaderFixedConsensusParameters, BlockHeaderPotParametersChange,
    BlockHeaderPrefix, BlockHeaderSeal, OwnedBlockHeaderConsensusParameters,
};
use crate::block::owned::{
    GenericOwnedBlock, OwnedBeaconChainBlock, OwnedBlock, OwnedIntermediateShardBlock,
    OwnedIntermediateShardBlockBuilderError, OwnedLeafShardBlock,
};
use crate::block::{
    BeaconChainBlock, Block, BlockNumber, BlockRoot, BlockTimestamp, GenericBlock,
    IntermediateShardBlock, LeafShardBlock,
};
use crate::ed25519::{Ed25519PublicKey, Ed25519Signature};
use crate::hashes::Blake3Hash;
use crate::pot::{PotCheckpoints, PotOutput, PotParametersChange, SlotNumber};
use crate::segments::{LocalSegmentIndex, SegmentRoot, SuperSegmentRoot};
use crate::shard::{NumShards, ShardIndex};
use crate::solutions::{Solution, SolutionRange};
use crate::transaction::owned::OwnedTransaction;
use crate::transaction::{Gas, TransactionHeader};
use crate::trees::ChildLeafShardBlocksMerkleTree;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::Align16;
use core::iter;
use core::iter::TrustedLen;
use core::num::{NonZeroU16, NonZeroU32};

fn prefix(shard_index: ShardIndex) -> BlockHeaderPrefix {
    BlockHeaderPrefix {
        number: BlockNumber::ZERO,
        shard_index,
        padding_0: [0; _],
        timestamp: BlockTimestamp::default(),
        parent_root: BlockRoot::default(),
        mmr_root: Blake3Hash::default(),
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

fn seal() -> BlockHeaderEd25519Seal {
    BlockHeaderEd25519Seal {
        public_key: Ed25519PublicKey::default(),
        signature: Ed25519Signature::default(),
    }
}

fn beacon_chain_info() -> BlockHeaderBeaconChainInfo {
    BlockHeaderBeaconChainInfo {
        number: BlockNumber::ZERO,
        root: BlockRoot::default(),
    }
}

fn beacon_chain_block() -> OwnedBeaconChainBlock {
    OwnedBeaconChainBlock::init(
        iter::empty(),
        iter::empty::<IntermediateShardBlockInfo<'_>>(),
        &[],
    )
    .unwrap()
    .with_header(
        &prefix(ShardIndex::BEACON_CHAIN),
        Blake3Hash::default(),
        &consensus_info(),
        &OwnedBlockHeaderConsensusParameters {
            fixed_parameters: BlockHeaderFixedConsensusParameters {
                solution_range: SolutionRange::MAX,
                slot_iterations: NonZeroU32::MIN,
                num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
            },
            super_segment_root: None,
            next_solution_range: None,
            pot_parameters_change: None,
        },
    )
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
    .unwrap()
}

fn intermediate_shard_block<'a, LeafShardBlocks>(
    leaf_shard_blocks: LeafShardBlocks,
) -> OwnedIntermediateShardBlock
where
    LeafShardBlocks: TrustedLen<Item = LeafShardBlockInfo<'a>> + 'a,
{
    OwnedIntermediateShardBlock::init(iter::empty(), leaf_shard_blocks)
        .unwrap()
        .with_header(
            &prefix(ShardIndex::new(1).unwrap()),
            Blake3Hash::default(),
            &consensus_info(),
            &beacon_chain_info(),
        )
        .unwrap()
        .with_seal(BlockHeaderSeal::Ed25519(&seal()))
        .unwrap()
}

fn transaction() -> OwnedTransaction {
    OwnedTransaction::from_parts(
        &TransactionHeader {
            version: TransactionHeader::TRANSACTION_VERSION,
            block_root: BlockRoot::default(),
            gas_limit: Gas::default(),
            contract: Address::SYSTEM_CODE,
        },
        &[],
        &[],
        &[1],
        &[2, 3, 4],
    )
    .unwrap()
}

fn leaf_shard_block() -> OwnedLeafShardBlock {
    leaf_shard_block_with_state_root(Blake3Hash::default())
}

fn leaf_shard_block_with_state_root(state_root: Blake3Hash) -> OwnedLeafShardBlock {
    leaf_shard_block_with(BlockNumber::ZERO, state_root)
}

fn leaf_shard_block_with(number: BlockNumber, state_root: Blake3Hash) -> OwnedLeafShardBlock {
    let mut builder = OwnedLeafShardBlock::init(iter::empty()).unwrap();
    builder.add_transaction(&transaction()).unwrap();
    builder
        .with_header(
            &BlockHeaderPrefix {
                number,
                // The first leaf shard of the first intermediate shard
                ..prefix(ShardIndex::new(1 | (1 << 10)).unwrap())
            },
            state_root,
            &consensus_info(),
            &beacon_chain_info(),
        )
        .unwrap()
        .with_seal(BlockHeaderSeal::Ed25519(&seal()))
        .unwrap()
}

/// Bytes of a block that starts `offset` bytes after a 16-byte boundary in the returned buffer.
///
/// The header is followed by zero padding up to the next 16-byte boundary in memory and the body,
/// which is how the parsers read a block at that position.
fn block_bytes<Block>(block: &Block, offset: usize) -> SharedAlignedBuffer
where
    Block: GenericOwnedBlock,
{
    parts_bytes(block.header().buffer(), block.body().buffer(), offset)
}

/// The same as [`block_bytes()`], but for a header and a body that may not form a valid block
fn parts_bytes(
    header: &SharedAlignedBuffer,
    body: &SharedAlignedBuffer,
    offset: usize,
) -> SharedAlignedBuffer {
    let mut bytes = vec![0; offset];
    bytes.extend_from_slice(header.as_slice());
    // The returned buffer is aligned to 16 bytes
    bytes.resize(bytes.len().next_multiple_of(16), 0);
    bytes.extend_from_slice(body.as_slice());
    SharedAlignedBuffer::from_bytes(&bytes)
}

#[test]
fn beacon_chain_block_bytes_must_be_16_bytes_aligned() {
    let block = beacon_chain_block();

    let aligned = block_bytes(&block, 0);
    let (parsed, remainder) = BeaconChainBlock::try_from_bytes(aligned.as_slice()).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(*parsed.header().root(), *block.header.root());
    assert_eq!(parsed.body().root(), block.body.root());

    // 8 bytes after a 16-byte boundary, which is enough for the header, and padding after it
    // puts the body at a 16-byte boundary again
    let misaligned = block_bytes(&block, 8);
    let misaligned = &misaligned.as_slice()[8..];
    assert!(BeaconChainBlock::try_from_bytes(misaligned).is_none());
    assert!(Block::try_from_bytes(misaligned).is_none());
}

#[test]
fn intermediate_shard_block_bytes_must_be_16_bytes_aligned() {
    let block = intermediate_shard_block(iter::empty());

    let aligned = block_bytes(&block, 0);
    let (parsed, remainder) = IntermediateShardBlock::try_from_bytes(aligned.as_slice()).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(*parsed.header().root(), *block.header.root());
    assert_eq!(parsed.body().root(), block.body.root());

    // 8 bytes after a 16-byte boundary, which is enough for the header, and padding after it
    // puts the body at a 16-byte boundary again
    let misaligned = block_bytes(&block, 8);
    let misaligned = &misaligned.as_slice()[8..];
    assert!(IntermediateShardBlock::try_from_bytes(misaligned).is_none());
    assert!(Block::try_from_bytes(misaligned).is_none());
}

#[test]
fn leaf_shard_block_bytes_must_be_16_bytes_aligned() {
    let block = leaf_shard_block();

    let aligned = block_bytes(&block, 0);
    let (parsed, remainder) = LeafShardBlock::try_from_bytes(aligned.as_slice()).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(*parsed.header().root(), *block.header.root());
    assert_eq!(parsed.body().root(), block.body.root());

    // 8 bytes after a 16-byte boundary, which is enough for the header, and padding after it
    // puts the body at a 16-byte boundary again
    let misaligned = block_bytes(&block, 8);
    let misaligned = &misaligned.as_slice()[8..];
    assert!(LeafShardBlock::try_from_bytes(misaligned).is_none());
    assert!(Block::try_from_bytes(misaligned).is_none());
}

#[test]
fn block_parsers_reject_seals_of_other_public_keys() {
    let beacon_chain_block = beacon_chain_block();
    let intermediate_shard_block = intermediate_shard_block(iter::empty());
    let leaf_shard_block = leaf_shard_block();

    for (bytes, header_size) in [
        (
            block_bytes(&beacon_chain_block, 0),
            beacon_chain_block.header().buffer().len(),
        ),
        (
            block_bytes(&intermediate_shard_block, 0),
            intermediate_shard_block.header().buffer().len(),
        ),
        (
            block_bytes(&leaf_shard_block, 0),
            leaf_shard_block.header().buffer().len(),
        ),
    ] {
        assert!(Block::try_from_bytes(bytes.as_slice()).is_some());

        // The header ends with the seal, which ends with the public key and the signature
        let mut bytes = bytes.as_slice().to_vec();
        let public_key_offset =
            usize::try_from(header_size).unwrap() - Ed25519Signature::SIZE - Ed25519PublicKey::SIZE;
        bytes[public_key_offset] ^= 1;
        let bytes = SharedAlignedBuffer::<Align16>::from_bytes(&bytes);

        assert!(Block::try_from_bytes(bytes.as_slice()).is_none());
    }
}

#[test]
fn block_builders_reject_inconsistent_header_parts() {
    let seal = seal();
    let seal = BlockHeaderSeal::Ed25519(&seal);
    let consensus_info = consensus_info();
    let inconsistent_prefix = |shard_index| BlockHeaderPrefix {
        padding_0: [1, 0, 0, 0],
        ..prefix(shard_index)
    };

    // Sealing parses the header, so inconsistent parts must be rejected before it
    let maybe_block = OwnedBeaconChainBlock::init(
        iter::empty(),
        iter::empty::<IntermediateShardBlockInfo<'_>>(),
        &[],
    )
    .unwrap()
    .with_header(
        &inconsistent_prefix(ShardIndex::BEACON_CHAIN),
        Blake3Hash::default(),
        &consensus_info,
        &OwnedBlockHeaderConsensusParameters {
            fixed_parameters: BlockHeaderFixedConsensusParameters {
                solution_range: SolutionRange::MAX,
                slot_iterations: NonZeroU32::MIN,
                num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
            },
            super_segment_root: None,
            next_solution_range: None,
            pot_parameters_change: None,
        },
    )
    .and_then(|block_unsealed| block_unsealed.with_seal(seal));
    assert_eq!(
        maybe_block.err(),
        Some(BlockHeaderConsistencyError::NonZeroPrefixPadding)
    );

    let maybe_block =
        OwnedIntermediateShardBlock::init(iter::empty(), iter::empty::<LeafShardBlockInfo<'_>>())
            .unwrap()
            .with_header(
                &inconsistent_prefix(ShardIndex::new(1).unwrap()),
                Blake3Hash::default(),
                &consensus_info,
                &beacon_chain_info(),
            )
            .and_then(|block_unsealed| {
                block_unsealed
                    .with_seal(seal)
                    .map_err(OwnedIntermediateShardBlockBuilderError::from)
            });
    assert_eq!(
        maybe_block.err(),
        Some(
            OwnedIntermediateShardBlockBuilderError::InvalidHeaderParts {
                error: BlockHeaderConsistencyError::NonZeroPrefixPadding
            }
        )
    );

    let maybe_block = OwnedLeafShardBlock::init(iter::empty())
        .unwrap()
        .with_header(
            &inconsistent_prefix(ShardIndex::new(1 | (1 << 10)).unwrap()),
            Blake3Hash::default(),
            &consensus_info,
            &beacon_chain_info(),
        )
        .and_then(|block_unsealed| block_unsealed.with_seal(seal));
    assert_eq!(
        maybe_block.err(),
        Some(BlockHeaderConsistencyError::NonZeroPrefixPadding)
    );
}

#[test]
fn block_builders_reject_seals_of_other_public_keys() {
    let seal = BlockHeaderEd25519Seal {
        public_key: Ed25519PublicKey::from([1; Ed25519PublicKey::SIZE]),
        signature: Ed25519Signature::default(),
    };
    let error = BlockHeaderConsistencyError::SealPublicKeyHashMismatch {
        solution_public_key_hash: consensus_info().solution.public_key_hash,
        seal_public_key_hash: seal.public_key.hash(),
    };
    let seal = BlockHeaderSeal::Ed25519(&seal);

    let block_unsealed = OwnedBeaconChainBlock::init(
        iter::empty(),
        iter::empty::<IntermediateShardBlockInfo<'_>>(),
        &[],
    )
    .unwrap()
    .with_header(
        &prefix(ShardIndex::BEACON_CHAIN),
        Blake3Hash::default(),
        &consensus_info(),
        &OwnedBlockHeaderConsensusParameters {
            fixed_parameters: BlockHeaderFixedConsensusParameters {
                solution_range: SolutionRange::MAX,
                slot_iterations: NonZeroU32::MIN,
                num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
            },
            super_segment_root: None,
            next_solution_range: None,
            pot_parameters_change: None,
        },
    )
    .unwrap();
    assert_eq!(block_unsealed.with_seal(seal).err(), Some(error));

    let block_unsealed =
        OwnedIntermediateShardBlock::init(iter::empty(), iter::empty::<LeafShardBlockInfo<'_>>())
            .unwrap()
            .with_header(
                &prefix(ShardIndex::new(1).unwrap()),
                Blake3Hash::default(),
                &consensus_info(),
                &beacon_chain_info(),
            )
            .unwrap();
    assert_eq!(block_unsealed.with_seal(seal).err(), Some(error));

    let block_unsealed = OwnedLeafShardBlock::init(iter::empty())
        .unwrap()
        .with_header(
            &prefix(ShardIndex::new(1 | (1 << 10)).unwrap()),
            Blake3Hash::default(),
            &consensus_info(),
            &beacon_chain_info(),
        )
        .unwrap();
    assert_eq!(block_unsealed.with_seal(seal).err(), Some(error));
}

#[test]
fn intermediate_shard_block_builder_rejects_leaf_shard_blocks_of_other_shards() {
    // Leaf shard block of the intermediate shard 1
    let leaf_shard_block = leaf_shard_block();
    let leaf_shard_block_info = OwnedLeafShardBlockInfo::from_block(&leaf_shard_block.block());
    let builder = OwnedIntermediateShardBlock::init(
        iter::empty(),
        iter::once(leaf_shard_block_info.info().clone()),
    )
    .unwrap();
    let with_header = |shard_index| {
        builder.clone().with_header(
            &prefix(ShardIndex::new(shard_index).unwrap()),
            Blake3Hash::default(),
            &consensus_info(),
            &beacon_chain_info(),
        )
    };

    let block = with_header(1)
        .unwrap()
        .with_seal(BlockHeaderSeal::Ed25519(&seal()))
        .unwrap();
    assert_eq!(
        with_header(2).err(),
        Some(
            OwnedIntermediateShardBlockBuilderError::LeafShardBlockOfAnotherShard {
                shard_index: ShardIndex::new(2).unwrap(),
                leaf_shard_index: leaf_shard_block.header.header().prefix.shard_index,
            }
        )
    );

    // Parsers reject the same body with a header of another shard that commits to it
    let header = block.header.header();
    let header = OwnedIntermediateShardHeader::from_parts(
        &prefix(ShardIndex::new(2).unwrap()),
        header.result,
        header.consensus_info,
        header.beacon_chain_info(),
        header.leaf_shard_blocks_root(),
    )
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
    .unwrap();
    let bytes = parts_bytes(header.buffer(), block.body.buffer(), 0);
    assert!(IntermediateShardBlock::try_from_bytes(bytes.as_slice()).is_none());
    OwnedIntermediateShardBlock::from_buffers(header.buffer().clone(), block.body.buffer().clone())
        .unwrap_err();
}

#[test]
fn blocks_round_trip() {
    let leaf_shard_block = leaf_shard_block();
    let bytes = block_bytes(&leaf_shard_block, 0);

    let (block, _remainder) = LeafShardBlock::try_from_bytes(bytes.as_slice()).unwrap();
    let block = block.to_owned();
    assert_eq!(
        block.header.buffer().as_slice(),
        leaf_shard_block.header.buffer().as_slice()
    );
    assert_eq!(
        block.body.buffer().as_slice(),
        leaf_shard_block.body.buffer().as_slice()
    );

    let leaf_shard_block_info = OwnedLeafShardBlockInfo::from_block(&leaf_shard_block.block());
    let intermediate_shard_block =
        intermediate_shard_block(iter::once(leaf_shard_block_info.info().clone()));
    let bytes = block_bytes(&intermediate_shard_block, 0);
    let (block, _remainder) = IntermediateShardBlock::try_from_bytes(bytes.as_slice()).unwrap();
    assert_eq!(block.body().leaf_shard_blocks().len(), 1);
    let block = block.to_owned();
    assert_eq!(
        block.header.buffer().as_slice(),
        intermediate_shard_block.header.buffer().as_slice()
    );
    assert_eq!(
        block.body.buffer().as_slice(),
        intermediate_shard_block.body.buffer().as_slice()
    );

    let beacon_chain_block = beacon_chain_block();
    let bytes = block_bytes(&beacon_chain_block, 0);

    // The header is copied out of the buffer with a whole block, while the body references it.
    // Without optional consensus parameters the header is shorter than the largest one, so bytes
    // copied with it don't belong to it.
    let block = OwnedBeaconChainBlock::from_buffer(bytes.clone()).unwrap();
    assert!(block.header.buffer().as_slice() == beacon_chain_block.header.buffer().as_slice());
    assert!(block.body.buffer().as_slice() == beacon_chain_block.body.buffer().as_slice());
    let bytes_range = bytes.as_slice().as_ptr_range();
    assert!(!bytes_range.contains(&block.header.buffer().as_slice().as_ptr()));
    assert!(bytes_range.contains(&block.body.buffer().as_slice().as_ptr()));

    let (block, _remainder) = BeaconChainBlock::try_from_bytes(bytes.as_slice()).unwrap();
    let block = block.to_owned();
    assert_eq!(
        block.header.buffer().as_slice(),
        beacon_chain_block.header.buffer().as_slice()
    );
    assert_eq!(
        block.body.buffer().as_slice(),
        beacon_chain_block.body.buffer().as_slice()
    );
}

#[test]
fn blocks_with_inconsistent_child_block_roots_are_rejected() {
    let leaf_shard_block = leaf_shard_block();
    let leaf_shard_block_info = OwnedLeafShardBlockInfo::from_block(&leaf_shard_block.block());
    let block = intermediate_shard_block(iter::once(leaf_shard_block_info.info().clone()));
    let header = block.header.header();
    assert_eq!(
        *header.leaf_shard_blocks_root(),
        block.body.body().leaf_shard_blocks().headers_root()
    );
    let bytes = block_bytes(&block, 0);
    assert!(IntermediateShardBlock::try_from_bytes(bytes.as_slice()).is_some());
    OwnedIntermediateShardBlock::from_buffers(
        block.header.buffer().clone(),
        block.body.buffer().clone(),
    )
    .unwrap();

    // A list of another leaf shard block
    let other_leaf_shard_block_info = OwnedLeafShardBlockInfo::from_block(
        &leaf_shard_block_with_state_root(Blake3Hash::new([1; _])).block(),
    );
    let other_block =
        intermediate_shard_block(iter::once(other_leaf_shard_block_info.info().clone()));
    for leaf_shard_blocks_root in [
        Blake3Hash::new([1; _]),
        // The root of an empty list
        Blake3Hash::default(),
        other_block.body.body().leaf_shard_blocks().headers_root(),
        // The root of the only leaf shard block
        **leaf_shard_block.header.root(),
    ] {
        // The same header with a different root of leaf shard block roots
        let inconsistent_header = OwnedIntermediateShardHeader::from_parts(
            header.prefix,
            header.result,
            header.consensus_info,
            header.beacon_chain_info(),
            &leaf_shard_blocks_root,
        )
        .unwrap()
        .with_seal(header.seal)
        .unwrap();
        let bytes = parts_bytes(inconsistent_header.buffer(), block.body.buffer(), 0);

        assert!(IntermediateShardBlock::try_from_bytes(bytes.as_slice()).is_none());
        OwnedIntermediateShardBlock::from_buffers(
            inconsistent_header.buffer().clone(),
            block.body.buffer().clone(),
        )
        .unwrap_err();
    }

    // A beacon chain block without intermediate shard blocks, which claims to have some
    let block = beacon_chain_block();
    let header = block.header.header();
    assert_eq!(
        *header.intermediate_shard_blocks_root(),
        Blake3Hash::default()
    );
    let inconsistent_header = OwnedBeaconChainHeader::from_parts(
        header.prefix,
        header.result,
        header.consensus_info,
        &Blake3Hash::new([1; _]),
        &OwnedBlockHeaderConsensusParameters::from(header.consensus_parameters()),
    )
    .unwrap()
    .with_seal(header.seal)
    .unwrap();
    let bytes = parts_bytes(inconsistent_header.buffer(), block.body.buffer(), 0);

    assert!(BeaconChainBlock::try_from_bytes(bytes.as_slice()).is_none());

    // Buffers and parts are returned back when they don't form a block
    let (returned_header, returned_body) = OwnedBeaconChainBlock::from_buffers(
        inconsistent_header.buffer().clone(),
        block.body.buffer().clone(),
    )
    .unwrap_err();
    assert!(returned_header.as_slice() == inconsistent_header.buffer().as_slice());
    assert!(returned_body.as_slice() == block.body.buffer().as_slice());
    // Also when the header or the body can't be parsed
    let (returned_header, returned_body) = OwnedBeaconChainBlock::from_buffers(
        block.body.buffer().clone(),
        block.body.buffer().clone(),
    )
    .unwrap_err();
    assert!(returned_header.as_slice() == block.body.buffer().as_slice());
    assert!(returned_body.as_slice() == block.body.buffer().as_slice());
    let (returned_header, returned_body) = OwnedBeaconChainBlock::from_buffers(
        block.header.buffer().clone(),
        block.header.buffer().clone(),
    )
    .unwrap_err();
    assert!(returned_header.as_slice() == block.header.buffer().as_slice());
    assert!(returned_body.as_slice() == block.header.buffer().as_slice());
    let (returned_header, returned_body) =
        OwnedBeaconChainBlock::from_parts(inconsistent_header.clone(), block.body.clone())
            .unwrap_err();
    assert!(returned_header.buffer().as_slice() == inconsistent_header.buffer().as_slice());
    assert!(returned_body.buffer().as_slice() == block.body.buffer().as_slice());
    let (header, body) = block.split();
    OwnedBeaconChainBlock::from_parts(header, body).unwrap();
}

#[test]
fn child_block_roots_are_tree_roots_of_header_roots() {
    let leaf_shard_blocks = (0..3)
        .map(|number| leaf_shard_block_with(BlockNumber::from(number), Blake3Hash::default()))
        .collect::<Vec<_>>();
    let leaf_shard_block_infos = leaf_shard_blocks
        .iter()
        .map(|leaf_shard_block| OwnedLeafShardBlockInfo::from_block(&leaf_shard_block.block()))
        .collect::<Vec<_>>();

    for num_leaf_shard_blocks in [1, 3] {
        let leaf_shard_blocks = &leaf_shard_blocks[..num_leaf_shard_blocks];
        let block = intermediate_shard_block(
            leaf_shard_block_infos[..num_leaf_shard_blocks]
                .iter()
                .map(|leaf_shard_block_info| leaf_shard_block_info.info().clone()),
        );
        let leaf_shard_block_roots = leaf_shard_blocks
            .iter()
            .map(|leaf_shard_block| **leaf_shard_block.header.root())
            .collect::<Vec<_>>();

        let leaf_shard_blocks_root = *block.header.header().leaf_shard_blocks_root();
        assert_eq!(
            *leaf_shard_blocks_root,
            ChildLeafShardBlocksMerkleTree::compute_root_only::<{ u64::from(u16::MAX) }, _, _>(
                leaf_shard_block_roots.iter().copied()
            )
            .unwrap()
        );
        if let [leaf_shard_block_root] = leaf_shard_block_roots.as_slice() {
            // The root of a tree with a single leaf is not the leaf itself
            assert_ne!(leaf_shard_blocks_root, *leaf_shard_block_root);
        }
    }
}

/// Check that `block` has a single encoding: every truncated encoding is rejected, and every
/// encoding with a bit flipped in one byte is either rejected or has a different header root.
///
/// Header roots commit to bodies through body roots, so this covers both, and both the lowest and
/// the highest bit are flipped to reach invalid values of counts, flags and padding. Invalid values
/// that would change the root if accepted, like unexpected flags, can't be caught this way and are
/// tested separately. Bytes after the block don't belong to it, while owned blocks, headers and
/// bodies must be exactly the bytes of their buffers.
fn check_encoding_is_canonical<Owned>(block: &Owned)
where
    Owned: GenericOwnedBlock,
{
    let header_root = *block.header().root();
    let block_buffer = block_bytes(block, 0);
    let mut bytes = block_buffer.clone().into_owned();

    let (parsed_block, remainder) = Block::try_from_bytes(bytes.as_slice()).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(*parsed_block.header().root(), header_root);

    // The shard kind of a block of any kind is that of its header
    let header = block.header().buffer();
    let body = block.body().buffer();
    assert_eq!(
        BlockHeader::shard_kind_from_bytes(header.as_slice()),
        Some(Owned::SHARD_KIND)
    );
    for owned_block in [
        parsed_block.to_owned(),
        OwnedBlock::from_buffers(header.clone(), body.clone()).unwrap(),
        OwnedBlock::from_buffer(block_buffer.clone()).unwrap(),
    ] {
        let (owned_header, owned_body) = owned_block.split();
        assert!(
            owned_header.buffer().as_slice() == header.as_slice(),
            "Header of a block of any kind"
        );
        assert!(
            owned_body.buffer().as_slice() == body.as_slice(),
            "Body of a block of any kind"
        );
    }

    // The header is copied out of the buffer with a whole block, while the body references it
    let owned_block = Owned::from_buffer(block_buffer.clone()).unwrap();
    assert!(owned_block.header().buffer().as_slice() == header.as_slice());
    assert!(owned_block.body().buffer().as_slice() == body.as_slice());
    let block_buffer_range = block_buffer.as_slice().as_ptr_range();
    assert!(!block_buffer_range.contains(&owned_block.header().buffer().as_slice().as_ptr()));
    assert!(block_buffer_range.contains(&owned_block.body().buffer().as_slice().as_ptr()));

    for len in 0..bytes.as_slice().len() {
        assert!(
            Block::try_from_bytes(&bytes.as_slice()[..len]).is_none(),
            "Truncated to {len} out of {} bytes",
            bytes.len()
        );
        Owned::from_buffer(SharedAlignedBuffer::from_bytes(&bytes.as_slice()[..len])).unwrap_err();
    }

    for offset in 0..bytes.as_slice().len() {
        for mask in [0b0000_0001, 0b1000_0000] {
            bytes.as_mut_slice()[offset] ^= mask;
            let maybe_parsed_block = Block::try_from_bytes(bytes.as_slice());
            if let Some((parsed_block, _remainder)) = &maybe_parsed_block {
                assert_ne!(
                    *parsed_block.header().root(),
                    header_root,
                    "Byte at offset {offset} with bits {mask:#010b} flipped"
                );
            }
            // A buffer with a whole block is accepted when the block is followed by nothing else
            assert_eq!(
                Owned::from_buffer(SharedAlignedBuffer::from_bytes(bytes.as_slice())).is_ok(),
                maybe_parsed_block.is_some_and(|(_parsed_block, remainder)| remainder.is_empty()),
                "Byte at offset {offset} with bits {mask:#010b} flipped"
            );
            bytes.as_mut_slice()[offset] ^= mask;
        }
    }

    let bytes_with_extra_byte =
        SharedAlignedBuffer::<Align16>::from_bytes(&[bytes.as_slice(), &[0]].concat());
    let (parsed_block, remainder) =
        Block::try_from_bytes(bytes_with_extra_byte.as_slice()).unwrap();
    assert_eq!(remainder, [0]);
    assert_eq!(*parsed_block.header().root(), header_root);
    let returned_buffer = Owned::from_buffer(bytes_with_extra_byte.clone()).unwrap_err();
    assert!(returned_buffer.as_slice() == bytes_with_extra_byte.as_slice());

    let header_with_extra_byte =
        SharedAlignedBuffer::from_bytes(&[block.header().buffer().as_slice(), &[0]].concat());
    let returned_buffer = Owned::Header::from_buffer(header_with_extra_byte.clone()).unwrap_err();
    assert_eq!(
        returned_buffer.as_slice(),
        header_with_extra_byte.as_slice()
    );
    let body_with_extra_byte =
        SharedAlignedBuffer::from_bytes(&[block.body().buffer().as_slice(), &[0]].concat());
    let returned_buffer = Owned::Body::from_buffer(body_with_extra_byte.clone()).unwrap_err();
    assert_eq!(returned_buffer.as_slice(), body_with_extra_byte.as_slice());
}

#[test]
#[cfg_attr(
    miri,
    ignore = "Parses tens of thousands of mutated blocks, too slow under Miri"
)]
fn block_encodings_are_canonical() {
    let own_segment_roots = [SegmentRoot::from([1; _]), SegmentRoot::from([2; _])];
    let own_segments = || {
        own_segment_roots
            .iter()
            .enumerate()
            .map(|(offset, &segment_root)| {
                let offset = u64::try_from(offset).unwrap();
                (LocalSegmentIndex::from(10 + offset), segment_root)
            })
    };

    // Leaf shard blocks of the same intermediate shard with and without own segments, with
    // transactions that have padding after them. Two of them with own segments make the record of
    // the intermediate shard block in the beacon chain body end with padding.
    let leaf_shard_blocks = [
        (ShardIndex::new(1 | (1 << 10)).unwrap(), true),
        (ShardIndex::new(1 | (2 << 10)).unwrap(), false),
        (ShardIndex::new(1 | (3 << 10)).unwrap(), true),
    ]
    .map(|(shard_index, with_own_segments)| {
        let mut builder =
            OwnedLeafShardBlock::init(own_segments().take(if with_own_segments { 2 } else { 0 }))
                .unwrap();
        builder.add_transaction(&transaction()).unwrap();
        builder.add_transaction(&transaction()).unwrap();
        builder
            .with_header(
                &prefix(shard_index),
                Blake3Hash::new([3; _]),
                &consensus_info(),
                &beacon_chain_info(),
            )
            .unwrap()
            .with_seal(BlockHeaderSeal::Ed25519(&seal()))
            .unwrap()
    });
    let leaf_shard_block_infos = leaf_shard_blocks
        .each_ref()
        .map(|leaf_shard_block| OwnedLeafShardBlockInfo::from_block(&leaf_shard_block.block()));

    let intermediate_shard_block = OwnedIntermediateShardBlock::init(
        own_segments(),
        leaf_shard_block_infos
            .iter()
            .map(|leaf_shard_block_info| leaf_shard_block_info.info().clone()),
    )
    .unwrap()
    .with_header(
        &prefix(ShardIndex::new(1).unwrap()),
        Blake3Hash::new([4; _]),
        &consensus_info(),
        &beacon_chain_info(),
    )
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
    .unwrap();
    let intermediate_shard_block_info =
        OwnedIntermediateShardBlockInfo::from_block(&intermediate_shard_block.block());

    // All optional consensus parameters are present
    let beacon_chain_block = OwnedBeaconChainBlock::init(
        own_segments(),
        iter::once(intermediate_shard_block_info.info().clone()),
        &[PotCheckpoints::default()],
    )
    .unwrap()
    .with_header(
        &prefix(ShardIndex::BEACON_CHAIN),
        Blake3Hash::new([5; _]),
        &consensus_info(),
        &OwnedBlockHeaderConsensusParameters {
            fixed_parameters: BlockHeaderFixedConsensusParameters {
                solution_range: SolutionRange::MAX,
                slot_iterations: NonZeroU32::MIN,
                num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
            },
            super_segment_root: Some(SuperSegmentRoot::from([6; _])),
            next_solution_range: Some(SolutionRange::from(7)),
            pot_parameters_change: Some(BlockHeaderPotParametersChange::from(
                PotParametersChange {
                    slot: SlotNumber::from(8),
                    slot_iterations: NonZeroU32::MIN,
                    entropy: Blake3Hash::new([9; _]),
                },
            )),
        },
    )
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
    .unwrap();

    for leaf_shard_block in &leaf_shard_blocks {
        check_encoding_is_canonical(leaf_shard_block);
    }
    check_encoding_is_canonical(&intermediate_shard_block);
    check_encoding_is_canonical(&beacon_chain_block);
}
