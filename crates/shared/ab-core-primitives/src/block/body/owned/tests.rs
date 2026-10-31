use crate::address::Address;
use crate::block::body::owned::{
    GenericOwnedBlockBody, OwnedBeaconChainBody, OwnedBeaconChainBodyError,
    OwnedIntermediateShardBody, OwnedIntermediateShardBodyError, OwnedLeafShardBlockBodyBuilder,
    OwnedLeafShardBody, OwnedLeafShardBodyError,
};
use crate::block::body::{
    BeaconChainBody, BlockBody, IntermediateShardBlockInfo, IntermediateShardBlocksInfo,
    IntermediateShardBody, LeafShardBlockInfo, LeafShardBlocksInfo, LeafShardBody,
    LeafShardOwnSegments, OwnSegments,
};
use crate::block::header::owned::{
    OwnedBeaconChainHeader, OwnedIntermediateShardHeader, OwnedLeafShardHeader,
};
use crate::block::header::{
    BlockHeaderBeaconChainInfo, BlockHeaderConsensusInfo, BlockHeaderConsensusParameters,
    BlockHeaderEd25519Seal, BlockHeaderFixedConsensusParameters, BlockHeaderPrefix,
    BlockHeaderResult, BlockHeaderSeal,
};
use crate::block::owned::OwnedBeaconChainBlock;
use crate::block::{BeaconChainBlock, BlockNumber, BlockRoot, BlockTimestamp};
use crate::ed25519::{Ed25519PublicKey, Ed25519Signature};
use crate::hashes::Blake3Hash;
use crate::pot::{PotCheckpoints, PotOutput, SlotNumber};
use crate::segments::{LocalSegmentIndex, SegmentRoot};
use crate::shard::{NumShards, RealShardKind, ShardIndex};
use crate::solutions::{Solution, SolutionRange};
use crate::transaction::owned::OwnedTransaction;
use crate::transaction::{Gas, TransactionHeader, TransactionSlot};
use crate::trees::{
    IntermediateShardBodyMerkleTree, LeafShardBodyMerkleTree, LeafShardsSegmentsMerkleTree,
};
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use core::num::{NonZeroU16, NonZeroU32};
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
        &Blake3Hash::default(),
    )
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

/// A beacon chain block is only consistent when the root of intermediate shard block roots in its
/// header matches intermediate shard blocks in its body
#[test]
fn beacon_chain_blocks_with_inconsistent_intermediate_shard_blocks_root_are_rejected() {
    fn intermediate_shard_blocks_root(
        intermediate_shard_blocks: &[TestIntermediateShardBlock],
    ) -> Blake3Hash {
        let bytes = beacon_chain_body_bytes(intermediate_shard_blocks, &[]);
        let (body, _remainder) = BeaconChainBody::try_from_bytes(bytes.as_slice()).unwrap();
        body.intermediate_shard_blocks().headers_root()
    }

    fn is_consistent(
        body: &SharedAlignedBuffer,
        intermediate_shard_blocks_root: &Blake3Hash,
    ) -> bool {
        let (parsed_body, _remainder) = BeaconChainBody::try_from_bytes(body.as_slice()).unwrap();
        let header = OwnedBeaconChainHeader::from_parts(
            &header_prefix(u32::from(ShardIndex::BEACON_CHAIN), 0),
            &header_result(parsed_body.root()),
            &consensus_info(),
            intermediate_shard_blocks_root,
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
        .with_seal(BlockHeaderSeal::Ed25519(&seal()));
        let mut bytes = header.buffer().as_slice().to_vec();
        // The body starts at a 16-byte boundary
        pad_to(&mut bytes, 16);
        bytes.extend_from_slice(body.as_slice());
        let bytes = SharedAlignedBuffer::from_bytes(&bytes);

        // Child block roots are checked at the block level only
        assert!(BeaconChainBlock::try_from_bytes_unchecked(bytes.as_slice()).is_some());
        let consistent = BeaconChainBlock::try_from_bytes(bytes.as_slice()).is_some();
        assert_eq!(
            OwnedBeaconChainBlock::from_buffers(header.buffer().clone(), body.clone()).is_some(),
            consistent
        );
        consistent
    }

    let intermediate_shard_blocks = (0..3)
        .map(|number| TestIntermediateShardBlock::new(1, number, 0, &[]))
        .collect::<Vec<_>>();
    let other_root = intermediate_shard_blocks_root(&intermediate_shard_blocks[1..]);

    for num_blocks in [1, 3] {
        let intermediate_shard_blocks = &intermediate_shard_blocks[..num_blocks];
        let body = beacon_chain_body_bytes(intermediate_shard_blocks, &[]);
        assert!(is_consistent(
            &body,
            &intermediate_shard_blocks_root(intermediate_shard_blocks)
        ));

        for wrong_root in [
            Blake3Hash::new([1; _]),
            // The root without intermediate shard blocks
            Blake3Hash::default(),
            other_root,
            // The root of the first intermediate shard block
            **intermediate_shard_blocks[0].header.header().root(),
        ] {
            assert!(!is_consistent(&body, &wrong_root));
        }
    }
}

#[test]
fn beacon_chain_body_with_too_many_pot_checkpoints() {
    // Their size is 4 GiB, which doesn't fit into `usize` on 32-bit hosts
    let num_pot_checkpoints =
        u32::try_from((1u64 << u32::BITS) / u64::try_from(PotCheckpoints::SIZE).unwrap()).unwrap();
    let mut bytes = num_pot_checkpoints.to_le_bytes().to_vec();
    // No own segments and no intermediate shard blocks, followed by padding
    bytes.extend_from_slice(&[0; 4]);
    let bytes = SharedAlignedBuffer::from_bytes(&bytes);

    assert!(BeaconChainBody::try_from_bytes(bytes.as_slice()).is_none());
    assert!(BeaconChainBody::try_from_bytes_unchecked(bytes.as_slice()).is_none());
}

/// A leaf shard block inside an intermediate shard body
struct TestLeafShardBlock {
    header: OwnedLeafShardHeader,
    segments: Option<(LocalSegmentIndex, Vec<SegmentRoot>)>,
    segment_roots_proof: [u8; 32],
}

impl TestLeafShardBlock {
    fn new(shard_index: u32, number: u64, num_segments: u8) -> Self {
        let segments = (num_segments > 0).then(|| segments(number, num_segments));

        Self::with_segments(shard_index, number, segments)
    }

    fn with_segments(
        shard_index: u32,
        number: u64,
        segments: Option<(LocalSegmentIndex, Vec<SegmentRoot>)>,
    ) -> Self {
        let segment_roots_proof = [u8::try_from(number).unwrap(); _];
        // The body root that the segment roots proof belongs to
        let body_root = segments.as_ref().map_or_else(
            Blake3Hash::default,
            |(first_local_segment_index, segment_roots)| {
                // Without checks, so that tests can compute roots of segments that parsers must
                // reject
                let own_segments =
                    OwnSegments::new_unchecked(*first_local_segment_index, segment_roots);
                Blake3Hash::new(LeafShardBodyMerkleTree::compute_root_only(&[
                    *own_segments.root(),
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

        // The builder writes the same bytes
        assert_eq!(body.to_owned().buffer().as_slice(), bytes.as_slice());
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

        Self::with_segments(shard_index, number, own_segments, leaf_shards_segments)
    }

    fn with_segments(
        shard_index: u32,
        number: u64,
        own_segments: Option<(LocalSegmentIndex, Vec<SegmentRoot>)>,
        leaf_shards_segments: Vec<(ShardIndex, (LocalSegmentIndex, Vec<SegmentRoot>))>,
    ) -> Self {
        let segments_proof = [u8::try_from(number).unwrap(); _];
        // The body root that the segments proof belongs to
        let body_root = if own_segments.is_none() && leaf_shards_segments.is_empty() {
            Blake3Hash::default()
        } else {
            // Without checks, so that tests can compute roots of segments that parsers must reject
            let own_segments_root = own_segments.as_ref().map_or_else(
                Blake3Hash::default,
                |(first_local_segment_index, segment_roots)| {
                    OwnSegments::new_unchecked(*first_local_segment_index, segment_roots).root()
                },
            );
            let leaf_shards_roots = leaf_shards_segments.iter().map(
                |(leaf_shard_index, (first_local_segment_index, segment_roots))| {
                    OwnSegments::new_unchecked(*first_local_segment_index, segment_roots)
                        .root_with_shard_index(*leaf_shard_index)
                },
            );
            let leaf_shards_root =
                LeafShardsSegmentsMerkleTree::compute_root_only::<{ u64::from(u16::MAX) }, _, _>(
                    leaf_shards_roots,
                )
                .unwrap_or_default();
            Blake3Hash::new(IntermediateShardBodyMerkleTree::compute_root_only_array(&[
                *own_segments_root,
                leaf_shards_root,
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

/// Parse `intermediate_shard_blocks` and check that the body has the same contents and that the
/// builder writes the same bytes
fn check_beacon_chain_body_with_intermediate_shard_blocks(
    intermediate_shard_blocks: &[TestIntermediateShardBlock],
    pot_checkpoints: &[PotCheckpoints],
) {
    let bytes = beacon_chain_body_bytes(intermediate_shard_blocks, pot_checkpoints);

    let (body, remainder) = BeaconChainBody::try_from_bytes(bytes.as_slice()).unwrap();
    assert!(remainder.is_empty());
    assert!(body.own_segments().is_none());
    assert_eq!(body.pot_checkpoints(), pot_checkpoints);
    assert_eq!(
        body.intermediate_shard_blocks().len(),
        intermediate_shard_blocks.len()
    );
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

    assert_eq!(body.to_owned().buffer().as_slice(), bytes.as_slice());
}

#[test]
fn beacon_chain_body_with_intermediate_shard_blocks() {
    // A few blocks of the first three intermediate shards with and without segments
    let intermediate_shard_blocks = [
        TestIntermediateShardBlock::new(1, 0, 0, &[]),
        TestIntermediateShardBlock::new(1, 1, 1, &[(1 | (1 << 10), 2), (1 | (2 << 10), 1)]),
        TestIntermediateShardBlock::new(2, 0, 0, &[(2 | (1 << 10), 1)]),
        TestIntermediateShardBlock::new(2, 1, 2, &[]),
        TestIntermediateShardBlock::new(3, 0, 0, &[(3 | (1 << 10), 1)]),
    ];
    let pot_checkpoints = [PotCheckpoints::default()];

    // Different numbers of blocks need different padding after the numbers of segment roots,
    // there is none for three blocks
    for num_blocks in 1..=intermediate_shard_blocks.len() {
        check_beacon_chain_body_with_intermediate_shard_blocks(
            &intermediate_shard_blocks[..num_blocks],
            &pot_checkpoints,
        );
    }
}

#[test]
fn beacon_chain_body_with_many_leaf_shards_segments() {
    // More leaf shard blocks with segments than fit into `u8`
    let many_leaf_shards_segments = (1..=256)
        .map(|leaf_shard| (3 | (leaf_shard << 10), 1))
        .collect::<Vec<_>>();
    let intermediate_shard_blocks = [TestIntermediateShardBlock::new(
        3,
        0,
        0,
        &many_leaf_shards_segments,
    )];

    check_beacon_chain_body_with_intermediate_shard_blocks(
        &intermediate_shard_blocks,
        &[PotCheckpoints::default()],
    );
}

#[test]
fn owned_intermediate_shard_body_with_inconsistent_leaf_shard_blocks() {
    let leaf_shard_blocks = [
        TestLeafShardBlock::new(1 | (1 << 10), 0, 1),
        TestLeafShardBlock::new(1 | (1 << 10), 1, 1),
        TestLeafShardBlock::new(1 | (2 << 10), 0, 1),
    ];
    let new_body = |leaf_shard_blocks: Vec<LeafShardBlockInfo<'_>>| {
        OwnedIntermediateShardBody::new(iter::empty(), leaf_shard_blocks.into_iter())
    };

    // A block created from parts, with its segment roots proof or with a different one
    let block = &leaf_shard_blocks[0];
    let (first_local_segment_index, segment_roots) = block.segments.as_ref().unwrap();
    let block_info = |segment_roots_proof| LeafShardBlockInfo {
        header: block.header.header().clone(),
        segments: Some(LeafShardOwnSegments {
            segment_roots_proof,
            own_segments: OwnSegments::new(*first_local_segment_index, segment_roots).unwrap(),
        }),
    };
    new_body(vec![block_info(&block.segment_roots_proof)]).unwrap();
    assert!(matches!(
        new_body(vec![block_info(&[0xff; _])]),
        Err(OwnedIntermediateShardBodyError::InconsistentLeafShardBlocks)
    ));

    // Blocks of a parsed body, which are in order there
    let bytes = intermediate_shard_body_bytes(&segments(3, 1), &leaf_shard_blocks);
    let (body, _remainder) = IntermediateShardBody::try_from_bytes(bytes.as_slice()).unwrap();
    let infos = body.leaf_shard_blocks().iter().collect::<Vec<_>>();
    new_body(infos.clone()).unwrap();
    // Block numbers of the same shard and then shard indices out of order
    for (first, second) in [(1, 0), (2, 0)] {
        assert!(matches!(
            new_body(vec![infos[first].clone(), infos[second].clone()]),
            Err(OwnedIntermediateShardBodyError::InconsistentLeafShardBlocks)
        ));
    }
}

#[test]
fn owned_beacon_chain_body_with_inconsistent_intermediate_shard_blocks() {
    let intermediate_shard_blocks = [
        TestIntermediateShardBlock::new(1, 0, 1, &[]),
        TestIntermediateShardBlock::new(1, 1, 1, &[(1 | (1 << 10), 1)]),
        TestIntermediateShardBlock::new(2, 0, 1, &[]),
    ];
    let bytes = beacon_chain_body_bytes(&intermediate_shard_blocks, &[]);
    let (body, _remainder) = BeaconChainBody::try_from_bytes(bytes.as_slice()).unwrap();
    let infos = body.intermediate_shard_blocks().iter().collect::<Vec<_>>();
    let new_body = |intermediate_shard_blocks: Vec<IntermediateShardBlockInfo<'_>>| {
        OwnedBeaconChainBody::new(iter::empty(), intermediate_shard_blocks.into_iter(), &[])
    };
    new_body(infos.clone()).unwrap();

    // A segments proof that doesn't match the body root in the header
    let mut info = infos[1].clone();
    info.segments_proof = Some(&[0xff; _]);
    assert!(matches!(
        new_body(vec![info]),
        Err(OwnedBeaconChainBodyError::InconsistentIntermediateShardBlocks)
    ));

    // A segments proof without segments
    let mut info = infos[0].clone();
    info.own_segments = None;
    assert!(matches!(
        new_body(vec![info]),
        Err(OwnedBeaconChainBodyError::InconsistentIntermediateShardBlocks)
    ));

    // Block numbers of the same shard and then shard indices out of order
    for (first, second) in [(1, 0), (2, 0)] {
        assert!(matches!(
            new_body(vec![infos[first].clone(), infos[second].clone()]),
            Err(OwnedBeaconChainBodyError::InconsistentIntermediateShardBlocks)
        ));
    }
}

#[test]
fn owned_beacon_chain_body_with_segments_but_without_segments_proof() {
    // A block with segments of a leaf shard block with one segment root. Its segments proof is its
    // number repeated, which the parser below reads as the number of segment roots.
    let block = TestIntermediateShardBlock::new(1, 2, 0, &[(1 | (1 << 10), 1)]);
    let bytes = beacon_chain_body_bytes(&[block], &[]);
    let (body, _remainder) = BeaconChainBody::try_from_bytes(bytes.as_slice()).unwrap();
    let info = body.intermediate_shard_blocks().iter().next().unwrap();
    let segments_proof_offset =
        ptr::from_ref(info.segments_proof.unwrap()).addr() - bytes.as_ptr().addr();

    // With 32 more bytes after the header, the parser reads them as the segments proof, and the
    // original proof and segments as segments of a leaf shard block with 2 segment roots. The body
    // root in the header doesn't match them, so only the parser without checks accepts it.
    let mut longer_bytes = bytes.as_slice()[..segments_proof_offset].to_vec();
    longer_bytes.extend_from_slice(&[0; 32]);
    longer_bytes.extend_from_slice(&bytes.as_slice()[segments_proof_offset..]);
    let longer_bytes = SharedAlignedBuffer::from_bytes(&longer_bytes);
    let (longer_body, _remainder) =
        BeaconChainBody::try_from_bytes_unchecked(longer_bytes.as_slice()).unwrap();
    let mut info = longer_body
        .intermediate_shard_blocks()
        .iter()
        .next()
        .unwrap();
    let (_leaf_shard_index, leaf_shard_segments) = info.leaf_shards_segments().next().unwrap();
    assert_eq!(leaf_shard_segments.segment_roots.len(), 2);

    // Without a segments proof, this block would be written exactly as the original bytes, which
    // the parser accepts as the original block
    info.segments_proof = None;
    assert!(matches!(
        OwnedBeaconChainBody::new(iter::empty(), iter::once(info), &[]),
        Err(OwnedBeaconChainBodyError::InconsistentIntermediateShardBlocks)
    ));
}

#[test]
fn beacon_chain_body_with_leaf_shard_segments_without_segment_roots() {
    // Segments of a leaf shard block with one segment root and with none
    for (num_segment_roots, valid) in [(1, true), (0, false)] {
        let block = TestIntermediateShardBlock::new(1, 0, 0, &[(1 | (1 << 10), num_segment_roots)]);
        let bytes = beacon_chain_body_bytes(&[block], &[]);
        assert_eq!(
            BeaconChainBody::try_from_bytes(bytes.as_slice()).is_some(),
            valid
        );
        assert_eq!(
            BeaconChainBody::try_from_bytes_unchecked(bytes.as_slice()).is_some(),
            valid
        );
    }
}

/// Two segment roots that start at the largest index for which the index after the last root
/// still fits into `u64`, and two that start one index later, each with whether they are valid
fn segments_near_u64_max() -> [((LocalSegmentIndex, Vec<SegmentRoot>), bool); 2] {
    [
        (segments(u64::MAX - 2, 2), true),
        (segments(u64::MAX - 1, 2), false),
    ]
}

#[test]
fn own_segments_constructor_checks_invariants() {
    // Between 1 and `u8::MAX` segment roots
    let segment_roots = [SegmentRoot::default(); 256];
    assert!(OwnSegments::new(LocalSegmentIndex::ZERO, &[]).is_none());
    assert!(OwnSegments::new(LocalSegmentIndex::ZERO, &segment_roots).is_none());
    for num_segment_roots in [1, u8::MAX] {
        let own_segments = OwnSegments::new(
            LocalSegmentIndex::ZERO,
            &segment_roots[..usize::from(num_segment_roots)],
        )
        .unwrap();
        assert_eq!(own_segments.num_segment_roots().get(), num_segment_roots);
    }

    // The local segment index after the last segment root fits into `u64` or doesn't
    for ((first_local_segment_index, segment_roots), valid) in segments_near_u64_max() {
        let own_segments = OwnSegments::new(first_local_segment_index, &segment_roots);
        assert_eq!(own_segments.is_some(), valid);
        if let Some(own_segments) = own_segments {
            // The last segment root is right before `u64::MAX`, which the iterator doesn't need to
            // compute
            assert_eq!(
                own_segments.iter().collect::<Vec<_>>(),
                [
                    (LocalSegmentIndex::from(u64::MAX - 2), &segment_roots[0]),
                    (LocalSegmentIndex::from(u64::MAX - 1), &segment_roots[1]),
                ]
            );
        }
    }
}

#[test]
fn own_segments_with_local_segment_indices_near_u64_max() {
    // Parsers without checks reject the same own segments, `OwnSegments::iter()` relies on it
    for (own_segments, valid) in segments_near_u64_max() {
        // No PoT checkpoints and no intermediate shard blocks
        let mut bytes = 0u32.to_le_bytes().to_vec();
        bytes.push(2);
        append_segments(&mut bytes, &own_segments);
        bytes.extend_from_slice(&0u16.to_le_bytes());
        pad_to(&mut bytes, 8);
        let bytes = SharedAlignedBuffer::from_bytes(&bytes);
        let body = BeaconChainBody::try_from_bytes(bytes.as_slice());
        assert_eq!(body.is_some(), valid);
        if let Some((body, _remainder)) = body {
            assert_eq!(body.to_owned().buffer().as_slice(), bytes.as_slice());
        }
        assert_eq!(
            BeaconChainBody::try_from_bytes_unchecked(bytes.as_slice()).is_some(),
            valid
        );

        let bytes = intermediate_shard_body_bytes(&own_segments, &[]);
        let body = IntermediateShardBody::try_from_bytes(bytes.as_slice());
        assert_eq!(body.is_some(), valid);
        if let Some((body, _remainder)) = body {
            assert_eq!(body.to_owned().buffer().as_slice(), bytes.as_slice());
        }
        assert_eq!(
            IntermediateShardBody::try_from_bytes_unchecked(bytes.as_slice()).is_some(),
            valid
        );

        // No transactions
        let mut bytes = vec![2];
        append_segments(&mut bytes, &own_segments);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        let bytes = SharedAlignedBuffer::from_bytes(&bytes);
        let body = LeafShardBody::try_from_bytes(bytes.as_slice());
        assert_eq!(body.is_some(), valid);
        if let Some((body, _remainder)) = body {
            assert_eq!(body.to_owned().buffer().as_slice(), bytes.as_slice());
        }
        assert_eq!(
            BlockBody::try_from_bytes_unchecked(bytes.as_slice(), RealShardKind::LeafShard)
                .is_some(),
            valid
        );
    }
}

#[test]
fn leaf_shard_blocks_with_local_segment_indices_near_u64_max() {
    for (leaf_shard_segments, valid) in segments_near_u64_max() {
        let leaf_shard_block =
            TestLeafShardBlock::with_segments(1 | (1 << 10), 0, Some(leaf_shard_segments));
        let bytes = intermediate_shard_body_bytes(&segments(3, 1), &[leaf_shard_block]);
        let body = IntermediateShardBody::try_from_bytes(bytes.as_slice());
        assert_eq!(body.is_some(), valid);
        if let Some((body, _remainder)) = body {
            assert_eq!(body.to_owned().buffer().as_slice(), bytes.as_slice());
        }
        // Parsers without checks reject the same blocks, `OwnSegments::iter()` relies on it
        assert_eq!(
            IntermediateShardBody::try_from_bytes_unchecked(bytes.as_slice()).is_some(),
            valid
        );

        // Leaf shard blocks follow the number of own segment roots, the local segment index of the
        // first one and the only root
        let leaf_shard_blocks_bytes = &bytes.as_slice()
            [size_of::<u8>() + size_of::<LocalSegmentIndex>() + SegmentRoot::SIZE..];
        assert_eq!(
            LeafShardBlocksInfo::try_from_bytes(leaf_shard_blocks_bytes).is_some(),
            valid
        );
        assert_eq!(
            LeafShardBlocksInfo::try_from_bytes_unchecked(leaf_shard_blocks_bytes).is_some(),
            valid
        );
    }
}

#[test]
fn intermediate_shard_blocks_with_local_segment_indices_near_u64_max() {
    let leaf_shard_index = ShardIndex::new(1 | (1 << 10)).unwrap();
    for (near_max_segments, valid) in segments_near_u64_max() {
        // Own segments and segments of a leaf shard
        for intermediate_shard_block in [
            TestIntermediateShardBlock::with_segments(
                1,
                0,
                Some(near_max_segments.clone()),
                Vec::new(),
            ),
            TestIntermediateShardBlock::with_segments(
                1,
                0,
                None,
                vec![(leaf_shard_index, near_max_segments.clone())],
            ),
        ] {
            let bytes = beacon_chain_body_bytes(&[intermediate_shard_block], &[]);
            let body = BeaconChainBody::try_from_bytes(bytes.as_slice());
            assert_eq!(body.is_some(), valid);
            if let Some((body, _remainder)) = body {
                assert_eq!(body.to_owned().buffer().as_slice(), bytes.as_slice());
            }
            // Parsers without checks reject the same blocks, `OwnSegments::iter()` relies on it
            assert_eq!(
                BeaconChainBody::try_from_bytes_unchecked(bytes.as_slice()).is_some(),
                valid
            );

            // Intermediate shard blocks follow the number of PoT checkpoints and the zero number
            // of own segment roots
            let intermediate_shard_blocks_bytes =
                &bytes.as_slice()[size_of::<u32>() + size_of::<u8>()..];
            assert_eq!(
                IntermediateShardBlocksInfo::try_from_bytes(intermediate_shard_blocks_bytes)
                    .is_some(),
                valid
            );
            assert_eq!(
                IntermediateShardBlocksInfo::try_from_bytes_unchecked(
                    intermediate_shard_blocks_bytes
                )
                .is_some(),
                valid
            );
        }
    }
}

#[test]
fn owned_bodies_with_local_segment_indices_near_u64_max() {
    for ((first_local_segment_index, segment_roots), valid) in segments_near_u64_max() {
        // Every index fits into `u64` here, only the index after the last root may not
        let own_segments = || {
            segment_roots
                .iter()
                .enumerate()
                .map(|(position, &segment_root)| {
                    let position = LocalSegmentIndex::from(u64::try_from(position).unwrap());
                    (first_local_segment_index + position, segment_root)
                })
        };

        let beacon_chain_body = OwnedBeaconChainBody::new(
            own_segments(),
            iter::empty::<IntermediateShardBlockInfo<'_>>(),
            &[],
        );
        let intermediate_shard_body = OwnedIntermediateShardBody::new(
            own_segments(),
            iter::empty::<LeafShardBlockInfo<'_>>(),
        );
        let leaf_shard_body =
            OwnedLeafShardBody::init(own_segments()).map(OwnedLeafShardBlockBodyBuilder::finish);

        if valid {
            let own_segments = (first_local_segment_index, segment_roots.clone());
            assert_segments_eq(
                &beacon_chain_body.unwrap().body().own_segments().unwrap(),
                &own_segments,
            );
            assert_segments_eq(
                &intermediate_shard_body
                    .unwrap()
                    .body()
                    .own_segments()
                    .unwrap(),
                &own_segments,
            );
            assert_segments_eq(
                &leaf_shard_body.unwrap().body().own_segments().unwrap(),
                &own_segments,
            );
        } else {
            assert!(matches!(
                beacon_chain_body,
                Err(OwnedBeaconChainBodyError::OwnSegmentIndexOverflow {
                    first_local_segment_index: index,
                }) if index == first_local_segment_index
            ));
            assert!(matches!(
                intermediate_shard_body,
                Err(OwnedIntermediateShardBodyError::OwnSegmentIndexOverflow {
                    first_local_segment_index: index,
                }) if index == first_local_segment_index
            ));
            assert!(matches!(
                leaf_shard_body,
                Err(OwnedLeafShardBodyError::OwnSegmentIndexOverflow {
                    first_local_segment_index: index,
                }) if index == first_local_segment_index
            ));
        }
    }
}
