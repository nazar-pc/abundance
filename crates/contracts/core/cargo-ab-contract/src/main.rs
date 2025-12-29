#![feature(try_blocks)]

mod convert;

use crate::convert::convert;
use ab_cli_utils::init_logger;
use ab_contract_file::ContractFile;
use anyhow::Context;
use clap::Parser;
use std::fs::{read, write};
use std::path::PathBuf;

/// Cargo extension for working with Abundance contracts
#[derive(Debug, Parser)]
#[clap(about, version)]
enum Command {
    /// Compile Rust contract using simple CLI
    Build,
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

    let command = Command::parse();

    match command {
        Command::Build => {
            unimplemented!("Building contracts is not implemented yet");
        }
        Command::Convert {
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
        Command::Verify { file } => {
            println!("Verifying {}", file.display());
            ContractFile::parse(&read(file)?, |_| Ok(()))
                .context("Failed to parse contract file")?;
            println!("Verification successful");
            Ok(())
        }
        Command::Recover { .. } => {
            unimplemented!("Recovering of ELF files is not implemented yet");
        }
    }
}
