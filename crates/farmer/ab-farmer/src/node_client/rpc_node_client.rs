//! Node client implementation that connects to node via RPC (WebSockets)

use crate::node_client::{NodeClient, NodeClientExt};
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_core_primitives::segments::{SegmentHeader, SegmentIndex};
use ab_farmer_rpc_primitives::{
    BlockSealInfo, BlockSealResponse, FarmerAppInfo, SlotInfo, SolutionResponse,
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

    /// Submit a block seal
    async fn submit_block_seal(&self, block_seal: BlockSealResponse) -> anyhow::Result<()> {
        Ok(self
            .client
            .request("submitBlockSeal", rpc_params![&block_seal])
            .await?)
    }

    async fn subscribe_archived_segment_headers(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = SegmentHeader> + Send + 'static>>> {
        let subscription = self
            .client
            .subscribe(
                "subscribeArchivedSegmentHeader",
                rpc_params![],
                "unsubscribeArchivedSegmentHeader",
            )
            .await?;

        Ok(Box::pin(subscription.filter_map(
            |archived_segment_header_result| async move { archived_segment_header_result.ok() },
        )))
    }

    async fn segment_headers(
        &self,
        segment_indices: Vec<SegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SegmentHeader>>> {
        Ok(self
            .client
            .request("segmentHeaders", rpc_params![&segment_indices])
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

    async fn acknowledge_archived_segment_header(
        &self,
        segment_index: SegmentIndex,
    ) -> anyhow::Result<()> {
        Ok(self
            .client
            .request(
                "acknowledgeArchivedSegmentHeader",
                rpc_params![&segment_index],
            )
            .await?)
    }
}

#[async_trait]
impl NodeClientExt for RpcNodeClient {
    async fn cached_segment_headers(
        &self,
        segment_indices: Vec<SegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SegmentHeader>>> {
        self.segment_headers(segment_indices).await
    }

    async fn last_segment_headers(&self, limit: u32) -> anyhow::Result<Vec<Option<SegmentHeader>>> {
        Ok(self
            .client
            .request("lastSegmentHeaders", rpc_params![limit])
            .await?)
    }
}
