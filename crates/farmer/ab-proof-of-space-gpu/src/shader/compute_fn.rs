// #[cfg(all(test, not(target_arch = "spirv")))]
// mod cpu_tests;
// #[cfg(all(test, not(miri), not(target_arch = "spirv")))]
// mod gpu_tests;

use crate::shader::constants::{K, PARAM_EXT};
use crate::shader::num::{U128, U128T};
use spirv_std::glam::{UVec2, UVec3};
use spirv_std::spirv;

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
    y: u32,
    left_metadata: U128,
    right_metadata: U128,
) -> (u32, U128) {
    let parent_metadata_bits = metadata_size_bits(K, PARENT_TABLE_NUMBER);

    // Only supports `K` from 15 to 25 (otherwise math will not be correct when concatenating y,
    // left metadata and right metadata)
    let mut input_words = [0; _];
    let byte_length = {
        // Take only bytes where bits were set
        let num_bytes_with_data =
            (y_size_bits(K) + metadata_size_bits(K, PARENT_TABLE_NUMBER) * 2).div_ceil(u8::BITS);

        // Collect `K` most significant bits of `y` at the final offset of eventual `input_a`
        let y_bits = U128::from(y) << (u128::BITS - y_size_bits(K));

        // Move bits of `left_metadata` at the final offset of eventual `input_a`
        let left_metadata_bits =
            left_metadata << (u128::BITS - parent_metadata_bits - y_size_bits(K));

        // Part of the `right_bits` at the final offset of eventual `input_a`
        let y_and_left_bits = y_size_bits(K) + parent_metadata_bits;
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
            input_words[..input_a_words.len()].copy_from_slice(&input_a_words);
            let input_b_words = input_b.as_be_bytes_to_le_u32_words();
            input_words[input_a_words.len()..].copy_from_slice(&input_b_words);

            size_of::<u128>() as u32 + right_bits_pushed_into_input_b.div_ceil(u8::BITS)
        } else {
            let right_bits_a = right_metadata << (right_bits_start_offset - y_and_left_bits);
            let input_a = y_bits | left_metadata_bits | right_bits_a;
            let input_a_words = input_a.as_be_bytes_to_le_u32_words();
            input_words[..input_a_words.len()].copy_from_slice(&input_a_words);

            num_bytes_with_data
        }
    };
    let hash = ab_blake3::single_block_hash_portable_words(&input_words, byte_length);

    let y_output = hash[0].to_be() >> (u32::BITS - y_size_bits(K));

    let metadata_size_bits = metadata_size_bits(K, TABLE_NUMBER);

    let metadata = if TABLE_NUMBER < 4 {
        (left_metadata << parent_metadata_bits) | right_metadata
    } else if metadata_size_bits > 0 {
        // For K under 24 it is guaranteed that metadata + bit offset will always fit into 4 `u32`
        // words (equivalent to `u128` size). For K=25 it'll be necessary to have fifth word, which
        // will become more cumbersome to handle.
        // We collect bytes necessary, potentially with extra bits at the start and end of the bytes
        // that will be taken care of later.
        let metadata = U128::from_le_u32_words_as_be_bytes(
            hash[(y_size_bits(K) / u32::BITS) as usize..][..size_of::<u128>() / size_of::<u32>()]
                .try_into()
                .expect("Always enough bits for any K; qed"),
        );
        // Remove extra bits at the beginning
        let metadata = metadata << (y_size_bits(K) % u8::BITS);
        // Move bits into the correct location
        metadata >> (u128::BITS - metadata_size_bits)
    } else {
        0
    };

    (y_output, metadata)
}

// /// Compute Chia's `fn()` function using previously computed ChaCha8 keystream
// #[spirv(compute(threads(256), entry_point_name = "compute_fn"))]
// pub fn compute_fn(
//     #[spirv(global_invocation_id)] invocation_id: UVec3,
//     #[spirv(num_workgroups)] num_workgroups: UVec3,
//     // TODO: Uncomment once https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
//     // #[spirv(workgroup_size)] workgroup_size: UVec3,
//     #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] chacha8_keystream: &[u32],
//     #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] xys: &mut [UVec2],
// ) {
//     // TODO: Make a single input bounds check and use unsafe to avoid bounds check later
//     let invocation_id = invocation_id.x;
//     let num_workgroups = num_workgroups.x;
//
//     // TODO: Same number as hardcoded in `#[spirv(compute(threads(..)))]` above, can be removed once
//     //  https://github.com/Rust-GPU/rust-gpu/discussions/287 is resolved
//     let workgroup_size = 256_u32;
//     let global_size = workgroup_size * num_workgroups;
//
//     // TODO: More idiomatic version currently doesn't compile:
//     //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
//     for x in (invocation_id..xys.len() as u32).step_by(global_size as usize) {
//         xys[x as usize] = UVec2 {
//             x,
//             y: compute_fn_impl(x, chacha8_keystream),
//         };
//     }
// }
