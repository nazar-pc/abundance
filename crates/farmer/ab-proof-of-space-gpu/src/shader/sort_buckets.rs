#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{MAX_BUCKET_SIZE, NUM_BUCKETS};
use crate::shader::types::{Position, PositionExt, PositionY, Y};
use spirv_std::arch::subgroup_shuffle;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;

#[inline(always)]
fn perform_local_compare_swap<const ELEMENTS_PER_THREAD: usize>(
    lane_id: u32,
    bit_position: u32,
    block_size: usize,
    local_data: &mut [PositionY; ELEMENTS_PER_THREAD],
) {
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

        let a = local_data[a_offset];
        let b = local_data[b_offset];

        // Determine the sort direction: ascending if `a_offset`'s bit at `block_size` is `0`.
        // This alternates direction in bitonic merges to create sorted sequences.
        let select_smaller =
            ((lane_id as usize * ELEMENTS_PER_THREAD + a_offset) & block_size) == 0;

        let (final_a, final_b) = if (a.position <= b.position) == select_smaller {
            (a, b)
        } else {
            (b, a)
        };

        local_data[a_offset] = final_a;
        local_data[b_offset] = final_b;
    }
}

#[inline(always)]
fn perform_cross_compare_swap<const ELEMENTS_PER_THREAD: usize>(
    lane_id: u32,
    bit_position: u32,
    block_size: usize,
    local_data: &mut [PositionY; ELEMENTS_PER_THREAD],
) {
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
        let a = local_data[a_offset];
        let b = {
            let position = subgroup_shuffle(a.position, partner_lane_id);
            let y = Y::from(subgroup_shuffle(u32::from(a.y), partner_lane_id));

            PositionY { position, y }
        };

        let selected_value = if (a.position <= b.position) == select_smaller {
            a
        } else {
            b
        };

        local_data[a_offset] = selected_value;
    }
}

// TODO: Make unsafe and avoid bounds check
/// Sort a bucket using bitonic sort
#[inline(always)]
fn sort_bucket_impl<const ELEMENTS_PER_THREAD: usize>(
    lane_id: u32,
    bucket_size: u32,
    bucket: &mut [PositionY; MAX_BUCKET_SIZE],
) {
    let mut local_data = [PositionY::EMPTY; ELEMENTS_PER_THREAD];

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    for local_offset in 0..ELEMENTS_PER_THREAD {
        let bucket_offset = lane_id as usize * ELEMENTS_PER_THREAD + local_offset;
        local_data[local_offset] = if bucket_offset < bucket_size as usize {
            bucket[bucket_offset]
        } else {
            PositionY {
                position: Position::SENTINEL,
                y: Y::SENTINEL,
            }
        };
    }

    // Iterate over merger stages, doubling block_size each time
    let mut block_size = 2;
    let mut merger_stage = 1;
    while block_size <= MAX_BUCKET_SIZE {
        // For each stage, process bit positions in reverse for bitonic comparisons
        for bit_position in (0..merger_stage).rev() {
            if bit_position < ELEMENTS_PER_THREAD.ilog2() {
                // Local swaps within thread's registers, no synchronization needed
                perform_local_compare_swap(lane_id, bit_position, block_size, &mut local_data);
            } else {
                // Cross-lane swaps using subgroup shuffles for communication
                perform_cross_compare_swap(lane_id, bit_position, block_size, &mut local_data);
            }
        }
        block_size *= 2;
        merger_stage += 1;
    }

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    #[expect(
        clippy::needless_range_loop,
        reason = "rust-gpu can't compile idiomatic version"
    )]
    for local_offset in 0..ELEMENTS_PER_THREAD {
        let bucket_offset = lane_id as usize * ELEMENTS_PER_THREAD + local_offset;
        bucket[bucket_offset] = local_data[local_offset];
    }
}

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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] bucket_sizes: &[u32; NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] buckets: &mut [[PositionY; MAX_BUCKET_SIZE];
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
        // Specify some common subgroup sizes so the driver can easily eliminate dead code. This is
        // important because `local_data` inside the function is generic and impacts the number of
        // registers used, so we want to minimize them.
        match subgroup_size {
            // Hypothetically possible
            1 => {
                sort_bucket_impl::<MAX_BUCKET_SIZE>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            // Hypothetically possible
            2 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 2 }>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            // llvmpipe
            4 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 4 }>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            // Hypothetically possible
            8 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 8 }>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            // Raspberry PI 5
            16 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 16 }>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            // Intel/Nvidia
            32 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 32 }>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            // AMD
            64 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 64 }>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            // Hypothetically possible
            128 => {
                sort_bucket_impl::<{ MAX_BUCKET_SIZE / 128 }>(
                    subgroup_local_invocation_id,
                    bucket_sizes[bucket_index as usize],
                    &mut buckets[bucket_index as usize],
                );
            }
            _ => {
                // https://registry.khronos.org/vulkan/specs/latest/man/html/SubgroupSize.html
                unreachable!("All Vulkan targets use power of two and subgroup size <= 128")
            }
        }
    }
}
