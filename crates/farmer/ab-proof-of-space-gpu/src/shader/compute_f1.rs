#[cfg(all(test, not(target_arch = "spirv")))]
mod cpu_tests;
#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{
    K, MAX_BUCKET_SIZE, MAX_TABLE_SIZE, NUM_BUCKETS, PARAM_BC, PARAM_EXT,
};
use crate::shader::num::{U64, U64T};
use crate::shader::types::{Position, PositionExt, PositionY, X, Y};
use core::mem::MaybeUninit;
use spirv_std::arch::atomic_i_add;
use spirv_std::glam::UVec3;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
const WORKGROUP_SIZE: u32 = 256;
// `+1` is needed due to the way `compute_fn_impl` does slightly outside what it, strictly speaking,
// needs (for efficiency purposes)
const KEYSTREAM_LEN_WORDS: usize = (K as usize * MAX_TABLE_SIZE as usize)
    .div_ceil(u8::BITS as usize)
    .div_ceil(size_of::<u32>())
    + 1;

// TODO: This is a polyfill to work around for this issue:
//  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
#[cfg(target_arch = "spirv")]
trait ArrayIndexingPolyfill<T> {
    /// The same as [`<[T]>::get_unchecked_mut()`]
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T;
}

#[cfg(target_arch = "spirv")]
impl<const N: usize, T> ArrayIndexingPolyfill<T> for [T; N] {
    #[inline(always)]
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        &mut self[index]
    }
}

// TODO: Make unsafe and avoid bounds check
// TODO: Reuse code from `ab-proof-of-space` after https://github.com/Rust-GPU/rust-gpu/pull/249 and
//  https://github.com/Rust-GPU/rust-gpu/discussions/301
/// `partial_y_offset` is in bits within `partial_y`
#[inline(always)]
fn compute_f1_impl(x: X, chacha8_keystream: &[u32; KEYSTREAM_LEN_WORDS]) -> Y {
    let skip_bits = u32::from(K) * u32::from(x);
    let skip_u32s = skip_bits / u32::BITS;
    let partial_y_offset = skip_bits % u32::BITS;

    let high = chacha8_keystream[skip_u32s as usize].to_be();
    let low = chacha8_keystream[skip_u32s as usize + 1].to_be();
    let partial_y = U64::from_low_high(low, high);

    let pre_y = partial_y >> (u64::BITS - u32::from(K + PARAM_EXT) - partial_y_offset);
    let pre_y = pre_y.as_u32();
    // Mask for clearing the rest of bits of `pre_y`.
    let pre_y_mask = (u32::MAX << PARAM_EXT) & (u32::MAX >> (u32::BITS - u32::from(K + PARAM_EXT)));

    // Extract `PARAM_EXT` most significant bits from `x` and store in the final offset of
    // eventual `y` with the rest of bits being zero (`x` is `0..2^K`)
    let pre_ext = u32::from(x) >> (K - PARAM_EXT);

    // Combine all of the bits together:
    // [padding zero bits][`K` bits rom `partial_y`][`PARAM_EXT` bits from `x`]
    Y::from((pre_y & pre_y_mask) | pre_ext)
}

/// Compute Chia's `f1()` function for the whole table using the previously computed ChaCha8
/// keystream straight into buckets of the first table.
///
/// Buckets need to be sorted by position afterward due to concurrent writes that do not have
/// deterministic order.
///
/// # Safety
/// `bucket_counts` must be zero-initialized, which is the case by default in `wgpu`.
#[spirv(compute(threads(256), entry_point_name = "compute_f1"))]
pub unsafe fn compute_f1(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)]
    chacha8_keystream: &[u32; KEYSTREAM_LEN_WORDS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] bucket_counts: &mut [u32;
             NUM_BUCKETS],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] buckets: &mut [[MaybeUninit<PositionY>; MAX_BUCKET_SIZE];
             NUM_BUCKETS],
) {
    // TODO: Make a single input bounds check and use unsafe to avoid bounds check later
    let global_invocation_id = global_invocation_id.x;
    let num_workgroups = num_workgroups.x;

    let global_size = WORKGROUP_SIZE * num_workgroups;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for x in (global_invocation_id..MAX_TABLE_SIZE).step_by(global_size as usize) {
        let y = compute_f1_impl(X::from(x), chacha8_keystream);

        let bucket_index = (u32::from(y) / u32::from(PARAM_BC)) as usize;
        // SAFETY: Bucket is obtained using division by `PARAM_BC` and fits by definition
        let bucket_count = unsafe { bucket_counts.get_unchecked_mut(bucket_index) };
        // TODO: Probably should not be unsafe to begin with:
        //  https://github.com/Rust-GPU/rust-gpu/pull/394#issuecomment-3316594485
        let position_in_bucket = unsafe {
            atomic_i_add::<_, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(
                bucket_count,
                1,
            )
        };

        // SAFETY: Bucket is obtained using division by `PARAM_BC` and fits by definition. Bucket
        // size upper bound is known statically to be [`MAX_BUCKET_SIZE`], so `position_in_bucket`
        // is also always within bounds.
        unsafe {
            buckets
                .get_unchecked_mut(bucket_index)
                .get_unchecked_mut(position_in_bucket as usize)
        }
        .write(PositionY {
            position: Position::from_u32(x),
            y,
        });
    }
}
