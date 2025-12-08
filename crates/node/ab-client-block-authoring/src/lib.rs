//! Block authoring implementation

#![feature(async_fn_traits, unboxed_closures)]

pub mod beacon_chain;
pub mod slot_worker;

use ab_core_primitives::block::header::{
    BeaconChainHeader, BlockHeaderConsensusInfo, OwnedBlockHeaderSeal,
};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::PotCheckpoints;

#[derive(Debug)]
pub struct ClaimedSlot {
    /// Consensus info for a block header
    pub consensus_info: BlockHeaderConsensusInfo,
    /// Proof of time checkpoints from after future proof of the parent beacon chain block to
    /// current block's future proof (inclusive) contained in the `consensus_info` field
    pub checkpoints: Vec<PotCheckpoints>,
}

/// Block builder interface
pub trait BlockProducer: Send {
    /// Produce (build and import) a new block for the claimed slot
    fn produce_block<SealBlock>(
        &mut self,
        claimed_slot: ClaimedSlot,
        best_beacon_chain_header: &BeaconChainHeader<'_>,
        seal_block: SealBlock,
    ) -> impl Future<Output = ()> + Send
    where
        SealBlock: AsyncFnOnce<(Blake3Hash,), Output = Option<OwnedBlockHeaderSeal>, CallOnceFuture: Send>
            + Send;
}
