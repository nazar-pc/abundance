#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, NUM_BUCKETS};
use crate::shader::types::PositionY;
use spirv_std::arch::workgroup_memory_barrier_with_group_sync;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;

#[inline(always)]
fn perform_compare_swap(
    local_invocation_id: u32,
    block_size: usize,
    bit_position: u32,
    bucket: &mut [PositionY; MAX_BUCKET_SIZE],
) {
    // Map contiguous `local_invocation_id` (0-255) to sparse `a_offset` (positions where bit
    // `bit_position == 0`). This "inserts" a `0` at bit position `bit_position` in the binary
    // representation of `local_invocation_id`, effectively treating `local_invocation_id` as an
    // 8-bit number and expanding it to 9 bits by skipping bit `bit_position`.
    //
    // Result: 256 unique `a_offset` in [0..511] with bit `bit_position` unset, in order.
    //
    // For `bit_position=0 (`distance=1`):
    //   `a_offset = local_invocation_id << 1 = even ids 0,2,...,510`
    // For `bit_position=1` (`distance=2`):
    //   `a_offset = (local_invocation_id & 1) | ((local_invocation_id >> 1) << 2) = 0,1,4,5,...`
    //
    // And similarly for `b_offset`, but setting `bit_position`th bit to `1`.
    // This ensures each of the 256 threads handles one unique disjoint pair without overlap or
    // idling.

    // Bits above `bit_position`
    let high = (local_invocation_id & (u32::MAX << bit_position)) << 1;
    // Bits below `bit_position`
    let low = local_invocation_id & u32::MAX.unbounded_shr(u32::BITS - bit_position);
    // `a_offset` and `b_offset` differ in `bit_position`th bit only: `a_offset` has it set to `0`
    // and `b_offset` to `1`
    let a_offset = (high | low) as usize;
    let b_offset = (high | (1u32 << bit_position) | low) as usize;

    let a = bucket[a_offset];
    let b = bucket[b_offset];

    let (smaller, larger) = if a.position <= b.position {
        (a, b)
    } else {
        (b, a)
    };
    let ascending = (a_offset & block_size) == 0;
    let (final_a, final_b) = if ascending {
        (smaller, larger)
    } else {
        (larger, smaller)
    };

    bucket[a_offset] = final_a;
    bucket[b_offset] = final_b;
}

// TODO: Make unsafe and avoid bounds check
// TODO: This can be heavily optimized by sorting a bucket per subgroup and storing everything in
//  registers instead of shared memory
/// Sort a bucket using bitonic sort
#[inline(always)]
fn sort_bucket_impl(
    local_invocation_id: u32,
    bucket: &mut [PositionY; MAX_BUCKET_SIZE],
    shared_bucket: &mut [PositionY; MAX_BUCKET_SIZE],
) {
    let bucket_offset_a = local_invocation_id;
    let bucket_offset_b = local_invocation_id + WORKGROUP_SIZE;

    shared_bucket[bucket_offset_a as usize] = bucket[bucket_offset_a as usize];
    shared_bucket[bucket_offset_b as usize] = bucket[bucket_offset_b as usize];

    workgroup_memory_barrier_with_group_sync();

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    let mut block_size = 2usize;
    let mut merger_stage = 1u32;
    while block_size <= MAX_BUCKET_SIZE {
        for bit_position in (0..merger_stage).rev() {
            perform_compare_swap(local_invocation_id, block_size, bit_position, shared_bucket);
            workgroup_memory_barrier_with_group_sync();
        }
        block_size *= 2;
        merger_stage += 1;
    }
    bucket[bucket_offset_a as usize] = shared_bucket[bucket_offset_a as usize];
    bucket[bucket_offset_b as usize] = shared_bucket[bucket_offset_b as usize];
}

#[spirv(compute(threads(256), entry_point_name = "sort_buckets"))]
pub fn sort_buckets(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] buckets: &mut [[PositionY; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(workgroup)] shared_bucket: &mut [PositionY; MAX_BUCKET_SIZE],
) {
    let local_invocation_id = local_invocation_id.x;
    let workgroup_id = workgroup_id.x;
    let num_workgroups = num_workgroups.x;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for bucket_index in (workgroup_id as usize..NUM_BUCKETS).step_by(num_workgroups as usize) {
        sort_bucket_impl(
            local_invocation_id,
            &mut buckets[bucket_index],
            shared_bucket,
        );
    }
}
