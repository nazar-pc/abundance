#![feature(try_blocks)]

mod convert;

use crate::convert::convert;
use ab_cli_utils::init_logger;
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
            let input_bytes = read(input_file).context("Failed to read input file")?;
            let output_bytes = convert(&input_bytes)?;
            write(output_file, output_bytes).context("Failed to write output file")
        }
        Command::Recover { .. } => {
            unimplemented!("Recovering of ELF files is not implemented yet");
        }
    }
}
