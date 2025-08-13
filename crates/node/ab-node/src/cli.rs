pub(crate) mod format_database;
pub(crate) mod run;

use crate::Error;

pub(crate) trait CliCommand {
    /// Run the command
    fn run(self) -> Result<(), Error>;
}
