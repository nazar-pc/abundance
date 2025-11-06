#[cfg(all(test, not(target_arch = "spirv")))]
pub(super) mod cpu_tests;
#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;
pub mod rmap;

use crate::shader::MIN_SUBGROUP_SIZE;
use crate::shader::constants::{
    MAX_BUCKET_SIZE, PARAM_B, PARAM_C, PARAM_M, REDUCED_BUCKET_SIZE, REDUCED_MATCHES_COUNT,
};
use crate::shader::find_matches_in_buckets::rmap::{Rmap, RmapBitPosition, RmapBitPositionExt};
use crate::shader::types::{Match, Position, PositionExt, PositionR};
use core::mem::MaybeUninit;
use spirv_std::arch::{
    control_barrier, subgroup_exclusive_i_add, subgroup_i_add,
    workgroup_memory_barrier_with_group_sync,
};
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;
/// Worst-case for the number of subgroups
pub const MAX_SUBGROUPS: usize = WORKGROUP_SIZE as usize / MIN_SUBGROUP_SIZE as usize;

// TODO: This is a polyfill to work around for this issue:
//  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
#[cfg(target_arch = "spirv")]
trait ArrayIndexingPolyfill<T> {
    /// The same as [`<[T]>::get_unchecked()`]
    unsafe fn get_unchecked(&self, index: usize) -> &T;
    /// The same as [`<[T]>::get_unchecked_mut()`]
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T;
}

#[cfg(target_arch = "spirv")]
impl<const N: usize, T> ArrayIndexingPolyfill<T> for [T; N] {
    #[inline(always)]
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        &self[index]
    }

    #[inline(always)]
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        &mut self[index]
    }
}

fn calculate_left_target_on_demand(parity: u32, r: u32, m: u32) -> u32 {
    let param_b = u32::from(PARAM_B);
    let param_c = u32::from(PARAM_C);

    ((r / param_c + m) % param_b) * param_c + (((2 * m + parity) * (2 * m + parity) + r) % param_c)
}

#[derive(Debug)]
pub struct SharedScratchSpace {
    bucket_size_a: [MaybeUninit<u32>; REDUCED_BUCKET_SIZE],
    bucket_size_b: [MaybeUninit<u32>; REDUCED_BUCKET_SIZE],
    num_subgroups_size_a: [MaybeUninit<u32>; MAX_SUBGROUPS],
}

// TODO: Reuse code from `ab-proof-of-space` after https://github.com/Rust-GPU/rust-gpu/pull/249 and
//  https://github.com/Rust-GPU/rust-gpu/discussions/301
/// Returns the number of matches found.
///
/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads with `local_invocation_id` corresponding to the
/// thread index. `num_subgroups` must be at most [`MAX_SUBGROUPS`] and `subgroup_id` must be within
/// `0..num_subgroups`. All buckets must contain valid positions and `r` values and come from
/// `sort_buckets_with_rmap_details` shader.
// TODO: Try to reduce the `matches` size further by processing `left_bucket` in chunks (like halves
//  for example)
#[expect(clippy::too_many_arguments, reason = "Function is inlined anyway")]
#[inline(always)]
pub(super) unsafe fn find_matches_in_buckets_impl(
    subgroup_local_invocation_id: u32,
    subgroup_id: u32,
    num_subgroups: u32,
    local_invocation_id: u32,
    left_bucket_index: u32,
    // TODO: These should use `REDUCED_BUCKET_SIZE`, but it currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    left_bucket: &[PositionR; MAX_BUCKET_SIZE],
    right_bucket: &[PositionR; MAX_BUCKET_SIZE],
    matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    scratch_space: &mut SharedScratchSpace,
    rmap: &mut MaybeUninit<Rmap>,
) -> u32 {
    let SharedScratchSpace {
        bucket_size_a,
        bucket_size_b,
        num_subgroups_size_a,
    } = scratch_space;

    // Initialize `rmap`
    let rmap = {
        const {
            assert!(size_of::<Rmap>().is_multiple_of(size_of::<u32>()));
            assert!(align_of::<Rmap>() == align_of::<u32>());

            const fn assert_copy<T: Copy>() {}
            assert_copy::<Rmap>();
        }

        Rmap::reset(rmap, local_invocation_id, WORKGROUP_SIZE);

        workgroup_memory_barrier_with_group_sync();

        // SAFETY: Initialized with zeroes
        let rmap = unsafe { rmap.assume_init_mut() };

        for index in
            (local_invocation_id as usize..REDUCED_BUCKET_SIZE).step_by(WORKGROUP_SIZE as usize)
        {
            let PositionR { position, r } = right_bucket[index];

            // TODO: Wouldn't it make more sense to check the size here instead of sentinel?
            if position == Position::SENTINEL {
                break;
            }

            // SAFETY: Guaranteed by function contract
            unsafe {
                rmap.add_with_data_parallel(r, position);
            }
        }

        workgroup_memory_barrier_with_group_sync();

        rmap
    };

    // Load both into shared memory and precompute `rmap_bit_positions`. `rmap_bit_position`s for
    // non-sentinel positions are guaranteed to be initialized
    let (left_bucket_positions, left_rs) = {
        let left_bucket_positions =
            <Position as PositionExt>::uninit_array_from_repr_mut(bucket_size_a);
        let left_rs = bucket_size_b;
        // TODO: More idiomatic version currently doesn't compile:
        //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
        // for ((&left_position, position), r) in left_bucket
        //     .iter()
        //     .zip(left_bucket_positions.iter_mut())
        //     .zip(left_rs.iter_mut())
        //     .skip(local_invocation_id as usize)
        //     .step_by(WORKGROUP_SIZE as usize)
        // {
        for index in
            (local_invocation_id as usize..REDUCED_BUCKET_SIZE).step_by(WORKGROUP_SIZE as usize)
        {
            let PositionR { position, r } = left_bucket[index];
            let (r, _data) = r.split();
            let left_bucket_position = &mut left_bucket_positions[index];
            let r_entry = &mut left_rs[index];

            left_bucket_position.write(position);
            r_entry.write(r);
        }

        workgroup_memory_barrier_with_group_sync();

        // TODO: Correct version currently doesn't compile:
        //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
        // // SAFETY: Just initialized
        // let left_bucket_positions = unsafe {
        //     mem::transmute::<
        //         &mut [MaybeUninit<Position>; REDUCED_BUCKETS_SIZE],
        //         &mut [Position; REDUCED_BUCKETS_SIZE],
        //     >(left_bucket_positions)
        // };
        //
        // (&*left_bucket_positions, &*left_rs)
        (left_bucket_positions, &*left_rs)
    };

    let parity = left_bucket_index % 2;

    const CHUNK_SIZE: usize = WORKGROUP_SIZE as usize / PARAM_M as usize;
    const {
        // `CHUNK_SIZE` with `PARAM_M` must cover workgroup exactly
        assert!(CHUNK_SIZE as u32 * PARAM_M as u32 == WORKGROUP_SIZE);
        // The bucket size should be possible to iterate in exact chunks
        assert!(REDUCED_BUCKET_SIZE.is_multiple_of(CHUNK_SIZE));
    }
    let shared_subgroup_totals = num_subgroups_size_a;
    let mut global_match_batch_offset = 0_u32;
    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    // for (&left_positions, left_rs) in left_bucket_positions
    //     .as_chunks::<CHUNK_SIZE>()
    //     .0
    //     .iter()
    //     .zip(left_rs.as_chunks::<CHUNK_SIZE>().0)
    // {
    //     let left_positions = &left_bucket_positions.as_chunks::<CHUNK_SIZE>().0[index];
    //     let left_rs = &left_rs.as_chunks::<CHUNK_SIZE>().0[index];
    //
    //     let index_within_chunk = local_invocation_id as usize / CHUNK_SIZE;
    //     let left_position = left_positions[index_within_chunk];
    for chunk_index in 0..left_bucket_positions.len() / CHUNK_SIZE {
        // First `PARAM_M` invocations in a workgroup process the first chunk index, next
        // `PARAM_M` process the second chunk index and so on, with each chunk index corresponding
        // to `PARAM_M` `r_target` values
        let index_within_chunk = local_invocation_id as usize / PARAM_M as usize;
        let bucket_offset = chunk_index * CHUNK_SIZE + index_within_chunk;
        let left_position = unsafe { left_bucket_positions[bucket_offset].assume_init() };

        // TODO: Wouldn't it make more sense to check the size here instead of sentinel?
        // Check if reached the end of the bucket
        let [right_position_a, right_position_b] = if left_position == Position::SENTINEL {
            // `left_r` value doesn't matter here, it will not be read/used anyway
            [Position::SENTINEL; _]
        } else {
            // TODO: More idiomatic version currently doesn't compile:
            //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
            // // SAFETY: `left_position` is not sentinel, hence `left_r` must be initialized
            // let left_r = unsafe { left_rs[bucket_offset].assume_init() };
            let left_r = unsafe { left_rs[bucket_offset].assume_init() };
            let m = local_invocation_id % PARAM_M as u32;
            let r_target = calculate_left_target_on_demand(parity, left_r, m);

            // SAFETY: Targets are always limited to `PARAM_BC`
            unsafe { rmap.get(RmapBitPosition::new(r_target)) }
        };

        let local_matches_count = (right_position_a != Position::SENTINEL) as u32
            + (right_position_b != Position::SENTINEL) as u32;

        // Add up the numbers of matches in the subgroup up to the current lane (exclusive)
        let local_matches_prefix = subgroup_exclusive_i_add(local_matches_count);
        {
            // Add up the numbers of matches in the subgroup (total)
            let subgroup_matches_count = subgroup_i_add(local_matches_count);

            // TODO: should have been `subgroup_elect()`, but it is not implemented in `wgpu` yet:
            //  https://github.com/gfx-rs/wgpu/issues/5555
            if subgroup_local_invocation_id == 0 {
                // SAFETY: Guaranteed by function contract
                unsafe { shared_subgroup_totals.get_unchecked_mut(subgroup_id as usize) }
                    .write(subgroup_matches_count);
            }
        }

        workgroup_memory_barrier_with_group_sync();

        // Calculate offset for matches written by this subgroup and update global match batch
        // offset
        let mut subgroup_matches_offset = global_match_batch_offset;
        for current_subgroup_id in 0..num_subgroups {
            // SAFETY: Guaranteed by function contract
            let subgroup_matches_count = unsafe {
                shared_subgroup_totals
                    .get_unchecked(current_subgroup_id as usize)
                    .assume_init()
            };
            if current_subgroup_id < subgroup_id {
                subgroup_matches_offset += subgroup_matches_count;
            }
            global_match_batch_offset += subgroup_matches_count;
        }

        // Calculate offset where to write local matches into
        let mut local_matches_offset = subgroup_matches_offset + local_matches_prefix;

        if right_position_a != Position::SENTINEL {
            // TODO: More idiomatic version currently doesn't compile:
            //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
            // let Some(m) = matches.get_mut(local_matches_offset as usize) else {
            //     continue;
            // };
            if (local_matches_offset as usize) < matches.len() {
                let m = &mut matches[local_matches_offset as usize];

                // SAFETY: Positions are coming from the parent table and are valid
                // according to function contract, `bucket_offset` is guaranteed to be within
                // `0..NUM_BUCKETS * MAX_BUCKET_SIZE` range
                m.write(unsafe { Match::new(bucket_offset as u32, right_position_a) });

                local_matches_offset += 1;

                if right_position_b != Position::SENTINEL {
                    // TODO: More idiomatic version currently doesn't compile:
                    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
                    // let Some(m) = matches.get_mut(local_matches_offset as usize) else {
                    //     continue;
                    // };
                    if (local_matches_offset as usize) < matches.len() {
                        let m = &mut matches[local_matches_offset as usize];

                        // SAFETY: Positions are coming from the parent table and are valid
                        // according to function contract, `bucket_offset` is guaranteed to be
                        // within `0..NUM_BUCKETS * MAX_BUCKET_SIZE` range
                        m.write(unsafe { Match::new(bucket_offset as u32, right_position_b) });
                    }
                }
            }
        }

        // Make sure workgroup progresses predictably in phases for offsets to work properly
        control_barrier::<
            { Scope::Workgroup as u32 },
            { Scope::Workgroup as u32 },
            { Semantics::NONE.bits() },
        >();
    }

    global_match_batch_offset.min(REDUCED_MATCHES_COUNT as u32)
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`]. All buckets must contain valid positions and `r` values and come from
/// `sort_buckets_with_rmap_details` shader.
#[spirv(compute(threads(256), entry_point_name = "find_matches_in_buckets"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_in_buckets(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(subgroup_local_invocation_id)] subgroup_local_invocation_id: u32,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] buckets: &[[PositionR;
          MAX_BUCKET_SIZE]],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)]
    matches: &mut [[MaybeUninit<Match>; REDUCED_MATCHES_COUNT]],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] matches_counts: &mut [MaybeUninit<
        u32,
    >],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)]
    rmap: &mut [MaybeUninit<Rmap>; MAX_SUBGROUPS],
) {
    let local_invocation_id = local_invocation_id.x;
    let workgroup_id = workgroup_id.x;

    let left_bucket_index = workgroup_id as usize;
    if left_bucket_index >= buckets.len() - 1 {
        return;
    }

    let left_bucket = &buckets[left_bucket_index];
    let right_bucket = &buckets[left_bucket_index + 1];
    let matches = &mut matches[left_bucket_index];
    let matches_count = &mut matches_counts[left_bucket_index];

    // TODO: Truncate buckets to reduced size here once it compiles:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    // SAFETY: Guaranteed by function contract
    matches_count.write(unsafe {
        find_matches_in_buckets_impl(
            subgroup_local_invocation_id,
            subgroup_id,
            num_subgroups,
            local_invocation_id,
            left_bucket_index as u32,
            left_bucket,
            right_bucket,
            matches,
            scratch_space,
            #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
            rmap,
            #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
            &mut rmap[subgroup_id as usize],
        )
    });
}
