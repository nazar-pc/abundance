#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, NUM_BUCKETS};
use crate::shader::types::{PositionR, R};
use spirv_std::arch::subgroup_shuffle;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;

#[inline(always)]
fn perform_local_compare_swap<const ELEMENTS_PER_THREAD: usize, LessOrEqual>(
    lane_id: u32,
    bit_position: u32,
    block_size: usize,
    local_bucket: &mut [PositionR; ELEMENTS_PER_THREAD],
    // TODO: Should have been just `fn()`, but https://github.com/Rust-GPU/rust-gpu/issues/452
    less_or_equal: LessOrEqual,
) where
    LessOrEqual: Fn(&PositionR, &PositionR) -> bool,
{
    let lane_base = lane_id as usize * ELEMENTS_PER_THREAD;
    // For local swaps within a thread's register data, iterate over half the elements to form pairs
    // `(a_offset, b_offset)` where indices differ only at `bit_position` and swaps them in-place
    for pair_id in 0..ELEMENTS_PER_THREAD / 2 {
        // Compute pair indices using bit manipulation to map `pair_id`
        // (`0..ELEMENTS_PER_THREAD / 2`) to sparse indices in `local_data`
        // (`0..ELEMENTS_PER_THREAD`) where bit_position is `0` or `1`

        // Bits above `bit_position
        let high = (pair_id & ((u32::MAX as usize) << bit_position)) << 1;
        // Bits below `bit_position`
        let low = pair_id & (u32::MAX as usize).unbounded_shr(u32::BITS - bit_position);
        let a_offset = high | low;
        let b_offset = a_offset | (1 << bit_position);

        let a = local_bucket[a_offset];
        let b = local_bucket[b_offset];

        // Determine the sort direction: ascending if `a_offset`'s bit at `block_size` is `0`.
        // This alternates direction in bitonic merges to create sorted sequences.
        let ascending = ((lane_base + a_offset) & block_size) == 0;

        let (final_a, final_b) = if less_or_equal(&a, &b) == ascending {
            (a, b)
        } else {
            (b, a)
        };

        local_bucket[a_offset] = final_a;
        local_bucket[b_offset] = final_b;
    }
}

#[inline(always)]
fn perform_cross_compare_swap<const ELEMENTS_PER_THREAD: usize, LessOrEqual>(
    lane_id: u32,
    bit_position: u32,
    block_size: usize,
    local_bucket: &mut [PositionR; ELEMENTS_PER_THREAD],
    // TODO: Should have been just `fn()`, but https://github.com/Rust-GPU/rust-gpu/issues/452
    less_or_equal: LessOrEqual,
) where
    LessOrEqual: Fn(&PositionR, &PositionR) -> bool,
{
    // For cross-subgroup swaps, compute partner lane differing at `lane_bit_position`
    let lane_bit_position = bit_position - ELEMENTS_PER_THREAD.ilog2();
    let partner_lane_id = lane_id ^ (1 << lane_bit_position);
    // When this lane's bit at `lane_bit_position` is `0`, then this is a lower lane (when compared
    // to `partner_lane_id`)
    let is_low = (lane_id as usize & (1 << lane_bit_position)) == 0;
    // If `block_size` bit is `0`, setting sort direction for bitonic merge.
    let ascending = ((lane_id as usize * ELEMENTS_PER_THREAD) & block_size) == 0;
    let select_smaller = is_low == ascending;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    // Iterate over all elements in `local_data`, swapping with partner lane
    for a_offset in 0..ELEMENTS_PER_THREAD {
        let a = local_bucket[a_offset];
        let b = {
            let position = subgroup_shuffle(a.position, partner_lane_id);
            // SAFETY: `R` is constructed from its own inner value
            let r =
                unsafe { R::new_from_inner(subgroup_shuffle(a.r.get_inner(), partner_lane_id)) };

            PositionR { position, r }
        };

        let selected_value = if less_or_equal(&a, &b) == select_smaller {
            a
        } else {
            b
        };

        local_bucket[a_offset] = selected_value;
    }
}

#[inline(always)]
pub(super) fn load_into_local_bucket<const ELEMENTS_PER_THREAD: usize>(
    lane_id: u32,
    bucket_size: u32,
    bucket: &[PositionR; MAX_BUCKET_SIZE],
    local_bucket: &mut [PositionR; ELEMENTS_PER_THREAD],
) {
    // TODO: Every item is a pair of `u32`s, should this be rewritten for coalesced reads instead?
    //  If so, casting to an array of `u32`s is needed here, but rust-gpu doesn't support it yet:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    for local_offset in 0..ELEMENTS_PER_THREAD {
        let bucket_offset = lane_id as usize * ELEMENTS_PER_THREAD + local_offset;
        local_bucket[local_offset] = if bucket_offset < bucket_size as usize {
            bucket[bucket_offset]
        } else {
            PositionR::SENTINEL
        };
    }
}

#[inline(always)]
pub(super) fn sort_local_bucket<const ELEMENTS_PER_THREAD: usize, LessOrEqual>(
    lane_id: u32,
    local_bucket: &mut [PositionR; ELEMENTS_PER_THREAD],
    // TODO: Should have been just `fn()`, but https://github.com/Rust-GPU/rust-gpu/issues/452
    less_or_equal: LessOrEqual,
) where
    LessOrEqual: Fn(&PositionR, &PositionR) -> bool,
{
    // Iterate over merger stages, doubling block_size each time
    let mut block_size = 2;
    let mut merger_stage = 1;
    // TODO: Can `MAX_BUCKET_SIZE` be replaced with `bucket_size.next_power_of_two()` here while
    //  preserving correctness?
    while block_size <= MAX_BUCKET_SIZE {
        // For each stage, process bit positions in reverse for bitonic comparisons
        for bit_position in (0..merger_stage).rev() {
            if bit_position < ELEMENTS_PER_THREAD.ilog2() {
                // Local swaps within thread's registers, no synchronization needed
                perform_local_compare_swap(
                    lane_id,
                    bit_position,
                    block_size,
                    local_bucket,
                    &less_or_equal,
                );
            } else {
                // Cross-lane swaps using subgroup shuffles for communication
                perform_cross_compare_swap(
                    lane_id,
                    bit_position,
                    block_size,
                    local_bucket,
                    &less_or_equal,
                );
            }
        }
        block_size *= 2;
        merger_stage += 1;
    }
}

#[inline(always)]
pub(super) fn store_from_local_bucket<const ELEMENTS_PER_THREAD: usize>(
    lane_id: u32,
    bucket: &mut [PositionR; MAX_BUCKET_SIZE],
    local_bucket: &[PositionR; ELEMENTS_PER_THREAD],
) {
    // TODO: Every item is a pair of `u32`s, should this be rewritten for coalesced writes instead?
    //  If so, casting to an array of `u32`s is needed here, but rust-gpu doesn't support it yet:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    for local_offset in 0..ELEMENTS_PER_THREAD {
        let bucket_offset = lane_id as usize * ELEMENTS_PER_THREAD + local_offset;
        bucket[bucket_offset] = local_bucket[local_offset];
    }
}

// TODO: Make unsafe and avoid bounds check
/// Sort a bucket using bitonic sort
fn sort_bucket_impl<const ELEMENTS_PER_THREAD: usize>(
    lane_id: u32,
    bucket_size: u32,
    bucket: &mut [PositionR; MAX_BUCKET_SIZE],
) {
    let mut local_bucket = [PositionR::SENTINEL; ELEMENTS_PER_THREAD];

    load_into_local_bucket(lane_id, bucket_size, bucket, &mut local_bucket);

    sort_local_bucket(
        lane_id,
        &mut local_bucket,
        #[inline(always)]
        |a, b| a.position <= b.position,
    );

    store_from_local_bucket(lane_id, bucket, &local_bucket);
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
                sort_bucket_impl::<MAX_BUCKET_SIZE>(
                    subgroup_local_invocation_id,
                    bucket_size,
                    bucket,
                );
            }
            // Hypothetically possible
            2 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 2 }>(
                    subgroup_local_invocation_id,
                    bucket_size,
                    bucket,
                );
            }
            // LLVMpipe (Mesa 24, SSE)
            4 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 4 }>(
                    subgroup_local_invocation_id,
                    bucket_size,
                    bucket,
                );
            }
            // LLVMpipe (Mesa 25, AVX/AVX2)
            8 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 8 }>(
                    subgroup_local_invocation_id,
                    bucket_size,
                    bucket,
                );
            }
            // Raspberry PI 5
            16 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 16 }>(
                    subgroup_local_invocation_id,
                    bucket_size,
                    bucket,
                );
            }
            // Intel/Nvidia
            32 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 32 }>(
                    subgroup_local_invocation_id,
                    bucket_size,
                    bucket,
                );
            }
            // AMD
            64 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 64 }>(
                    subgroup_local_invocation_id,
                    bucket_size,
                    bucket,
                );
            }
            // Hypothetically possible
            128 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 128 }>(
                    subgroup_local_invocation_id,
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
