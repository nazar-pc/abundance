pub mod compute_f1;
pub mod compute_fn;
// TODO: Reuse constants from `ab-proof-of-space` once it compiles with `rust-gpu`
mod constants;
pub mod find_matches_and_compute_fn;
pub mod find_matches_and_compute_last;
pub mod find_matches_in_buckets;
mod num;
#[cfg(not(target_arch = "spirv"))]
mod shader_bytes;
pub mod sort_buckets;
// TODO: Reuse types from `ab-proof-of-space` once it compiles with `rust-gpu`
pub mod types;

#[cfg(not(target_arch = "spirv"))]
use wgpu::{Adapter, Features, Limits};

/// `4` is used by LLVMpipe, hence such a low number here
const MIN_SUBGROUP_SIZE: u32 = 4;

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

/// For a given set of adapter features and limits, this function returns the appropriate shader
/// version, required features, required limits and a boolean flag indicating whether the adapter is
/// modern or not.
///
/// Returns `None` for unsupported adapter.
#[cfg(not(target_arch = "spirv"))]
pub fn select_shader_features_limits(
    adapter: &Adapter,
) -> Option<(
    wgpu::ShaderModuleDescriptor<'static>,
    Features,
    Limits,
    bool,
)> {
    const SHADER_BASELINE_FEATURES: Features = Features::SUBGROUP;
    const SHADER_MODERN_FEATURES: Features = SHADER_BASELINE_FEATURES.union(Features::SHADER_INT64);
    // Modern GPUs have at least 32 kiB of shared memory
    const MODERN_SHADER_STORAGE_SIZE: u32 = 32 * 1024;

    let adapter_features = adapter.features();
    let adapter_limits = adapter.limits();

    if adapter_features.contains(SHADER_MODERN_FEATURES)
        && adapter_limits.min_subgroup_size >= MIN_SUBGROUP_SIZE
        && adapter_limits.max_compute_workgroup_storage_size >= MODERN_SHADER_STORAGE_SIZE
    {
        Some((
            SHADER_MODERN,
            SHADER_MODERN_FEATURES,
            Limits {
                min_subgroup_size: MIN_SUBGROUP_SIZE,
                max_compute_workgroup_storage_size: MODERN_SHADER_STORAGE_SIZE,
                ..Limits::defaults()
            },
            true,
        ))
    } else if adapter_limits.min_subgroup_size >= MIN_SUBGROUP_SIZE {
        // Fallback GPU supports only baseline features and no extras
        Some((
            SHADER_FALLBACK,
            SHADER_BASELINE_FEATURES,
            Limits {
                min_subgroup_size: MIN_SUBGROUP_SIZE,
                ..Limits::defaults()
            },
            false,
        ))
    } else {
        None
    }
}
