#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, NUM_BUCKETS, REDUCED_BUCKET_SIZE};
use crate::shader::find_matches_in_buckets::rmap::Rmap;
use crate::shader::sort_buckets::{
    load_into_local_bucket, sort_local_bucket, store_from_local_bucket,
};
use crate::shader::types::PositionR;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;

// TODO: Make unsafe and avoid bounds check
/// Sort a bucket using bitonic sort and store `Rmap` details in `r`'s data
fn sort_buckets_with_rmap_details_impl<const ELEMENTS_PER_THREAD: usize>(
    lane_id: u32,
    subgroup_size: u32,
    bucket_size: u32,
    bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    let mut local_bucket = [PositionR::SENTINEL; ELEMENTS_PER_THREAD];

    load_into_local_bucket(lane_id, bucket_size, bucket, &mut local_bucket);

    // Sort by position first
    sort_local_bucket(
        lane_id,
        &mut local_bucket,
        #[inline(always)]
        |a, b| a.position <= b.position,
    );
    // Truncate bucket to `REDUCED_BUCKET_SIZE`
    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    for local_offset in 0..ELEMENTS_PER_THREAD {
        let bucket_offset = lane_id as usize * ELEMENTS_PER_THREAD + local_offset;
        if bucket_offset >= REDUCED_BUCKET_SIZE {
            local_bucket[local_offset] = PositionR::SENTINEL;
        }
    }
    // Stable sort by `r`, which retains relative `position` order
    sort_local_bucket(
        lane_id,
        &mut local_bucket,
        #[inline(always)]
        |a, b| {
            // TODO: Comparison of `r` doesn't compile right now:
            //  https://github.com/Rust-GPU/rust-gpu/issues/409
            a.r.get_inner() < b.r.get_inner() || (a.r == b.r && a.position <= b.position)
        },
    );

    // SAFETY: Bucket is truncated to `REDUCED_BUCKET_SIZE` above
    unsafe {
        Rmap::update_local_bucket_r_data(lane_id, subgroup_size, &mut local_bucket);
    }

    // TODO: Maybe retain original order and avoid sorting here?
    // Re-sort back to position order
    sort_local_bucket(
        lane_id,
        &mut local_bucket,
        #[inline(always)]
        |a, b| a.position <= b.position,
    );

    store_from_local_bucket(lane_id, bucket, &local_bucket);
}

/// NOTE: bucket sizes are zeroed after use
#[spirv(compute(threads(256), entry_point_name = "sort_buckets_with_rmap_details"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub fn sort_buckets_with_rmap_details(
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(subgroup_size)] subgroup_size: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(subgroup_local_invocation_id)] subgroup_local_invocation_id: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] bucket_sizes: &mut [u32; NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] buckets: &mut [[PositionR; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
) {
    let workgroup_id = workgroup_id.x;
    let num_workgroups = num_workgroups.x;

    let total_subgroups = num_workgroups * num_subgroups;
    let global_subgroup_id = workgroup_id * num_subgroups + subgroup_id;

    // Process one bucket per subgroup
    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for bucket_index in (global_subgroup_id..NUM_BUCKETS as u32).step_by(total_subgroups as usize) {
        let bucket_size = bucket_sizes[bucket_index as usize];
        let bucket = &mut buckets[bucket_index as usize];
        // TODO: should have been `subgroup_elect()`, but it is not implemented in `wgpu` yet:
        //  https://github.com/gfx-rs/wgpu/issues/5555
        if subgroup_local_invocation_id == 0 {
            bucket_sizes[bucket_index as usize] = 0;
        }

        // Specify some common subgroup sizes so the driver can easily eliminate dead code. This is
        // important because `local_data` inside the function is generic and impacts the number of
        // registers used, so we want to minimize them.
        match subgroup_size {
            // Hypothetically possible
            1 => {
                sort_buckets_with_rmap_details_impl::<MAX_BUCKET_SIZE>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            // Hypothetically possible
            2 => {
                sort_buckets_with_rmap_details_impl::<{ MAX_BUCKET_SIZE / 2 }>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            // LLVMpipe (Mesa 24, SSE)
            4 => {
                sort_buckets_with_rmap_details_impl::<{ MAX_BUCKET_SIZE / 4 }>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            // LLVMpipe (Mesa 25, AVX/AVX2)
            8 => {
                sort_buckets_with_rmap_details_impl::<{ MAX_BUCKET_SIZE / 8 }>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            // Raspberry PI 5
            16 => {
                sort_buckets_with_rmap_details_impl::<{ MAX_BUCKET_SIZE / 16 }>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            // Intel/Nvidia
            32 => {
                sort_buckets_with_rmap_details_impl::<{ MAX_BUCKET_SIZE / 32 }>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            // AMD
            64 => {
                sort_buckets_with_rmap_details_impl::<{ MAX_BUCKET_SIZE / 64 }>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            // Hypothetically possible
            128 => {
                sort_buckets_with_rmap_details_impl::<{ MAX_BUCKET_SIZE / 128 }>(
                    subgroup_local_invocation_id,
                    subgroup_size,
                    bucket_size,
                    bucket,
                );
            }
            _ => {
                // https://registry.khronos.org/vulkan/specs/latest/man/html/SubgroupSize.html
                unreachable!("All Vulkan targets use power of two and subgroup size <= 128")
            }
        }
    }
}
