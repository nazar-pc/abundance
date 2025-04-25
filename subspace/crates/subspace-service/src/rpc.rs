//! A collection of node-specific RPC methods.
//!
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use ab_erasure_coding::ErasureCoding;
use jsonrpsee::RpcModule;
use sc_client_api::{AuxStore, BlockBackend};
use sc_consensus_subspace::archiver::{ArchivedSegmentNotification, SegmentHeadersStore};
use sc_consensus_subspace::notification::SubspaceNotificationStream;
use sc_consensus_subspace::slot_worker::{
    NewSlotNotification, RewardSigningNotification, SubspaceSyncOracle,
};
use sc_consensus_subspace_rpc::{SubspaceRpc, SubspaceRpcApiServer, SubspaceRpcConfig};
use sc_rpc::SubscriptionTaskExecutor;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_consensus::SyncOracle;
use sp_consensus_subspace::SubspaceApi;
use sp_objects::ObjectsApi;
use std::sync::Arc;
use subspace_networking::libp2p::Multiaddr;
use subspace_runtime_primitives::opaque::Block;

/// Full client dependencies.
pub struct FullDeps<C, SO, AS>
where
    SO: SyncOracle + Send + Sync + Clone,
{
    /// The client instance to use.
    pub client: Arc<C>,
    /// Executor to drive the subscription manager in the Grandpa RPC handler.
    pub subscription_executor: SubscriptionTaskExecutor,
    /// A stream with notifications about new slot arrival with ability to send solution back.
    pub new_slot_notification_stream: SubspaceNotificationStream<NewSlotNotification>,
    /// A stream with notifications about headers that need to be signed with ability to send
    /// signature back.
    pub reward_signing_notification_stream: SubspaceNotificationStream<RewardSigningNotification>,
    /// A stream with notifications about archived segment creation.
    pub archived_segment_notification_stream:
        SubspaceNotificationStream<ArchivedSegmentNotification>,
    /// Bootstrap nodes for DSN.
    pub dsn_bootstrap_nodes: Vec<Multiaddr>,
    /// Segment header provider.
    pub segment_headers_store: SegmentHeadersStore<AS>,
    /// Subspace sync oracle.
    pub sync_oracle: SubspaceSyncOracle<SO>,
    /// Erasure coding instance.
    pub erasure_coding: ErasureCoding,
}

/// Instantiate all full RPC extensions.
pub fn create_full<C, SO, AS>(
    deps: FullDeps<C, SO, AS>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>
        + BlockBackend<Block>
        + HeaderBackend<Block>
        + Send
        + Sync
        + 'static,
    C::Api: SubspaceApi<Block> + ObjectsApi<Block>,
    SO: SyncOracle + Send + Sync + Clone + 'static,
    AS: AuxStore + Send + Sync + 'static,
{
    let FullDeps {
        client,
        subscription_executor,
        new_slot_notification_stream,
        reward_signing_notification_stream,
        archived_segment_notification_stream,
        dsn_bootstrap_nodes,
        segment_headers_store,
        sync_oracle,
        erasure_coding,
    } = deps;

    let mut module = RpcModule::new(());
    module.merge(
        SubspaceRpc::new(SubspaceRpcConfig {
            client: client.clone(),
            subscription_executor,
            new_slot_notification_stream,
            reward_signing_notification_stream,
            archived_segment_notification_stream,
            dsn_bootstrap_nodes,
            segment_headers_store,
            sync_oracle,
            erasure_coding,
        })?
        .into_rpc(),
    )?;

    Ok(module)
}
