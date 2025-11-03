#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, NUM_BUCKETS, REDUCED_BUCKET_SIZE};
use crate::shader::find_matches_in_buckets::rmap::Rmap;
use crate::shader::sort_buckets::{
    load_into_shared_bucket, sort_shared_bucket, store_from_shared_bucket,
};
use crate::shader::types::PositionR;
use spirv_std::arch::workgroup_memory_barrier_with_group_sync;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;

const _: () = {
    assert!(WORKGROUP_SIZE == crate::shader::sort_buckets::WORKGROUP_SIZE);
};

// TODO: Make unsafe and avoid bounds check
/// Sort a bucket using bitonic sort and store `Rmap` details in `r`'s data
fn sort_buckets_with_rmap_details_impl(
    local_invocation_id: u32,
    subgroup_local_invocation_id: u32,
    subgroup_id: u32,
    subgroup_size: u32,
    bucket_size: u32,
    bucket: &mut [PositionR; MAX_BUCKET_SIZE],
    shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    load_into_shared_bucket(local_invocation_id, bucket_size, bucket, shared_bucket);

    workgroup_memory_barrier_with_group_sync();

    // Sort by position first
    sort_shared_bucket(
        local_invocation_id,
        shared_bucket,
        #[inline(always)]
        |a, b| a.position <= b.position,
    );

    workgroup_memory_barrier_with_group_sync();

    // Truncate bucket to `REDUCED_BUCKET_SIZE`
    for bucket_offset in
        (local_invocation_id as usize..MAX_BUCKET_SIZE).step_by(WORKGROUP_SIZE as usize)
    {
        if bucket_offset >= REDUCED_BUCKET_SIZE {
            shared_bucket[bucket_offset] = PositionR::SENTINEL;
        }
    }

    workgroup_memory_barrier_with_group_sync();

    // Stable sort by `r`, which retains relative `position` order
    sort_shared_bucket(
        local_invocation_id,
        shared_bucket,
        #[inline(always)]
        |a, b| {
            // TODO: Comparison of `r` doesn't compile right now:
            //  https://github.com/Rust-GPU/rust-gpu/issues/409
            a.r.get_inner() < b.r.get_inner() || (a.r == b.r && a.position <= b.position)
        },
    );

    workgroup_memory_barrier_with_group_sync();

    // SAFETY: Bucket is truncated to `REDUCED_BUCKET_SIZE` above, called from a single subgroup
    if subgroup_id == 0 {
        unsafe {
            Rmap::update_local_bucket_r_data(
                subgroup_local_invocation_id,
                subgroup_size,
                shared_bucket,
            );
        }
    }

    workgroup_memory_barrier_with_group_sync();

    // TODO: Maybe retain original order and avoid sorting here?
    // Re-sort back to position order
    sort_shared_bucket(
        local_invocation_id,
        shared_bucket,
        #[inline(always)]
        |a, b| a.position <= b.position,
    );

    workgroup_memory_barrier_with_group_sync();

    store_from_shared_bucket(local_invocation_id, bucket, shared_bucket);
}

/// NOTE: bucket sizes are zeroed after use
#[spirv(compute(threads(256), entry_point_name = "sort_buckets_with_rmap_details"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub fn sort_buckets_with_rmap_details(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(subgroup_size)] subgroup_size: u32,
    #[spirv(subgroup_local_invocation_id)] subgroup_local_invocation_id: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] bucket_sizes: &mut [u32; NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] buckets: &mut [[PositionR; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(workgroup)] shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    let local_invocation_id = local_invocation_id.x;
    let workgroup_id = workgroup_id.x;
    let num_workgroups = num_workgroups.x;

    // Process one bucket per subgroup
    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for bucket_index in (workgroup_id..NUM_BUCKETS as u32).step_by(num_workgroups as usize) {
        let bucket_size = bucket_sizes[bucket_index as usize];
        let bucket = &mut buckets[bucket_index as usize];

        sort_buckets_with_rmap_details_impl(
            local_invocation_id,
            subgroup_local_invocation_id,
            subgroup_id,
            subgroup_size,
            bucket_size,
            bucket,
            shared_bucket,
        );

        if local_invocation_id == 0 {
            bucket_sizes[bucket_index as usize] = 0;
        }

        workgroup_memory_barrier_with_group_sync();
    }
}
