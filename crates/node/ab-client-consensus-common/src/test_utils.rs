use ab_client_api::{
    BeaconChainInfo, BlockDetails, ChainInfo, ReadBlockError, ShardSegmentRoot,
    ShardSegmentRootsError,
};
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::header::{
    BlockHeaderConsensusInfo, BlockHeaderConsensusParameters, BlockHeaderEd25519Seal,
    BlockHeaderFixedConsensusParameters, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot, BlockTimestamp};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotOutput, SlotNumber};
use ab_core_primitives::segments::{
    LocalSegmentIndex, SegmentHeader, SegmentIndex, SuperSegmentHeader, SuperSegmentIndex,
    SuperSegmentRoot,
};
use ab_core_primitives::shard::{NumShards, ShardIndex};
use ab_core_primitives::solutions::{Solution, SolutionRange};
use std::collections::HashMap;
use std::num::{NonZeroU16, NonZeroU32};
use triomphe::Arc;

/// Minimal beacon chain header, `proof_of_time` makes headers with the same number and parent
/// unique
fn beacon_chain_header(
    number: BlockNumber,
    parent_root: BlockRoot,
    proof_of_time: PotOutput,
) -> OwnedBeaconChainHeader {
    OwnedBeaconChainHeader::from_parts(
        &BlockHeaderPrefix {
            number,
            shard_index: ShardIndex::BEACON_CHAIN,
            padding_0: [0; _],
            timestamp: BlockTimestamp::default(),
            parent_root,
            mmr_root: Blake3Hash::default(),
        },
        &BlockHeaderResult {
            body_root: Blake3Hash::default(),
            state_root: Blake3Hash::default(),
        },
        &BlockHeaderConsensusInfo {
            slot: SlotNumber::ZERO,
            proof_of_time,
            future_proof_of_time: PotOutput::default(),
            solution: Solution::genesis_solution(),
        },
        &Blake3Hash::default(),
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
    .with_seal(BlockHeaderSeal::Ed25519(&BlockHeaderEd25519Seal {
        public_key: Ed25519PublicKey::default(),
        signature: Ed25519Signature::default(),
    }))
    .unwrap()
}

#[derive(Debug, Clone)]
struct MockBlock {
    header: OwnedBeaconChainHeader,
    shard_segment_roots: Arc<[ShardSegmentRoot]>,
}

/// Mock of [`BeaconChainInfo`] with explicit forks: a tree of blocks, one of which is the best
/// block.
///
/// Like the database, it returns shard segment roots of blocks on the best chain by block number.
/// Every block has a unique proof of time. Methods that are not needed by tests are not
/// implemented.
#[derive(Debug, Clone)]
pub(crate) struct MockBeaconChain {
    blocks: HashMap<BlockRoot, MockBlock>,
    best_root: BlockRoot,
}

impl ChainInfo<OwnedBeaconChainBlock> for MockBeaconChain {
    fn best_root(&self) -> BlockRoot {
        self.best_root
    }

    fn best_header(&self) -> OwnedBeaconChainHeader {
        unimplemented!()
    }

    fn best_header_with_details(&self) -> (OwnedBeaconChainHeader, BlockDetails) {
        unimplemented!()
    }

    fn ancestor_header(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<OwnedBeaconChainHeader> {
        let mut block_root = *descendant_block_root;
        while let Some(block) = self.blocks.get(&block_root)
            && block.header.header().prefix.number >= ancestor_block_number
        {
            if block.header.header().prefix.number == ancestor_block_number {
                return Some(block.header.clone());
            }
            block_root = block.header.header().prefix.parent_root;
        }

        None
    }

    fn header(&self, _block_root: &BlockRoot) -> Option<OwnedBeaconChainHeader> {
        unimplemented!()
    }

    fn header_with_details(
        &self,
        _block_root: &BlockRoot,
    ) -> Option<(OwnedBeaconChainHeader, BlockDetails)> {
        unimplemented!()
    }

    async fn block(
        &self,
        _block_root: &BlockRoot,
    ) -> Result<OwnedBeaconChainBlock, ReadBlockError> {
        unimplemented!()
    }

    fn last_segment_header(&self) -> Option<SegmentHeader> {
        unimplemented!()
    }

    fn get_segment_header(&self, _segment_index: LocalSegmentIndex) -> Option<SegmentHeader> {
        unimplemented!()
    }

    fn segment_headers_for_block(&self, _block_number: BlockNumber) -> Vec<SegmentHeader> {
        // No beacon chain segments
        Vec::new()
    }
}

impl BeaconChainInfo for MockBeaconChain {
    fn shard_segment_roots(
        &self,
        block_number: BlockNumber,
    ) -> Result<Arc<[ShardSegmentRoot]>, ShardSegmentRootsError> {
        let mut block_root = self.best_root;
        while let Some(block) = self.blocks.get(&block_root)
            && block.header.header().prefix.number >= block_number
        {
            if block.header.header().prefix.number == block_number {
                return Ok(Arc::clone(&block.shard_segment_roots));
            }
            block_root = block.header.header().prefix.parent_root;
        }

        Err(ShardSegmentRootsError::BlockMissing { block_number })
    }

    fn last_super_segment_header(&self) -> Option<SuperSegmentHeader> {
        unimplemented!()
    }

    fn previous_super_segment_header(
        &self,
        _block_number: BlockNumber,
    ) -> Option<SuperSegmentHeader> {
        // The same for all blocks, so only segment roots make a difference
        Some(SuperSegmentHeader {
            index: SuperSegmentIndex::ZERO.into(),
            root: SuperSegmentRoot::default(),
            prev_super_segment_header_hash: Blake3Hash::default(),
            max_segment_index: SegmentIndex::ZERO.into(),
            target_beacon_chain_block_number: BlockNumber::ZERO.into(),
            num_segments: 1,
        })
    }

    fn get_super_segment_header(
        &self,
        _super_segment_index: SuperSegmentIndex,
    ) -> Option<SuperSegmentHeader> {
        unimplemented!()
    }

    fn get_super_segment_header_for_segment_index(
        &self,
        _segment_index: SegmentIndex,
    ) -> Option<SuperSegmentHeader> {
        unimplemented!()
    }
}

impl MockBeaconChain {
    /// Create a chain with just the genesis block, which is the best block
    pub(crate) fn new() -> Self {
        let genesis_header = beacon_chain_header(
            BlockNumber::ZERO,
            BlockRoot::default(),
            PotOutput::default(),
        );
        let genesis_root = *genesis_header.header().root();
        let genesis_block = MockBlock {
            header: genesis_header,
            shard_segment_roots: Arc::from(Vec::new()),
        };

        Self {
            blocks: HashMap::from([(genesis_root, genesis_block)]),
            best_root: genesis_root,
        }
    }

    /// Add a child of `parent_root` that includes `shard_segment_roots` and return its root, the
    /// best block doesn't change
    pub(crate) fn add_block(
        &mut self,
        parent_root: BlockRoot,
        shard_segment_roots: Vec<ShardSegmentRoot>,
    ) -> BlockRoot {
        let parent_number = self
            .blocks
            .get(&parent_root)
            .expect("Parent must be added first")
            .header
            .header()
            .prefix
            .number;
        let mut proof_of_time = PotOutput::default();
        proof_of_time[..8]
            .copy_from_slice(&u64::try_from(self.blocks.len()).unwrap().to_le_bytes());
        let header =
            beacon_chain_header(parent_number + BlockNumber::ONE, parent_root, proof_of_time);
        let block_root = *header.header().root();

        self.blocks.insert(
            block_root,
            MockBlock {
                header,
                shard_segment_roots: Arc::from(shard_segment_roots),
            },
        );

        block_root
    }

    /// Make `block_root` the best block
    pub(crate) fn set_best(&mut self, block_root: BlockRoot) {
        assert!(self.blocks.contains_key(&block_root));
        self.best_root = block_root;
    }

    /// Proof of time of the block with `block_root`
    pub(crate) fn proof_of_time(&self, block_root: &BlockRoot) -> PotOutput {
        self.blocks
            .get(block_root)
            .expect("Block must be added first")
            .header
            .header()
            .consensus_info
            .proof_of_time
    }
}
