//! Consensus node CLI

mod cli;
mod storage_backend;

use crate::cli::CliCommand;
use crate::cli::format_database::{FormatDatabase, FormatDatabaseError};
use crate::cli::run::{Run, RunError};
use ab_cli_utils::{init_logger, set_exit_on_panic};
use ab_client_database::storage_backend::AlignedPage;
use bytesize::ByteSize;
use clap::Parser;
use std::num::NonZeroU32;

/// This is the current recommended page group size.
///
/// It might become customizable in the future, but should not be necessary for now.
const PAGE_GROUP_SIZE: NonZeroU32 =
    NonZeroU32::new((ByteSize::mib(256).as_u64() / AlignedPage::SIZE as u64) as u32)
        .expect("Not zero; qed");

/// Node CLI
#[derive(Debug, Parser)]
#[clap(about, version)]
enum Cli {
    /// Format a database file/disk
    FormatDatabase(FormatDatabase),
    /// Run the blockchain node
    Run(Run),
}

#[derive(Debug, thiserror::Error)]
enum Error {
    /// Format database error
    #[error("Format database error: {0}")]
    FormatDatabase(#[from] FormatDatabaseError),
    /// Run error
    #[error("Run error: {0}")]
    Run(#[from] RunError),
}

fn main() -> Result<(), Error> {
    set_exit_on_panic();
    init_logger();

    match Cli::parse() {
        Cli::FormatDatabase(cmd) => cmd.run(),
        Cli::Run(cmd) => cmd.run(),
    }
}
