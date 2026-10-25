use crate::address::Address;
use crate::block::body::owned::{
    GenericOwnedBlockBody, OwnedBeaconChainBody, OwnedIntermediateShardBody, OwnedLeafShardBody,
};
use crate::block::body::{
    BeaconChainBody, BlockBody, IntermediateShardBlockInfo, IntermediateShardBody,
    LeafShardBlockInfo, OwnSegments,
};
use crate::block::header::owned::{OwnedIntermediateShardHeader, OwnedLeafShardHeader};
use crate::block::header::{
    BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo, BlockHeaderEd25519Seal,
    BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
};
use crate::block::{BlockNumber, BlockRoot, BlockTimestamp};
use crate::ed25519::{Ed25519PublicKey, Ed25519Signature};
use crate::hashes::Blake3Hash;
use crate::pot::{PotCheckpoints, PotOutput, SlotNumber};
use crate::segments::{LocalSegmentIndex, SegmentRoot};
use crate::shard::{RealShardKind, ShardIndex};
use crate::solutions::Solution;
use crate::transaction::owned::OwnedTransaction;
use crate::transaction::{Gas, TransactionHeader, TransactionSlot};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use ab_merkle_tree::balanced::BalancedMerkleTree;
use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;
use core::ptr;
use std::iter;

fn transaction_header() -> TransactionHeader {
    TransactionHeader {
        version: TransactionHeader::TRANSACTION_VERSION,
        block_root: BlockRoot::default(),
        gas_limit: Gas::default(),
        contract: Address::SYSTEM_CODE,
    }
}

/// A transaction that ends 3 bytes after a 16-byte boundary, so padding follows it
fn transaction() -> OwnedTransaction {
    OwnedTransaction::from_parts(&transaction_header(), &[], &[], &[1], &[2, 3, 4]).unwrap()
}

/// Segments that start at `first_local_segment_index`, each with a different root
fn segments(
    first_local_segment_index: u64,
    num_segments: u8,
) -> (LocalSegmentIndex, Vec<SegmentRoot>) {
    (
        LocalSegmentIndex::from(first_local_segment_index),
        (0..num_segments)
            .map(|index| SegmentRoot::from([index.wrapping_add(10); _]))
            .collect(),
    )
}

fn header_prefix(shard_index: u32, number: u64) -> BlockHeaderPrefix {
    BlockHeaderPrefix {
        number: BlockNumber::from(number),
        shard_index: ShardIndex::new(shard_index).unwrap(),
        padding_0: [0; _],
        timestamp: BlockTimestamp::default(),
        parent_root: BlockRoot::default(),
        mmr_root: Blake3Hash::default(),
    }
}

fn header_result(body_root: Blake3Hash) -> BlockHeaderResult {
    BlockHeaderResult {
        body_root,
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

fn leaf_shard_header(shard_index: u32, number: u64, body_root: Blake3Hash) -> OwnedLeafShardHeader {
    OwnedLeafShardHeader::from_parts(
        &header_prefix(shard_index, number),
        &header_result(body_root),
        &consensus_info(),
        &beacon_chain_info(),
    )
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
}

fn intermediate_shard_header(
    shard_index: u32,
    number: u64,
    body_root: Blake3Hash,
) -> OwnedIntermediateShardHeader {
    OwnedIntermediateShardHeader::from_parts(
        &header_prefix(shard_index, number),
        &header_result(body_root),
        &consensus_info(),
        &beacon_chain_info(),
        &[],
    )
    .unwrap()
    .with_seal(BlockHeaderSeal::Ed25519(&seal()))
}

fn pad_to(bytes: &mut Vec<u8>, alignment: usize) {
    bytes.resize(bytes.len().next_multiple_of(alignment), 0);
}

fn append_segments(
    bytes: &mut Vec<u8>,
    (first_local_segment_index, segment_roots): &(LocalSegmentIndex, Vec<SegmentRoot>),
) {
    bytes.extend_from_slice(&u64::from(*first_local_segment_index).to_le_bytes());
    for segment_root in segment_roots {
        bytes.extend_from_slice(segment_root.as_ref());
    }
}

fn assert_segments_eq(
    actual: &OwnSegments<'_>,
    (first_local_segment_index, segment_roots): &(LocalSegmentIndex, Vec<SegmentRoot>),
) {
    assert_eq!(actual.first_local_segment_index, *first_local_segment_index);
    assert_eq!(actual.segment_roots, segment_roots.as_slice());
}

/// A copy of `bytes` that starts `offset` bytes after a 16-byte boundary
fn bytes_at(bytes: &[u8], offset: usize) -> SharedAlignedBuffer {
    let mut buffer = vec![0; offset];
    buffer.extend_from_slice(bytes);
    SharedAlignedBuffer::from_bytes(&buffer)
}

#[test]
fn owned_leaf_shard_body_multiple_transactions() {
    let header = transaction_header();
    let slot = TransactionSlot {
        owner: Address::from(100),
        contract: Address::SYSTEM_CODE,
    };
    // Different sizes and contents, with seals that require padding after them
    let transactions = [
        OwnedTransaction::from_parts(&header, &[slot], &[], &[1], &[2, 3, 4]).unwrap(),
        OwnedTransaction::from_parts(&header, &[], &[slot, slot], &[5, 6], &[7]).unwrap(),
        OwnedTransaction::from_parts(&header, &[], &[], &[], &[8, 9]).unwrap(),
    ];

    let mut builder = OwnedLeafShardBody::init(iter::empty()).unwrap();
    for transaction in &transactions {
        builder.add_transaction(transaction).unwrap();
    }
    let body = builder.finish();

    let body_transactions = body.body().transactions();
    assert_eq!(body_transactions.len(), transactions.len());
    for (body_transaction, transaction) in body_transactions.iter().zip(&transactions) {
        assert_eq!(
            body_transaction.encoded_size(),
            usize::try_from(transaction.buffer().len()).unwrap()
        );
        assert_eq!(
            body_transaction.write_slots,
            transaction.transaction().write_slots
        );
        assert_eq!(body_transaction.payload, transaction.transaction().payload);
        assert_eq!(body_transaction.seal, transaction.transaction().seal);
    }
}

#[test]
fn owned_leaf_shard_body_aligns_transactions_to_max_alignment() {
    // The fields before the first transaction take 5 bytes, so both transactions need padding
    // before them
    let transaction = transaction();
    let mut builder = OwnedLeafShardBody::init(iter::empty()).unwrap();
    builder.add_transaction(&transaction).unwrap();
    builder.add_transaction(&transaction).unwrap();
    let body = builder.finish();

    let buffer_start = body.buffer().as_ptr().addr();
    let body_transactions = body.body().transactions();
    assert_eq!(body_transactions.len(), 2);
    for body_transaction in body_transactions.iter() {
        let offset = ptr::from_ref(body_transaction.header).addr() - buffer_start;
        assert!(offset.is_multiple_of(16), "Offset {offset}");
        assert_eq!(body_transaction.payload, [1]);
        assert_eq!(body_transaction.seal, [2, 3, 4]);
    }
}

#[test]
fn beacon_chain_body_bytes_must_be_4_bytes_aligned() {
    for offset in 0..8_usize {
        // An empty body at `offset`: zero numbers of PoT checkpoints, own segment roots and
        // intermediate shard blocks, then zero padding up to the next 8-byte boundary in memory
        let len = (offset + 7).next_multiple_of(8) - offset;
        let bytes = bytes_at(&vec![0; len], offset);
        let bytes = &bytes.as_slice()[offset..];

        // The number of PoT checkpoints is read as an aligned `u32`, and the padding adapts to
        // the position of the rest
        let expected = (offset % 4 == 0).then_some(0);
        assert_eq!(
            BeaconChainBody::try_from_bytes(bytes).map(|(_body, remainder)| remainder.len()),
            expected
        );
        assert_eq!(
            BeaconChainBody::try_from_bytes_unchecked(bytes)
                .map(|(_body, remainder)| remainder.len()),
            expected
        );
        assert_eq!(
            BlockBody::try_from_bytes(bytes, RealShardKind::BeaconChain)
                .map(|(_body, remainder)| remainder.len()),
            expected
        );
    }
}

/// The buffer of an owned body, which keeps the capacity it was allocated with unless it had to
/// grow
fn into_buffer<Body>(body: Body) -> OwnedAlignedBuffer
where
    Body: GenericOwnedBlockBody,
{
    let buffer = body.buffer().clone();
    // The buffer is not shared anymore after this, so it is reused rather than copied
    drop(body);
    buffer.into_owned()
}

#[test]
fn owned_bodies_without_child_shard_blocks_fit_into_their_capacity() {
    // As many own segment roots as bodies can have, and none
    for num_own_segment_roots in [u8::MAX, 0] {
        let own_segments = || {
            (0..num_own_segment_roots).map(|index| {
                (
                    LocalSegmentIndex::from(u64::from(index)),
                    SegmentRoot::default(),
                )
            })
        };

        let body = OwnedBeaconChainBody::new(
            own_segments(),
            iter::empty::<IntermediateShardBlockInfo<'_>>(),
            &[PotCheckpoints::default(); 3],
        )
        .unwrap();
        let buffer = into_buffer(body);
        assert_eq!(buffer.len(), buffer.capacity());

        let body = OwnedIntermediateShardBody::new(
            own_segments(),
            iter::empty::<LeafShardBlockInfo<'_>>(),
        )
        .unwrap();
        let buffer = into_buffer(body);
        assert_eq!(buffer.len(), buffer.capacity());

        // Transactions are added after the buffer is allocated, so there are none here
        let body = OwnedLeafShardBody::init(own_segments()).unwrap().finish();
        let buffer = into_buffer(body);
        assert_eq!(buffer.len(), buffer.capacity());
    }
}

/// A leaf shard block inside an intermediate shard body
struct TestLeafShardBlock {
    header: OwnedLeafShardHeader,
    segments: Option<(LocalSegmentIndex, Vec<SegmentRoot>)>,
    segment_roots_proof: [u8; 32],
}

impl TestLeafShardBlock {
    fn new(shard_index: u32, number: u64, num_segments: u8) -> Self {
        let segment_roots_proof = [u8::try_from(number).unwrap(); _];
        let segments = (num_segments > 0).then(|| segments(number, num_segments));
        // The body root that the segment roots proof belongs to
        let body_root = segments.as_ref().map_or_else(
            Blake3Hash::default,
            |(first_local_segment_index, segment_roots)| {
                let own_segments_root = OwnSegments {
                    first_local_segment_index: *first_local_segment_index,
                    segment_roots,
                }
                .root();
                Blake3Hash::new(BalancedMerkleTree::compute_root_only(&[
                    *own_segments_root,
                    segment_roots_proof,
                ]))
            },
        );

        Self {
            header: leaf_shard_header(shard_index, number, body_root),
            segments,
            segment_roots_proof,
        }
    }
}

/// Bytes of an intermediate shard body with leaf shard blocks, following the layout described in
/// `LeafShardBlocksInfo::try_from_bytes()`
fn intermediate_shard_body_bytes(
    own_segments: &(LocalSegmentIndex, Vec<SegmentRoot>),
    leaf_shard_blocks: &[TestLeafShardBlock],
) -> SharedAlignedBuffer {
    let (_first_local_segment_index, own_segment_roots) = own_segments;
    let mut bytes = vec![u8::try_from(own_segment_roots.len()).unwrap()];
    append_segments(&mut bytes, own_segments);

    bytes.extend_from_slice(
        &u16::try_from(leaf_shard_blocks.len())
            .unwrap()
            .to_le_bytes(),
    );
    for block in leaf_shard_blocks {
        bytes.push(block.segments.as_ref().map_or(
            0,
            |(_first_local_segment_index, segment_roots)| {
                u8::try_from(segment_roots.len()).unwrap()
            },
        ));
    }
    pad_to(&mut bytes, 8);
    for block in leaf_shard_blocks {
        bytes.extend_from_slice(block.header.buffer().as_slice());
        pad_to(&mut bytes, 8);
        if let Some((first_local_segment_index, segment_roots)) = &block.segments {
            bytes.extend_from_slice(&u64::from(*first_local_segment_index).to_le_bytes());
            bytes.extend_from_slice(&block.segment_roots_proof);
            for segment_root in segment_roots {
                bytes.extend_from_slice(segment_root.as_ref());
            }
        }
    }

    SharedAlignedBuffer::from_bytes(&bytes)
}

#[test]
fn intermediate_shard_body_with_leaf_shard_blocks() {
    let own_segments = segments(3, 1);
    // A few blocks of two leaf shards of the first intermediate shard
    let leaf_shard_blocks = [
        TestLeafShardBlock::new(1 | (1 << 10), 0, 0),
        TestLeafShardBlock::new(1 | (1 << 10), 1, 2),
        TestLeafShardBlock::new(1 | (2 << 10), 0, 1),
    ];

    // Different numbers of blocks need different padding after the numbers of segment roots
    for num_blocks in 1..=leaf_shard_blocks.len() {
        let leaf_shard_blocks = &leaf_shard_blocks[..num_blocks];
        let bytes = intermediate_shard_body_bytes(&own_segments, leaf_shard_blocks);

        let (body, remainder) = IntermediateShardBody::try_from_bytes(bytes.as_slice()).unwrap();
        assert!(remainder.is_empty());
        assert_segments_eq(&body.own_segments().unwrap(), &own_segments);
        assert_eq!(body.leaf_shard_blocks().len(), num_blocks);
        for (info, block) in body.leaf_shard_blocks().iter().zip(leaf_shard_blocks) {
            assert_eq!(*info.header.root(), *block.header.header().root());
            match (&info.segments, &block.segments) {
                (Some(info_segments), Some(segments)) => {
                    assert_eq!(
                        *info_segments.segment_roots_proof,
                        block.segment_roots_proof
                    );
                    assert_segments_eq(&info_segments.own_segments, segments);
                }
                (None, None) => {}
                _ => panic!("Segments mismatch"),
            }
        }
    }
}

/// An intermediate shard block inside a beacon chain body
struct TestIntermediateShardBlock {
    header: OwnedIntermediateShardHeader,
    own_segments: Option<(LocalSegmentIndex, Vec<SegmentRoot>)>,
    leaf_shards_segments: Vec<(ShardIndex, (LocalSegmentIndex, Vec<SegmentRoot>))>,
    segments_proof: [u8; 32],
}

impl TestIntermediateShardBlock {
    fn new(
        shard_index: u32,
        number: u64,
        num_own_segments: u8,
        leaf_shards_segments: &[(u32, u8)],
    ) -> Self {
        let segments_proof = [u8::try_from(number).unwrap(); _];
        let own_segments = (num_own_segments > 0).then(|| segments(number, num_own_segments));
        let leaf_shards_segments = leaf_shards_segments
            .iter()
            .map(|&(leaf_shard_index, num_segments)| {
                (
                    ShardIndex::new(leaf_shard_index).unwrap(),
                    segments(number, num_segments),
                )
            })
            .collect::<Vec<_>>();
        // The body root that the segments proof belongs to
        let body_root = if own_segments.is_none() && leaf_shards_segments.is_empty() {
            Blake3Hash::default()
        } else {
            let own_segments_root = own_segments.as_ref().map_or_else(
                Blake3Hash::default,
                |(first_local_segment_index, segment_roots)| {
                    OwnSegments {
                        first_local_segment_index: *first_local_segment_index,
                        segment_roots,
                    }
                    .root()
                },
            );
            let leaf_shards_root =
                UnbalancedMerkleTree::compute_root_only::<{ u64::from(u16::MAX) }, _, _>(
                    leaf_shards_segments.iter().map(
                        |(leaf_shard_index, (first_local_segment_index, segment_roots))| {
                            OwnSegments {
                                first_local_segment_index: *first_local_segment_index,
                                segment_roots,
                            }
                            .root_with_shard_index(*leaf_shard_index)
                        },
                    ),
                )
                .unwrap_or_default();
            Blake3Hash::new(BalancedMerkleTree::compute_root_only(&[
                BalancedMerkleTree::compute_root_only(&[*own_segments_root, leaf_shards_root]),
                segments_proof,
            ]))
        };

        Self {
            header: intermediate_shard_header(shard_index, number, body_root),
            own_segments,
            leaf_shards_segments,
            segments_proof,
        }
    }
}

/// Bytes of a beacon chain body with intermediate shard blocks, following the layout described in
/// `IntermediateShardBlocksInfo::try_from_bytes()`
fn beacon_chain_body_bytes(
    intermediate_shard_blocks: &[TestIntermediateShardBlock],
    pot_checkpoints: &[PotCheckpoints],
) -> SharedAlignedBuffer {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&u32::try_from(pot_checkpoints.len()).unwrap().to_le_bytes());
    // No own segments
    bytes.push(0);

    bytes.extend_from_slice(
        &u16::try_from(intermediate_shard_blocks.len())
            .unwrap()
            .to_le_bytes(),
    );
    for block in intermediate_shard_blocks {
        bytes.push(block.own_segments.as_ref().map_or(
            0,
            |(_first_local_segment_index, own_segment_roots)| {
                u8::try_from(own_segment_roots.len()).unwrap()
            },
        ));
        bytes.extend_from_slice(
            &u16::try_from(block.leaf_shards_segments.len())
                .unwrap()
                .to_le_bytes(),
        );
    }
    pad_to(&mut bytes, 8);
    for block in intermediate_shard_blocks {
        bytes.extend_from_slice(block.header.buffer().as_slice());
        if block.own_segments.is_some() || !block.leaf_shards_segments.is_empty() {
            bytes.extend_from_slice(&block.segments_proof);
        }
        if let Some(own_segments) = &block.own_segments {
            append_segments(&mut bytes, own_segments);
        }
        for (_leaf_shard_index, (_first_local_segment_index, segment_roots)) in
            &block.leaf_shards_segments
        {
            bytes.push(u8::try_from(segment_roots.len()).unwrap());
        }
        for (leaf_shard_index, segments) in &block.leaf_shards_segments {
            bytes.extend_from_slice(&u32::from(*leaf_shard_index).to_le_bytes());
            append_segments(&mut bytes, segments);
        }
        pad_to(&mut bytes, 8);
    }

    for pot_checkpoints in pot_checkpoints {
        for pot_output in pot_checkpoints.iter() {
            bytes.extend_from_slice(pot_output.as_ref());
        }
    }

    SharedAlignedBuffer::from_bytes(&bytes)
}

#[test]
fn beacon_chain_body_with_intermediate_shard_blocks() {
    // More leaf shard blocks with segments than fit into `u8`
    let many_leaf_shards_segments = (1..=256)
        .map(|leaf_shard| (3 | (leaf_shard << 10), 1))
        .collect::<Vec<_>>();
    // A few blocks of the first three intermediate shards with and without segments
    let intermediate_shard_blocks = [
        TestIntermediateShardBlock::new(1, 0, 0, &[]),
        TestIntermediateShardBlock::new(1, 1, 1, &[(1 | (1 << 10), 2), (1 | (2 << 10), 1)]),
        TestIntermediateShardBlock::new(2, 0, 0, &[(2 | (1 << 10), 1)]),
        TestIntermediateShardBlock::new(2, 1, 2, &[]),
        TestIntermediateShardBlock::new(3, 0, 0, &many_leaf_shards_segments),
    ];
    let pot_checkpoints = [PotCheckpoints::default()];

    // Different numbers of blocks need different padding after the numbers of segment roots,
    // there is none for three blocks
    for num_blocks in 1..=intermediate_shard_blocks.len() {
        let intermediate_shard_blocks = &intermediate_shard_blocks[..num_blocks];
        let bytes = beacon_chain_body_bytes(intermediate_shard_blocks, &pot_checkpoints);

        let (body, remainder) = BeaconChainBody::try_from_bytes(bytes.as_slice()).unwrap();
        assert!(remainder.is_empty());
        assert!(body.own_segments().is_none());
        assert_eq!(body.pot_checkpoints(), pot_checkpoints);
        assert_eq!(body.intermediate_shard_blocks().len(), num_blocks);
        for (info, block) in body
            .intermediate_shard_blocks()
            .iter()
            .zip(intermediate_shard_blocks)
        {
            assert_eq!(*info.header.root(), *block.header.header().root());
            match (&info.own_segments, &block.own_segments) {
                (Some(info_own_segments), Some(own_segments)) => {
                    assert_segments_eq(info_own_segments, own_segments);
                }
                (None, None) => {}
                _ => panic!("Own segments mismatch"),
            }
            assert_eq!(
                info.segments_proof.copied(),
                (block.own_segments.is_some() || !block.leaf_shards_segments.is_empty())
                    .then_some(block.segments_proof)
            );
            assert_eq!(
                info.leaf_shards_segments().len(),
                block.leaf_shards_segments.len()
            );
            for ((info_shard_index, info_segments), (shard_index, segments)) in
                info.leaf_shards_segments().zip(&block.leaf_shards_segments)
            {
                assert_eq!(info_shard_index, *shard_index);
                assert_segments_eq(&info_segments, segments);
            }
        }
    }
}
