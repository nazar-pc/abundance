#[cfg(not(target_arch = "spirv"))]
use std::borrow::Cow;
#[cfg(not(target_arch = "spirv"))]
use std::slice;
#[cfg(not(target_arch = "spirv"))]
use wgpu::{ShaderModuleDescriptor, ShaderSource};

pub mod chacha8;
pub mod compute_f1;
// TODO: Reuse constants from `ab-proof-of-space` once https://github.com/Rust-GPU/rust-gpu/pull/249 is
//  merged
mod constants;
mod num;

/// Compiled SPIR-V shader
#[cfg(not(target_arch = "spirv"))]
pub const SHADER: ShaderModuleDescriptor<'static> = {
    assert!(
        u16::from_ne_bytes(1u16.to_le_bytes()) == 1u16,
        "Only little-endian platform is supported"
    );

    #[repr(align(4))]
    struct ShaderBytes<T: ?Sized>(T);

    const SHADER_BYTES_INTERNAL: &ShaderBytes<[u8]> =
        &ShaderBytes(*include_bytes!(env!("SHADER_PATH")));

    assert!(align_of_val(SHADER_BYTES_INTERNAL) == align_of::<u32>());
    let shader_bytes = &SHADER_BYTES_INTERNAL.0;

    // SAFETY: Correctly aligned, all bit patterns are valid, lifetime is static before and after
    let shader_bytes = unsafe {
        slice::from_raw_parts(
            shader_bytes.as_ptr().cast::<u32>(),
            shader_bytes.len() / size_of::<u32>(),
        )
    };

    ShaderModuleDescriptor {
        label: Some(env!("CARGO_PKG_NAME")),
        source: ShaderSource::SpirV(Cow::Borrowed(shader_bytes)),
    }
};
