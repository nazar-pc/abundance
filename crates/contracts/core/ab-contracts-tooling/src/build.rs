//! Build an ELF `cdylib` with the contract

use anyhow::Context;
use cargo_metadata::MetadataCommand;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::debug;

/// Options for building a contract
#[derive(Debug)]
pub struct BuildOptions<'a> {
    /// Package to build.
    ///
    /// A package in the current directory is built if not specified explicitly.
    pub package: Option<&'a str>,
    /// Comma separated list of features to activate
    pub features: Option<&'a str>,
    /// Build artifacts with the specified profile
    pub profile: &'a str,
    /// Path to the target specification JSON file
    pub target_specification_path: &'a Path,
    /// Custom target directory to use instead of the default one
    pub target_dir: Option<&'a Path>,
}

/// Build a `cdylib` with the contract and return the path to the resulting ELF file
pub fn build_cdylib(options: BuildOptions<'_>) -> anyhow::Result<PathBuf> {
    let BuildOptions {
        package,
        features,
        profile,
        target_specification_path,
        target_dir,
    } = options;

    let mut command_builder = Command::new("cargo");
    command_builder
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .args([
            "rustc",
            "-Z",
            "build-std=core",
            "--crate-type",
            "cdylib",
            "--target",
            target_specification_path
                .to_str()
                .context("Path to target specification file is not valid UTF-8")?,
        ]);

    if let Some(package) = package {
        command_builder.args([
            "--package",
            package,
            "--features",
            &format!("{package}/guest"),
        ]);
    } else {
        command_builder.args(["--features", "guest"]);
    }
    if let Some(features) = features {
        command_builder.args(["--features", features]);
    }

    command_builder.args(["--profile", profile]);

    let metadata = MetadataCommand::new()
        .exec()
        .context("Failed to fetch cargo metadata")?;

    let target_directory = if let Some(target_dir) = target_dir {
        command_builder.args([
            "--target-dir",
            target_dir
                .to_str()
                .context("Path to target directory is not valid UTF-8")?,
        ]);
        target_dir
    } else {
        metadata.target_directory.as_std_path()
    };

    let cdylib_path = target_directory
        .join("riscv64em-unknown-none-abundance")
        .join(profile)
        .join({
            let package_name = if let Some(package) = package {
                package
            } else {
                let current_dir = env::current_dir().context("Failed to get current directory")?;
                let current_manifest = current_dir.join("Cargo.toml");
                metadata
                    .packages
                    .iter()
                    .find_map(|package| {
                        if package.manifest_path == current_manifest {
                            Some(&package.name)
                        } else {
                            None
                        }
                    })
                    .context("Failed to find package name")?
            };

            format!("{}.contract.so", package_name.replace('-', "_"))
        });

    debug!(
        ?package,
        ?features,
        ?profile,
        ?target_specification_path,
        cdylib_path = ?cdylib_path,
        command = ?command_builder,
        "Building ELF `cdylib` contract"
    );

    let status = command_builder
        .status()
        .context("Failed to build a contract")?;

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to build a contract"));
    }

    Ok(cdylib_path)
}
