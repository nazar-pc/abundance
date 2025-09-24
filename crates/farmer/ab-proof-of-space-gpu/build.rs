use cargo_gpu::spirv_builder::{Capability, MetadataPrintout, SpirvBuilderError, SpirvMetadata};
use std::error::Error;
use std::path::PathBuf;
use std::{env, fs, thread};

fn main() -> Result<(), Box<dyn Error>> {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("Always set by Cargo; qed");

    if target_arch != "spirv" {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Always set by Cargo; qed"));

        // Skip compilation under Clippy, it doesn't work for some reason and isn't really needed
        // anyway. Same about Miri and rustdoc.
        if ["CLIPPY_ARGS", "MIRI_SYSROOT", "RUSTDOCFLAGS"]
            .iter()
            .any(|var| env::var(var).is_ok())
        {
            let empty_file = out_dir.join("empty.bin");
            fs::write(&empty_file, [])?;
            println!(
                "cargo::rustc-env=SHADER_PATH_FALLBACK={}",
                empty_file.display()
            );
            println!(
                "cargo::rustc-env=SHADER_PATH_MODERN={}",
                empty_file.display()
            );

            return Ok(());
        }

        let cargo_manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Always set by Cargo; qed");
        let profile = env::var("PROFILE").expect("Always set by Cargo; qed");

        let shader_crate = PathBuf::from(cargo_manifest_dir);

        let backend = cargo_gpu::Install::from_shader_crate(shader_crate.clone()).run()?;

        let spirv_builder = backend
            .to_spirv_builder(shader_crate, "spirv-unknown-vulkan1.2")
            .print_metadata(MetadataPrintout::DependencyOnly)
            .spirv_metadata(if profile == "debug" {
                SpirvMetadata::NameVariables
            } else {
                SpirvMetadata::None
            })
            .release(profile != "debug")
            // TODO: This should not be needed: https://github.com/Rust-GPU/rust-gpu/issues/386
            .capability(Capability::GroupNonUniformArithmetic);

        thread::scope(|scope| -> Result<(), Box<dyn Error>> {
            // Compile SPIR-V shader for GPU that only supports baseline Vulkan features
            let handle_fallback = scope.spawn(|| {
                let compile_result = spirv_builder
                    .clone()
                    // Avoid Cargo deadlock, customize target
                    .target_dir_path(out_dir.join("fallback").to_string_lossy().to_string())
                    .build()?;
                let path_to_spv = compile_result.module.unwrap_single();

                println!(
                    "cargo::rustc-env=SHADER_PATH_FALLBACK={}",
                    path_to_spv.display()
                );

                Ok::<(), SpirvBuilderError>(())
            });

            // Compile SPIR-V shader for GPUs that supports modern Vulkan features
            let handle_modern = scope.spawn(|| {
                let compile_result = spirv_builder
                    .clone()
                    .shader_crate_features(["__modern-gpu".to_string()])
                    // Avoid Cargo deadlock, customize target
                    .target_dir_path(out_dir.join("modern").to_string_lossy().to_string())
                    .capability(Capability::Int64)
                    .build()?;
                let path_to_spv = compile_result.module.unwrap_single();

                println!(
                    "cargo::rustc-env=SHADER_PATH_MODERN={}",
                    path_to_spv.display()
                );

                Ok::<(), SpirvBuilderError>(())
            });

            handle_fallback
                .join()
                .expect("Spawning threads must succeed")?;
            handle_modern
                .join()
                .expect("Spawning threads must succeed")?;

            Ok(())
        })?;
    }

    Ok(())
}
