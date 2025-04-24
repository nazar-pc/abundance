//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.
#![feature(
    duration_constructors,
    impl_trait_in_assoc_type,
    int_roundings,
    let_chains,
    type_alias_impl_trait,
    type_changing_struct_update
)]

pub mod config;
pub mod dsn;
mod metrics;
pub mod rpc;
pub mod sync_from_dsn;
mod task_spawner;
mod utils;

use crate::config::{ChainSyncMode, SubspaceConfiguration, SubspaceNetworking};
use crate::dsn::{create_dsn_instance, DsnConfigurationError};
use crate::metrics::NodeMetrics;
use crate::sync_from_dsn::piece_validator::SegmentRootPieceValidator;
use crate::sync_from_dsn::snap_sync::snap_sync;
use crate::sync_from_dsn::DsnPieceGetter;
use crate::task_spawner::SpawnTasksParams;
use ab_erasure_coding::ErasureCoding;
use async_lock::Semaphore;
use core::sync::atomic::{AtomicU32, Ordering};
use frame_system_rpc_runtime_api::AccountNonceApi;
use futures::channel::oneshot;
use jsonrpsee::RpcModule;
use pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi;
use parking_lot::Mutex;
use prometheus_client::registry::Registry;
use sc_basic_authorship::ProposerFactory;
use sc_chain_spec::GenesisBlockBuilder;
use sc_client_api::{AuxStore, BlockBackend, BlockchainEvents, HeaderBackend};
use sc_consensus::{
    BasicQueue, BlockCheckParams, BlockImport, BlockImportParams, BoxBlockImport,
    DefaultImportQueue, ImportQueue, ImportResult,
};
use sc_consensus_slots::SlotProportion;
use sc_consensus_subspace::archiver::{
    create_subspace_archiver, ArchivedSegmentNotification, SegmentHeadersStore,
};
use sc_consensus_subspace::block_import::{BlockImportingNotification, SubspaceBlockImport};
use sc_consensus_subspace::notification::SubspaceNotificationStream;
use sc_consensus_subspace::slot_worker::{
    NewSlotNotification, RewardSigningNotification, SubspaceSlotWorker, SubspaceSlotWorkerOptions,
    SubspaceSyncOracle,
};
use sc_consensus_subspace::verifier::{SubspaceVerifier, SubspaceVerifierOptions};
use sc_consensus_subspace::SubspaceLink;
use sc_network::service::traits::NetworkService;
use sc_network::{NetworkWorker, NotificationMetrics, Roles};
use sc_network_sync::engine::SyncingEngine;
use sc_network_sync::service::network::NetworkServiceProvider;
use sc_proof_of_time::source::gossip::pot_gossip_peers_set_config;
use sc_proof_of_time::source::{PotSlotInfo, PotSourceWorker};
use sc_proof_of_time::verifier::PotVerifier;
use sc_service::error::Error as ServiceError;
use sc_service::{
    build_default_block_downloader, build_network_advanced, build_polkadot_syncing_strategy,
    BuildNetworkAdvancedParams, Configuration, NetworkStarter, TaskManager,
};
use sc_transaction_pool::TransactionPoolHandle;
use sp_api::{ApiExt, ConstructRuntimeApi, Metadata, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::HeaderMetadata;
use sp_consensus::block_validation::DefaultBlockAnnounceValidator;
use sp_consensus_subspace::SubspaceApi;
use sp_core::traits::SpawnEssentialNamed;
use sp_objects::ObjectsApi;
use sp_offchain::OffchainWorkerApi;
use sp_runtime::traits::{Block as BlockT, BlockIdTo};
use sp_session::SessionKeys;
use sp_transaction_pool::runtime_api::TaggedTransactionQueue;
use static_assertions::const_assert;
use std::sync::Arc;
use std::time::Duration;
use subspace_core_primitives::pot::PotSeed;
use subspace_networking::libp2p::multiaddr::Protocol;
use subspace_networking::utils::piece_provider::PieceProvider;
use subspace_proof_of_space::Table;
use subspace_runtime_primitives::opaque::Block;
use subspace_runtime_primitives::{AccountId, Balance, Nonce};
use subspace_verification::sr25519::REWARD_SIGNING_CONTEXT;
use tokio::sync::broadcast;
use tracing::{debug, error, info, Instrument};
pub use utils::wait_for_block_import;

// There are multiple places where it is assumed that node is running on 64-bit system, refuse to
// compile otherwise
const_assert!(std::mem::size_of::<usize>() >= std::mem::size_of::<u64>());

/// This is over 15 minutes of slots assuming there are no forks, should be both sufficient and not
/// too large to handle
const POT_VERIFIER_CACHE_SIZE: u32 = 30_000;
const SYNC_TARGET_UPDATE_INTERVAL: Duration = Duration::from_secs(1);
/// Multiplier on top of outgoing connections number for piece downloading purposes
const PIECE_PROVIDER_MULTIPLIER: usize = 10;

/// Error type for Subspace service.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// IO error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Address parsing error.
    #[error(transparent)]
    AddrFormatInvalid(#[from] std::net::AddrParseError),

    /// Substrate service error.
    #[error(transparent)]
    Sub(#[from] sc_service::Error),

    /// Substrate consensus error.
    #[error(transparent)]
    Consensus(#[from] sp_consensus::Error),

    /// Subspace networking (DSN) error.
    #[error(transparent)]
    SubspaceDsn(#[from] DsnConfigurationError),

    /// Other.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

// Simple wrapper whose ony purpose is to convert error type
#[derive(Clone)]
struct BlockImportWrapper<BI>(BI);

#[async_trait::async_trait]
impl<Block, BI> BlockImport<Block> for BlockImportWrapper<BI>
where
    Block: BlockT,
    BI: BlockImport<Block, Error = sc_consensus_subspace::block_import::Error<Block::Header>>
        + Send
        + Sync,
{
    type Error = sp_consensus::Error;

    async fn check_block(
        &self,
        block: BlockCheckParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        self.0
            .check_block(block)
            .await
            .map_err(|error| sp_consensus::Error::Other(error.into()))
    }

    async fn import_block(
        &self,
        block: BlockImportParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        self.0
            .import_block(block)
            .await
            .map_err(|error| sp_consensus::Error::Other(error.into()))
    }
}

/// Host functions required for Subspace
#[cfg(not(feature = "runtime-benchmarks"))]
pub type HostFunctions = (sp_io::SubstrateHostFunctions,);

/// Host functions required for Subspace
#[cfg(feature = "runtime-benchmarks")]
pub type HostFunctions = (
    sp_io::SubstrateHostFunctions,
    frame_benchmarking::benchmarking::HostFunctions,
);

/// Runtime executor for Subspace
pub type RuntimeExecutor = sc_executor::WasmExecutor<HostFunctions>;

/// Subspace-like full client.
pub type FullClient<RuntimeApi> = sc_service::TFullClient<Block, RuntimeApi, RuntimeExecutor>;

pub type FullBackend = sc_service::TFullBackend<Block>;
pub type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

/// Other partial components returned by [`new_partial()`]
pub struct OtherPartialComponents<RuntimeApi>
where
    RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
{
    /// Subspace block import
    pub block_import: BoxBlockImport<Block>,
    /// Subspace link
    pub subspace_link: SubspaceLink<Block>,
    /// Segment headers store
    pub segment_headers_store: SegmentHeadersStore<FullClient<RuntimeApi>>,
    /// Proof of time verifier
    pub pot_verifier: PotVerifier,
    /// Approximate target block number for syncing purposes
    pub sync_target_block_number: Arc<AtomicU32>,
}

type PartialComponents<RuntimeApi> = sc_service::PartialComponents<
    FullClient<RuntimeApi>,
    FullBackend,
    FullSelectChain,
    DefaultImportQueue<Block>,
    TransactionPoolHandle<Block, FullClient<RuntimeApi>>,
    OtherPartialComponents<RuntimeApi>,
>;

/// Creates `PartialComponents` for Subspace client.
#[expect(clippy::result_large_err, reason = "Comes from Substrate")]
pub fn new_partial<PosTable, RuntimeApi>(
    // TODO: Stop using `Configuration` once
    //  https://github.com/paritytech/polkadot-sdk/pull/5364 is in our fork
    config: &Configuration,
    // TODO: Replace with check for `ChainSyncMode` once we get rid of ^ `Configuration`
    snap_sync: bool,
    pot_external_entropy: &[u8],
) -> Result<PartialComponents<RuntimeApi>, ServiceError>
where
    PosTable: Table,
    RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
    RuntimeApi::RuntimeApi: ApiExt<Block>
        + Metadata<Block>
        + BlockBuilder<Block>
        + OffchainWorkerApi<Block>
        + SessionKeys<Block>
        + TaggedTransactionQueue<Block>
        + SubspaceApi<Block>
        + ObjectsApi<Block>,
{
    let executor = sc_service::new_wasm_executor(&config.executor);

    let backend = sc_service::new_db_backend(config.db_config())?;

    let genesis_block_builder = GenesisBlockBuilder::new(
        config.chain_spec.as_storage_builder(),
        !snap_sync,
        backend.clone(),
        executor.clone(),
    )?;

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts_with_genesis_builder::<Block, RuntimeApi, _, _>(
            config,
            None,
            executor.clone(),
            backend,
            genesis_block_builder,
            false,
        )?;

    let erasure_coding = ErasureCoding::new();

    let client = Arc::new(client);
    let client_info = client.info();
    let chain_constants = client
        .runtime_api()
        .chain_constants(client_info.best_hash)
        .map_err(|error| ServiceError::Application(error.into()))?;

    let pot_verifier = PotVerifier::new(
        PotSeed::from_genesis(client_info.genesis_hash.as_ref(), pot_external_entropy),
        POT_VERIFIER_CACHE_SIZE,
    );

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let segment_headers_store = tokio::task::block_in_place(|| {
        SegmentHeadersStore::new(client.clone(), chain_constants.confirmation_depth_k())
    })
    .map_err(|error| ServiceError::Application(error.into()))?;

    let subspace_link = SubspaceLink::new(chain_constants, erasure_coding);
    let segment_headers_store = segment_headers_store.clone();

    let block_import = SubspaceBlockImport::<PosTable, _, _, _, _, _>::new(
        client.clone(),
        client.clone(),
        subspace_link.clone(),
        {
            let client = client.clone();
            let segment_headers_store = segment_headers_store.clone();

            move |parent_hash, ()| {
                let client = client.clone();
                let segment_headers_store = segment_headers_store.clone();

                async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let parent_header = client
                        .header(parent_hash)?
                        .expect("Parent header must always exist when block is created; qed");

                    let parent_block_number = parent_header.number;

                    let subspace_inherents =
                        sp_consensus_subspace::inherents::InherentDataProvider::new(
                            segment_headers_store
                                .segment_headers_for_block(parent_block_number + 1),
                        );

                    Ok((timestamp, subspace_inherents))
                }
            }
        },
        segment_headers_store.clone(),
        pot_verifier.clone(),
    );

    let sync_target_block_number = Arc::new(AtomicU32::new(0));
    let transaction_pool = Arc::from(
        sc_transaction_pool::Builder::new(
            task_manager.spawn_essential_handle(),
            client.clone(),
            config.role.is_authority().into(),
        )
        .with_options(config.transaction_pool.clone())
        .with_prometheus(config.prometheus_registry())
        .build(),
    );

    let verifier = SubspaceVerifier::<PosTable, _, _>::new(SubspaceVerifierOptions {
        client: client.clone(),
        chain_constants,
        reward_signing_context: schnorrkel::context::signing_context(REWARD_SIGNING_CONTEXT),
        sync_target_block_number: Arc::clone(&sync_target_block_number),
        is_authoring_blocks: config.role.is_authority(),
        pot_verifier: pot_verifier.clone(),
    });

    let import_queue = BasicQueue::new(
        verifier,
        Box::new(BlockImportWrapper(block_import.clone())),
        None,
        &task_manager.spawn_essential_handle(),
        config.prometheus_registry(),
    );

    let other = OtherPartialComponents {
        block_import: Box::new(BlockImportWrapper(block_import.clone())),
        subspace_link,
        segment_headers_store,
        pot_verifier,
        sync_target_block_number,
    };

    Ok(PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other,
    })
}

/// Full node along with some other components.
pub struct NewFull<Client>
where
    Client: ProvideRuntimeApi<Block>
        + AuxStore
        + BlockBackend<Block>
        + BlockIdTo<Block>
        + HeaderBackend<Block>
        + HeaderMetadata<Block, Error = sp_blockchain::Error>
        + 'static,
    Client::Api: TaggedTransactionQueue<Block> + SubspaceApi<Block>,
{
    /// Task manager.
    pub task_manager: TaskManager,
    /// Full client.
    pub client: Arc<Client>,
    /// Chain selection rule.
    pub select_chain: FullSelectChain,
    /// Network service.
    pub network_service: Arc<dyn NetworkService + Send + Sync>,
    /// Sync service.
    pub sync_service: Arc<sc_network_sync::SyncingService<Block>>,
    /// Full client backend.
    pub backend: Arc<FullBackend>,
    /// Pot slot info stream.
    pub pot_slot_info_stream: broadcast::Receiver<PotSlotInfo>,
    /// New slot stream.
    /// Note: this is currently used to send solutions from the farmer during tests.
    pub new_slot_notification_stream: SubspaceNotificationStream<NewSlotNotification>,
    /// Block signing stream.
    pub reward_signing_notification_stream: SubspaceNotificationStream<RewardSigningNotification>,
    /// Stream of notifications about blocks about to be imported.
    pub block_importing_notification_stream:
        SubspaceNotificationStream<BlockImportingNotification<Block>>,
    /// Archived segment stream.
    pub archived_segment_notification_stream:
        SubspaceNotificationStream<ArchivedSegmentNotification>,
    /// Network starter.
    pub network_starter: NetworkStarter,
    /// Transaction pool.
    pub transaction_pool: Arc<TransactionPoolHandle<Block, Client>>,
}

type FullNode<RuntimeApi> = NewFull<FullClient<RuntimeApi>>;

/// Builds a new service for a full client.
pub async fn new_full<PosTable, RuntimeApi>(
    mut config: SubspaceConfiguration,
    partial_components: PartialComponents<RuntimeApi>,
    prometheus_registry: Option<&mut Registry>,
    enable_rpc_extensions: bool,
    block_proposal_slot_portion: SlotProportion,
) -> Result<FullNode<RuntimeApi>, Error>
where
    PosTable: Table,
    RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
    RuntimeApi::RuntimeApi: ApiExt<Block>
        + Metadata<Block>
        + AccountNonceApi<Block, AccountId, Nonce>
        + BlockBuilder<Block>
        + OffchainWorkerApi<Block>
        + SessionKeys<Block>
        + TaggedTransactionQueue<Block>
        + TransactionPaymentApi<Block, Balance>
        + SubspaceApi<Block>
        + ObjectsApi<Block>,
{
    let PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container: _,
        select_chain,
        transaction_pool,
        other,
    } = partial_components;
    let OtherPartialComponents {
        block_import,
        subspace_link,
        segment_headers_store,
        pot_verifier,
        sync_target_block_number,
    } = other;

    let (node, bootstrap_nodes, piece_getter) = match config.subspace_networking {
        SubspaceNetworking::Reuse {
            node,
            bootstrap_nodes,
            piece_getter,
        } => (node, bootstrap_nodes, piece_getter),
        SubspaceNetworking::Create { config: dsn_config } => {
            let dsn_protocol_version = hex::encode(client.chain_info().genesis_hash);

            debug!(
                chain_type=?config.base.chain_spec.chain_type(),
                genesis_hash=%hex::encode(client.chain_info().genesis_hash),
                "Setting DSN protocol version..."
            );

            let out_connections = dsn_config.max_out_connections;
            let (node, mut node_runner) = create_dsn_instance(
                dsn_protocol_version,
                dsn_config.clone(),
                prometheus_registry,
            )?;

            info!("Subspace networking initialized: Node ID is {}", node.id());

            node.on_new_listener(Arc::new({
                let node = node.clone();

                move |address| {
                    info!(
                        "DSN listening on {}",
                        address.clone().with(Protocol::P2p(node.id()))
                    );
                }
            }))
            .detach();

            task_manager
                .spawn_essential_handle()
                .spawn_essential_blocking(
                    "node-runner",
                    Some("subspace-networking"),
                    Box::pin(
                        async move {
                            node_runner.run().await;
                        }
                        .in_current_span(),
                    ),
                );

            let piece_provider = PieceProvider::new(
                node.clone(),
                SegmentRootPieceValidator::new(node.clone(), segment_headers_store.clone()),
                Arc::new(Semaphore::new(
                    out_connections as usize * PIECE_PROVIDER_MULTIPLIER,
                )),
            );

            (
                node,
                dsn_config.bootstrap_nodes,
                Arc::new(DsnPieceGetter::new(piece_provider)) as _,
            )
        }
    };

    let dsn_bootstrap_nodes = {
        // Fall back to node itself as bootstrap node for DSN so farmer always has someone to
        // connect to
        if bootstrap_nodes.is_empty() {
            let (node_address_sender, node_address_receiver) = oneshot::channel();
            let _handler = node.on_new_listener(Arc::new({
                let node_address_sender = Mutex::new(Some(node_address_sender));

                move |address| {
                    if matches!(address.iter().next(), Some(Protocol::Ip4(_))) {
                        if let Some(node_address_sender) = node_address_sender.lock().take() {
                            if let Err(err) = node_address_sender.send(address.clone()) {
                                debug!(?err, "Couldn't send a node address to the channel.");
                            }
                        }
                    }
                }
            }));

            let mut node_listeners = node.listeners();

            if node_listeners.is_empty() {
                let Ok(listener) = node_address_receiver.await else {
                    return Err(Error::Other(
                        "Oneshot receiver dropped before DSN node listener was ready"
                            .to_string()
                            .into(),
                    ));
                };

                node_listeners = vec![listener];
            }

            node_listeners.iter_mut().for_each(|multiaddr| {
                multiaddr.push(Protocol::P2p(node.id()));
            });

            node_listeners
        } else {
            bootstrap_nodes
        }
    };

    let substrate_prometheus_registry = config
        .base
        .prometheus_config
        .as_ref()
        .map(|prometheus_config| prometheus_config.registry.clone());
    let import_queue_service1 = import_queue.service();
    let import_queue_service2 = import_queue.service();
    let mut net_config = sc_network::config::FullNetworkConfiguration::new(
        &config.base.network,
        substrate_prometheus_registry.clone(),
    );
    let (pot_gossip_notification_config, pot_gossip_notification_service) =
        pot_gossip_peers_set_config();
    net_config.add_notification_protocol(pot_gossip_notification_config);
    let pause_sync = Arc::clone(&net_config.network_config.pause_sync);

    let protocol_id = config.base.protocol_id();
    let fork_id = config.base.chain_spec.fork_id();

    let network_service_provider = NetworkServiceProvider::new();
    let network_service_handle = network_service_provider.handle();
    let (network_service, _system_rpc_tx, tx_handler_controller, network_starter, sync_service) = {
        let spawn_handle = task_manager.spawn_handle();
        let metrics = NotificationMetrics::new(substrate_prometheus_registry.as_ref());

        let num_peers_hint = net_config.network_config.default_peers_set.in_peers as usize
            + net_config.network_config.default_peers_set.out_peers as usize;
        let block_downloader = build_default_block_downloader(
            &protocol_id,
            fork_id,
            &mut net_config,
            network_service_provider.handle(),
            client.clone(),
            num_peers_hint,
            &spawn_handle,
        );

        let syncing_strategy = build_polkadot_syncing_strategy(
            protocol_id.clone(),
            fork_id,
            &mut net_config,
            None,
            block_downloader,
            client.clone(),
            &spawn_handle,
            substrate_prometheus_registry.as_ref(),
        )?;

        let (syncing_engine, sync_service, block_announce_config) =
            SyncingEngine::new::<NetworkWorker<_, _>>(
                Roles::from(&config.base.role),
                Arc::clone(&client),
                substrate_prometheus_registry.as_ref(),
                metrics.clone(),
                &net_config,
                protocol_id.clone(),
                fork_id,
                Box::new(DefaultBlockAnnounceValidator),
                syncing_strategy,
                network_service_provider.handle(),
                import_queue.service(),
                net_config.peer_store_handle(),
                config.base.network.force_synced,
            )
            .map_err(sc_service::Error::from)?;

        spawn_handle.spawn_blocking("syncing", None, syncing_engine.run());

        build_network_advanced(BuildNetworkAdvancedParams {
            role: config.base.role,
            protocol_id,
            fork_id,
            ipfs_server: config.base.network.ipfs_server,
            announce_block: config.base.announce_block,
            net_config,
            client: Arc::clone(&client),
            transaction_pool: Arc::clone(&transaction_pool),
            spawn_handle,
            import_queue,
            sync_service,
            block_announce_config,
            network_service_provider,
            metrics_registry: substrate_prometheus_registry.as_ref(),
            metrics,
        })?
    };

    task_manager.spawn_handle().spawn(
        "sync-target-follower",
        None,
        Box::pin({
            let sync_service = sync_service.clone();
            let sync_target_block_number = Arc::clone(&sync_target_block_number);

            async move {
                loop {
                    let best_seen_block = sync_service
                        .status()
                        .await
                        .map(|status| status.best_seen_block.unwrap_or_default())
                        .unwrap_or_default();
                    sync_target_block_number.store(best_seen_block, Ordering::Relaxed);

                    tokio::time::sleep(SYNC_TARGET_UPDATE_INTERVAL).await;
                }
            }
        }),
    );

    let sync_oracle = SubspaceSyncOracle::new(
        config.base.force_authoring,
        Arc::clone(&pause_sync),
        sync_service.clone(),
    );

    let subspace_archiver = tokio::task::block_in_place(|| {
        create_subspace_archiver(
            segment_headers_store.clone(),
            subspace_link.clone(),
            client.clone(),
            config.create_object_mappings,
        )
    })
    .map_err(ServiceError::Client)?;

    task_manager
        .spawn_essential_handle()
        .spawn_essential_blocking(
            "subspace-archiver",
            None,
            Box::pin(async move {
                if let Err(error) = subspace_archiver.await {
                    error!(%error, "Archiver exited with error");
                }
            }),
        );

    if !config.base.network.force_synced {
        // Start with DSN sync in this case
        pause_sync.store(true, Ordering::Release);
    }

    let snap_sync_task = snap_sync(
        segment_headers_store.clone(),
        node.clone(),
        fork_id.map(|fork_id| fork_id.to_string()),
        Arc::clone(&client),
        import_queue_service1,
        pause_sync.clone(),
        piece_getter.clone(),
        sync_service.clone(),
        network_service_handle,
        subspace_link.erasure_coding().clone(),
    );

    let (observer, worker) = sync_from_dsn::create_observer_and_worker(
        segment_headers_store.clone(),
        Arc::clone(&network_service),
        node.clone(),
        Arc::clone(&client),
        import_queue_service2,
        sync_service.clone(),
        sync_target_block_number,
        pause_sync,
        piece_getter,
        subspace_link.erasure_coding().clone(),
    );
    task_manager
        .spawn_handle()
        .spawn("observer", Some("sync-from-dsn"), observer);
    task_manager
        .spawn_essential_handle()
        .spawn_essential_blocking(
            "worker",
            Some("sync-from-dsn"),
            Box::pin(async move {
                // Run snap-sync before DSN-sync.
                if config.sync == ChainSyncMode::Snap {
                    if let Err(error) = snap_sync_task.in_current_span().await {
                        error!(%error, "Snap sync exited with a fatal error");
                        return;
                    }
                }

                if let Err(error) = worker.await {
                    error!(%error, "Sync from DSN exited with an error");
                }
            }),
        );

    if let Some(registry) = substrate_prometheus_registry.as_ref() {
        match NodeMetrics::new(
            client.clone(),
            client.every_import_notification_stream(),
            registry,
        ) {
            Ok(node_metrics) => {
                task_manager.spawn_handle().spawn(
                    "node_metrics",
                    None,
                    Box::pin(async move {
                        node_metrics.run().await;
                    }),
                );
            }
            Err(err) => {
                error!("Failed to initialize node metrics: {err:?}");
            }
        }
    }

    let backoff_authoring_blocks: Option<()> = None;

    let new_slot_notification_stream = subspace_link.new_slot_notification_stream();
    let reward_signing_notification_stream = subspace_link.reward_signing_notification_stream();
    let block_importing_notification_stream = subspace_link.block_importing_notification_stream();
    let archived_segment_notification_stream = subspace_link.archived_segment_notification_stream();

    let (pot_source_worker, pot_gossip_worker, pot_slot_info_stream) = PotSourceWorker::new(
        config.is_timekeeper,
        config.timekeeper_cpu_cores,
        client.clone(),
        pot_verifier.clone(),
        Arc::clone(&network_service),
        pot_gossip_notification_service,
        sync_service.clone(),
        sync_oracle.clone(),
    )
    .map_err(|error| Error::Other(error.into()))?;

    let additional_pot_slot_info_stream = pot_source_worker.subscribe_pot_slot_info_stream();

    task_manager
        .spawn_essential_handle()
        .spawn("pot-source", Some("pot"), pot_source_worker.run());
    task_manager
        .spawn_essential_handle()
        .spawn("pot-gossip", Some("pot"), pot_gossip_worker.run());

    if config.base.role.is_authority() || config.force_new_slot_notifications {
        let proposer_factory = ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            substrate_prometheus_registry.as_ref(),
            None,
        );

        let subspace_slot_worker =
            SubspaceSlotWorker::<PosTable, _, _, _, _, _, _, _>::new(SubspaceSlotWorkerOptions {
                client: client.clone(),
                env: proposer_factory,
                block_import,
                sync_oracle: sync_oracle.clone(),
                justification_sync_link: sync_service.clone(),
                force_authoring: config.base.force_authoring,
                backoff_authoring_blocks,
                subspace_link: subspace_link.clone(),
                segment_headers_store: segment_headers_store.clone(),
                block_proposal_slot_portion,
                max_block_proposal_slot_portion: None,
                pot_verifier,
            });

        let create_inherent_data_providers = {
            let client = client.clone();
            let segment_headers_store = segment_headers_store.clone();

            move |parent_hash, ()| {
                let client = client.clone();
                let segment_headers_store = segment_headers_store.clone();

                async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let parent_header = client
                        .header(parent_hash)?
                        .expect("Parent header must always exist when block is created; qed");

                    let parent_block_number = parent_header.number;

                    let subspace_inherents =
                        sp_consensus_subspace::inherents::InherentDataProvider::new(
                            segment_headers_store
                                .segment_headers_for_block(parent_block_number + 1),
                        );

                    Ok((timestamp, subspace_inherents))
                }
            }
        };

        info!(target: "subspace", "🧑‍🌾 Starting Subspace Authorship worker");
        let slot_worker_task = sc_proof_of_time::start_slot_worker(
            subspace_link.chain_constants().slot_duration(),
            client.clone(),
            select_chain.clone(),
            subspace_slot_worker,
            sync_oracle.clone(),
            create_inherent_data_providers,
            pot_slot_info_stream,
        );

        // Subspace authoring task is considered essential, i.e. if it fails we take down the
        // service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "subspace-proposer",
            Some("block-authoring"),
            slot_worker_task,
        );
    }

    // We replace the Substrate implementation of metrics server with our own.
    config.base.prometheus_config.take();

    task_spawner::spawn_tasks(SpawnTasksParams {
        network: network_service.clone(),
        client: client.clone(),
        task_manager: &mut task_manager,
        transaction_pool: transaction_pool.clone(),
        rpc_builder: if enable_rpc_extensions {
            let client = client.clone();
            let new_slot_notification_stream = new_slot_notification_stream.clone();
            let reward_signing_notification_stream = reward_signing_notification_stream.clone();
            let archived_segment_notification_stream = archived_segment_notification_stream.clone();

            Box::new(move |subscription_executor| {
                let deps = rpc::FullDeps {
                    client: client.clone(),
                    subscription_executor,
                    new_slot_notification_stream: new_slot_notification_stream.clone(),
                    reward_signing_notification_stream: reward_signing_notification_stream.clone(),
                    archived_segment_notification_stream: archived_segment_notification_stream
                        .clone(),
                    dsn_bootstrap_nodes: dsn_bootstrap_nodes.clone(),
                    segment_headers_store: segment_headers_store.clone(),
                    sync_oracle: sync_oracle.clone(),
                    erasure_coding: subspace_link.erasure_coding().clone(),
                };

                rpc::create_full(deps).map_err(Into::into)
            })
        } else {
            Box::new(|_| Ok(RpcModule::new(())))
        },
        config: config.base,
        tx_handler_controller,
        sync_service: sync_service.clone(),
    })?;

    Ok(NewFull {
        task_manager,
        client,
        select_chain,
        network_service,
        sync_service,
        backend,
        pot_slot_info_stream: additional_pot_slot_info_stream,
        new_slot_notification_stream,
        reward_signing_notification_stream,
        block_importing_notification_stream,
        archived_segment_notification_stream,
        network_starter,
        transaction_pool,
    })
}
