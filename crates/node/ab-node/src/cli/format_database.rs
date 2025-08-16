use crate::cli::CliCommand;
use crate::storage_backend::FileStorageBackend;
use crate::{Error, PAGE_GROUP_SIZE};
use ab_client_database::{ClientDatabase, ClientDatabaseFormatError, ClientDatabaseFormatOptions};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_direct_io_file::DirectIoFile;
use bytesize::ByteSize;
use clap::Parser;
use rclite::Arc;
use std::fs::OpenOptions;
use std::io;
use std::path::PathBuf;

/// Error for [`FormatDatabase`]
#[derive(Debug, thiserror::Error)]
pub(crate) enum FormatDatabaseError {
    /// Failed to open the database
    #[error("Failed to open the database: {error}")]
    OpenDatabase {
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
}

/// Format a database file/disk
#[derive(Debug, Parser)]
pub(crate) struct FormatDatabase {
    /// Path to the database/disk
    path: PathBuf,
    /// Database size to format to (for files).
    ///
    /// For disks (block devices) can be skipped.
    #[arg(long)]
    size: Option<ByteSize>,
    /// Force formatting of the existing database
    #[arg(long)]
    force: bool,
}

impl CliCommand for FormatDatabase {
    fn run(self) -> Result<(), Error> {
        Ok(self.run()?)
    }
}

impl FormatDatabase {
    #[tokio::main]
    async fn run(self) -> Result<(), FormatDatabaseError> {
        let Self { path, size, force } = self;

        let file = DirectIoFile::open(
            {
                let mut open_options = OpenOptions::new();
                open_options
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false);

                open_options
            },
            path,
        )
        .map_err(|error| FormatDatabaseError::OpenDatabase { error })?;

        if let Some(size) = size {
            let size = size.as_u64();

            // Allocating the whole file (`set_len` below can create a sparse file, which will cause
            // writes to fail later)
            file.allocate(size)
                .map_err(|error| FormatDatabaseError::AllocateDatabase { error })?;

            // Truncating file (if necessary)
            file.set_len(size)
                .map_err(|error| FormatDatabaseError::AllocateDatabase { error })?;
        }

        let storage_backend = FileStorageBackend::new(Arc::new(file))
            .map_err(|error| FormatDatabaseError::InstantiateStorageBackend { error })?;

        ClientDatabase::<OwnedBeaconChainBlock, _>::format(
            &storage_backend,
            ClientDatabaseFormatOptions {
                page_group_size: PAGE_GROUP_SIZE,
                force,
            },
        )
        .await?;

        Ok(())
    }
}
