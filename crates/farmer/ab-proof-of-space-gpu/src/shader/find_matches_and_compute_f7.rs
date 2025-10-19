#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod cpu_tests;
#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::compute_fn::compute_fn_impl;
use crate::shader::constants::{
    MAX_BUCKET_SIZE, NUM_BUCKETS, NUM_MATCH_BUCKETS, NUM_S_BUCKETS, REDUCED_MATCHES_COUNT,
};
use crate::shader::find_matches_in_buckets::rmap::Rmap;
use crate::shader::find_matches_in_buckets::{
    MAX_SUBGROUPS, Match, SharedScratchSpace, find_matches_in_buckets_impl,
};
use crate::shader::types::{Metadata, Position, PositionY};
use core::mem::MaybeUninit;
use spirv_std::arch::{atomic_i_increment, workgroup_memory_barrier_with_group_sync};
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
pub const WORKGROUP_SIZE: u32 = 256;
const TABLE_NUMBER: u8 = 7;
const PARENT_TABLE_NUMBER: u8 = 6;

const _: () = {
    assert!(crate::shader::find_matches_in_buckets::WORKGROUP_SIZE == WORKGROUP_SIZE);
};

const PROOFS_BUCKET_SIZE_UPPER_BOUND_SECURITY_BITS: u8 = 128;
/// Upper-bound estimation of the number of matched elements per s-bucket
pub const NUM_ELEMENTS_PER_S_BUCKET: usize =
    proofs_bucket_upper_bound(PROOFS_BUCKET_SIZE_UPPER_BOUND_SECURITY_BITS) as usize;

/// Upper-bound estimation of the number of matched elements per s-bucket.
///
/// Buckets are defined by the lower `NUM_S_BUCKETS.ilog2()` bits of the values. This is based on a
/// Chernoff bound for the Poisson distribution with mean `lambda = 1`, ensuring the probability
/// that any bucket exceeds the bound is less than `2^{-security_bits}`. The bound is
/// `lambda + ceil(sqrt(3 * lambda * (NUM_S_BUCKETS.ilog2() + security_bits) * ln(2)))`.
/// Accounts for the filter to values in `0..NUM_S_BUCKETS-1` by using the expected number of
/// remaining elements ~`NUM_S_BUCKETS`, distributed uniformly across all `NUM_S_BUCKETS` buckets.
const fn proofs_bucket_upper_bound(security_bits: u8) -> u64 {
    // Lambda is the expected number of entries in a bucket:
    // ~`NUM_S_BUCKETS / NUM_S_BUCKETS = 1`
    const LAMBDA: u64 = 1;
    // Approximation of ln(2) as a fraction: `ln(2) ≈ LN2_NUM / LN2_DEN`.
    // This allows integer-only computation of the square root term involving ln(2).
    const LN2_NUM: u128 = 693147;
    const LN2_DEN: u128 = 1000000;

    // `log2(NUM_S_BUCKETS) + security_bits` for the union bound over `NUM_S_BUCKETS` buckets
    let ks = NUM_S_BUCKETS.ilog2() as u128 + security_bits as u128;
    // Compute numerator for the expression under the square root:
    // `3 * lambda * ks * LN2_NUM`
    let num = 3u128 * LAMBDA as u128 * ks * LN2_NUM;
    // Denominator for ln(2): `LN2_DEN`
    let den = LN2_DEN;

    let ceil_div = num.div_ceil(den);

    // Binary search to find the smallest `x` such that `x * x >= ceil_div`,
    // which computes `ceil(sqrt(num / den))` without floating-point.
    // We use a custom binary search over `u64` range because binary search in the standard library
    // operates on sorted slices, not directly on integer ranges for solving inequalities like this.
    let mut low = 0u64;
    let mut high = u64::MAX;
    while low < high {
        let mid = low + (high - low) / 2;
        let left = (mid as u128) * (mid as u128);
        if left >= ceil_div {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    let add_term = low;

    LAMBDA + add_term
}

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
unsafe fn compute_fn_into_buckets(
    local_invocation_id: u32,
    matches_count: usize,
    // TODO: `&[Match]` would have been nicer, but it currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    // TODO: This should have been `&[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS]`, but it
    //  currently doesn't compile if flattened:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    bucket_sizes: &mut [u32; NUM_S_BUCKETS],
    table_6_proof_targets: &mut [[MaybeUninit<[Position; 2]>; NUM_ELEMENTS_PER_S_BUCKET];
             NUM_S_BUCKETS],
) {
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

        let (y, _) = compute_fn_impl::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
            m.left_y,
            left_metadata,
            right_metadata,
        );

        let s_bucket = y.first_k_bits() as usize;
        // TODO: More idiomatic version currently doesn't compile:
        //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
        // let Some(bucket_count) = bucket_sizes.get_mut(s_bucket) else {
        //     continue;
        // };
        if s_bucket >= NUM_S_BUCKETS {
            continue;
        }
        let bucket_size = &mut bucket_sizes[s_bucket];
        // TODO: Probably should not be unsafe to begin with:
        //  https://github.com/Rust-GPU/rust-gpu/pull/394#issuecomment-3316594485
        let bucket_offset = unsafe {
            atomic_i_increment::<_, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(
                bucket_size,
            )
        };

        // SAFETY: `s_bucket` is checked above to be correct. Bucket size upper bound is known
        // statically to be [`NUM_ELEMENTS_PER_S_BUCKET`], so `bucket_offset` is also always within
        // bounds.
        unsafe {
            table_6_proof_targets
                .get_unchecked_mut(s_bucket)
                .get_unchecked_mut(bucket_offset as usize)
        }
        .write([m.left_position, m.right_position]);
    }
}

/// This is similar to `find_matches_and_compute_fn`, but it stores results in buckets grouped by
/// s-buckets, which is how proofs can later be found efficiently.
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order. Content of the bucket beyond the size specified in `bucket_sizes` is
/// undefined.
///
/// # Safety
/// Must be called from [`WORKGROUP_SIZE`] threads. `num_subgroups` must be at most
/// [`MAX_SUBGROUPS`].
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] parent_buckets: &[[PositionY; MAX_BUCKET_SIZE];
         NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)]
    parent_metadatas: &[Metadata; REDUCED_MATCHES_COUNT * NUM_MATCH_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] bucket_sizes: &mut [u32;
             NUM_S_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)]
    table_6_proof_targets: &mut [[MaybeUninit<[Position; 2]>; NUM_ELEMENTS_PER_S_BUCKET];
             NUM_S_BUCKETS],
    #[spirv(workgroup)] matches: &mut [MaybeUninit<Match>; REDUCED_MATCHES_COUNT],
    #[spirv(workgroup)] scratch_space: &mut SharedScratchSpace,
    // Non-modern GPUs do not have enough space in the shared memory
    #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
    #[spirv(workgroup)]
    rmap: &mut MaybeUninit<Rmap>,
    #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)]
    rmap: &mut [MaybeUninit<Rmap>; MAX_SUBGROUPS],
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
                scratch_space,
                #[cfg(all(target_arch = "spirv", feature = "__modern-gpu"))]
                rmap,
                #[cfg(not(all(target_arch = "spirv", feature = "__modern-gpu")))]
                &mut rmap[subgroup_id as usize],
            )
        };

        workgroup_memory_barrier_with_group_sync();

        unsafe {
            compute_fn_into_buckets(
                local_invocation_id,
                matches_count as usize,
                matches,
                parent_metadatas,
                bucket_sizes,
                table_6_proof_targets,
            );
        }

        // No need for explicit synchronization, `matches` will not be touched before extra
        // synchronization in `find_matches_in_buckets_impl` again anyway
    }
}
