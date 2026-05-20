use ab_riscv_macros::process_instruction_macros;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

const TAG: &str = "v1.01";
const COMMIT: &str = "cfa9ab377835911f23d9b0831c7be302ed1f58de";
const URL: &str = "https://github.com/eembc/coremark";

/// RISC-V cross-compiler to use. Override with the `RISCV_CC` environment variable if your
/// toolchain is installed under a different name.
const DEFAULT_RISCV_CC: &str = "riscv64-unknown-elf-gcc";

fn main() -> Result<(), Box<dyn Error>> {
    process_instruction_macros()?;

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Always set by Cargo; qed"));
    let coremark_dir = out_dir.join("coremark");

    clone_if_needed(&coremark_dir)?;

    let cc = env::var("RISCV_CC").unwrap_or_else(|_| DEFAULT_RISCV_CC.to_string());

    let elf_path = out_dir.join("coremark.elf");

    let coremark_sources = [
        coremark_dir.join("core_list_join.c"),
        coremark_dir.join("core_matrix.c"),
        coremark_dir.join("core_state.c"),
        coremark_dir.join("core_util.c"),
        coremark_dir.join("core_main.c"),
    ];

    let maybe_status = Command::new(&cc)
        // TODO: `b` is not recognized by the default RISC-V toolchain on Ubuntu 24.04 for some
        //  reason, hence zba_zbb_zbs
        .args(["-O3", "-march=rv64imc_zba_zbb_zbs", "-mabi=lp64"])
        .arg(format!("-I{}", coremark_dir.display()))
        .arg("-Isrc/coremark_port")
        .arg("-DPERFORMANCE_RUN=1")
        // 0 means "autodetect"
        .arg("-DITERATIONS=0")
        .args([
            "-ffreestanding",
            "-nostdlib",
            "-nostartfiles",
            "-static-pie",
            "-Wl,--entry=main",
            "-Werror",
        ])
        .args(coremark_sources)
        // Our ee_printf.c replaces barebones/ee_printf.c: identical except the uart_send_char stub
        // with #error is removed; that function is defined in core_portme.c.
        .arg("src/coremark_port/ee_printf.c")
        .arg("src/coremark_port/core_portme.c")
        .arg("-o")
        .arg(&elf_path)
        .status();

    match maybe_status {
        Ok(status) => {
            if !status.success() {
                if env::var("CARGO_FEATURE_BUILD_ELF_REQUIRED").is_ok() {
                    return Err("building coremark.elf failed".into());
                }

                // Quietly create an empty file so that the build succeeds even if the ELF is
                // not
                fs::write(&elf_path, [])?;
            }
        }
        Err(error) => {
            if env::var("CARGO_FEATURE_BUILD_ELF_REQUIRED").is_ok() {
                return Err(error.into());
            }

            // Quietly create an empty file so that the build succeeds even if the ELF is
            // not
            fs::write(&elf_path, [])?;
        }
    }

    println!("cargo::rustc-env=COREMARK_ELF={}", elf_path.display());

    println!("cargo::rerun-if-changed=src");
    println!("cargo::rerun-if-env-changed=RISCV_CC");

    Ok(())
}

fn clone_if_needed(dest: &PathBuf) -> Result<(), Box<dyn Error>> {
    if dest.exists() {
        if let Ok(output) = Command::new("git")
            .current_dir(dest)
            .args(["rev-parse", "HEAD"])
            .output()
            && output.status.success()
            && output.stdout.trim_ascii() == COMMIT.as_bytes()
        {
            return Ok(());
        }

        println!(
            "cargo::warning=`coremark` commit mismatch or invalid repo; removing and re-cloning"
        );
        fs::remove_dir_all(dest)?;
    }

    let output = Command::new("git")
        .args(["clone", "--depth", "1", "--branch", TAG, "--quiet", URL])
        .arg(dest)
        .output()?;

    if !output.status.success() {
        println!("cargo::error=`coremark` clone failed:");
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            println!("cargo::error={line}");
        }
        for line in String::from_utf8_lossy(&output.stderr).lines() {
            println!("cargo::error={line}");
        }
        return Err("`coremark` clone failed".into());
    }

    Ok(())
}
