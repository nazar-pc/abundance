//! Node client implementation that connects to node via RPC (WebSockets)

use crate::node_client::{NodeClient, NodeClientExt};
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_core_primitives::segments::{
    SegmentIndex, SuperSegmentHeader, SuperSegmentIndex, SuperSegmentRoot,
};
use ab_farmer_rpc_primitives::{
    BlockSealInfo, BlockSealResponse, FarmerAppInfo, FarmerShardMembershipInfo, SlotInfo,
    SolutionResponse,
};
use async_lock::Semaphore;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use jsonrpsee::core::client::{ClientT, Error as JsonError, SubscriptionClientT};
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use std::pin::Pin;
use std::sync::Arc;

/// TODO: Node is having a hard time responding for many piece requests, specifically this results
///  in subscriptions become broken on the node: https://github.com/paritytech/jsonrpsee/issues/1409
///  This needs to be removed after Substrate upgrade when we can take advantage of new Substrate
///  API that will prevent subscription breakage:
///  https://github.com/paritytech/jsonrpsee/issues/1409#issuecomment-2303914643
const MAX_CONCURRENT_PIECE_REQUESTS: usize = 10;

/// Node client implementation that connects to node via RPC (WebSockets).
///
/// This implementation is supposed to be used on local network and not via public Internet due to
/// sensitive contents.
#[derive(Debug, Clone)]
pub struct RpcNodeClient {
    client: Arc<WsClient>,
    piece_request_semaphore: Arc<Semaphore>,
}

impl RpcNodeClient {
    /// Create a new instance of [`NodeClient`].
    pub async fn new(url: &str) -> Result<Self, JsonError> {
        let client = Arc::new(
            WsClientBuilder::default()
                .max_request_size(20 * 1024 * 1024)
                .build(url)
                .await?,
        );
        let piece_request_semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_PIECE_REQUESTS));
        Ok(Self {
            client,
            piece_request_semaphore,
        })
    }
}

#[async_trait]
impl NodeClient for RpcNodeClient {
    async fn farmer_app_info(&self) -> anyhow::Result<FarmerAppInfo> {
        Ok(self
            .client
            .request("getFarmerAppInfo", rpc_params![])
            .await?)
    }

    async fn subscribe_slot_info(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = SlotInfo> + Send + 'static>>> {
        let subscription = self
            .client
            .subscribe("subscribeSlotInfo", rpc_params![], "unsubscribeSlotInfo")
            .await?;

        Ok(Box::pin(subscription.filter_map(
            |slot_info_result| async move { slot_info_result.ok() },
        )))
    }

    async fn submit_solution_response(
        &self,
        solution_response: SolutionResponse,
    ) -> anyhow::Result<()> {
        Ok(self
            .client
            .request("submitSolutionResponse", rpc_params![&solution_response])
            .await?)
    }

    async fn subscribe_block_sealing(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = BlockSealInfo> + Send + 'static>>> {
        let subscription = self
            .client
            .subscribe(
                "subscribeBlockSealing",
                rpc_params![],
                "unsubscribeBlockSealing",
            )
            .await?;

        Ok(Box::pin(subscription.filter_map(
            |block_sealing_info_result| async move { block_sealing_info_result.ok() },
        )))
    }

    async fn submit_block_seal(&self, block_seal: BlockSealResponse) -> anyhow::Result<()> {
        Ok(self
            .client
            .request("submitBlockSeal", rpc_params![&block_seal])
            .await?)
    }

    async fn subscribe_new_super_segment_headers(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = SuperSegmentHeader> + Send + 'static>>> {
        let subscription = self
            .client
            .subscribe(
                "subscribeNewSuperSegmentHeader",
                rpc_params![],
                "unsubscribeNewSuperSegmentHeader",
            )
            .await?;

        Ok(Box::pin(subscription.filter_map(
            |new_super_segment_header_result| async move { new_super_segment_header_result.ok() },
        )))
    }

    async fn super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>> {
        Ok(self
            .client
            .request("superSegmentHeaders", rpc_params![&super_segment_indices])
            .await?)
    }

    async fn super_segment_root_for_segment_index(
        &self,
        segment_index: SegmentIndex,
    ) -> anyhow::Result<Option<SuperSegmentRoot>> {
        Ok(self
            .client
            .request(
                "superSegmentRootForSegmentIndex",
                rpc_params![&segment_index],
            )
            .await?)
    }

    async fn piece(&self, piece_index: PieceIndex) -> anyhow::Result<Option<Piece>> {
        let _permit = self.piece_request_semaphore.acquire().await;
        let client = Arc::clone(&self.client);
        // Spawn a separate task to improve concurrency due to slow-ish JSON decoding that causes
        // issues for jsonrpsee
        let piece_fut =
            tokio::task::spawn(
                async move { client.request("piece", rpc_params![&piece_index]).await },
            );
        Ok(piece_fut.await??)
    }

    async fn update_shard_membership_info(
        &self,
        info: FarmerShardMembershipInfo,
    ) -> anyhow::Result<()> {
        Ok(self
            .client
            .request("updateShardMembershipInfo", rpc_params![&info])
            .await?)
    }
}

#[async_trait]
impl NodeClientExt for RpcNodeClient {
    async fn cached_super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>> {
        self.super_segment_headers(super_segment_indices).await
    }

    async fn last_super_segment_headers(
        &self,
        limit: u32,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>> {
        Ok(self
            .client
            .request("lastSuperSegmentHeaders", rpc_params![limit])
            .await?)
    }
}
