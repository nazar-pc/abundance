use ab_riscv_macros::process_instruction_macros;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

const TAG: &str = "ctp-release-e9514aa-2025-12-28";
const COMMIT: &str = "281d71ef3d61e32111217b20305ea3ef9b1582e2";
const URL: &str = "https://github.com/riscv-non-isa/riscv-arch-test";

fn main() -> Result<(), Box<dyn Error>> {
    process_instruction_macros()?;

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Always set by Cargo; qed"));
    let dest = out_dir.join("riscv-arch-test");

    if dest.exists() {
        if let Ok(output) = Command::new("git")
            .current_dir(&dest)
            .args(["rev-parse", "HEAD"])
            .output()
            && output.status.success()
            && output.stdout.trim_ascii() == COMMIT.as_bytes()
        {
            return Ok(());
        }

        println!(
            "cargo::warning=`riscv-arch-test` commit mismatch or invalid repo; removing and \
            re-cloning"
        );
        let _ = fs::remove_dir_all(&dest);
    }

    let output = Command::new("git")
        .args(["clone", "--depth", "1", "--branch", TAG, "--quiet", URL])
        .arg(&dest)
        .output()?;

    if !output.status.success() {
        println!("cargo::error=`riscv-arch-test` clone failed:");
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            println!("cargo::error={line}");
        }
        for line in String::from_utf8_lossy(&output.stderr).lines() {
            println!("cargo::error={line}");
        }
        return Err("`riscv-arch-test` clone failed".into());
    }

    Ok(())
}
