use crate::Error;
use crate::cli::CliCommand;
use clap::Parser;

/// Error for [`Run`]
#[derive(Debug, thiserror::Error)]
pub(crate) enum RunError {
    // /// Failed to open the database
    // #[error("Failed to open the database: {error}")]
    // OpenDatabase {
    //     /// Low-level error
    //     error: io::Error,
    // },
}

/// Run the blockchain node
#[derive(Debug, Parser)]
pub(crate) struct Run {
    // TODO
}

impl CliCommand for Run {
    fn run(self) -> Result<(), Error> {
        todo!()
    }
}
