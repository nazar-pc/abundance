#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, NUM_BUCKETS};
use crate::shader::types::{PositionR, R};
use spirv_std::arch::workgroup_memory_barrier_with_group_sync;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;

#[inline(always)]
fn perform_compare_swap(
    thread_id: u32,
    j: u32,
    k: u32,
    shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    let m = j.trailing_zeros();
    let i = (thread_id & (j - 1)) | ((thread_id >> m) << (m + 1));
    let ixj = i ^ j;
    let ascending = (i & k) == 0;
    let a = shared_bucket[i as usize];
    let b = shared_bucket[ixj as usize];
    let (final_a, final_b) = if (a.position <= b.position) == ascending {
        (a, b)
    } else {
        (b, a)
    };
    shared_bucket[i as usize] = final_a;
    shared_bucket[ixj as usize] = final_b;
}

#[inline(always)]
fn sort_shared_bucket(thread_id: u32, shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE]) {
    let n = MAX_BUCKET_SIZE as u32;
    let mut k = 2u32;
    while k <= n {
        let mut j = k / 2;
        while j > 0 {
            perform_compare_swap(thread_id, j, k, shared_bucket);
            workgroup_memory_barrier_with_group_sync();
            j /= 2;
        }
        k *= 2;
    }
}

#[inline(always)]
pub(super) fn load_into_shared(
    thread_id: u32,
    bucket_size: u32,
    bucket: &[PositionR; MAX_BUCKET_SIZE],
    shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    const ELEMENTS_PER_THREAD: usize = MAX_BUCKET_SIZE / WORKGROUP_SIZE as usize;
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    for local_offset in 0..ELEMENTS_PER_THREAD {
        let bucket_offset = thread_id as usize * ELEMENTS_PER_THREAD + local_offset;
        shared_bucket[bucket_offset] = if bucket_offset < bucket_size as usize {
            bucket[bucket_offset]
        } else {
            PositionR::SENTINEL
        };
    }
}

#[inline(always)]
pub(super) fn store_from_shared(
    thread_id: u32,
    bucket: &mut [PositionR; MAX_BUCKET_SIZE],
    shared_bucket: &[PositionR; MAX_BUCKET_SIZE],
) {
    const ELEMENTS_PER_THREAD: usize = MAX_BUCKET_SIZE / WORKGROUP_SIZE as usize;
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    for local_offset in 0..ELEMENTS_PER_THREAD {
        let bucket_offset = thread_id as usize * ELEMENTS_PER_THREAD + local_offset;
        bucket[bucket_offset] = shared_bucket[bucket_offset];
    }
}

// TODO: Make unsafe and avoid bounds check
/// Sort a bucket using bitonic sort
fn sort_bucket_impl(
    thread_id: u32,
    bucket_size: u32,
    bucket: &mut [PositionR; MAX_BUCKET_SIZE],
    shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    load_into_shared(thread_id, bucket_size, bucket, shared_bucket);

    workgroup_memory_barrier_with_group_sync();

    sort_shared_bucket(thread_id, shared_bucket);

    workgroup_memory_barrier_with_group_sync();

    store_from_shared(thread_id, bucket, shared_bucket);
}

/// NOTE: bucket sizes are zeroed after use
#[spirv(compute(threads(256), entry_point_name = "sort_buckets"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub fn sort_buckets(
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] bucket_sizes: &mut [u32; NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] buckets: &mut [[PositionR; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(workgroup)] shared_bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    let local_invocation_id = local_invocation_id.x;
    let workgroup_id = workgroup_id.x;
    let num_workgroups = num_workgroups.x;

    let total_workgroups = num_workgroups;

    // Process one bucket per workgroup
    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for bucket_index in (workgroup_id..NUM_BUCKETS as u32).step_by(total_workgroups as usize) {
        let bucket_size = bucket_sizes[bucket_index as usize];
        let bucket = &mut buckets[bucket_index as usize];
        if local_invocation_id == 0 {
            bucket_sizes[bucket_index as usize] = 0;
        }

        sort_bucket_impl(local_invocation_id, bucket_size, bucket, shared_bucket);
    }
}
