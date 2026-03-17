//! Node client abstraction
//!
//! During farmer operation it needs to communicate with a node, for example, to receive slot
//! notifications and send solutions to seal blocks.
//!
//! Implementation is abstracted away behind a trait to allow various implementations depending on
//! the use case. Implementation may connect to a node via RPC directly, through some kind of
//! networked middleware, or even wired without a network directly if node and farmer are both
//! running in the same process.

pub mod caching_proxy_node_client;
pub mod rpc_node_client;

use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_core_primitives::segments::{
    SegmentIndex, SuperSegmentHeader, SuperSegmentIndex, SuperSegmentRoot,
};
use ab_farmer_rpc_primitives::{
    BlockSealInfo, BlockSealResponse, FarmerAppInfo, FarmerShardMembershipInfo, SlotInfo,
    SolutionResponse,
};
use async_trait::async_trait;
use futures::Stream;
use std::fmt;
use std::pin::Pin;

/// Abstraction of the Node Client
#[async_trait]
pub trait NodeClient: fmt::Debug + Send + Sync + 'static {
    /// Get farmer app info
    async fn farmer_app_info(&self) -> anyhow::Result<FarmerAppInfo>;

    /// Subscribe to slot
    async fn subscribe_slot_info(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = SlotInfo> + Send + 'static>>>;

    /// Submit a slot solution
    async fn submit_solution_response(
        &self,
        solution_response: SolutionResponse,
    ) -> anyhow::Result<()>;

    /// Subscribe to block sealing requests
    async fn subscribe_block_sealing(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = BlockSealInfo> + Send + 'static>>>;

    /// Submit a block seal
    async fn submit_block_seal(&self, block_seal: BlockSealResponse) -> anyhow::Result<()>;

    /// Subscribe to new super segment headers
    async fn subscribe_new_super_segment_headers(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = SuperSegmentHeader> + Send + 'static>>>;

    /// Get super segment headers
    async fn super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>>;

    /// Get super segment root for a segment index
    async fn super_segment_root_for_segment_index(
        &self,
        segment_index: SegmentIndex,
    ) -> anyhow::Result<Option<SuperSegmentRoot>>;

    /// Get piece by index
    async fn piece(&self, piece_index: PieceIndex) -> anyhow::Result<Option<Piece>>;

    // TODO: Move into `NodeClientExt`?
    /// Must be called while there is an active `shard_membership_entropy_update` subscription
    async fn update_shard_membership_info(
        &self,
        info: FarmerShardMembershipInfo,
    ) -> anyhow::Result<()>;
}

/// Node Client extension methods that are not necessary for a farmer as a library but might be
/// useful for an app
#[async_trait]
pub trait NodeClientExt: NodeClient {
    /// Get the cached super segment headers for the given super segment indices.
    /// If there is a cache, it is not updated to avoid remote denial of service.
    ///
    /// Returns `None` for super segment indices that are not in the cache.
    async fn cached_super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>>;

    /// Get up to `limit` most recent super segment headers.
    /// If there is a cache, it is not updated to avoid remote denial of service.
    ///
    /// If the node or cache has less than `limit` super segment headers, the returned vector will
    /// be shorter. Each returned super segment header is wrapped in `Some`.
    async fn last_super_segment_headers(
        &self,
        limit: u32,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>>;
}
