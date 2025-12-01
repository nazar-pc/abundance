//! Client informer, which logs node state periodically

use ab_client_api::ChainInfo;
use ab_core_primitives::block::header::GenericBlockHeader;
use ab_core_primitives::block::header::owned::GenericOwnedBlockHeader;
use ab_core_primitives::block::owned::GenericOwnedBlock;
use ab_core_primitives::shard::ShardKind;
use std::time::Duration;
use tracing::info;

pub async fn run_informer<Block, CI>(chain_info: &CI, log_interval: Duration)
where
    Block: GenericOwnedBlock,
    CI: ChainInfo<Block>,
{
    let shard = match Block::SHARD_KIND {
        ShardKind::BeaconChain => "BeaconChain".to_string(),
        ShardKind::IntermediateShard => {
            format!(
                "Intermediate[{}]",
                chain_info
                    .best_header()
                    .header()
                    .prefix
                    .shard_index
                    .as_u32()
            )
        }
        ShardKind::LeafShard => {
            format!(
                "Leaf[{}]",
                chain_info
                    .best_header()
                    .header()
                    .prefix
                    .shard_index
                    .as_u32()
            )
        }
        ShardKind::Phantom | ShardKind::Invalid => {
            unreachable!("Invalid shard kind; qed");
        }
    };
    loop {
        // TODO: Sync and networking status once implemented

        let best_header = chain_info.best_header();
        info!(
            %shard,
            best_number = %best_header.header().prefix.number,
            best_root = %*best_header.header().root(),
            "💤"
        );

        tokio::time::sleep(log_interval).await;
    }
}
