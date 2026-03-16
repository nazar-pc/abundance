//! Node client wrapper around another node client that caches some data for better performance and
//! proxies other requests through

use crate::node_client::{NodeClient, NodeClientExt};
use crate::utils::AsyncJoinOnDrop;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use ab_core_primitives::segments::{
    SegmentIndex, SuperSegmentHeader, SuperSegmentIndex, SuperSegmentRoot,
};
use ab_farmer_rpc_primitives::{
    BlockSealInfo, BlockSealResponse, FarmerAppInfo, FarmerShardMembershipInfo,
    MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST, SlotInfo, SolutionResponse,
};
use async_lock::{
    Mutex as AsyncMutex, RwLock as AsyncRwLock,
    RwLockUpgradableReadGuardArc as AsyncRwLockUpgradableReadGuard,
    RwLockWriteGuardArc as AsyncRwLockWriteGuard,
};
use async_trait::async_trait;
use futures::{FutureExt, Stream, StreamExt, select};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tracing::{info, trace, warn};

const SEGMENT_HEADERS_SYNC_INTERVAL: Duration = Duration::from_secs(1);
const FARMER_APP_INFO_DEDUPLICATION_WINDOW: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
struct SuperSegmentHeaders {
    super_segment_headers: Vec<SuperSegmentHeader>,
    last_synced: Option<Instant>,
}

impl SuperSegmentHeaders {
    /// Push a new super segment header to the cache if it is the next super segment header.
    /// Otherwise, skip the push.
    fn push(&mut self, new_segment_header: SuperSegmentHeader) {
        if self.super_segment_headers.len() as u64 == u64::from(new_segment_header.index.as_inner())
        {
            self.super_segment_headers.push(new_segment_header);
        }
    }

    /// Get cached super segment headers for the given super segment indices.
    ///
    /// Returns `None` for super segment indices that are not in the cache.
    fn get_super_segment_headers(
        &self,
        super_segment_indices: &[SuperSegmentIndex],
    ) -> Vec<Option<SuperSegmentHeader>> {
        super_segment_indices
            .iter()
            .map(|super_segment_index| {
                self.super_segment_headers
                    .get(u64::from(*super_segment_index) as usize)
                    .copied()
            })
            .collect::<Vec<_>>()
    }

    /// Get the last `limit` super segment headers from the cache
    fn last_super_segment_headers(&self, limit: u32) -> Vec<Option<SuperSegmentHeader>> {
        self.super_segment_headers
            .iter()
            .rev()
            .take(limit as usize)
            .rev()
            .copied()
            .map(Some)
            .collect()
    }

    // TODO: Maybe caching or more compact storage that points segment indices to super segments
    //  when this is called thousands of times during replotting?
    fn super_segment_root_for_segment_index(
        &self,
        segment_index: SegmentIndex,
    ) -> Option<SuperSegmentRoot> {
        let index = self
            .super_segment_headers
            .binary_search_by_key(&segment_index, |super_segment_header| {
                super_segment_header.max_segment_index.as_inner()
            })
            .unwrap_or_else(|insert_index| insert_index);

        let super_segment_header = self.super_segment_headers.get(index).copied()?;

        let max_segment_index = super_segment_header.max_segment_index.as_inner();
        let first_segment_index = max_segment_index
            - SegmentIndex::from(u64::from(super_segment_header.num_segments))
            + SegmentIndex::ONE;

        (first_segment_index..=max_segment_index)
            .contains(&segment_index)
            .then_some(super_segment_header.root)
    }

    /// Get uncached headers from the node if we're not rate-limited.
    /// This only requires a read lock.
    ///
    /// Returns any extra super segment headers if the download succeeds, or an error if it fails.
    /// The caller must write the returned super segment headers to the cache and reset the sync
    /// rate-limit timer.
    async fn request_uncached_headers<NC>(
        &self,
        client: &NC,
    ) -> anyhow::Result<Vec<SuperSegmentHeader>>
    where
        NC: NodeClient,
    {
        // Skip the sync if we're still within the sync rate limit.
        if let Some(last_synced) = &self.last_synced
            && last_synced.elapsed() < SEGMENT_HEADERS_SYNC_INTERVAL
        {
            return Ok(Vec::new());
        }

        let mut extra_super_segment_headers = Vec::new();
        let mut super_segment_index_offset =
            SuperSegmentIndex::from(self.super_segment_headers.len() as u64);
        let segment_index_step =
            SuperSegmentIndex::from(MAX_SUPER_SEGMENT_HEADERS_PER_REQUEST as u64);

        'outer: loop {
            let from = super_segment_index_offset;
            let to = super_segment_index_offset + segment_index_step;
            trace!(%from, %to, "Requesting super segment headers");

            for maybe_super_segment_header in client
                .super_segment_headers((from..to).collect::<Vec<_>>())
                .await
                .map_err(|error| {
                    anyhow::anyhow!(
                        "Failed to download super segment headers {from}..{to} from node: {error}"
                    )
                })?
            {
                let Some(super_segment_header) = maybe_super_segment_header else {
                    // Reached non-existent super segment header
                    break 'outer;
                };

                extra_super_segment_headers.push(super_segment_header);
            }

            super_segment_index_offset += segment_index_step;
        }

        Ok(extra_super_segment_headers)
    }

    /// Write the sync results to the cache, and reset the sync rate-limit timer.
    fn write_cache(&mut self, extra_super_segment_headers: Vec<SuperSegmentHeader>) {
        for super_segment_header in extra_super_segment_headers {
            self.push(super_segment_header);
        }
        self.last_synced.replace(Instant::now());
    }
}

/// Node client wrapper around another node client that caches some data for better performance and
/// proxies other requests through.
///
/// NOTE: Archived segment acknowledgement is ignored in this client, all subscriptions are
/// acknowledged implicitly and immediately.
/// NOTE: Subscription messages that are not processed in time will be skipped for performance
/// reasons!
#[derive(Debug, Clone)]
pub struct CachingProxyNodeClient<NC> {
    inner: NC,
    slot_info_receiver: watch::Receiver<Option<SlotInfo>>,
    new_super_segment_headers_receiver: watch::Receiver<Option<SuperSegmentHeader>>,
    block_sealing_receiver: watch::Receiver<Option<BlockSealInfo>>,
    super_segment_headers: Arc<AsyncRwLock<SuperSegmentHeaders>>,
    last_farmer_app_info: Arc<AsyncMutex<(FarmerAppInfo, Instant)>>,
    _background_task: Arc<AsyncJoinOnDrop<()>>,
}

impl<NC> CachingProxyNodeClient<NC>
where
    NC: NodeClient + Clone,
{
    /// Create a new instance
    pub async fn new(client: NC) -> anyhow::Result<Self> {
        let mut super_segment_headers = SuperSegmentHeaders::default();
        let mut new_super_segments_notifications =
            client.subscribe_new_super_segment_headers().await?;

        info!("Downloading all super segment headers from node...");
        // No locking is needed, we are the first and only instance right now.
        let headers = super_segment_headers
            .request_uncached_headers(&client)
            .await?;
        super_segment_headers.write_cache(headers);
        info!("Downloaded all super segment headers from node successfully");

        let super_segment_headers = Arc::new(AsyncRwLock::new(super_segment_headers));

        let (slot_info_sender, slot_info_receiver) = watch::channel(None::<SlotInfo>);
        let slot_info_proxy_fut = {
            let mut slot_info_subscription = client.subscribe_slot_info().await?;

            async move {
                let mut last_slot_number = None;
                while let Some(slot_info) = slot_info_subscription.next().await {
                    if let Some(last_slot_number) = last_slot_number
                        && last_slot_number >= slot_info.slot
                    {
                        continue;
                    }
                    last_slot_number.replace(slot_info.slot);

                    if let Err(error) = slot_info_sender.send(Some(slot_info)) {
                        warn!(%error, "Failed to proxy slot info notification");
                        return;
                    }
                }
            }
        };

        let (new_super_segment_headers_sender, new_super_segment_headers_receiver) =
            watch::channel(None::<SuperSegmentHeader>);
        let super_segment_headers_maintenance_fut = {
            let super_segment_headers = Arc::clone(&super_segment_headers);

            async move {
                let mut last_super_segment_index = None;
                while let Some(new_segment_header) = new_super_segments_notifications.next().await {
                    let super_segment_index = new_segment_header.index;
                    trace!(?new_segment_header, "New super segment header notification");

                    if let Some(last_super_segment_index) = last_super_segment_index
                        && last_super_segment_index >= super_segment_index
                    {
                        continue;
                    }
                    last_super_segment_index.replace(super_segment_index);

                    super_segment_headers.write().await.push(new_segment_header);

                    if let Err(error) =
                        new_super_segment_headers_sender.send(Some(new_segment_header))
                    {
                        warn!(%error, "Failed to proxy new super segment header notification");
                        return;
                    }
                }
            }
        };

        let (block_sealing_sender, block_sealing_receiver) = watch::channel(None::<BlockSealInfo>);
        let block_sealing_proxy_fut = {
            let mut block_sealing_subscription = client.subscribe_block_sealing().await?;

            async move {
                while let Some(block_sealing_info) = block_sealing_subscription.next().await {
                    if let Err(error) = block_sealing_sender.send(Some(block_sealing_info)) {
                        warn!(%error, "Failed to proxy block sealing notification");
                        return;
                    }
                }
            }
        };

        let farmer_app_info = client
            .farmer_app_info()
            .await
            .map_err(|error| anyhow::anyhow!("Failed to get farmer app info: {error}"))?;
        let last_farmer_app_info = Arc::new(AsyncMutex::new((farmer_app_info, Instant::now())));

        let background_task = tokio::spawn(async move {
            select! {
                _ = slot_info_proxy_fut.fuse() => {},
                _ = super_segment_headers_maintenance_fut.fuse() => {},
                _ = block_sealing_proxy_fut.fuse() => {},
            }
        });

        let node_client = Self {
            inner: client,
            slot_info_receiver,
            new_super_segment_headers_receiver,
            block_sealing_receiver,
            super_segment_headers,
            last_farmer_app_info,
            _background_task: Arc::new(AsyncJoinOnDrop::new(background_task, true)),
        };

        Ok(node_client)
    }
}

#[async_trait]
impl<NC> NodeClient for CachingProxyNodeClient<NC>
where
    NC: NodeClient,
{
    async fn farmer_app_info(&self) -> anyhow::Result<FarmerAppInfo> {
        let (last_farmer_app_info, last_farmer_app_info_request) =
            &mut *self.last_farmer_app_info.lock().await;

        if last_farmer_app_info_request.elapsed() > FARMER_APP_INFO_DEDUPLICATION_WINDOW {
            let new_last_farmer_app_info = self.inner.farmer_app_info().await?;

            *last_farmer_app_info = new_last_farmer_app_info;
            *last_farmer_app_info_request = Instant::now();
        }

        Ok(last_farmer_app_info.clone())
    }

    async fn subscribe_slot_info(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = SlotInfo> + Send + 'static>>> {
        Ok(Box::pin(
            WatchStream::new(self.slot_info_receiver.clone())
                .filter_map(|maybe_slot_info| async move { maybe_slot_info }),
        ))
    }

    async fn submit_solution_response(
        &self,
        solution_response: SolutionResponse,
    ) -> anyhow::Result<()> {
        self.inner.submit_solution_response(solution_response).await
    }

    async fn subscribe_block_sealing(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = BlockSealInfo> + Send + 'static>>> {
        Ok(Box::pin(
            WatchStream::new(self.block_sealing_receiver.clone())
                .filter_map(|maybe_block_sealing_info| async move { maybe_block_sealing_info }),
        ))
    }

    async fn submit_block_seal(&self, block_seal: BlockSealResponse) -> anyhow::Result<()> {
        self.inner.submit_block_seal(block_seal).await
    }

    async fn subscribe_new_super_segment_headers(
        &self,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = SuperSegmentHeader> + Send + 'static>>> {
        Ok(Box::pin(
            WatchStream::new(self.new_super_segment_headers_receiver.clone())
                .filter_map(|maybe_super_segment_header| async move { maybe_super_segment_header }),
        ))
    }

    async fn super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>> {
        let retrieved_super_segment_headers = self
            .super_segment_headers
            .read()
            .await
            .get_super_segment_headers(&super_segment_indices);

        if retrieved_super_segment_headers.iter().all(Option::is_some) {
            return Ok(retrieved_super_segment_headers);
        }

        // We might be missing a requested super segment header.
        // Sync the cache with the node, apply a rate limit, and return cached super segment
        // headers.

        // If we took a write lock here, a queue of writers could starve all the readers, even if
        // those writers are rate-limited. So we take an upgradable read lock for the rate limit
        // check.
        let super_segment_headers = self.super_segment_headers.upgradable_read_arc().await;

        // Try again after acquiring the upgradeable read lock, in case another caller already
        // synced the headers
        let retrieved_super_segment_headers =
            super_segment_headers.get_super_segment_headers(&super_segment_indices);
        if retrieved_super_segment_headers.iter().all(Option::is_some) {
            return Ok(retrieved_super_segment_headers);
        }

        // Try to sync the cache with the node
        let extra_super_segment_headers = super_segment_headers
            .request_uncached_headers(&self.inner)
            .await?;

        if extra_super_segment_headers.is_empty() {
            // No extra super segment headers on the node, or we are rate-limited, so just return
            // what is in the cache
            return Ok(retrieved_super_segment_headers);
        }

        // Need to update the cached super segment headers, so take the write lock
        let mut super_segment_headers =
            AsyncRwLockUpgradableReadGuard::upgrade(super_segment_headers).await;
        super_segment_headers.write_cache(extra_super_segment_headers);

        // Downgrade the write lock to a read lock to get the updated super segment headers for the
        // query
        Ok(AsyncRwLockWriteGuard::downgrade(super_segment_headers)
            .get_super_segment_headers(&super_segment_indices))
    }

    async fn super_segment_root_for_segment_index(
        &self,
        segment_index: SegmentIndex,
    ) -> anyhow::Result<Option<SuperSegmentRoot>> {
        Ok(self
            .super_segment_headers
            .read()
            .await
            .super_segment_root_for_segment_index(segment_index))
    }

    async fn piece(&self, piece_index: PieceIndex) -> anyhow::Result<Option<Piece>> {
        self.inner.piece(piece_index).await
    }

    async fn update_shard_membership_info(
        &self,
        info: FarmerShardMembershipInfo,
    ) -> anyhow::Result<()> {
        self.inner.update_shard_membership_info(info).await
    }
}

#[async_trait]
impl<NC> NodeClientExt for CachingProxyNodeClient<NC>
where
    NC: NodeClientExt,
{
    async fn cached_super_segment_headers(
        &self,
        super_segment_indices: Vec<SuperSegmentIndex>,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>> {
        // To avoid remote denial of service, we don't update the cache here because it is called
        // from network code
        Ok(self
            .super_segment_headers
            .read()
            .await
            .get_super_segment_headers(&super_segment_indices))
    }

    async fn last_super_segment_headers(
        &self,
        limit: u32,
    ) -> anyhow::Result<Vec<Option<SuperSegmentHeader>>> {
        Ok(self
            .super_segment_headers
            .read()
            .await
            .last_super_segment_headers(limit))
    }
}
