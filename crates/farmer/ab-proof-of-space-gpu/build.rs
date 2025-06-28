use cargo_gpu::spirv_builder::{MetadataPrintout, SpirvMetadata};
use std::path::PathBuf;
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("Always set by Cargo; qed");

    if target_arch != "spirv" {
        let out_dir = env::var("OUT_DIR").expect("Always set by Cargo; qed");

        // Skip compilation under Clippy, it doesn't work for some reason and isn't really needed
        // anyway
        if env::var("CLIPPY_ARGS").is_ok() || env::var("MIRI_SYSROOT").is_ok() {
            let empty_file = PathBuf::from(out_dir).join("empty.bin");
            fs::write(&empty_file, [])?;
            println!("cargo::rustc-env=SHADER_PATH={}", empty_file.display());

            return Ok(());
        }

        let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Always set by Cargo; qed");
        let profile = env::var("PROFILE").expect("Always set by Cargo; qed");

        let shader_crate = PathBuf::from(cargo_manifest_dir);
        // TODO: Remove after https://github.com/Rust-GPU/rust-gpu/pull/249, together with the whole
        //  `rust-gpu-workaround`
        let shader_crate = shader_crate.join("rust-gpu-workaround");
        {
            env::set_current_dir(&shader_crate)?;
        }

        // TODO: Workaround for https://github.com/Rust-GPU/cargo-gpu/issues/90
        let cargo_target_dir = env::var("CARGO_TARGET_DIR").ok();
        // SAFETY: Single-threaded
        unsafe {
            env::remove_var("CARGO_TARGET_DIR");
        }

        let backend = cargo_gpu::Install::from_shader_crate(shader_crate.clone()).run()?;

        // TODO: Workaround for https://github.com/Rust-GPU/cargo-gpu/issues/90
        if let Some(cargo_target_dir) = cargo_target_dir {
            // SAFETY: Single-threaded
            unsafe {
                env::set_var("CARGO_TARGET_DIR", cargo_target_dir);
            }
        }

        let compile_result = backend
            .to_spirv_builder(shader_crate, "spirv-unknown-vulkan1.2")
            .print_metadata(MetadataPrintout::DependencyOnly)
            .spirv_metadata(if profile == "debug" {
                SpirvMetadata::NameVariables
            } else {
                SpirvMetadata::None
            })
            .release(profile != "debug")
            // Avoid Cargo deadlock
            .target_dir_path(out_dir)
            .build()?;
        let path_to_spv = compile_result.module.unwrap_single();

        println!("cargo::rustc-env=SHADER_PATH={}", path_to_spv.display());
    }

    Ok(())
}
