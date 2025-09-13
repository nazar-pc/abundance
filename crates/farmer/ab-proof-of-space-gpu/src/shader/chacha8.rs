#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use ab_chacha8::{ChaCha8Block, ChaCha8State};
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
const WORKGROUP_SIZE: u32 = 256;

/// Produce a ChaCha8 keystream.
///
/// NOTE: Length of the keystream is limited by `u32`.
#[spirv(compute(threads(256), entry_point_name = "chacha8_keystream"))]
pub fn chacha8_keystream(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] initial_state: &ChaCha8Block,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] keystream: &mut [ChaCha8Block],
) {
    let global_invocation_id = global_invocation_id.x;
    let num_workgroups = num_workgroups.x;

    let global_size = WORKGROUP_SIZE * num_workgroups;
    let initial_state = ChaCha8State::from_repr(*initial_state);

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for position in (global_invocation_id..keystream.len() as u32).step_by(global_size as usize) {
        // TODO: Make sure bounds check here is elided
        keystream[position as usize] = initial_state.compute_block(position);
    }
}
