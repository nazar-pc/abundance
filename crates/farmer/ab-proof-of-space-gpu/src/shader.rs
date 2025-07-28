pub mod chacha8;
pub mod compute_f1;
pub mod compute_fn;
// TODO: Reuse constants from `ab-proof-of-space` once https://github.com/Rust-GPU/rust-gpu/pull/249 is
//  merged
mod constants;
mod num;
#[cfg(not(target_arch = "spirv"))]
mod shader_bytes;

/// Compiled SPIR-V shader for GPU that only supports `u32` (no `Int64` capability).
///
/// For shader with `Int64` capability see [`SHADER_U64`].
#[cfg(not(target_arch = "spirv"))]
pub const SHADER_U32: wgpu::ShaderModuleDescriptor<'static> = {
    use crate::shader::shader_bytes::ShaderBytes;

    const SHADER_BYTES_INTERNAL: &ShaderBytes<[u8]> =
        &ShaderBytes(*include_bytes!(env!("SHADER_PATH_U32")));

    SHADER_BYTES_INTERNAL.to_module()
};

/// Compiled SPIR-V shader for GPUs that supports `u64` (`Int64` capability).
///
/// For shader without `Int64` capability see [`SHADER_U32`].
#[cfg(not(target_arch = "spirv"))]
pub const SHADER_U64: wgpu::ShaderModuleDescriptor<'static> = {
    use crate::shader::shader_bytes::ShaderBytes;

    const SHADER_BYTES_INTERNAL: &ShaderBytes<[u8]> =
        &ShaderBytes(*include_bytes!(env!("SHADER_PATH_U64")));

    SHADER_BYTES_INTERNAL.to_module()
};
