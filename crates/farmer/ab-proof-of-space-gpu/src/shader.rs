pub mod chacha8;
pub mod compute_f1;
pub mod compute_fn;
// TODO: Reuse constants from `ab-proof-of-space` once it compiles with `rust-gpu`
mod constants;
mod num;
#[cfg(not(target_arch = "spirv"))]
mod shader_bytes;
// TODO: Reuse constants from `ab-proof-of-space` once it compiles with `rust-gpu`
pub mod types;

#[cfg(not(target_arch = "spirv"))]
use wgpu::{Features, Limits};

/// Compiled SPIR-V shader for GPU that only supports baseline Vulkan features.
///
/// For a shader with modern features, see [`SHADER_MODERN`].
#[cfg(not(target_arch = "spirv"))]
const SHADER_FALLBACK: wgpu::ShaderModuleDescriptor<'static> = {
    use crate::shader::shader_bytes::ShaderBytes;

    const SHADER_BYTES_INTERNAL: &ShaderBytes<[u8]> =
        &ShaderBytes(*include_bytes!(env!("SHADER_PATH_FALLBACK")));

    SHADER_BYTES_INTERNAL.to_module()
};

/// Compiled SPIR-V shader for GPUs that supports modern Vulkan features.
///
/// For a shader without modern features, see [`SHADER_FALLBACK`].
#[cfg(not(target_arch = "spirv"))]
const SHADER_MODERN: wgpu::ShaderModuleDescriptor<'static> = {
    use crate::shader::shader_bytes::ShaderBytes;

    const SHADER_BYTES_INTERNAL: &ShaderBytes<[u8]> =
        &ShaderBytes(*include_bytes!(env!("SHADER_PATH_MODERN")));

    SHADER_BYTES_INTERNAL.to_module()
};

#[cfg(not(target_arch = "spirv"))]
pub fn select_shader_features_limits(
    adapter_features: Features,
) -> (wgpu::ShaderModuleDescriptor<'static>, Features, Limits) {
    const SHADER_MODERN_FEATURES: Features = Features::SHADER_INT64;

    if adapter_features.contains(SHADER_MODERN_FEATURES) {
        (
            SHADER_MODERN,
            SHADER_MODERN_FEATURES,
            Limits {
                // Modern GPUs have at least 32 kiB of shared memory
                max_compute_workgroup_storage_size: 32 * 1024,
                ..Limits::defaults()
            },
        )
    } else {
        // Fallback GPU supports only baseline features and no extras
        (SHADER_FALLBACK, Features::default(), Limits::defaults())
    }
}
