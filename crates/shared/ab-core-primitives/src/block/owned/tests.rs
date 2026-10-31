use crate::address::Address;
use crate::block::body::owned::{GenericOwnedBlockBody, OwnedIntermediateShardBodyError};
use crate::block::body::{IntermediateShardBlockInfo, LeafShardBlockInfo};
use crate::block::header::owned::{
    GenericOwnedBlockHeader, OwnedBeaconChainHeader, OwnedIntermediateShardHeader,
};
use crate::block::header::{
    BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters,
    BlockHeaderEd25519Seal, BlockHeaderFixedConsensusParameters, BlockHeaderPrefix,
    BlockHeaderSeal,
};
use crate::block::owned::{
    GenericOwnedBlock, OwnedBeaconChainBlock, OwnedIntermediateShardBlock, OwnedLeafShardBlock,
};
use crate::block::{
    BeaconChainBlock, Block, BlockNumber, BlockRoot, BlockTimestamp, GenericBlock,
    IntermediateShardBlock, LeafShardBlock,
};
use crate::ed25519::{Ed25519PublicKey, Ed25519Signature};
use crate::hashes::Blake3Hash;
use crate::pot::{PotOutput, SlotNumber};
use crate::shard::{NumShards, RealShardKind, ShardIndex};
use crate::solutions::{Solution, SolutionRange};
use crate::transaction::owned::OwnedTransaction;
use crate::transaction::{Gas, TransactionHeader};
use crate::trees::ChildLeafShardBlocksMerkleTree;
use ab_aligned_buffer::SharedAlignedBuffer;
use core::iter::TrustedLen;
use std::iter;
use std::num::{NonZeroU16, NonZeroU32};

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
        &BlockHeaderConsensusParameters {
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
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
}

fn intermediate_shard_block<'a, LeafShardBlocks>(
    leaf_shard_blocks: LeafShardBlocks,
) -> OwnedIntermediateShardBlock
where
    LeafShardBlocks: TrustedLen<Item = LeafShardBlockInfo<'a>> + Clone + 'a,
{
    OwnedIntermediateShardBlock::init(iter::empty(), leaf_shard_blocks)
        .unwrap()
        .with_header(
            &prefix(ShardIndex::new(1).unwrap()),
            Blake3Hash::default(),
            &consensus_info(),
            &beacon_chain_info(),
        )
        .with_seal(BlockHeaderSeal::Ed25519(&seal()))
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
        .with_seal(BlockHeaderSeal::Ed25519(&seal()))
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
    for (parsed, remainder) in [
        BeaconChainBlock::try_from_bytes(aligned.as_slice()).unwrap(),
        BeaconChainBlock::try_from_bytes_unchecked(aligned.as_slice()).unwrap(),
    ] {
        assert!(remainder.is_empty());
        assert_eq!(*parsed.header().root(), *block.header.header().root());
        assert_eq!(parsed.body().root(), block.body.body().root());
    }

    // 8 bytes after a 16-byte boundary, which is enough for the header, and padding after it
    // puts the body at a 16-byte boundary again
    let misaligned = block_bytes(&block, 8);
    let misaligned = &misaligned.as_slice()[8..];
    assert!(BeaconChainBlock::try_from_bytes(misaligned).is_none());
    assert!(BeaconChainBlock::try_from_bytes_unchecked(misaligned).is_none());
    assert!(Block::try_from_bytes(misaligned, RealShardKind::BeaconChain).is_none());
}

#[test]
fn intermediate_shard_block_bytes_must_be_16_bytes_aligned() {
    let block = intermediate_shard_block(iter::empty());

    let aligned = block_bytes(&block, 0);
    for (parsed, remainder) in [
        IntermediateShardBlock::try_from_bytes(aligned.as_slice()).unwrap(),
        IntermediateShardBlock::try_from_bytes_unchecked(aligned.as_slice()).unwrap(),
    ] {
        assert!(remainder.is_empty());
        assert_eq!(*parsed.header().root(), *block.header.header().root());
        assert_eq!(parsed.body().root(), block.body.body().root());
    }

    // 8 bytes after a 16-byte boundary, which is enough for the header, and padding after it
    // puts the body at a 16-byte boundary again
    let misaligned = block_bytes(&block, 8);
    let misaligned = &misaligned.as_slice()[8..];
    assert!(IntermediateShardBlock::try_from_bytes(misaligned).is_none());
    assert!(IntermediateShardBlock::try_from_bytes_unchecked(misaligned).is_none());
    assert!(Block::try_from_bytes(misaligned, RealShardKind::IntermediateShard).is_none());
}

#[test]
fn leaf_shard_block_bytes_must_be_16_bytes_aligned() {
    let block = leaf_shard_block();

    let aligned = block_bytes(&block, 0);
    for (parsed, remainder) in [
        LeafShardBlock::try_from_bytes(aligned.as_slice()).unwrap(),
        LeafShardBlock::try_from_bytes_unchecked(aligned.as_slice()).unwrap(),
    ] {
        assert!(remainder.is_empty());
        assert_eq!(*parsed.header().root(), *block.header.header().root());
        assert_eq!(parsed.body().root(), block.body.body().root());
    }

    // 8 bytes after a 16-byte boundary, which is enough for the header, and padding after it
    // puts the body at a 16-byte boundary again
    let misaligned = block_bytes(&block, 8);
    let misaligned = &misaligned.as_slice()[8..];
    assert!(LeafShardBlock::try_from_bytes(misaligned).is_none());
    assert!(LeafShardBlock::try_from_bytes_unchecked(misaligned).is_none());
    assert!(Block::try_from_bytes(misaligned, RealShardKind::LeafShard).is_none());
}

#[test]
fn unchecked_parsers_reject_seals_of_other_public_keys() {
    let beacon_chain_block = beacon_chain_block();
    let intermediate_shard_block = intermediate_shard_block(iter::empty());
    let leaf_shard_block = leaf_shard_block();

    for (shard_kind, bytes, header_size) in [
        (
            RealShardKind::BeaconChain,
            block_bytes(&beacon_chain_block, 0),
            beacon_chain_block.header().buffer().len(),
        ),
        (
            RealShardKind::IntermediateShard,
            block_bytes(&intermediate_shard_block, 0),
            intermediate_shard_block.header().buffer().len(),
        ),
        (
            RealShardKind::LeafShard,
            block_bytes(&leaf_shard_block, 0),
            leaf_shard_block.header().buffer().len(),
        ),
    ] {
        assert!(Block::try_from_bytes_unchecked(bytes.as_slice(), shard_kind).is_some());

        // The header ends with the seal, which ends with the public key and the signature
        let mut bytes = bytes.as_slice().to_vec();
        let public_key_offset =
            usize::try_from(header_size).unwrap() - Ed25519Signature::SIZE - Ed25519PublicKey::SIZE;
        bytes[public_key_offset] ^= 1;
        let bytes = SharedAlignedBuffer::from_bytes(&bytes);

        assert!(Block::try_from_bytes(bytes.as_slice(), shard_kind).is_none());
        // `to_owned()` of such a block would panic
        assert!(Block::try_from_bytes_unchecked(bytes.as_slice(), shard_kind).is_none());
    }
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

    let transactions_root = leaf_shard_block.body.body().transactions().root();
    let intermediate_shard_block = intermediate_shard_block(iter::once(LeafShardBlockInfo {
        header: leaf_shard_block.header.header().clone(),
        transactions_root: &transactions_root,
        own_segments: None,
    }));
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
fn intermediate_shard_blocks_check_leaf_shard_blocks_without_segments() {
    let leaf_shard_block = leaf_shard_block();
    let transactions_root = leaf_shard_block.body.body().transactions().root();
    let init = |transactions_root| {
        OwnedIntermediateShardBlock::init(
            iter::empty(),
            iter::once(LeafShardBlockInfo {
                header: leaf_shard_block.header.header().clone(),
                transactions_root,
                own_segments: None,
            }),
        )
    };

    init(&transactions_root).unwrap();
    // The root of transactions of a leaf shard block without transactions
    assert!(matches!(
        init(&Blake3Hash::default()),
        Err(OwnedIntermediateShardBodyError::InconsistentLeafShardBlocks)
    ));
}

#[test]
fn blocks_with_inconsistent_child_block_roots_are_rejected() {
    fn leaf_shard_block_info<'a>(
        leaf_shard_block: &'a OwnedLeafShardBlock,
        transactions_root: &'a Blake3Hash,
    ) -> LeafShardBlockInfo<'a> {
        LeafShardBlockInfo {
            header: leaf_shard_block.header.header().clone(),
            transactions_root,
            own_segments: None,
        }
    }

    let leaf_shard_block = leaf_shard_block();
    // Both leaf shard blocks below have the same transactions
    let transactions_root = leaf_shard_block.body.body().transactions().root();
    let block = intermediate_shard_block(iter::once(leaf_shard_block_info(
        &leaf_shard_block,
        &transactions_root,
    )));
    let header = block.header.header();
    assert_eq!(
        *header.leaf_shard_blocks_root(),
        block.body.body().leaf_shard_blocks().headers_root()
    );
    let bytes = block_bytes(&block, 0);
    assert!(IntermediateShardBlock::try_from_bytes(bytes.as_slice()).is_some());
    assert!(
        OwnedIntermediateShardBlock::from_buffers(
            block.header.buffer().clone(),
            block.body.buffer().clone()
        )
        .is_some()
    );

    // A list of another leaf shard block
    let other_block = intermediate_shard_block(iter::once(leaf_shard_block_info(
        &leaf_shard_block_with_state_root(Blake3Hash::new([1; _])),
        &transactions_root,
    )));
    for leaf_shard_blocks_root in [
        Blake3Hash::new([1; _]),
        // The root of an empty list
        Blake3Hash::default(),
        other_block.body.body().leaf_shard_blocks().headers_root(),
        // The root of the only leaf shard block
        **leaf_shard_block.header.header().root(),
    ] {
        // The same header with a different root of leaf shard block roots
        let inconsistent_header = OwnedIntermediateShardHeader::from_parts(
            header.prefix,
            header.result,
            header.consensus_info,
            header.beacon_chain_info(),
            &leaf_shard_blocks_root,
        )
        .with_seal(header.seal);
        let bytes = parts_bytes(inconsistent_header.buffer(), block.body.buffer(), 0);

        assert!(IntermediateShardBlock::try_from_bytes(bytes.as_slice()).is_none());
        assert!(
            OwnedIntermediateShardBlock::from_buffers(
                inconsistent_header.buffer().clone(),
                block.body.buffer().clone()
            )
            .is_none()
        );
        // Child block roots are checked at the block level only
        assert!(IntermediateShardBlock::try_from_bytes_unchecked(bytes.as_slice()).is_some());
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
        header.consensus_parameters(),
    )
    .with_seal(header.seal);
    let bytes = parts_bytes(inconsistent_header.buffer(), block.body.buffer(), 0);

    assert!(BeaconChainBlock::try_from_bytes(bytes.as_slice()).is_none());
    assert!(
        OwnedBeaconChainBlock::from_buffers(
            inconsistent_header.buffer().clone(),
            block.body.buffer().clone()
        )
        .is_none()
    );
    assert!(BeaconChainBlock::try_from_bytes_unchecked(bytes.as_slice()).is_some());
}

#[test]
fn child_block_roots_are_tree_roots_of_header_roots() {
    let leaf_shard_blocks = (0..3)
        .map(|number| leaf_shard_block_with(BlockNumber::from(number), Blake3Hash::default()))
        .collect::<Vec<_>>();
    // All leaf shard blocks have the same transactions
    let transactions_root = leaf_shard_blocks[0].body.body().transactions().root();

    for num_leaf_shard_blocks in [1, 3] {
        let leaf_shard_blocks = &leaf_shard_blocks[..num_leaf_shard_blocks];
        let block = intermediate_shard_block(leaf_shard_blocks.iter().map(|leaf_shard_block| {
            LeafShardBlockInfo {
                header: leaf_shard_block.header.header().clone(),
                transactions_root: &transactions_root,
                own_segments: None,
            }
        }));
        let leaf_shard_block_roots = leaf_shard_blocks
            .iter()
            .map(|leaf_shard_block| **leaf_shard_block.header.header().root())
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
