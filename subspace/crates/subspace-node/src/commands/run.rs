mod consensus;
mod shared;

use crate::commands::run::consensus::{
    create_consensus_chain_configuration, ConsensusChainConfiguration, ConsensusChainOptions,
};
use crate::{set_default_ss58_version, Error, PosTable};
use clap::Parser;
use futures::FutureExt;
use sc_cli::Signals;
use sc_consensus_slots::SlotProportion;
use sc_storage_monitor::StorageMonitorService;
use std::env;
use subspace_logging::init_logger;
use subspace_metrics::{start_prometheus_metrics_server, RegistryAdapter};
use subspace_runtime::RuntimeApi;
use subspace_service::config::ChainSyncMode;
use tracing::{debug, info, info_span, warn};

/// Options for running a node
#[derive(Debug, Parser)]
pub struct RunOptions {
    /// Consensus chain options
    #[clap(flatten)]
    consensus: ConsensusChainOptions,
}

fn raise_fd_limit() {
    match fdlimit::raise_fd_limit() {
        Ok(fdlimit::Outcome::LimitRaised { from, to }) => {
            debug!(
                "Increased file descriptor limit from previous (most likely soft) limit {} to \
                new (most likely hard) limit {}",
                from, to
            );
        }
        Ok(fdlimit::Outcome::Unsupported) => {
            // Unsupported platform (a platform other than Linux or macOS)
        }
        Err(error) => {
            warn!(
                "Failed to increase file descriptor limit for the process due to an error: {}.",
                error
            );
        }
    }
}

/// Default run command for node
#[tokio::main]
pub async fn run(run_options: RunOptions) -> Result<(), Error> {
    init_logger();
    raise_fd_limit();
    let signals = Signals::capture()?;

    let RunOptions { consensus } = run_options;

    let ConsensusChainConfiguration {
        maybe_tmp_dir: _maybe_tmp_dir,
        subspace_configuration,
        pot_external_entropy,
        storage_monitor,
        mut prometheus_configuration,
    } = create_consensus_chain_configuration(consensus)?;

    set_default_ss58_version(subspace_configuration.chain_spec.as_ref());

    let base_path = subspace_configuration.base_path.path().to_path_buf();

    info!("Subspace");
    info!("✌️  version {}", env!("SUBSTRATE_CLI_IMPL_VERSION"));
    info!("❤️  by {}", env!("CARGO_PKG_AUTHORS"));
    info!(
        "📋 Chain specification: {}",
        subspace_configuration.chain_spec.name()
    );
    info!("🏷  Node name: {}", subspace_configuration.network.node_name);
    info!("💾 Node path: {}", base_path.display());

    let mut task_manager = {
        let consensus_chain_node = {
            let span = info_span!("Consensus");
            let _enter = span.enter();

            let partial_components = subspace_service::new_partial::<PosTable, RuntimeApi>(
                &subspace_configuration,
                match subspace_configuration.sync {
                    ChainSyncMode::Full => false,
                    ChainSyncMode::Snap => true,
                },
                &pot_external_entropy,
            )
            .map_err(|error| {
                sc_service::Error::Other(format!(
                    "Failed to build a full subspace node 1: {error:?}"
                ))
            })?;

            let full_node_fut = subspace_service::new_full::<PosTable, _>(
                subspace_configuration,
                partial_components,
                prometheus_configuration
                    .as_mut()
                    .map(|prometheus_configuration| {
                        &mut prometheus_configuration.prometheus_registry
                    }),
                true,
                SlotProportion::new(3f32 / 4f32),
            );

            full_node_fut.await.map_err(|error| {
                sc_service::Error::Other(format!(
                    "Failed to build a full subspace node 3: {error:?}"
                ))
            })?
        };

        StorageMonitorService::try_spawn(
            storage_monitor,
            base_path,
            &consensus_chain_node.task_manager.spawn_essential_handle(),
        )
        .map_err(|error| {
            sc_service::Error::Other(format!("Failed to start storage monitor: {error:?}"))
        })?;

        consensus_chain_node.network_starter.start_network();

        if let Some(prometheus_configuration) = prometheus_configuration.take() {
            let metrics_server = start_prometheus_metrics_server(
                vec![prometheus_configuration.listen_on],
                RegistryAdapter::Both(
                    prometheus_configuration.prometheus_registry,
                    prometheus_configuration.substrate_registry,
                ),
            )
            .map_err(|error| Error::SubspaceService(error.into()))?
            .map(|error| {
                debug!(?error, "Metrics server error.");
            });

            consensus_chain_node.task_manager.spawn_handle().spawn(
                "metrics-server",
                None,
                metrics_server,
            );
        };

        consensus_chain_node.task_manager
    };

    signals
        .run_until_signal(task_manager.future().fuse())
        .await
        .map_err(Into::into)
}
