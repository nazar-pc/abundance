use ab_contract_file::ContractFile;
use ab_contracts_tooling::TARGET_ENV;
use ab_contracts_tooling::build::{BuildOptions, build_cdylib};
use ab_contracts_tooling::convert::convert;
use ab_contracts_tooling::target_specification::TargetSpecification;
use std::env;
use std::error::Error;
use std::fs::{read, write};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let target_env = env::var("CARGO_CFG_TARGET_ENV").expect("Always set by Cargo; qed");

    if target_env == TARGET_ENV {
        return Ok(());
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Always set by Cargo; qed"));

    let target_specification = TargetSpecification::create(&out_dir)?;

    let cdylib_path = build_cdylib(BuildOptions {
        package: None,
        features: None,
        profile: "release",
        target_specification_path: target_specification.path(),
        target_dir: Some(&out_dir),
    })?;

    let contract_path = cdylib_path.with_extension("");

    let input_bytes = read(cdylib_path)?;
    let output_bytes = convert(&input_bytes)?;
    ContractFile::parse(&output_bytes, |_| Ok(()))?;
    write(&contract_path, output_bytes)?;

    println!("cargo::rustc-env=CONTRACT_PATH={}", contract_path.display());

    Ok(())
}
