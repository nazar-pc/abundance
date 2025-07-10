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
            println!("cargo::rustc-env=SHADER_PATH_U32={}", empty_file.display());
            println!("cargo::rustc-env=SHADER_PATH_U64={}", empty_file.display());

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
            .release(profile != "debug");

        thread::scope(|scope| -> Result<(), Box<dyn Error>> {
            // Compile with defaults (no `Int64` capability)
            let handle_u32 = scope.spawn(|| {
                let compile_result = spirv_builder
                    .clone()
                    // Avoid Cargo deadlock, customize target
                    .target_dir_path(out_dir.join("u32").to_string_lossy().to_string())
                    .build()?;
                let path_to_spv = compile_result.module.unwrap_single();

                println!("cargo::rustc-env=SHADER_PATH_U32={}", path_to_spv.display());

                Ok::<(), SpirvBuilderError>(())
            });

            // Compile with `Int64` capability
            let handle_u64 = scope.spawn(|| {
                let compile_result = spirv_builder
                    .clone()
                    // Avoid Cargo deadlock, customize target
                    .target_dir_path(out_dir.join("u64").to_string_lossy().to_string())
                    .capability(Capability::Int64)
                    .build()?;
                let path_to_spv = compile_result.module.unwrap_single();

                println!("cargo::rustc-env=SHADER_PATH_U64={}", path_to_spv.display());

                Ok::<(), SpirvBuilderError>(())
            });

            handle_u32.join().expect("Spawning threads must succeed")?;
            handle_u64.join().expect("Spawning threads must succeed")?;

            Ok(())
        })?;
    }

    Ok(())
}
