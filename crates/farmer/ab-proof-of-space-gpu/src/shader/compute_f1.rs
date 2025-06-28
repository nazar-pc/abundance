#[cfg(all(test, not(target_arch = "spirv")))]
mod cpu_tests;
#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::PARAM_EXT;
use crate::shader::num::{U64, U64T};
use spirv_std::glam::{UVec2, UVec3};
use spirv_std::spirv;

// TODO: Replace this constant with usage of `PosProof::K` after
//  https://github.com/Rust-GPU/rust-gpu/pull/249 is merged
const K: u8 = 20;
#[cfg(not(target_arch = "spirv"))]
const _: () = {
    assert!(K == ab_core_primitives::pos::PosProof::K);
};
// TODO: Should not be necessary, but https://github.com/Rust-GPU/rust-gpu/issues/300
const K_U32: u32 = K as u32;
// TODO: Should not be necessary, but https://github.com/Rust-GPU/rust-gpu/issues/300
const PARAM_EXT_U32: u32 = PARAM_EXT as u32;
// TODO: Should not be necessary, but https://github.com/Rust-GPU/rust-gpu/issues/300
const K_PLUS_PARAM_EXT_U32: u32 = K_U32 + PARAM_EXT_U32;
// TODO: Should not be necessary, but https://github.com/Rust-GPU/rust-gpu/issues/300
const K_MINUS_PARAM_EXT_U32: u32 = K_U32 - PARAM_EXT_U32;

// TODO: Reuse code from `ab-proof-of-space` after https://github.com/Rust-GPU/rust-gpu/pull/249 and
//  https://github.com/Rust-GPU/rust-gpu/discussions/301
/// `partial_y_offset` is in bits within `partial_y`
#[inline(always)]
pub(super) fn compute_f1_impl(x: u32, chacha8_keystream: &[u32]) -> u32 {
    let skip_bits = K_U32 * x;
    let skip_u32s = skip_bits / u32::BITS;
    let partial_y_offset = skip_bits % u32::BITS;

    let hi = chacha8_keystream[skip_u32s as usize].to_be();
    let lo = chacha8_keystream[skip_u32s as usize + 1].to_be();
    let partial_y = U64::from_lo_hi(lo, hi);

    let pre_y = partial_y >> (u64::BITS - K_PLUS_PARAM_EXT_U32 - partial_y_offset);
    let pre_y = pre_y.as_u32();
    // Mask for clearing the rest of bits of `pre_y`.
    let pre_y_mask = (u32::MAX << PARAM_EXT_U32) & (u32::MAX >> (u32::BITS - K_PLUS_PARAM_EXT_U32));

    // Extract `PARAM_EXT` most significant bits from `x` and store in the final offset of
    // eventual `y` with the rest of bits being zero (`x` is `0..2^K`)
    let pre_ext = x >> K_MINUS_PARAM_EXT_U32;

    // Combine all of the bits together:
    // [padding zero bits][`K` bits rom `partial_y`][`PARAM_EXT` bits from `x`]
    (pre_y & pre_y_mask) | pre_ext
}

/// Compute Chia's `f1()` function using previously computed ChaCha8 keystream
#[spirv(compute(threads(256), entry_point_name = "compute_f1"))]
pub fn compute_f1(
    #[spirv(global_invocation_id)] invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    // TODO: Uncomment once https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
    // #[spirv(workgroup_size)] workgroup_size: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] chacha8_keystream: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] xys: &mut [UVec2],
) {
    let invocation_id = invocation_id.x;
    let num_workgroups = num_workgroups.x;

    // TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` above, can be removed once
    //  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
    let workgroup_size = 256_u32;
    let global_size = workgroup_size * num_workgroups;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for x in (invocation_id..xys.len() as u32).step_by(global_size as usize) {
        xys[x as usize] = UVec2 {
            x,
            y: compute_f1_impl(x, chacha8_keystream),
        };
    }
}
