use ab_client_api::{
    BeaconChainInfo, BlockDetails, ChainInfo, ReadBlockError, ShardSegmentRoot,
    ShardSegmentRootsError,
};
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::segments::{
    LocalSegmentIndex, SegmentHeader, SegmentIndex, SuperSegmentHeader, SuperSegmentIndex,
    SuperSegmentRoot,
};
use std::collections::HashMap;
use triomphe::Arc;

#[derive(Debug, Clone)]
struct MockBlock {
    number: BlockNumber,
    parent_root: BlockRoot,
    shard_segment_roots: Arc<[ShardSegmentRoot]>,
}

/// Mock of [`BeaconChainInfo`] with explicit forks: a tree of blocks, one of which is the best
/// block.
///
/// Like the database, it returns shard segment roots of blocks on the best chain by block number.
/// Methods that are not needed by tests are not implemented.
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
        _ancestor_block_number: BlockNumber,
        _descendant_block_root: &BlockRoot,
    ) -> Option<OwnedBeaconChainHeader> {
        unimplemented!()
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
            && block.number >= block_number
        {
            if block.number == block_number {
                return Ok(Arc::clone(&block.shard_segment_roots));
            }
            block_root = block.parent_root;
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
        let genesis_root = BlockRoot::default();
        let genesis_block = MockBlock {
            number: BlockNumber::ZERO,
            parent_root: BlockRoot::default(),
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
            .number;
        let mut root_bytes = [0; _];
        root_bytes[..8].copy_from_slice(&u64::try_from(self.blocks.len()).unwrap().to_le_bytes());
        let block_root = BlockRoot::new(Blake3Hash::new(root_bytes));

        self.blocks.insert(
            block_root,
            MockBlock {
                number: parent_number + BlockNumber::ONE,
                parent_root,
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
}
