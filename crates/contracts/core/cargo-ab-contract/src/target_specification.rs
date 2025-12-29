use anyhow::Context;
use dirs::cache_dir;
use std::fs::{File, create_dir_all};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

const TARGET_SPECIFICATION: &str = include_str!("riscv64em-unknown-none-abundance.json");

/// Target specification for contracts
#[derive(Debug)]
pub struct TargetSpecification {
    path: PathBuf,
    _file: File,
}

impl TargetSpecification {
    /// Create a target specification instance
    pub fn create() -> anyhow::Result<Self> {
        let app_dir = cache_dir()
            .context("Failed to get cache directory")?
            .join(env!("CARGO_PKG_NAME"));
        create_dir_all(&app_dir)
            .with_context(|| format!("Failed to create cache directory {}", app_dir.display()))?;

        let path = app_dir.join("riscv64em-unknown-none-abundance.json");
        let mut file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .context("Failed to open target specification file")?;

        // Ensure the target specification file has expected content
        loop {
            file.lock_shared()
                .context("Failed to lock target specification file")?;

            let mut actual_target_specification = String::with_capacity(TARGET_SPECIFICATION.len());
            file.seek(std::io::SeekFrom::Start(0))
                .context("Failed to seek to start of target specification file")?;
            file.read_to_string(&mut actual_target_specification)
                .context("Failed to read target specification file")?;

            if actual_target_specification == TARGET_SPECIFICATION {
                break;
            }

            file.unlock()
                .context("Failed to unlock target specification file")?;
            file.lock()
                .context("Failed to lock target specification file")?;
            file.set_len(0)
                .context("Failed to truncate target specification file")?;
            file.seek(std::io::SeekFrom::Start(0))
                .context("Failed to seek to start of target specification file")?;
            file.write_all(TARGET_SPECIFICATION.as_bytes())
                .context("Failed to write target specification file")?;
            file.sync_all()
                .context("Failed to sync target specification file")?;
            file.unlock()
                .context("Failed to unlock target specification file")?;
        }

        Ok(Self { path, _file: file })
    }

    /// Get the path to the target specification file
    pub fn path(&self) -> &Path {
        &self.path
    }
}
