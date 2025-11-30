mod chain_spec;

use crate::cli::CliCommand;
use crate::cli::run::chain_spec::ChainSpec;
use crate::storage_backend::FileStorageBackend;
use crate::{Error, PAGE_GROUP_SIZE};
use ab_cli_utils::shutdown_signal;
use ab_client_api::{ChainInfo, ChainSyncStatus};
use ab_client_archiving::archiving::{
    ArchiverTaskError, CreateObjectMappings, create_archiver_task,
};
use ab_client_archiving::segment_headers_store::SegmentHeadersStore;
use ab_client_block_authoring::slot_worker::{SubspaceSlotWorker, SubspaceSlotWorkerOptions};
use ab_client_block_builder::beacon_chain::BeaconChainBlockBuilder;
use ab_client_block_import::beacon_chain::BeaconChainBlockImport;
use ab_client_block_verification::beacon_chain::BeaconChainBlockVerification;
use ab_client_database::{
    ClientDatabase, ClientDatabaseError, ClientDatabaseFormatError, ClientDatabaseFormatOptions,
    ClientDatabaseOptions, GenesisBlockBuilderResult,
};
use ab_client_informer::run_informer;
use ab_client_proof_of_time::source::timekeeper::Timekeeper;
use ab_client_proof_of_time::source::{PotSourceWorker, init_pot_state};
use ab_client_proof_of_time::verifier::PotVerifier;
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::pot::PotSeed;
use ab_direct_io_file::DirectIoFile;
use ab_erasure_coding::ErasureCoding;
use ab_networking::libp2p::Multiaddr;
use ab_node_rpc_server::{FarmerRpc, FarmerRpcApiServer, FarmerRpcConfig};
use ab_proof_of_space::chia::ChiaTable;
use bytesize::ByteSize;
use clap::{Parser, ValueEnum};
use core_affinity::CoreId;
use futures::channel::mpsc;
use futures::prelude::*;
use futures::select;
use futures::task::noop_waker_ref;
use jsonrpsee::server::Server;
use rclite::Arc;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::pin::pin;
use std::sync::Arc as StdArc;
use std::task::Context;
use std::time::Duration;
use std::{io, thread};
use thread_priority::{ThreadPriority, set_current_thread_priority};
use tracing::{Span, error, info, warn};

// TODO: Get rid of this, make verifier clean up cache based on slots of finalized blocks
/// This is over 15 minutes of slots assuming there are no forks, should be both sufficient and not
/// too large to handle
const POT_VERIFIER_CACHE_SIZE: u32 = 30_000;
const INFORMER_INTERVAL: Duration = Duration::from_secs(5);

type PosTable = ChiaTable;

#[derive(Debug, Clone)]
struct ChainSyncStatusPlaceholder {}

impl ChainSyncStatus for ChainSyncStatusPlaceholder {
    #[inline(always)]
    fn target_block_number(&self) -> BlockNumber {
        BlockNumber::new(0)
    }

    #[inline(always)]
    fn is_syncing(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_offline(&self) -> bool {
        false
    }
}

/// Error for [`Run`]
#[derive(Debug, thiserror::Error)]
pub(crate) enum RunError {
    /// Bad option
    #[error("Bad option: {error}")]
    BadOption {
        /// Low-level error
        error: &'static str,
    },
    /// Failed to create a temporary database
    #[error("Failed to create a temporary database: {error}")]
    TemporaryDatabase {
        /// Low-level error
        error: io::Error,
    },
    /// Database path required
    #[error("Database path required, specify it with `--db-path`")]
    DatabasePathRequired,
    /// Failed to open the database file
    #[error("Failed to open the database file: {error}")]
    OpenDatabaseFile {
        /// Low-level error
        error: io::Error,
    },
    /// Failed to allocate the database
    #[error("Failed to allocate the database: {error}")]
    AllocateDatabase {
        /// Low-level error
        error: io::Error,
    },
    /// Failed to instantiate the storage backend
    #[error("Failed to instantiate the storage backend: {error}")]
    InstantiateStorageBackend {
        /// Low-level error
        error: io::Error,
    },
    /// Failed to format the database
    #[error("Failed to format the database: {error}")]
    FormatDatabase {
        /// Low-level error
        #[from]
        error: ClientDatabaseFormatError,
    },
    /// Failed to open the client database
    #[error("Failed to open the client database: {error}")]
    OpenClientDatabase {
        /// Low-level error
        #[from]
        error: ClientDatabaseError,
    },
    /// Failed to create an archiver task
    #[error("Failed to create an archiver task: {error}")]
    ArchiverTask {
        /// Low-level error
        #[from]
        error: ArchiverTaskError,
    },
    /// Failed to start farmer RPC server
    #[error("Failed to start farmer RPC server: {error}")]
    FarmerRpcServer {
        /// Low-level error
        error: io::Error,
    },
}

// TODO: Support loading serialized chain spec from a file?
/// Chain kind
#[derive(Debug, Copy, Clone, ValueEnum)]
enum ChainKind {
    Dev,
}

fn parse_timekeeper_cpu_cores(
    s: &str,
) -> Result<HashSet<usize>, Box<dyn std::error::Error + Send + Sync>> {
    if s.is_empty() {
        return Ok(HashSet::new());
    }

    let mut cpu_cores = HashSet::new();
    for s in s.split(',') {
        let mut parts = s.split('-');
        let range_start = parts
            .next()
            .ok_or("Bad string format. Must be a comma-separated list of CPU cores or ranges.")?
            .parse()?;
        if let Some(range_end) = parts.next() {
            let range_end = range_end.parse()?;

            cpu_cores.extend(range_start..=range_end);
        } else {
            cpu_cores.insert(range_start);
        }
    }

    Ok(cpu_cores)
}

/// Options for timekeeper
#[derive(Debug, Parser)]
struct TimekeeperOptions {
    /// Assigned PoT role for this node.
    #[arg(long)]
    timekeeper: bool,

    /// CPU cores that timekeeper can use.
    ///
    /// At least 2 cores should be provided, if more cores than necessary are provided, random cores
    /// out of provided will be utilized, if not enough cores are provided, timekeeper may occupy
    /// random CPU cores.
    ///
    /// Comma-separated list of individual cores or ranges of cores.
    ///
    /// Examples:
    /// * `0,1` - use cores 0 and 1
    /// * `0-3` - use cores 0, 1, 2 and 3
    /// * `0,1,6-7` - use cores 0, 1, 6 and 7
    #[arg(long, default_value = "", value_parser = parse_timekeeper_cpu_cores, verbatim_doc_comment)]
    timekeeper_cpu_cores: HashSet<usize>,
}

/// Options for DSN
#[derive(Debug, Parser)]
struct NetworkOptions {
    // TODO: Un-comment once networking stack is added to dependencies
    // /// Listen for incoming connections on these multiaddresses
    // #[arg(long, default_values_t = [
    //     Multiaddr::from(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
    //         .with(Protocol::Tcp(30433)),
    //     Multiaddr::from(IpAddr::V6(Ipv6Addr::UNSPECIFIED))
    //         .with(Protocol::Tcp(30433))
    // ])]
    // listen_on: Vec<Multiaddr>,
    /// Bootstrap nodes
    #[arg(long = "bootstrap-node")]
    bootstrap_nodes: Vec<Multiaddr>,
    // /// Reserved peers
    // #[arg(long = "reserved-peer")]
    // reserved_peers: Vec<Multiaddr>,
    //
    // /// Maximum established incoming connection limit
    // #[arg(long, default_value_t = 50)]
    // in_connections: u32,
    //
    // /// Maximum established outgoing swarm connection limit
    // #[arg(long, default_value_t = 150)]
    // out_connections: u32,
    //
    // /// Maximum pending incoming connection limit
    // #[arg(long, default_value_t = 100)]
    // pending_in_connections: u32,
    //
    // /// Maximum pending outgoing swarm connection limit
    // #[arg(long, default_value_t = 150)]
    // pending_out_connections: u32,
    //
    // /// Known external addresses.
    // #[arg(long = "external-address")]
    // external_addresses: Vec<Multiaddr>,
}

fn derive_pot_external_entropy<'a>(
    chain_spec: &'a ChainSpec,
    maybe_pot_external_entropy: Option<&'a [u8]>,
) -> &'a [u8] {
    let maybe_chain_spec_pot_external_entropy = chain_spec.pot_external_entropy();
    if maybe_chain_spec_pot_external_entropy.is_some()
        && maybe_pot_external_entropy.is_some()
        && maybe_chain_spec_pot_external_entropy != maybe_pot_external_entropy
    {
        warn!(
            "`--pot-external-entropy` CLI argument was ignored due to chain spec having a \
            different explicit value"
        );
    }
    maybe_chain_spec_pot_external_entropy
        .or(maybe_pot_external_entropy)
        .unwrap_or_default()
}

/// Run the blockchain node
#[derive(Debug, Parser)]
pub(crate) struct Run {
    /// Path to the database file.
    ///
    /// Required unless --dev mode is used.
    #[arg(long)]
    db_path: Option<PathBuf>,
    // TODO: Use enum with chain specs instead of a string
    /// Chain kind to use
    #[arg(long)]
    chain: Option<ChainKind>,
    // TODO: Update flags in the docs once something is actually runnable
    /// Enable development mode.
    ///
    /// Implies following flags (unless customized):
    /// * `--chain dev` (unless specified explicitly)
    /// * `--farmer`
    /// * `--tmp` (unless `--db-path` specified explicitly)
    /// * `--force-synced`
    /// * `--force-authoring`
    /// * `--create-object-mappings`
    /// * `--allow-private-ips`
    /// * `--dsn-disable-bootstrap-on-start`
    /// * `--timekeeper`
    #[arg(long, verbatim_doc_comment)]
    dev: bool,
    // TODO: This should take database size as an argument like on the farmer
    /// Run a temporary node.
    ///
    /// This will create a temporary database file that will be deleted when the node exits.
    #[arg(long)]
    tmp: bool,
    // TODO: This is only for farmer, would be nice to have a binary protocol instead of JSON-RPC
    /// IP and port (TCP) on which to listen for farmer RPC requests.
    #[arg(long, default_value_t = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        9944,
    ))]
    farmer_rpc_listen_on: SocketAddr,
    /// IP and port (TCP) to start Prometheus exporter on
    #[clap(long)]
    prometheus_listen_on: Option<SocketAddr>,
    /// Make the node forcefully assume it is synced, needed for network bootstrapping only. As
    /// long as two synced nodes remain on the network at any time, this doesn't need to be used.
    ///
    /// --dev mode enables this option automatically.
    #[clap(long)]
    force_synced: bool,
    /// Enable authoring even when offline, needed for network bootstrapping only.
    #[arg(long)]
    force_authoring: bool,
    // TODO: A better type than a string here
    /// External entropy, used initially when the PoT chain starts to derive the first seed
    #[arg(long)]
    pot_external_entropy: Option<String>,
    /// Network options
    #[clap(flatten)]
    network_options: NetworkOptions,
    // TODO: Timekeeper should eventually be a separate binary, meaning this will only be used for
    //  `--dev` mode and timekeeper feature should probably be removed from the node
    #[clap(flatten)]
    timekeeper_options: TimekeeperOptions,
}

impl CliCommand for Run {
    fn run(self) -> Result<(), Error> {
        Ok(self.run()?)
    }
}

impl Run {
    #[tokio::main]
    async fn run(self) -> Result<(), RunError> {
        let Self {
            db_path,
            mut chain,
            dev,
            mut tmp,
            farmer_rpc_listen_on,
            prometheus_listen_on,
            mut force_synced,
            mut force_authoring,
            pot_external_entropy,
            network_options,
            mut timekeeper_options,
        } = self;

        let mut shutdown_signal_fut = pin!(shutdown_signal());
        // Poll once to register signal handlers and ensure a graceful shutdown later
        let _ = shutdown_signal_fut.poll_unpin(&mut Context::from_waker(noop_waker_ref()));

        // Development mode handling is limited to this section
        {
            if dev {
                if chain.is_none() {
                    chain = Some(ChainKind::Dev);
                }
                tmp = true;
                force_synced = true;
                force_authoring = true;
                // TODO: Un-comment once networking stack is integrated
                // network_options.allow_private_ips = true;
                timekeeper_options.timekeeper = true;
            }
        }

        let chain_spec = match chain {
            Some(ChainKind::Dev) => ChainSpec::new(),
            None => {
                return Err(RunError::BadOption {
                    error: "Chain must be provided unless `--dev` mode is used",
                });
            }
        };

        let mut maybe_tmp_file = None;
        let db_path = match db_path {
            Some(db_path) => db_path,
            None => {
                if tmp {
                    let tmp = tempfile::Builder::new()
                        .prefix("ab-node-")
                        .tempfile()
                        .map_err(|error| RunError::TemporaryDatabase { error })?;

                    maybe_tmp_file.insert(tmp).path().to_path_buf()
                } else {
                    return Err(RunError::DatabasePathRequired);
                }
            }
        };

        let file = DirectIoFile::open(
            {
                let mut open_options = OpenOptions::new();
                open_options.read(true).write(true);
                open_options
            },
            &db_path,
        )
        .map_err(|error| RunError::OpenDatabaseFile { error })?;

        if maybe_tmp_file.is_some() {
            // TODO: Proper database size calculation here
            let size = ByteSize::gib(1).as_u64();

            // Allocating the whole file (`set_len` below can create a sparse file, which will cause
            // writes to fail later)
            file.allocate(size)
                .map_err(|error| RunError::AllocateDatabase { error })?;

            // Truncating the file (if necessary)
            file.set_len(size)
                .map_err(|error| RunError::AllocateDatabase { error })?;
        }

        let storage_backend = FileStorageBackend::new(Arc::new(file))
            .map_err(|error| RunError::InstantiateStorageBackend { error })?;

        if maybe_tmp_file.is_some() {
            ClientDatabase::<OwnedBeaconChainBlock, _>::format(
                &storage_backend,
                ClientDatabaseFormatOptions {
                    page_group_size: PAGE_GROUP_SIZE,
                    force: true,
                },
            )
            .await?;
        }

        let genesis_block = chain_spec.genesis_block();
        let consensus_constants = *chain_spec.consensus_constants();

        let client_database =
            ClientDatabase::<OwnedBeaconChainBlock, _>::open(ClientDatabaseOptions {
                confirmation_depth_k: consensus_constants.confirmation_depth_k,
                genesis_block_builder: || GenesisBlockBuilderResult {
                    block: genesis_block.clone(),
                    // TODO: Fill correct initial state
                    system_contract_states: StdArc::new([]),
                },
                storage_backend,
                ..
            })
            .await?;

        info!("✌️ Abundance {}", env!("CARGO_PKG_VERSION"));
        // TODO: Un-comment when there is a chain spec notion
        info!("📋 Chain specification: {}", chain_spec.name(),);
        info!("💾 Database path: {}", db_path.display());

        let pot_external_entropy = derive_pot_external_entropy(
            &chain_spec,
            pot_external_entropy.as_deref().map(|s| s.as_bytes()),
        );

        let pot_verifier = PotVerifier::new(
            PotSeed::from_genesis(&genesis_block.header.header().root(), pot_external_entropy),
            POT_VERIFIER_CACHE_SIZE,
        );

        // TODO: This should move into the database
        let segment_headers_store =
            SegmentHeadersStore::new(consensus_constants.confirmation_depth_k);

        let best_beacon_chain_header = client_database.best_header();

        let pot_state = Arc::new(init_pot_state(
            best_beacon_chain_header.header(),
            pot_verifier.clone(),
            consensus_constants.block_authoring_delay,
        ));

        let mut timekeeper_proof_receiver = None;
        if timekeeper_options.timekeeper {
            let span = Span::current();
            let (timekeeper_source, proof_receiver) =
                Timekeeper::new(Arc::clone(&pot_state), pot_verifier.clone());
            timekeeper_proof_receiver.replace(proof_receiver);

            thread::Builder::new()
                .name("timekeeper".to_string())
                .spawn(move || {
                    let _guard = span.enter();

                    if let Some(core) = timekeeper_options.timekeeper_cpu_cores.into_iter().next()
                        && !core_affinity::set_for_current(CoreId { id: core })
                    {
                        warn!(
                            %core,
                            "Failed to set core affinity, timekeeper will run on random CPU \
                            core",
                        );
                    }

                    if let Err(error) = set_current_thread_priority(ThreadPriority::Max) {
                        warn!(
                            %error,
                            "Failed to set thread priority, timekeeper performance may be \
                            negatively impacted by other software running on this machine",
                        );
                    }

                    if let Err(error) = timekeeper_source.run() {
                        error!(%error, "Timekeeper exited with an error");
                    }
                })
                .expect("Thread creation must not panic");
        }

        // TODO: These are currently not implementable, but should be eventually
        // let (pot_gossip_worker, to_gossip_sender, from_gossip_receiver) =
        //     PotGossipWorker::<Block>::new(
        //         pot_verifier.clone(),
        //         Arc::clone(&pot_state),
        //         StdArc::clone(&network_service),
        //         pot_gossip_notification_service,
        //         sync_service.clone(),
        //         sync_oracle.clone(),
        //     );
        // let (best_block_pot_source, best_block_pot_info_receiver) =
        //     BestBlockPotSource::new(client.clone()).map_err(|error| Error::Other(error.into()))?;
        // TODO: Code below is just a placeholder
        let (to_gossip_sender, to_gossip_receiver) = mpsc::channel(10);
        let (from_gossip_sender, from_gossip_receiver) = mpsc::channel(10);
        let (best_block_pot_info_sender, best_block_pot_info_receiver) = mpsc::channel(1);

        let chain_sync_status = ChainSyncStatusPlaceholder {};

        let (pot_source_worker, pot_slot_info_stream) = PotSourceWorker::new(
            timekeeper_proof_receiver,
            to_gossip_sender,
            from_gossip_receiver,
            best_block_pot_info_receiver,
            chain_sync_status.clone(),
            pot_state,
        );

        // TODO: Better thread management, probably move to its own dedicated thread
        tokio::spawn(pot_source_worker.run());

        let block_builder = BeaconChainBlockBuilder::new(
            segment_headers_store.clone(),
            consensus_constants,
            client_database.clone(),
        );

        let block_verification = BeaconChainBlockVerification::<PosTable, _, _>::new(
            segment_headers_store.clone(),
            consensus_constants,
            pot_verifier.clone(),
            client_database.clone(),
            chain_sync_status.clone(),
        );

        let (block_importing_notification_sender, block_importing_notification_receiver) =
            mpsc::channel(1);
        let block_import = BeaconChainBlockImport::<PosTable, _, _>::new(
            client_database.clone(),
            block_verification,
            block_importing_notification_sender,
        );

        let (new_slot_notification_sender, new_slot_notification_receiver) = mpsc::channel(1);
        let (block_sealing_notification_sender, block_sealing_notification_receiver) =
            mpsc::channel(0);
        let (archived_segment_notification_sender, archived_segment_notification_receiver) =
            mpsc::channel(0);

        let erasure_coding = ErasureCoding::new();

        let (farmer_rpc, farmer_rpc_worker) = FarmerRpc::new(FarmerRpcConfig {
            genesis_block,
            consensus_constants,
            // TODO: Query it from an actual chain
            max_pieces_in_sector: 1000,
            new_slot_notification_receiver,
            block_sealing_notification_receiver,
            archived_segment_notification_receiver,
            // TODO: Correct values once networking stack is integrated
            dsn_bootstrap_nodes: Vec::new(),
            segment_headers_store: segment_headers_store.clone(),
            chain_sync_status: chain_sync_status.clone(),
            erasure_coding: erasure_coding.clone(),
        });

        let server = Server::builder()
            .build(farmer_rpc_listen_on)
            .await
            .map_err(|error| RunError::FarmerRpcServer { error })?;

        {
            let address = server
                .local_addr()
                .map_err(|error| RunError::FarmerRpcServer { error })?;
            info!(%address, "Started farmer RPC server");
        }

        // TODO: Better thread management, probably move to its own dedicated thread
        tokio::spawn(farmer_rpc_worker.run());

        // TODO: Initialize in a blocking task
        let archiver_task = tokio::task::block_in_place(|| {
            create_archiver_task(
                segment_headers_store.clone(),
                client_database.clone(),
                block_importing_notification_receiver,
                archived_segment_notification_sender,
                consensus_constants,
                CreateObjectMappings::No,
                erasure_coding,
            )
        })?;

        // TODO: Better thread management, probably move to its own dedicated thread
        tokio::spawn(archiver_task);

        let slot_worker =
            SubspaceSlotWorker::<PosTable, _, _, _, _, _, _>::new(SubspaceSlotWorkerOptions {
                block_builder,
                block_import,
                beacon_chain_info: client_database.clone(),
                chain_info: client_database.clone(),
                chain_sync_status,
                force_authoring,
                new_slot_notification_sender,
                block_sealing_notification_sender,
                segment_headers_store,
                consensus_constants,
                pot_verifier,
            });

        // TODO: Better thread management, probably move to its own dedicated thread
        tokio::spawn(slot_worker.run(pot_slot_info_stream));

        // TODO: Code below is just a placeholder
        tokio::spawn(async move {
            let mut to_gossip_receiver = to_gossip_receiver.fuse();

            select! {
                _ = to_gossip_receiver.next() => {
                    // TODO
                }
            }

            std::future::pending::<()>().await;

            drop(from_gossip_sender);
            drop(best_block_pot_info_sender);
        });

        // TODO: Better thread management, probably move to its own dedicated thread
        tokio::spawn(server.start(farmer_rpc.into_rpc()).stopped());

        // TODO: Better thread management, probably move to its own dedicated thread
        tokio::spawn(async move { run_informer(&client_database, INFORMER_INTERVAL).await });

        // TODO: This is just a placeholder to keep the node running
        shutdown_signal_fut.await;

        // TODO: These should be used
        let _ = force_synced;
        let _ = prometheus_listen_on;
        let _ = network_options;

        Ok(())
    }
}
