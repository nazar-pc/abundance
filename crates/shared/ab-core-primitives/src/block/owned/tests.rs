use crate::address::Address;
use crate::block::body::owned::GenericOwnedBlockBody;
use crate::block::body::{IntermediateShardBlockInfo, LeafShardBlockInfo};
use crate::block::header::owned::GenericOwnedBlockHeader;
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
use ab_aligned_buffer::SharedAlignedBuffer;
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
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
}

fn intermediate_shard_block() -> OwnedIntermediateShardBlock {
    OwnedIntermediateShardBlock::init(iter::empty(), iter::empty::<LeafShardBlockInfo<'_>>())
        .unwrap()
        .with_header(
            &prefix(ShardIndex::new(1).unwrap()),
            Blake3Hash::default(),
            &consensus_info(),
            &beacon_chain_info(),
        )
        .unwrap()
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
    let mut builder = OwnedLeafShardBlock::init(iter::empty()).unwrap();
    builder.add_transaction(&transaction()).unwrap();
    builder
        .with_header(
            // The first leaf shard of the first intermediate shard
            &prefix(ShardIndex::new(1 | (1 << 10)).unwrap()),
            Blake3Hash::default(),
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
    let mut bytes = vec![0; offset];
    bytes.extend_from_slice(block.header().buffer().as_slice());
    // The returned buffer is aligned to 16 bytes
    bytes.resize(bytes.len().next_multiple_of(16), 0);
    bytes.extend_from_slice(block.body().buffer().as_slice());
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
    let block = intermediate_shard_block();

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
