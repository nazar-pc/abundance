use crate::{BlockProducer, ClaimedSlot};
use ab_client_api::{BlockOrigin, ChainInfo};
use ab_client_block_builder::BlockBuilder;
use ab_client_block_import::BlockImport;
use ab_core_primitives::block::header::{BeaconChainHeader, OwnedBlockHeaderSeal};
use ab_core_primitives::block::owned::{GenericOwnedBlock, OwnedBeaconChainBlock};
use ab_core_primitives::hashes::Blake3Hash;
use tracing::{error, info};

/// Beacon chain block producer
#[derive(Debug)]
pub struct BeaconChainBlockProducer<BB, BI, CI> {
    block_builder: BB,
    block_import: BI,
    chain_info: CI,
}

impl<BB, BI, CI> BlockProducer for BeaconChainBlockProducer<BB, BI, CI>
where
    BB: BlockBuilder<OwnedBeaconChainBlock>,
    BI: BlockImport<OwnedBeaconChainBlock>,
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    async fn produce_block<SealBlock>(
        &mut self,
        claimed_slot: ClaimedSlot,
        best_beacon_chain_header: &BeaconChainHeader<'_>,
        seal_block: SealBlock,
    ) where
        SealBlock: AsyncFnOnce<(Blake3Hash,), Output = Option<OwnedBlockHeaderSeal>, CallOnceFuture: Send>
            + Send,
    {
        let slot = claimed_slot.consensus_info.slot;

        let best_header = best_beacon_chain_header;
        let parent_block_root = &*best_header.root();
        let (_, best_block_details) = self
            .chain_info
            .header_with_details(parent_block_root)
            .expect("Best beacon chain block is never missing during block production; qed");

        let block_builder_result = match self
            .block_builder
            .build(
                parent_block_root,
                best_header,
                &best_block_details,
                &claimed_slot.consensus_info,
                &claimed_slot.checkpoints,
                seal_block,
            )
            .await
        {
            Ok(block_builder_result) => block_builder_result,
            Err(error) => {
                error!(%slot, %parent_block_root, %error, "Failed to build a block");
                return;
            }
        };

        let header = block_builder_result.block.header().header();
        info!(
            slot = %header.consensus_info.slot,
            number = %header.prefix.number,
            root = %&*header.root(),
            pre_seal_hash = %header.pre_seal_hash(),
            "🔖 Built new block",
        );

        let block_import_fut = match self.block_import.import(
            block_builder_result.block,
            BlockOrigin::LocalBlockBuilder {
                block_details: block_builder_result.block_details,
            },
        ) {
            Ok(block_import_fut) => block_import_fut,
            Err(error) => {
                error!(
                    best_root = %*best_header.root(),
                    %error,
                    "Failed to queue a newly produced block for import"
                );
                return;
            }
        };

        match block_import_fut.await {
            Ok(()) => {
                // Nothing else to do
            }
            Err(error) => {
                error!(
                    best_root = %*best_header.root(),
                    %error,
                    "Failed to import a newly produced block"
                );
            }
        }
    }
}

impl<BB, BI, CI> BeaconChainBlockProducer<BB, BI, CI>
where
    BB: BlockBuilder<OwnedBeaconChainBlock>,
    BI: BlockImport<OwnedBeaconChainBlock>,
    CI: ChainInfo<OwnedBeaconChainBlock>,
{
    /// Create a new instance
    pub fn new(block_builder: BB, block_import: BI, chain_info: CI) -> Self {
        Self {
            block_builder,
            block_import,
            chain_info,
        }
    }
}
