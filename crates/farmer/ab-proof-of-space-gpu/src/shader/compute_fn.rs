#[cfg(all(test, not(target_arch = "spirv")))]
mod cpu_tests;
#[cfg(all(test, not(miri), not(target_arch = "spirv")))]
mod gpu_tests;

use crate::shader::constants::{K, PARAM_EXT};
use crate::shader::find_matches_in_buckets::Match;
use crate::shader::num::{U128, U128T};
use crate::shader::types::{Metadata, Y};
use core::mem::MaybeUninit;
use spirv_std::glam::UVec3;
use spirv_std::spirv;

// TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` below, can be removed once
//  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
const WORKGROUP_SIZE: u32 = 256;

// TODO: Reuse code from `ab-proof-of-space` after https://github.com/Rust-GPU/rust-gpu/pull/249 and
//  https://github.com/Rust-GPU/rust-gpu/discussions/301
/// Compute the size of `y` in bits
const fn y_size_bits(k: u8) -> u32 {
    k as u32 + PARAM_EXT as u32
}

// TODO: Reuse code from `ab-proof-of-space` after https://github.com/Rust-GPU/rust-gpu/pull/249 and
//  https://github.com/Rust-GPU/rust-gpu/discussions/301
/// Metadata size in bits
const fn metadata_size_bits(k: u8, table_number: u8) -> u32 {
    k as u32
        * match table_number {
            1 => 1,
            2 => 2,
            3 | 4 => 4,
            5 => 3,
            6 => 2,
            7 => 0,
            _ => unreachable!(),
        }
}

// TODO: Make unsafe and avoid bounds check
// TODO: Reuse code from `ab-proof-of-space` after https://github.com/Rust-GPU/rust-gpu/pull/249 and
//  https://github.com/Rust-GPU/rust-gpu/discussions/301
#[inline(always)]
pub(super) fn compute_fn_impl<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    y: Y,
    left_metadata: Metadata,
    right_metadata: Metadata,
) -> (Y, Metadata) {
    let left_metadata = U128::from(left_metadata);
    let right_metadata = U128::from(right_metadata);

    // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
    //  shouldn't be necessary otherwise
    let parent_metadata_bits = const { metadata_size_bits(K, PARENT_TABLE_NUMBER) };

    // Only supports `K` from 15 to 25 (otherwise math will not be correct when concatenating y,
    // left metadata and right metadata)
    let mut input_words = [0; _];
    let byte_length = {
        // Take only bytes where bits were set
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        let num_bytes_with_data =
            (const { y_size_bits(K) } + parent_metadata_bits * 2).div_ceil(u8::BITS);

        // Collect `K` most significant bits of `y` at the final offset of eventual `input_a`
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        let y_bits = U128::from(y) << (u128::BITS - const { y_size_bits(K) });

        // Move bits of `left_metadata` at the final offset of eventual `input_a`
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        let left_metadata_bits =
            left_metadata << (u128::BITS - parent_metadata_bits - const { y_size_bits(K) });

        // Part of the `right_bits` at the final offset of eventual `input_a`
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        let y_and_left_bits = const { y_size_bits(K) } + parent_metadata_bits;
        let right_bits_start_offset = u128::BITS - parent_metadata_bits;

        // If `right_metadata` bits start to the left of the desired position in `input_a` move
        // bits right, else move left
        if right_bits_start_offset < y_and_left_bits {
            let right_bits_pushed_into_input_b = y_and_left_bits - right_bits_start_offset;
            // Collect bits of `right_metadata` that will fit into `input_a` at the final offset in
            // eventual `input_a`
            let right_bits_a = right_metadata >> right_bits_pushed_into_input_b;
            let input_a = y_bits | left_metadata_bits | right_bits_a;
            // Collect bits of `right_metadata` that will spill over into `input_b`
            let input_b = right_metadata << (u128::BITS - right_bits_pushed_into_input_b);

            let input_a_words = input_a.as_be_bytes_to_le_u32_words();
            // TODO: Manually indexing elements and constructing an array is a workaround for
            //  rust-gpu to compile
            // input_words[..input_a_words.len()].copy_from_slice(&input_a_words);
            input_words[0] = input_a_words[0];
            input_words[1] = input_a_words[1];
            input_words[2] = input_a_words[2];
            input_words[3] = input_a_words[3];
            let input_b_words = input_b.as_be_bytes_to_le_u32_words();
            // TODO: Manually indexing elements and constructing an array is a workaround for
            //  rust-gpu to compile
            // input_words[input_a_words.len()..].copy_from_slice(&input_b_words);
            input_words[4] = input_b_words[0];
            input_words[5] = input_b_words[1];
            input_words[6] = input_b_words[2];
            input_words[7] = input_b_words[3];

            size_of::<u128>() as u32 + right_bits_pushed_into_input_b.div_ceil(u8::BITS)
        } else {
            let right_bits_a = right_metadata << (right_bits_start_offset - y_and_left_bits);
            let input_a = y_bits | left_metadata_bits | right_bits_a;
            let input_a_words = input_a.as_be_bytes_to_le_u32_words();
            // TODO: Manually indexing elements and constructing an array is a workaround for
            //  rust-gpu to compile
            // input_words[..input_a_words.len()].copy_from_slice(&input_a_words);
            input_words[0] = input_a_words[0];
            input_words[1] = input_a_words[1];
            input_words[2] = input_a_words[2];
            input_words[3] = input_a_words[3];

            num_bytes_with_data
        }
    };
    let hash = ab_blake3::single_block_hash_portable_words(&input_words, byte_length);

    // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
    //  shouldn't be necessary otherwise
    let y_output = Y::from(hash[0].to_be() >> (u32::BITS - const { y_size_bits(K) }));

    // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
    //  shouldn't be necessary otherwise
    let metadata_size_bits = const { metadata_size_bits(K, TABLE_NUMBER) };

    let metadata = if TABLE_NUMBER < 4 {
        Metadata::from((left_metadata << parent_metadata_bits) | right_metadata)
    } else if metadata_size_bits > 0 {
        // For K up to 24 it is guaranteed that metadata + bit offset will always fit into 4 `u32`
        // words (equivalent to `u128` size). For K=25 it'll be necessary to have fifth word, which
        // will become more cumbersome to handle. We collect bytes necessary, potentially with extra
        // bits at the start and end of the bytes that will be taken care of later.
        // TODO: Manually indexing elements and constructing an array is a workaround for rust-gpu
        //  to compile
        // let metadata = U128::from_le_u32_words_as_be_bytes(
        //     hash[(y_size_bits(K) / u32::BITS) as usize..][..size_of::<u128>() / size_of::<u32>()]
        //         .try_into()
        //         .expect("Always enough bits for any K; qed"),
        // );
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        let first_element = (const { y_size_bits(K) } / u32::BITS) as usize;
        let metadata = U128::from_le_u32_words_as_be_bytes(&[
            hash[first_element],
            hash[first_element + 1],
            hash[first_element + 2],
            hash[first_element + 3],
        ]);
        // Remove extra bits at the beginning
        // TODO: `const {}` is a workaround for https://github.com/Rust-GPU/rust-gpu/issues/322 and
        //  shouldn't be necessary otherwise
        let metadata = metadata << (const { y_size_bits(K) } % u32::BITS);
        // Move bits into the correct location
        Metadata::from(metadata >> (u128::BITS - metadata_size_bits))
    } else {
        Metadata::default()
    };

    (y_output, metadata)
}

#[inline(always)]
fn compute_fn<const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    global_invocation_id: UVec3,
    num_workgroups: UVec3,
    matches: &[Match],
    parent_metadatas: &[Metadata],
    ys: &mut [MaybeUninit<Y>],
    metadatas: &mut [MaybeUninit<Metadata>],
) {
    // TODO: Make a single input bounds check and use unsafe to avoid bounds check later
    let global_invocation_id = global_invocation_id.x;
    let num_workgroups = num_workgroups.x;

    let global_size = WORKGROUP_SIZE * num_workgroups;

    // TODO: More idiomatic version currently doesn't compile:
    //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
    for index in (global_invocation_id as usize..matches.len()).step_by(global_size as usize) {
        let m = matches[index];
        // TODO: Correct version currently doesn't compile:
        //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
        // let left_metadata = parent_metadatas[usize::from(m.left_position)];
        // let right_metadata = parent_metadatas[usize::from(m.right_position)];
        let left_metadata = parent_metadatas[m.left_position as usize];
        let right_metadata = parent_metadatas[m.right_position as usize];

        let (y, metadata) = compute_fn_impl::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
            m.left_y,
            left_metadata,
            right_metadata,
        );

        ys[index].write(y);
        // The last table doesn't have any metadata
        if TABLE_NUMBER < 7 {
            metadatas[index].write(metadata);
        }
    }
}

/// Compute Chia's `f2()` function from matches in the parent table and corresponding metadata
#[spirv(compute(threads(256), entry_point_name = "compute_f2"))]
pub fn compute_f2(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] matches: &[Match],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_metadatas: &[Metadata],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ys: &mut [MaybeUninit<Y>],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] metadatas: &mut [MaybeUninit<
        Metadata,
    >],
) {
    compute_fn::<2, 1>(
        global_invocation_id,
        num_workgroups,
        matches,
        parent_metadatas,
        ys,
        metadatas,
    )
}

/// Compute Chia's `f3()` function from matches in the parent table and corresponding metadata
#[spirv(compute(threads(256), entry_point_name = "compute_f3"))]
pub fn compute_f3(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] matches: &[Match],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_metadatas: &[Metadata],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ys: &mut [MaybeUninit<Y>],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] metadatas: &mut [MaybeUninit<
        Metadata,
    >],
) {
    compute_fn::<3, 2>(
        global_invocation_id,
        num_workgroups,
        matches,
        parent_metadatas,
        ys,
        metadatas,
    )
}

/// Compute Chia's `f4()` function from matches in the parent table and corresponding metadata
#[spirv(compute(threads(256), entry_point_name = "compute_f4"))]
pub fn compute_f4(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] matches: &[Match],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_metadatas: &[Metadata],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ys: &mut [MaybeUninit<Y>],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] metadatas: &mut [MaybeUninit<
        Metadata,
    >],
) {
    compute_fn::<4, 3>(
        global_invocation_id,
        num_workgroups,
        matches,
        parent_metadatas,
        ys,
        metadatas,
    )
}

/// Compute Chia's `f5()` function from matches in the parent table and corresponding metadata
#[spirv(compute(threads(256), entry_point_name = "compute_f5"))]
pub fn compute_f5(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] matches: &[Match],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_metadatas: &[Metadata],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ys: &mut [MaybeUninit<Y>],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] metadatas: &mut [MaybeUninit<
        Metadata,
    >],
) {
    compute_fn::<5, 4>(
        global_invocation_id,
        num_workgroups,
        matches,
        parent_metadatas,
        ys,
        metadatas,
    )
}

/// Compute Chia's `f6()` function from matches in the parent table and corresponding metadata
#[spirv(compute(threads(256), entry_point_name = "compute_f6"))]
pub fn compute_f6(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] matches: &[Match],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_metadatas: &[Metadata],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ys: &mut [MaybeUninit<Y>],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] metadatas: &mut [MaybeUninit<
        Metadata,
    >],
) {
    compute_fn::<6, 5>(
        global_invocation_id,
        num_workgroups,
        matches,
        parent_metadatas,
        ys,
        metadatas,
    )
}

/// Compute Chia's `f7()` function from matches in the parent table and corresponding metadata
#[spirv(compute(threads(256), entry_point_name = "compute_f7"))]
pub fn compute_f7(
    #[spirv(global_invocation_id)] global_invocation_id: UVec3,
    #[spirv(num_workgroups)] num_workgroups: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] matches: &[Match],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] parent_metadatas: &[Metadata],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ys: &mut [MaybeUninit<Y>],
    // TODO: This argument should not be required, but it is currently not possible to compile
    //  `&mut []` under `rust-gpu` directly
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] metadatas: &mut [MaybeUninit<
        Metadata,
    >],
) {
    compute_fn::<7, 6>(
        global_invocation_id,
        num_workgroups,
        matches,
        parent_metadatas,
        ys,
        metadatas,
    )
}
