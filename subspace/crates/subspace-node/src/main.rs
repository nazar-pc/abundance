//! Subspace node implementation.

mod commands;

mod chain_spec;
mod chain_spec_utils;
mod cli;

use crate::cli::{Cli, SubspaceCliPlaceholder};
use crate::commands::set_exit_on_panic;
use ab_proof_of_space::chia::ChiaTable;
use clap::Parser;
use futures::future::TryFutureExt;
use sc_cli::SubstrateCli;
use sc_service::{Configuration, PartialComponents};
use serde_json::Value;
use subspace_runtime::{Block, RuntimeApi};
use tracing::warn;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

type PosTable = ChiaTable;

/// Subspace node error.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Subspace service error.
    #[error(transparent)]
    SubspaceService(#[from] subspace_service::Error),

    /// CLI error.
    #[error(transparent)]
    SubstrateCli(#[from] sc_cli::Error),

    /// Substrate service error.
    #[error(transparent)]
    SubstrateService(#[from] sc_service::Error),

    /// Other kind of error.
    #[error("Other: {0}")]
    Other(String),
}

impl From<String> for Error {
    #[inline]
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

#[expect(clippy::result_large_err, reason = "Comes from Substrate")]
fn derive_pot_external_entropy(
    consensus_chain_config: &Configuration,
    maybe_pot_external_entropy: Option<String>,
) -> Result<Vec<u8>, sc_service::Error> {
    let maybe_chain_spec_pot_external_entropy = consensus_chain_config
        .chain_spec
        .properties()
        .get("potExternalEntropy")
        .map(|d| match d.clone() {
            Value::String(s) => Ok(Some(s)),
            Value::Null => Ok(None),
            _ => Err(sc_service::Error::Other(
                "Failed to decode PoT initial key".to_string(),
            )),
        })
        .transpose()?
        .flatten();
    if maybe_chain_spec_pot_external_entropy.is_some()
        && maybe_pot_external_entropy.is_some()
        && maybe_chain_spec_pot_external_entropy != maybe_pot_external_entropy
    {
        warn!(
            "--pot-external-entropy CLI argument was ignored due to chain spec having a different \
            explicit value"
        );
    }
    Ok(maybe_chain_spec_pot_external_entropy
        .or(maybe_pot_external_entropy)
        .unwrap_or_default()
        .into_bytes())
}

#[expect(clippy::result_large_err, reason = "Comes from Substrate")]
fn main() -> Result<(), Error> {
    set_exit_on_panic();

    match Cli::parse() {
        Cli::Run(run_options) => {
            commands::run(run_options)?;
        }
        Cli::BuildSpec(cmd) => {
            let runner = SubspaceCliPlaceholder.create_runner(&cmd)?;
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))?
        }
        Cli::CheckBlock(cmd) => {
            let runner = SubspaceCliPlaceholder.create_runner(&cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    import_queue,
                    task_manager,
                    ..
                } = subspace_service::new_partial::<PosTable, RuntimeApi>(
                    &config,
                    false,
                    &derive_pot_external_entropy(&config, None)?,
                )?;
                Ok((
                    cmd.run(client, import_queue).map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?;
        }
        Cli::ExportBlocks(cmd) => {
            let runner = SubspaceCliPlaceholder.create_runner(&cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = subspace_service::new_partial::<PosTable, RuntimeApi>(
                    &config,
                    false,
                    &derive_pot_external_entropy(&config, None)?,
                )?;
                Ok((
                    cmd.run(client, config.database)
                        .map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?;
        }
        Cli::ExportState(cmd) => {
            let runner = SubspaceCliPlaceholder.create_runner(&cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = subspace_service::new_partial::<PosTable, RuntimeApi>(
                    &config,
                    false,
                    &derive_pot_external_entropy(&config, None)?,
                )?;
                Ok((
                    cmd.run(client, config.chain_spec)
                        .map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?;
        }
        Cli::ImportBlocks(cmd) => {
            let runner = SubspaceCliPlaceholder.create_runner(&cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    import_queue,
                    task_manager,
                    ..
                } = subspace_service::new_partial::<PosTable, RuntimeApi>(
                    &config,
                    false,
                    &derive_pot_external_entropy(&config, None)?,
                )?;
                Ok((
                    cmd.run(client, import_queue).map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?;
        }
        Cli::Wipe(wipe_options) => {
            commands::wipe(wipe_options).map_err(|error| Error::Other(error.to_string()))?;
        }
        Cli::Revert(cmd) => {
            let runner = SubspaceCliPlaceholder.create_runner(&cmd)?;
            runner.async_run(|config| {
                let PartialComponents {
                    client,
                    backend,
                    task_manager,
                    ..
                } = subspace_service::new_partial::<PosTable, RuntimeApi>(
                    &config,
                    false,
                    &derive_pot_external_entropy(&config, None)?,
                )?;
                Ok((
                    cmd.run(client, backend, None).map_err(Error::SubstrateCli),
                    task_manager,
                ))
            })?;
        }
        Cli::ChainInfo(cmd) => {
            let runner = SubspaceCliPlaceholder.create_runner(&cmd)?;
            runner.sync_run(|config| cmd.run::<Block>(&config))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use sc_cli::Database;

    #[test]
    fn rocksdb_disabled_in_substrate() {
        assert_eq!(
            Database::variants(),
            &["paritydb", "paritydb-experimental", "auto"],
        );
    }
}
