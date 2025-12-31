use ab_cli_utils::init_logger;
use ab_contract_file::ContractFile;
use ab_contracts_tooling::build::{BuildOptions, build_cdylib};
use ab_contracts_tooling::convert::convert;
use ab_contracts_tooling::target_specification::TargetSpecification;
use anyhow::Context;
use clap::Parser;
use std::env;
use std::fs::{read, write};
use std::path::PathBuf;
use std::process::Command;

/// Cargo extension for working with Abundance contracts
#[derive(Debug, Parser)]
#[clap(about, version)]
enum Cli {
    /// Write and print a path to the target specification JSON file
    TargetSpecPath,
    /// Compile a contract using simple CLI.
    ///
    /// Note that unoptimized builds are not supported, hence `release` by default.
    Build {
        /// Package to build.
        ///
        /// A package in the current directory is built if not specified explicitly.
        #[arg(long, short = 'p')]
        package: Option<String>,
        /// Comma separated list of features to activate
        #[arg(long)]
        features: Option<String>,
        /// Build artifacts with the specified profile
        #[arg(long, default_value = "release")]
        profile: String,
    },
    /// Convert `.contract.so` ELF file to `.contract` for execution environment
    Convert {
        /// Input file with `.contract.so` extension
        input_file: PathBuf,
        /// Output file with `.contract` extension
        output_file: PathBuf,
    },
    /// Verify `.contract` file for correctness
    Verify {
        /// Path to `.contract` file
        file: PathBuf,
    },
    /// Recover `.contract.so` ELF file from `.contract`
    Recover {
        /// Input file with `.contract` extension
        input_file: PathBuf,
        /// Output file with `.contract.so` extension
        output_file: PathBuf,
    },
}

pub fn main() -> anyhow::Result<()> {
    init_logger();

    let cli = Cli::parse_from({
        let mut args = env::args().collect::<Vec<_>>();
        if args
            .get(1)
            .map(|arg| arg == "ab-contract")
            .unwrap_or_default()
        {
            // Remove the first argument when running under Cargo
            args.remove(1);
        }

        args
    });

    match cli {
        Cli::TargetSpecPath => {
            let target_specification =
                TargetSpecification::create(&TargetSpecification::default_base_dir()?)?;

            println!("{}", target_specification.path().display());

            Ok(())
        }
        Cli::Build {
            package,
            features,
            profile,
        } => {
            let target_specification =
                TargetSpecification::create(&TargetSpecification::default_base_dir()?)?;

            let cdylib_path = build_cdylib(BuildOptions {
                package: package.as_deref(),
                features: features.as_deref(),
                profile: &profile,
                target_specification_path: target_specification.path(),
                target_dir: None,
            })?;

            let mut command_builder = Command::new("cargo");
            command_builder.args([
                "rustc",
                "-Z",
                "build-std=core",
                "--crate-type",
                "cdylib",
                "--target",
                target_specification
                    .path()
                    .to_str()
                    .context("Path to target specification file is not valid UTF-8")?,
            ]);

            let contract_path = cdylib_path.with_extension("");

            println!("Built ELF `cdylib` successfully, converting to `.contract` file:");
            println!("  Input file: {}", cdylib_path.display());
            println!("  Output file: {}", contract_path.display());

            let input_bytes = read(cdylib_path).context("Failed to read input file")?;
            let output_bytes = convert(&input_bytes)?;
            ContractFile::parse(&output_bytes, |_| Ok(()))
                .context("Failed to parse converted contract file")?;
            write(contract_path, output_bytes).context("Failed to write output file")?;

            println!("Build successful");

            Ok(())
        }
        Cli::Convert {
            input_file,
            output_file,
        } => {
            println!("Converting:");
            println!("  Input file: {}", input_file.display());
            println!("  Output file: {}", output_file.display());
            let input_bytes = read(input_file).context("Failed to read input file")?;
            let output_bytes = convert(&input_bytes)?;
            ContractFile::parse(&output_bytes, |_| Ok(()))
                .context("Failed to parse converted contract file")?;
            write(output_file, output_bytes).context("Failed to write output file")?;
            println!("Conversion successful");
            Ok(())
        }
        Cli::Verify { file } => {
            println!("Verifying {}", file.display());
            ContractFile::parse(&read(file)?, |_| Ok(()))
                .context("Failed to parse contract file")?;
            println!("Verification successful");
            Ok(())
        }
        Cli::Recover { .. } => {
            unimplemented!("Recovering of ELF files is not implemented yet");
        }
    }
}
