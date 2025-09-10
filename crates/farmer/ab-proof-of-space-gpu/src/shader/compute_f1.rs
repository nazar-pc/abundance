#[cfg(all(test, not(target_arch = "spirv")))]
mod cpu_tests;
#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{K, PARAM_EXT};
use crate::shader::num::{U64, U64T};
use crate::shader::types::{X, Y};
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Make unsafe and avoid bounds check
// TODO: Reuse code from `ab-proof-of-space` after https://github.com/Rust-GPU/rust-gpu/pull/249 and
//  https://github.com/Rust-GPU/rust-gpu/discussions/301
/// `partial_y_offset` is in bits within `partial_y`
#[inline(always)]
pub(super) fn compute_f1_impl(x: X, chacha8_keystream: &[u32]) -> Y {
    let skip_bits = u32::from(K) * u32::from(x);
    let skip_u32s = skip_bits / u32::BITS;
    let partial_y_offset = skip_bits % u32::BITS;

    let high = chacha8_keystream[skip_u32s as usize].to_be();
    let low = chacha8_keystream[skip_u32s as usize + 1].to_be();
    let partial_y = U64::from_low_high(low, high);

    let pre_y = partial_y >> (u64::BITS - u32::from(K + PARAM_EXT) - partial_y_offset);
    let pre_y = pre_y.as_u32();
    // Mask for clearing the rest of bits of `pre_y`.
    let pre_y_mask = (u32::MAX << PARAM_EXT) & (u32::MAX >> (u32::BITS - u32::from(K + PARAM_EXT)));

    // Extract `PARAM_EXT` most significant bits from `x` and store in the final offset of
    // eventual `y` with the rest of bits being zero (`x` is `0..2^K`)
    let pre_ext = u32::from(x) >> (K - PARAM_EXT);

    // Combine all of the bits together:
    // [padding zero bits][`K` bits rom `partial_y`][`PARAM_EXT` bits from `x`]
    Y::from((pre_y & pre_y_mask) | pre_ext)
}

/// Compute Chia's `f1()` function using previously computed ChaCha8 keystream
#[spirv(compute(threads(256), entry_point_name = "compute_f1"))]
pub fn compute_f1(
    #[spirv(global_invocation_id)] invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    // TODO: Uncomment once https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
    // #[spirv(workgroup_size)] workgroup_size: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] chacha8_keystream: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ys: &mut [Y],
) {
    // TODO: Make a single input bounds check and use unsafe to avoid bounds check later
    let invocation_id = invocation_id.x;
    let num_workgroups = num_workgroups.x;

    // TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` above, can be removed once
    //  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
    let workgroup_size = 256_u32;
    let global_size = workgroup_size * num_workgroups;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for x in (invocation_id..ys.len() as u32).step_by(global_size as usize) {
        ys[x as usize] = compute_f1_impl(X::from(x), chacha8_keystream);
    }
}
