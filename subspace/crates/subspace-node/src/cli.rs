use crate::chain_spec;
use crate::commands::{RunOptions, WipeOptions};
use clap::Parser;
use sc_chain_spec::GenericChainSpec;
use sc_cli::SubstrateCli;
use sc_service::ChainSpec;

/// Commands for working with a node.
#[derive(Debug, Parser)]
#[clap(about, version)]
#[allow(clippy::large_enum_variant)]
pub enum Cli {
    /// Run blockchain node
    Run(RunOptions),

    /// Build a chain specification.
    BuildSpec(sc_cli::BuildSpecCmd),

    /// Validate blocks.
    CheckBlock(sc_cli::CheckBlockCmd),

    /// Export blocks.
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),

    /// Import blocks.
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// Remove all node's data
    Wipe(WipeOptions),

    /// Revert the chain to a previous state.
    Revert(sc_cli::RevertCmd),

    /// Db meta columns information.
    ChainInfo(sc_cli::ChainInfoCmd),
}

/// Fake Subspace CLI just to satisfy Substrate's API
pub struct SubspaceCliPlaceholder;

impl SubstrateCli for SubspaceCliPlaceholder {
    fn impl_name() -> String {
        "Subspace".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn executable_name() -> String {
        // Customize to make sure directory used for data by default is the same regardless of the
        // name of the executable file.
        "subspace-node".to_string()
    }

    fn description() -> String {
        env!("CARGO_PKG_DESCRIPTION").into()
    }

    fn author() -> String {
        env!("CARGO_PKG_AUTHORS").into()
    }

    fn support_url() -> String {
        "https://forum.subspace.network".into()
    }

    fn copyright_start_year() -> i32 {
        2021
    }

    fn load_spec(&self, id: &str) -> Result<Box<dyn ChainSpec>, String> {
        let chain_spec = match id {
            "mainnet-compiled" => chain_spec::mainnet_compiled()?,
            "devnet-compiled" => chain_spec::devnet_config_compiled()?,
            "dev" => chain_spec::dev_config()?,
            path => GenericChainSpec::from_json_file(std::path::PathBuf::from(path))?,
        };

        Ok(Box::new(chain_spec))
    }
}
