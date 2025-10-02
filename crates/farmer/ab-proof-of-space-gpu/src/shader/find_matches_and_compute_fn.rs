#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod cpu_tests;
#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::compute_fn::compute_fn_impl;
use crate::shader::constants::{
    MAX_BUCKET_SIZE, NUM_BUCKETS, NUM_MATCH_BUCKETS, PARAM_BC, REDUCED_MATCHES_COUNT,
};
use crate::shader::find_matches_in_buckets::rmap::Rmap;
use crate::shader::find_matches_in_buckets::{
    LeftTargets, Match, SharedScratchSpace, find_matches_in_buckets_impl,
};
use crate::shader::types::{Metadata, Position, PositionExt, PositionY};
use core::mem::MaybeUninit;
use spirv_std::arch::{atomic_i_increment, workgroup_memory_barrier_with_group_sync};
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;

const _: () = {
    assert!(crate::shader::find_matches_in_buckets::WORKGROUP_SIZE == WORKGROUP_SIZE);
};

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

/// # Safety
/// `bucket_index` must be within range `0..REDUCED_MATCHES_COUNT`. `matches_count` elements in
/// `matches` must be initialized, `matches` must have valid pointers into `parent_metadatas`.
#[inline(always)]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
unsafe fn compute_fn_into_buckets<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    local_invocation_id: u32,
    bucket_index: u32,
    matches_count: usize,
    // TODO: `&[Match]` would have been nicer, but it currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    // TODO: This should have been `&[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]`, but it
    //  currently doesn't compile if flattened:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    bucket_counts: &mut [u32; NUM_BUCKETS],
    buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    positions: &mut [MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT],
    metadatas: &mut [MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT],
) {
    let metadatas_offset = bucket_index * REDUCED_MATCHES_COUNT as u32;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for index in (local_invocation_id..matches_count as u32).step_by(WORKGROUP_SIZE as usize) {
        // SAFETY: Guaranteed by function contract
        let m = unsafe { matches.get_unchecked(index as usize).assume_init() };
        // TODO: Correct version currently doesn't compile:
        //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
        // let left_metadata = parent_metadatas[usize::from(m.left_position)];
        // let right_metadata = parent_metadatas[usize::from(m.right_position)];
        // SAFETY: Guaranteed by function contract
        let left_metadata = *unsafe { parent_metadatas.get_unchecked(m.left_position as usize) };
        // SAFETY: Guaranteed by function contract
        let right_metadata = *unsafe { parent_metadatas.get_unchecked(m.right_position as usize) };

        let (y, metadata) = compute_fn_impl::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
            m.left_y,
            left_metadata,
            right_metadata,
        );

        let bucket_index = (u32::from(y) / u32::from(PARAM_BC)) as usize;
        // SAFETY: Bucket is obtained using division by `PARAM_BC` and fits by definition
        let bucket_count = unsafe { bucket_counts.get_unchecked_mut(bucket_index) };
        // TODO: Probably should not be unsafe to begin with:
        //  https://github.com/Rust-GPU/rust-gpu/pull/394#issuecomment-3316594485
        let bucket_offset = unsafe {
            atomic_i_increment::<_, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(
                bucket_count,
            )
        };

        // SAFETY: Bucket is obtained using division by `PARAM_BC` and fits by definition. Bucket
        // size upper bound is known statically to be [`MAX_BUCKET_SIZE`], so `bucket_offset`
        // is also always within bounds.
        unsafe {
            buckets
                .get_unchecked_mut(bucket_index)
                .get_unchecked_mut(bucket_offset as usize)
        }
        .write(PositionY {
            position: Position::from_u32(metadatas_offset + index),
            y,
        });

        positions[index as usize].write([m.left_position, m.right_position]);

        // The last table doesn't have any metadata
        if TABLE_NUMBER < 7 {
            metadatas[index as usize].write(metadata);
        }
    }
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
///
/// [`MAX_SUBGROUPS`]: crate::shader::find_matches_in_buckets::MAX_SUBGROUPS
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_and_compute_fn<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    local_invocation_id: UVec3,
    workgroup_id: UVec3,
    num_workgroups: UVec3,
    subgroup_id: u32,
    num_subgroups: u32,
    left_targets: &LeftTargets,
    parent_buckets: &[[PositionY; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    bucket_counts: &mut [u32; NUM_BUCKETS],
    buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    positions: &mut [[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS],
    metadatas: &mut [[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS],
    matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    rmap: &mut MaybeUninit<Rmap>,
) {
    let local_invocation_id = local_invocation_id.x;
    let workgroup_id = workgroup_id.x;
    let num_workgroups = num_workgroups.x;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    // for (left_bucket_index, (([left_bucket, right_bucket], matches), matches_count)) in buckets
    //     .array_windows::<2>()
    //     .zip(matches)
    //     .zip(matches_counts)
    //     .enumerate()
    //     .skip(workgroup_id as usize)
    //     .step_by(num_workgroups as usize)
    // {
    for left_bucket_index in
        (workgroup_id as usize..NUM_MATCH_BUCKETS).step_by(num_workgroups as usize)
    {
        let left_bucket = &parent_buckets[left_bucket_index];
        let right_bucket = &parent_buckets[left_bucket_index + 1];
        let positions = &mut positions[left_bucket_index];
        let metadatas = &mut metadatas[left_bucket_index];
        let left_bucket_index = left_bucket_index as u32;

        // TODO: Truncate buckets to reduced size here once it compiles:
        //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
        // SAFETY: Guaranteed by function contract
        let matches_count = unsafe {
            find_matches_in_buckets_impl(
                subgroup_id,
                num_subgroups,
                local_invocation_id,
                left_bucket_index,
                left_bucket,
                right_bucket,
                matches,
                left_targets,
                scratch_space,
                rmap,
            )
        };

        workgroup_memory_barrier_with_group_sync();

        unsafe {
            compute_fn_into_buckets::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
                local_invocation_id,
                left_bucket_index,
                matches_count as usize,
                matches,
                parent_metadatas,
                bucket_counts,
                buckets,
                positions,
                metadatas,
            );
        }

        // No need for explicit synchronization, `matches` will not be touched before extra
        // synchronization in `find_matches_in_buckets_impl` again anyway
    }
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order. Content of the bucket beyond the size specified in `bucket_counts` is
/// undefined.
///
/// [`MAX_SUBGROUPS`]: crate::shader::find_matches_in_buckets::MAX_SUBGROUPS
#[spirv(compute(threads(256), entry_point_name = "find_matches_and_compute_f2"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_and_compute_f2(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] left_targets: &LeftTargets,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_buckets: &[[PositionY; MAX_BUCKET_SIZE];
         NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)]
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] bucket_counts: &mut [u32;
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] positions: &mut [[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] metadatas: &mut [[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(workgroup)] matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)]
    rmap: &mut MaybeUninit<Rmap>,
) {
    // SAFETY: Guaranteed by function contract
    unsafe {
        find_matches_and_compute_fn::<2, 1>(
            local_invocation_id,
            workgroup_id,
            num_workgroups,
            subgroup_id,
            num_subgroups,
            left_targets,
            parent_buckets,
            parent_metadatas,
            bucket_counts,
            buckets,
            positions,
            metadatas,
            matches,
            scratch_space,
            rmap,
        );
    }
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order. Content of the bucket beyond the size specified in `bucket_counts` is
/// undefined.
///
/// [`MAX_SUBGROUPS`]: crate::shader::find_matches_in_buckets::MAX_SUBGROUPS
#[spirv(compute(threads(256), entry_point_name = "find_matches_and_compute_f3"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_and_compute_f3(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] left_targets: &LeftTargets,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_buckets: &[[PositionY; MAX_BUCKET_SIZE];
         NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)]
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] bucket_counts: &mut [u32;
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] positions: &mut [[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] metadatas: &mut [[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(workgroup)] matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 76)]
    rmap: &mut MaybeUninit<Rmap>,
) {
    // SAFETY: Guaranteed by function contract
    unsafe {
        find_matches_and_compute_fn::<3, 2>(
            local_invocation_id,
            workgroup_id,
            num_workgroups,
            subgroup_id,
            num_subgroups,
            left_targets,
            parent_buckets,
            parent_metadatas,
            bucket_counts,
            buckets,
            positions,
            metadatas,
            matches,
            scratch_space,
            rmap,
        );
    }
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order. Content of the bucket beyond the size specified in `bucket_counts` is
/// undefined.
///
/// [`MAX_SUBGROUPS`]: crate::shader::find_matches_in_buckets::MAX_SUBGROUPS
#[spirv(compute(threads(256), entry_point_name = "find_matches_and_compute_f4"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_and_compute_f4(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] left_targets: &LeftTargets,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_buckets: &[[PositionY; MAX_BUCKET_SIZE];
         NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)]
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] bucket_counts: &mut [u32;
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] positions: &mut [[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] metadatas: &mut [[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(workgroup)] matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 76)]
    rmap: &mut MaybeUninit<Rmap>,
) {
    // SAFETY: Guaranteed by function contract
    unsafe {
        find_matches_and_compute_fn::<4, 3>(
            local_invocation_id,
            workgroup_id,
            num_workgroups,
            subgroup_id,
            num_subgroups,
            left_targets,
            parent_buckets,
            parent_metadatas,
            bucket_counts,
            buckets,
            positions,
            metadatas,
            matches,
            scratch_space,
            rmap,
        );
    }
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order. Content of the bucket beyond the size specified in `bucket_counts` is
/// undefined.
///
/// [`MAX_SUBGROUPS`]: crate::shader::find_matches_in_buckets::MAX_SUBGROUPS
#[spirv(compute(threads(256), entry_point_name = "find_matches_and_compute_f5"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_and_compute_f5(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] left_targets: &LeftTargets,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_buckets: &[[PositionY; MAX_BUCKET_SIZE];
         NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)]
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] bucket_counts: &mut [u32;
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] positions: &mut [[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] metadatas: &mut [[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(workgroup)] matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 76)]
    rmap: &mut MaybeUninit<Rmap>,
) {
    // SAFETY: Guaranteed by function contract
    unsafe {
        find_matches_and_compute_fn::<5, 4>(
            local_invocation_id,
            workgroup_id,
            num_workgroups,
            subgroup_id,
            num_subgroups,
            left_targets,
            parent_buckets,
            parent_metadatas,
            bucket_counts,
            buckets,
            positions,
            metadatas,
            matches,
            scratch_space,
            rmap,
        );
    }
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order. Content of the bucket beyond the size specified in `bucket_counts` is
/// undefined.
///
/// [`MAX_SUBGROUPS`]: crate::shader::find_matches_in_buckets::MAX_SUBGROUPS
#[spirv(compute(threads(256), entry_point_name = "find_matches_and_compute_f6"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_and_compute_f6(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] left_targets: &LeftTargets,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_buckets: &[[PositionY; MAX_BUCKET_SIZE];
         NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)]
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] bucket_counts: &mut [u32;
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] positions: &mut [[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] metadatas: &mut [[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(workgroup)] matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 76)]
    rmap: &mut MaybeUninit<Rmap>,
) {
    // SAFETY: Guaranteed by function contract
    unsafe {
        find_matches_and_compute_fn::<6, 5>(
            local_invocation_id,
            workgroup_id,
            num_workgroups,
            subgroup_id,
            num_subgroups,
            left_targets,
            parent_buckets,
            parent_metadatas,
            bucket_counts,
            buckets,
            positions,
            metadatas,
            matches,
            scratch_space,
            rmap,
        );
    }
}

/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order. Content of the bucket beyond the size specified in `bucket_counts` is
/// undefined.
///
/// [`MAX_SUBGROUPS`]: crate::shader::find_matches_in_buckets::MAX_SUBGROUPS
#[spirv(compute(threads(256), entry_point_name = "find_matches_and_compute_f7"))]
#[expect(
    clippy::too_many_arguments,
    reason = "Both I/O and Vulkan stuff together take a lot of arguments"
)]
pub unsafe fn find_matches_and_compute_f7(
    #[spirv(local_invocation_id)] local_invocation_id: UVec3,
    #[spirv(workgroup_id)] workgroup_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(subgroup_id)] subgroup_id: u32,
    #[spirv(num_subgroups)] num_subgroups: u32,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] left_targets: &LeftTargets,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_buckets: &[[PositionY; MAX_BUCKET_SIZE];
         NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)]
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] bucket_counts: &mut [u32;
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] positions: &mut [[MaybeUninit<[Position; 2]>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] metadatas: &mut [[MaybeUninit<Metadata>; REDUCED_MATCHES_COUNT];
             NUM_MATCH_BUCKETS],
    #[spirv(workgroup)] matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 76)]
    rmap: &mut MaybeUninit<Rmap>,
) {
    // SAFETY: Guaranteed by function contract
    unsafe {
        find_matches_and_compute_fn::<7, 6>(
            local_invocation_id,
            workgroup_id,
            num_workgroups,
            subgroup_id,
            num_subgroups,
            left_targets,
            parent_buckets,
            parent_metadatas,
            bucket_counts,
            buckets,
            positions,
            metadatas,
            matches,
            scratch_space,
            rmap,
        );
    }
}
