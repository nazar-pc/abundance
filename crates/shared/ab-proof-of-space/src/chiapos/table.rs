#[cfg(test)]
mod tests;
pub(super) mod types;

#[cfg(not(feature = "std"))]
extern crate alloc;

use crate::chiapos::Seed;
use crate::chiapos::constants::{PARAM_B, PARAM_BC, PARAM_C, PARAM_EXT, PARAM_M};
use crate::chiapos::table::types::{Metadata, Position, X, Y};
use crate::chiapos::utils::EvaluatableUsize;
use ab_chacha8::{ChaCha8Block, ChaCha8State};
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use chacha20::cipher::{Iv, KeyIvInit, StreamCipher};
use chacha20::{ChaCha8, Key};
use core::hint::assert_unchecked;
use core::mem::MaybeUninit;
use core::simd::Simd;
use core::simd::num::SimdUint;
use core::{array, mem};
#[cfg(all(feature = "std", any(feature = "parallel", test)))]
use parking_lot::Mutex;
#[cfg(any(feature = "parallel", test))]
use rayon::prelude::*;
use seq_macro::seq;
#[cfg(all(not(feature = "std"), any(feature = "parallel", test)))]
use spin::Mutex;

pub(super) const COMPUTE_F1_SIMD_FACTOR: usize = 8;
const FIND_MATCHES_UNROLL_FACTOR: usize = 8;
const COMPUTE_FN_SIMD_FACTOR: usize = 16;

/// Compute the size of `y` in bits
pub(super) const fn y_size_bits(k: u8) -> usize {
    k as usize + PARAM_EXT as usize
}

/// Metadata size in bytes
pub const fn metadata_size_bytes(k: u8, table_number: u8) -> usize {
    metadata_size_bits(k, table_number).div_ceil(u8::BITS as usize)
}

/// Metadata size in bits
pub(super) const fn metadata_size_bits(k: u8, table_number: u8) -> usize {
    k as usize
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

/// ChaCha8 [`Vec`] sufficient for the whole first table for [`K`].
/// Prefer [`partial_y`] if you need partial y just for a single `x`.
fn partial_ys<const K: u8>(seed: Seed) -> Vec<u8> {
    let output_len_bits = usize::from(K) * (1 << K);
    let mut output = vec![0; output_len_bits.div_ceil(u8::BITS as usize)];

    let key = Key::from(seed);
    let iv = Iv::<ChaCha8>::default();

    let mut cipher = ChaCha8::new(&key, &iv);

    cipher.write_keystream(&mut output);

    output
}

/// Mapping from `parity` to `r` to `m`
type LeftTargets = [[Simd<u16, { PARAM_M as usize }>; PARAM_BC as usize]; 2];

fn calculate_left_targets() -> Box<LeftTargets> {
    let mut left_targets = Box::<LeftTargets>::new_uninit();
    // SAFETY: Same layout and uninitialized in both cases
    let left_targets_slice = unsafe {
        mem::transmute::<
            &mut MaybeUninit<[[Simd<u16, { PARAM_M as usize }>; PARAM_BC as usize]; 2]>,
            &mut [[MaybeUninit<Simd<u16, { PARAM_M as usize }>>; PARAM_BC as usize]; 2],
        >(left_targets.as_mut())
    };

    for parity in 0..=1 {
        for r in 0..PARAM_BC {
            let c = r / PARAM_C;

            let mut arr = array::from_fn(|m| {
                let m = m as u16;
                ((c + m) % PARAM_B) * PARAM_C
                    + (((2 * m + parity) * (2 * m + parity) + r) % PARAM_C)
            });
            arr.sort_unstable();
            left_targets_slice[parity as usize][r as usize].write(Simd::from_array(arr));
        }
    }

    // SAFETY: Initialized all entries
    unsafe { left_targets.assume_init() }
}

fn calculate_left_target_on_demand(parity: u32, r: u32, m: u32) -> u32 {
    let param_b = u32::from(PARAM_B);
    let param_c = u32::from(PARAM_C);

    ((r / param_c + m) % param_b) * param_c + (((2 * m + parity) * (2 * m + parity) + r) % param_c)
}

/// Caches that can be used to optimize the creation of multiple [`Tables`](super::Tables).
#[derive(Debug, Clone)]
pub struct TablesCache<const K: u8> {
    matches: Vec<Match>,
    left_targets: Box<LeftTargets>,
}

impl<const K: u8> Default for TablesCache<K> {
    /// Create a new instance
    fn default() -> Self {
        Self {
            matches: Vec::new(),
            left_targets: calculate_left_targets(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct Match {
    left_position: Position,
    left_y: Y,
    right_position: Position,
}

#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
struct Bucket {
    /// Bucket index
    bucket_index: u32,
    /// The start position of this bucket in the table
    start_position: Position,
    /// Size of this bucket
    size: Position,
}

/// Container that stores position and count.
///
/// Position is limited to 25 bits and count is limited to 7 bits. This means it only supports `K`
/// to 25 (just like some other parts of the code).
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub(super) struct RmapItem(u32);

impl RmapItem {
    const COUNT_BITS: u32 = 7;
    const EMPTY: Self = Self(0);

    /// Create a new instance with the count set to zero
    #[inline(always)]
    fn new(start_position: Position) -> Self {
        Self(u32::from(start_position) << Self::COUNT_BITS)
    }

    /// Increment count
    #[inline(always)]
    fn increment(&mut self) {
        self.0 += 1;
    }

    /// Returns start position and count
    #[inline(always)]
    fn split(self) -> (Position, u32) {
        let start_position = self.0 >> Self::COUNT_BITS;
        let count = self.0 & (u32::MAX >> (u32::BITS - Self::COUNT_BITS));
        (Position::from(start_position), count)
    }
}

/// `partial_y_offset` is in bits within `partial_y`
pub(super) fn compute_f1<const K: u8>(x: X, seed: &Seed) -> Y {
    let skip_bits = u32::from(K) * u32::from(x);
    let skip_u32s = skip_bits / u32::BITS;
    let partial_y_offset = skip_bits % u32::BITS;

    const U32S_PER_BLOCK: usize = size_of::<ChaCha8Block>() / size_of::<u32>();

    let initial_state = ChaCha8State::init(seed, &[0; _]);
    let first_block_counter = skip_u32s / U32S_PER_BLOCK as u32;
    let u32_in_first_block = skip_u32s as usize % U32S_PER_BLOCK;

    let first_block = initial_state.compute_block(first_block_counter);
    let hi = first_block[u32_in_first_block].to_be();

    // TODO: Is SIMD version of `compute_block()` that produces two blocks at once possible?
    let lo = if u32_in_first_block + 1 == U32S_PER_BLOCK {
        // Spilled over into the second block
        let second_block = initial_state.compute_block(first_block_counter + 1);
        second_block[0].to_be()
    } else {
        first_block[u32_in_first_block + 1].to_be()
    };

    let partial_y = (u64::from(hi) << u32::BITS) | u64::from(lo);

    let pre_y = partial_y >> (u64::BITS - u32::from(K + PARAM_EXT) - partial_y_offset);
    let pre_y = pre_y as u32;
    // Mask for clearing the rest of bits of `pre_y`.
    let pre_y_mask = (u32::MAX << PARAM_EXT) & (u32::MAX >> (u32::BITS - u32::from(K + PARAM_EXT)));

    // Extract `PARAM_EXT` most significant bits from `x` and store in the final offset of
    // eventual `y` with the rest of bits being zero (`x` is `0..2^K`)
    let pre_ext = u32::from(x) >> (K - PARAM_EXT);

    // Combine all of the bits together:
    // [padding zero bits][`K` bits rom `partial_y`][`PARAM_EXT` bits from `x`]
    Y::from((pre_y & pre_y_mask) | pre_ext)
}

pub(super) fn compute_f1_simd<const K: u8>(
    xs: [u32; COMPUTE_F1_SIMD_FACTOR],
    partial_ys: &[u8; K as usize * COMPUTE_F1_SIMD_FACTOR / u8::BITS as usize],
) -> [Y; COMPUTE_F1_SIMD_FACTOR] {
    // Each element contains `K` desired bits of `partial_ys` in the final offset of eventual `ys`
    // with the rest of bits being in undefined state
    let pre_ys_bytes = array::from_fn(|i| {
        let partial_y_offset = i * usize::from(K);
        let partial_y_length =
            (partial_y_offset % u8::BITS as usize + usize::from(K)).div_ceil(u8::BITS as usize);
        let mut pre_y_bytes = 0u64.to_be_bytes();
        pre_y_bytes[..partial_y_length].copy_from_slice(
            &partial_ys[partial_y_offset / u8::BITS as usize..][..partial_y_length],
        );

        u64::from_be_bytes(pre_y_bytes)
    });
    let pre_ys_right_offset = array::from_fn(|i| {
        let partial_y_offset = i as u32 * u32::from(K);
        u64::from(u64::BITS - u32::from(K + PARAM_EXT) - partial_y_offset % u8::BITS)
    });
    let pre_ys = Simd::from_array(pre_ys_bytes) >> Simd::from_array(pre_ys_right_offset);

    // Mask for clearing the rest of bits of `pre_ys`.
    let pre_ys_mask = Simd::splat(
        (u32::MAX << usize::from(PARAM_EXT))
            & (u32::MAX >> (u32::BITS as usize - usize::from(K + PARAM_EXT))),
    );

    // Extract `PARAM_EXT` most significant bits from `xs` and store in the final offset of
    // eventual `ys` with the rest of bits being in undefined state.
    let pre_exts = Simd::from_array(xs) >> Simd::splat(u32::from(K - PARAM_EXT));

    // Combine all of the bits together:
    // [padding zero bits][`K` bits rom `partial_y`][`PARAM_EXT` bits from `x`]
    let ys = (pre_ys.cast() & pre_ys_mask) | pre_exts;

    Y::array_from_repr(ys.to_array())
}

/// For verification purposes use [`has_match`] instead.
///
/// Returns `None` if either of buckets is empty.
fn find_matches(
    left_bucket_ys: &[Y],
    left_bucket_start_position: Position,
    right_bucket_ys: &[Y],
    right_bucket_start_position: Position,
    matches: &mut Vec<Match>,
    left_targets: &LeftTargets,
) {
    let mut rmap = [RmapItem::default(); PARAM_BC as usize];

    let Some(&first_right_bucket_y) = right_bucket_ys.first() else {
        return;
    };
    // Since all entries in a bucket are obtained after division by `PARAM_BC`, we can compute
    // quotient more efficiently by subtracting base value rather than computing the remainder of
    // the division
    let right_base =
        (usize::from(first_right_bucket_y) / usize::from(PARAM_BC)) * usize::from(PARAM_BC);
    for (&y, right_position) in right_bucket_ys.iter().zip(right_bucket_start_position..) {
        let r = usize::from(y) - right_base;
        // SAFETY: `r` is within a bucket and exists by definition
        let rmap_item = unsafe { rmap.get_unchecked_mut(r) };

        // The same `y` and as a result `r` can appear in the table multiple times, in which case
        // they'll all occupy consecutive slots in `right_bucket` and all we need to store is just
        // the first position and number of elements.
        if *rmap_item == RmapItem::EMPTY {
            *rmap_item = RmapItem::new(right_position);
        }
        rmap_item.increment();
    }

    // Same idea as above, but avoids division by leveraging the fact that each bucket is exactly
    // `PARAM_BC` away from the previous one in terms of divisor by `PARAM_BC`
    let left_base = right_base - usize::from(PARAM_BC);
    let parity = left_base % 2;
    let left_targets_parity = &left_targets[parity];

    for (&y, left_position) in left_bucket_ys.iter().zip(left_bucket_start_position..) {
        let r = usize::from(y) - left_base;
        // SAFETY: `r` is within a bucket and exists by definition
        let left_targets_r = unsafe { left_targets_parity.get_unchecked(r) }.as_array();

        const _: () = {
            assert!((PARAM_M as usize).is_multiple_of(FIND_MATCHES_UNROLL_FACTOR));
        };

        for r_targets in left_targets_r
            .as_chunks::<FIND_MATCHES_UNROLL_FACTOR>()
            .0
            .iter()
        {
            let rmap_items: [_; FIND_MATCHES_UNROLL_FACTOR] = seq!(N in 0..8 {
                [
                #(
                    // SAFETY: target is within a bucket and exists by definition
                    *unsafe { rmap.get_unchecked(usize::from(r_targets[N])) },
                )*
                ]
            });

            if rmap_items == [RmapItem::EMPTY; _] {
                // Common case for the whole batch of items to have zero counts
                continue;
            }

            let _: [(); FIND_MATCHES_UNROLL_FACTOR] = seq!(N in 0..8 {
                [
                #(
                {
                    let (start_position, count) = rmap_items[N].split();

                    for right_position in start_position..start_position + Position::from(count) {
                        matches.push(Match {
                            left_position,
                            left_y: y,
                            right_position,
                        });
                    }
                },
                )*
                ]
            });
        }
    }
}

/// Simplified version of [`find_matches`] for verification purposes.
pub(super) fn has_match(left_y: Y, right_y: Y) -> bool {
    let right_r = u32::from(right_y) % u32::from(PARAM_BC);
    let parity = (u32::from(left_y) / u32::from(PARAM_BC)) % 2;
    let left_r = u32::from(left_y) % u32::from(PARAM_BC);

    let r_targets = array::from_fn::<_, { PARAM_M as usize }, _>(|i| {
        calculate_left_target_on_demand(parity, left_r, i as u32)
    });

    r_targets.contains(&right_r)
}

#[inline(always)]
pub(super) fn compute_fn<const K: u8, const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    y: Y,
    left_metadata: Metadata<K, PARENT_TABLE_NUMBER>,
    right_metadata: Metadata<K, PARENT_TABLE_NUMBER>,
) -> (Y, Metadata<K, TABLE_NUMBER>)
where
    EvaluatableUsize<{ metadata_size_bytes(K, PARENT_TABLE_NUMBER) }>: Sized,
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    let left_metadata = u128::from(left_metadata);
    let right_metadata = u128::from(right_metadata);

    let parent_metadata_bits = metadata_size_bits(K, PARENT_TABLE_NUMBER);

    // Part of the `right_bits` at the final offset of eventual `input_a`
    let y_and_left_bits = y_size_bits(K) + parent_metadata_bits;
    let right_bits_start_offset = u128::BITS as usize - parent_metadata_bits;

    // Take only bytes where bits were set
    let num_bytes_with_data =
        (y_size_bits(K) + parent_metadata_bits * 2).div_ceil(u8::BITS as usize);

    // Only supports `K` from 15 to 25 (otherwise math will not be correct when concatenating y,
    // left metadata and right metadata)
    let hash = {
        // Collect `K` most significant bits of `y` at the final offset of eventual `input_a`
        let y_bits = u128::from(y) << (u128::BITS as usize - y_size_bits(K));

        // Move bits of `left_metadata` at the final offset of eventual `input_a`
        let left_metadata_bits =
            left_metadata << (u128::BITS as usize - parent_metadata_bits - y_size_bits(K));

        // If `right_metadata` bits start to the left of the desired position in `input_a` move
        // bits right, else move left
        if right_bits_start_offset < y_and_left_bits {
            let right_bits_pushed_into_input_b = y_and_left_bits - right_bits_start_offset;
            // Collect bits of `right_metadata` that will fit into `input_a` at the final offset in
            // eventual `input_a`
            let right_bits_a = right_metadata >> right_bits_pushed_into_input_b;
            let input_a = y_bits | left_metadata_bits | right_bits_a;
            // Collect bits of `right_metadata` that will spill over into `input_b`
            let input_b = right_metadata << (u128::BITS as usize - right_bits_pushed_into_input_b);

            let input = [input_a.to_be_bytes(), input_b.to_be_bytes()];
            let input_len =
                size_of::<u128>() + right_bits_pushed_into_input_b.div_ceil(u8::BITS as usize);
            ab_blake3::single_block_hash(&input.as_flattened()[..input_len])
                .expect("Exactly a single block worth of bytes; qed")
        } else {
            let right_bits_a = right_metadata << (right_bits_start_offset - y_and_left_bits);
            let input_a = y_bits | left_metadata_bits | right_bits_a;

            ab_blake3::single_block_hash(&input_a.to_be_bytes()[..num_bytes_with_data])
                .expect("Less than a single block worth of bytes; qed")
        }
    };

    let y_output = Y::from(
        u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]])
            >> (u32::BITS as usize - y_size_bits(K)),
    );

    let metadata_size_bits = metadata_size_bits(K, TABLE_NUMBER);

    let metadata = if TABLE_NUMBER < 4 {
        (left_metadata << parent_metadata_bits) | right_metadata
    } else if metadata_size_bits > 0 {
        // For K up to 25 it is guaranteed that metadata + bit offset will always fit into u128.
        // We collect the bytes necessary, potentially with extra bits at the start and end of the
        // bytes that will be taken care of later.
        let metadata = u128::from_be_bytes(
            hash[y_size_bits(K) / u8::BITS as usize..][..size_of::<u128>()]
                .try_into()
                .expect("Always enough bits for any K; qed"),
        );
        // Remove extra bits at the beginning
        let metadata = metadata << (y_size_bits(K) % u8::BITS as usize);
        // Move bits into the correct location
        metadata >> (u128::BITS as usize - metadata_size_bits)
    } else {
        0
    };

    (y_output, Metadata::from(metadata))
}

// TODO: This is actually using only pipelining rather than real SIMD (at least explicitly) due to:
//  * https://github.com/rust-lang/portable-simd/issues/108
//  * https://github.com/BLAKE3-team/BLAKE3/issues/478#issuecomment-3200106103
fn compute_fn_simd<const K: u8, const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    left_ys: [Y; COMPUTE_FN_SIMD_FACTOR],
    left_metadatas: [Metadata<K, PARENT_TABLE_NUMBER>; COMPUTE_FN_SIMD_FACTOR],
    right_metadatas: [Metadata<K, PARENT_TABLE_NUMBER>; COMPUTE_FN_SIMD_FACTOR],
) -> (
    [Y; COMPUTE_FN_SIMD_FACTOR],
    [Metadata<K, TABLE_NUMBER>; COMPUTE_FN_SIMD_FACTOR],
)
where
    EvaluatableUsize<{ metadata_size_bytes(K, PARENT_TABLE_NUMBER) }>: Sized,
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    let parent_metadata_bits = metadata_size_bits(K, PARENT_TABLE_NUMBER);
    let metadata_size_bits = metadata_size_bits(K, TABLE_NUMBER);

    // TODO: `u128` is not supported as SIMD element yet, see
    //  https://github.com/rust-lang/portable-simd/issues/108
    let left_metadatas: [u128; COMPUTE_FN_SIMD_FACTOR] = seq!(N in 0..16 {
        [
        #(
            u128::from(left_metadatas[N]),
        )*
        ]
    });
    let right_metadatas: [u128; COMPUTE_FN_SIMD_FACTOR] = seq!(N in 0..16 {
        [
        #(
            u128::from(right_metadatas[N]),
        )*
        ]
    });

    // Part of the `right_bits` at the final offset of eventual `input_a`
    let y_and_left_bits = y_size_bits(K) + parent_metadata_bits;
    let right_bits_start_offset = u128::BITS as usize - parent_metadata_bits;

    // Take only bytes where bits were set
    let num_bytes_with_data =
        (y_size_bits(K) + parent_metadata_bits * 2).div_ceil(u8::BITS as usize);

    // Only supports `K` from 15 to 25 (otherwise math will not be correct when concatenating y,
    // left metadata and right metadata)
    // TODO: SIMD hashing once this is possible:
    //  https://github.com/BLAKE3-team/BLAKE3/issues/478#issuecomment-3200106103
    let hashes: [_; COMPUTE_FN_SIMD_FACTOR] = seq!(N in 0..16 {
        [
        #(
        {
            let y = left_ys[N];
            let left_metadata = left_metadatas[N];
            let right_metadata = right_metadatas[N];

            // Collect `K` most significant bits of `y` at the final offset of eventual
            // `input_a`
            let y_bits = u128::from(y) << (u128::BITS as usize - y_size_bits(K));

            // Move bits of `left_metadata` at the final offset of eventual `input_a`
            let left_metadata_bits =
                left_metadata << (u128::BITS as usize - parent_metadata_bits - y_size_bits(K));

            // If `right_metadata` bits start to the left of the desired position in `input_a` move
            // bits right, else move left
            if right_bits_start_offset < y_and_left_bits {
                let right_bits_pushed_into_input_b = y_and_left_bits - right_bits_start_offset;
                // Collect bits of `right_metadata` that will fit into `input_a` at the final offset
                // in eventual `input_a`
                let right_bits_a = right_metadata >> right_bits_pushed_into_input_b;
                let input_a = y_bits | left_metadata_bits | right_bits_a;
                // Collect bits of `right_metadata` that will spill over into `input_b`
                let input_b = right_metadata << (u128::BITS as usize - right_bits_pushed_into_input_b);

                let input = [input_a.to_be_bytes(), input_b.to_be_bytes()];
                let input_len =
                    size_of::<u128>() + right_bits_pushed_into_input_b.div_ceil(u8::BITS as usize);
                ab_blake3::single_block_hash(&input.as_flattened()[..input_len])
                    .expect("Exactly a single block worth of bytes; qed")
            } else {
                let right_bits_a = right_metadata << (right_bits_start_offset - y_and_left_bits);
                let input_a = y_bits | left_metadata_bits | right_bits_a;

                ab_blake3::single_block_hash(&input_a.to_be_bytes()[..num_bytes_with_data])
                    .expect("Exactly a single block worth of bytes; qed")
            }
        },
        )*
        ]
    });

    let y_outputs = Simd::from_array(
        hashes.map(|hash| u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]])),
    ) >> (u32::BITS - y_size_bits(K) as u32);
    let y_outputs = Y::array_from_repr(y_outputs.to_array());

    let metadatas = if TABLE_NUMBER < 4 {
        seq!(N in 0..16 {
            [
            #(
                Metadata::from((left_metadatas[N] << parent_metadata_bits) | right_metadatas[N]),
            )*
            ]
        })
    } else if metadata_size_bits > 0 {
        // For K up to 25 it is guaranteed that metadata + bit offset will always fit into u128.
        // We collect the bytes necessary, potentially with extra bits at the start and end of the
        // bytes that will be taken care of later.
        seq!(N in 0..16 {
            [
            #(
            {
                let metadata = u128::from_be_bytes(
                    hashes[N][y_size_bits(K) / u8::BITS as usize..][..size_of::<u128>()]
                        .try_into()
                        .expect("Always enough bits for any K; qed"),
                );
                // Remove extra bits at the beginning
                let metadata = metadata << (y_size_bits(K) % u8::BITS as usize);
                // Move bits into the correct location
                Metadata::from(metadata >> (u128::BITS as usize - metadata_size_bits))
            },
            )*
            ]
        })
    } else {
        [Metadata::default(); _]
    };

    (y_outputs, metadatas)
}

fn match_to_result<const K: u8, const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    last_table: &Table<K, PARENT_TABLE_NUMBER>,
    m: &Match,
) -> (Y, [Position; 2], Metadata<K, TABLE_NUMBER>)
where
    EvaluatableUsize<{ metadata_size_bytes(K, PARENT_TABLE_NUMBER) }>: Sized,
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    let left_metadata = last_table
        .metadata(m.left_position)
        .expect("Position resulted from matching is correct; qed");
    let right_metadata = last_table
        .metadata(m.right_position)
        .expect("Position resulted from matching is correct; qed");

    let (y, metadata) =
        compute_fn::<K, TABLE_NUMBER, PARENT_TABLE_NUMBER>(m.left_y, left_metadata, right_metadata);

    (y, [m.left_position, m.right_position], metadata)
}

fn match_to_result_simd<const K: u8, const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    last_table: &Table<K, PARENT_TABLE_NUMBER>,
    matches: &[Match; COMPUTE_FN_SIMD_FACTOR],
) -> [(Y, [Position; 2], Metadata<K, TABLE_NUMBER>); COMPUTE_FN_SIMD_FACTOR]
where
    EvaluatableUsize<{ metadata_size_bytes(K, PARENT_TABLE_NUMBER) }>: Sized,
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    let left_ys: [_; COMPUTE_FN_SIMD_FACTOR] = seq!(N in 0..16 {
        [
        #(
            matches[N].left_y,
        )*
        ]
    });
    let left_metadatas: [_; COMPUTE_FN_SIMD_FACTOR] = seq!(N in 0..16 {
        [
        #(
            last_table
                .metadata(matches[N].left_position)
                .expect("Position resulted from matching is correct; qed"),
        )*
        ]
    });
    let right_metadatas: [_; COMPUTE_FN_SIMD_FACTOR] = seq!(N in 0..16 {
        [
        #(
            last_table
                .metadata(matches[N].right_position)
                .expect("Position resulted from matching is correct; qed"),
        )*
        ]
    });

    let (y_outputs, metadatas) = compute_fn_simd::<K, TABLE_NUMBER, PARENT_TABLE_NUMBER>(
        left_ys,
        left_metadatas,
        right_metadatas,
    );

    seq!(N in 0..16 {
        [
        #(
            (
                y_outputs[N],
                [
                    matches[N].left_position,
                    matches[N].right_position,
                ],
                metadatas[N]
            ),
        )*
        ]
    })
}

fn matches_to_results<const K: u8, const TABLE_NUMBER: u8, const PARENT_TABLE_NUMBER: u8>(
    last_table: &Table<K, PARENT_TABLE_NUMBER>,
    matches: &[Match],
    entries: &mut Vec<(Y, [Position; 2], Metadata<K, TABLE_NUMBER>)>,
) where
    EvaluatableUsize<{ metadata_size_bytes(K, PARENT_TABLE_NUMBER) }>: Sized,
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    let (grouped_matches, other_matches) = matches.as_chunks::<COMPUTE_FN_SIMD_FACTOR>();
    for &grouped_matches in grouped_matches {
        entries.extend(match_to_result_simd(last_table, &grouped_matches));
    }
    for m in other_matches {
        entries.push(match_to_result(last_table, m));
    }
}

#[derive(Debug)]
pub(super) enum Table<const K: u8, const TABLE_NUMBER: u8>
where
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    /// First table with the contents of entries split into separate vectors for more efficient
    /// access
    First {
        /// Derived values computed from `x`
        ys: Vec<Y>,
        /// X values
        xs: Vec<X>,
    },
    /// Other tables
    Other {
        /// Derived values computed from the previous table
        ys: Vec<Y>,
        /// Left and right entry positions in a previous table encoded into bits
        positions: Vec<[Position; 2]>,
        /// Metadata corresponding to each entry
        metadatas: Vec<Metadata<K, TABLE_NUMBER>>,
    },
}

impl<const K: u8> Table<K, 1>
where
    EvaluatableUsize<{ metadata_size_bytes(K, 1) }>: Sized,
{
    /// Create the table
    pub(super) fn create(seed: Seed) -> Self
    where
        EvaluatableUsize<{ K as usize * COMPUTE_F1_SIMD_FACTOR / u8::BITS as usize }>: Sized,
    {
        let partial_ys = partial_ys::<K>(seed);

        let mut t_1 = Vec::with_capacity(1_usize << K);
        for (x_batch, partial_ys) in partial_ys
            .as_chunks::<{ K as usize * COMPUTE_F1_SIMD_FACTOR / u8::BITS as usize }>()
            .0
            .iter()
            .copied()
            .enumerate()
        {
            let xs = array::from_fn::<_, COMPUTE_F1_SIMD_FACTOR, _>(|i| {
                (x_batch * COMPUTE_F1_SIMD_FACTOR + i) as u32
            });
            let ys = compute_f1_simd::<K>(xs, &partial_ys);
            t_1.extend(ys.into_iter().zip(X::array_from_repr(xs)));
        }

        t_1.sort_by_key(|(y, _x)| *y);

        let (ys, xs) = t_1.into_iter().unzip();

        Self::First { ys, xs }
    }

    /// Create the table, leverages available parallelism
    #[cfg(any(feature = "parallel", test))]
    pub(super) fn create_parallel(seed: Seed) -> Self
    where
        EvaluatableUsize<{ K as usize * COMPUTE_F1_SIMD_FACTOR / u8::BITS as usize }>: Sized,
    {
        let partial_ys = partial_ys::<K>(seed);

        let mut t_1 = Vec::with_capacity(1_usize << K);
        for (x_batch, partial_ys) in partial_ys
            .as_chunks::<{ K as usize * COMPUTE_F1_SIMD_FACTOR / u8::BITS as usize }>()
            .0
            .iter()
            .copied()
            .enumerate()
        {
            let xs = array::from_fn::<_, COMPUTE_F1_SIMD_FACTOR, _>(|i| {
                (x_batch * COMPUTE_F1_SIMD_FACTOR + i) as u32
            });
            let ys = compute_f1_simd::<K>(xs, &partial_ys);
            t_1.extend(ys.into_iter().zip(X::array_from_repr(xs)));
        }

        t_1.par_sort_unstable();

        let (ys, xs) = t_1.into_iter().unzip();

        Self::First { ys, xs }
    }

    /// All `x`s
    #[inline(always)]
    pub(super) fn xs(&self) -> &[X] {
        match self {
            Table::First { xs, .. } => xs,
            _ => {
                unreachable!()
            }
        }
    }
}

mod private {
    pub(in super::super) trait SupportedOtherTables {}
}

impl<const K: u8> private::SupportedOtherTables for Table<K, 2> where
    EvaluatableUsize<{ metadata_size_bytes(K, 2) }>: Sized
{
}

impl<const K: u8> private::SupportedOtherTables for Table<K, 3> where
    EvaluatableUsize<{ metadata_size_bytes(K, 3) }>: Sized
{
}

impl<const K: u8> private::SupportedOtherTables for Table<K, 4> where
    EvaluatableUsize<{ metadata_size_bytes(K, 4) }>: Sized
{
}

impl<const K: u8> private::SupportedOtherTables for Table<K, 5> where
    EvaluatableUsize<{ metadata_size_bytes(K, 5) }>: Sized
{
}

impl<const K: u8> private::SupportedOtherTables for Table<K, 6> where
    EvaluatableUsize<{ metadata_size_bytes(K, 6) }>: Sized
{
}

impl<const K: u8> private::SupportedOtherTables for Table<K, 7> where
    EvaluatableUsize<{ metadata_size_bytes(K, 7) }>: Sized
{
}

impl<const K: u8, const TABLE_NUMBER: u8> Table<K, TABLE_NUMBER>
where
    Self: private::SupportedOtherTables,
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    /// Creates a new [`TABLE_NUMBER`] table. There also exists [`Self::create_parallel()`] that
    /// trades CPU efficiency and memory usage for lower latency and with multiple parallel calls,
    /// better overall performance.
    pub(super) fn create<const PARENT_TABLE_NUMBER: u8>(
        last_table: &Table<K, PARENT_TABLE_NUMBER>,
        cache: &mut TablesCache<K>,
    ) -> Self
    where
        EvaluatableUsize<{ metadata_size_bytes(K, PARENT_TABLE_NUMBER) }>: Sized,
    {
        let matches = &mut cache.matches;
        let left_targets = &*cache.left_targets;

        let mut left_bucket = Bucket {
            bucket_index: 0,
            start_position: Position::ZERO,
            size: Position::ZERO,
        };
        let mut right_bucket = Bucket {
            bucket_index: 0,
            start_position: Position::ZERO,
            size: Position::ZERO,
        };
        for (&y, position) in last_table.ys().iter().zip(Position::ZERO..) {
            let bucket_index = u32::from(y) / u32::from(PARAM_BC);

            if bucket_index == right_bucket.bucket_index {
                right_bucket.size += Position::ONE;
                continue;
            }

            if right_bucket.size > Position::ZERO {
                if left_bucket.size > Position::ZERO {
                    find_matches(
                        &last_table.ys()[usize::from(left_bucket.start_position)..]
                            [..usize::from(left_bucket.size)],
                        left_bucket.start_position,
                        &last_table.ys()[usize::from(right_bucket.start_position)..]
                            [..usize::from(right_bucket.size)],
                        right_bucket.start_position,
                        matches,
                        left_targets,
                    );
                }

                left_bucket = right_bucket;
            }

            right_bucket = Bucket {
                bucket_index,
                start_position: position,
                size: Position::ONE,
            };
        }

        // Iteration stopped, but we did not process the last pair of buckets yet
        if left_bucket.size > Position::ZERO && right_bucket.size > Position::ZERO {
            find_matches(
                &last_table.ys()[usize::from(left_bucket.start_position)..]
                    [..usize::from(left_bucket.size)],
                left_bucket.start_position,
                &last_table.ys()[usize::from(right_bucket.start_position)..]
                    [..usize::from(right_bucket.size)],
                right_bucket.start_position,
                matches,
                left_targets,
            );
        }

        let num_values = 1 << K;
        let mut t_n = Vec::with_capacity(num_values);
        matches_to_results(last_table, matches, &mut t_n);
        matches.clear();
        t_n.sort_by_key(|(y, _positions, _metadata)| *y);

        let mut ys = Vec::with_capacity(num_values);
        let mut positions = Vec::with_capacity(num_values);
        // The last table doesn't have metadata
        let mut metadatas = Vec::with_capacity(if metadata_size_bits(K, TABLE_NUMBER) > 0 {
            num_values
        } else {
            0
        });

        for (y, [left_position, right_position], metadata) in t_n {
            ys.push(y);
            positions.push([left_position, right_position]);
            // The last table doesn't have metadata
            if metadata_size_bits(K, TABLE_NUMBER) > 0 {
                metadatas.push(metadata);
            }
        }

        Self::Other {
            ys,
            positions,
            metadatas,
        }
    }

    /// Almost the same as [`Self::create()`], but uses parallelism internally for better
    /// performance (though not efficiency of CPU and memory usage), if you create multiple tables
    /// in parallel, prefer [`Self::create_parallel()`] for better overall performance.
    #[cfg(any(feature = "parallel", test))]
    pub(super) fn create_parallel<const PARENT_TABLE_NUMBER: u8>(
        last_table: &Table<K, PARENT_TABLE_NUMBER>,
        cache: &mut TablesCache<K>,
    ) -> Self
    where
        EvaluatableUsize<{ metadata_size_bytes(K, PARENT_TABLE_NUMBER) }>: Sized,
    {
        let left_targets = &*cache.left_targets;

        let mut first_bucket = Bucket {
            bucket_index: u32::from(last_table.ys()[0]) / u32::from(PARAM_BC),
            start_position: Position::ZERO,
            size: Position::ZERO,
        };
        for &y in last_table.ys() {
            let bucket_index = u32::from(y) / u32::from(PARAM_BC);

            if bucket_index == first_bucket.bucket_index {
                first_bucket.size += Position::ONE;
            } else {
                break;
            }
        }

        let previous_bucket = Mutex::new(first_bucket);

        let entries = rayon::broadcast(|_ctx| {
            let mut entries = Vec::new();
            let mut matches = Vec::new();

            loop {
                let left_bucket;
                let right_bucket;
                {
                    let mut previous_bucket = previous_bucket.lock();

                    let right_bucket_start_position =
                        previous_bucket.start_position + previous_bucket.size;
                    let right_bucket_index = match last_table
                        .ys()
                        .get(usize::from(right_bucket_start_position))
                    {
                        Some(&y) => u32::from(y) / u32::from(PARAM_BC),
                        None => {
                            break;
                        }
                    };
                    let mut right_bucket_size = Position::ZERO;

                    for &y in &last_table.ys()[usize::from(right_bucket_start_position)..] {
                        let bucket_index = u32::from(y) / u32::from(PARAM_BC);

                        if bucket_index == right_bucket_index {
                            right_bucket_size += Position::ONE;
                        } else {
                            break;
                        }
                    }

                    right_bucket = Bucket {
                        bucket_index: right_bucket_index,
                        start_position: right_bucket_start_position,
                        size: right_bucket_size,
                    };

                    left_bucket = *previous_bucket;
                    *previous_bucket = right_bucket;
                }

                find_matches(
                    &last_table.ys()[usize::from(left_bucket.start_position)..]
                        [..usize::from(left_bucket.size)],
                    left_bucket.start_position,
                    &last_table.ys()[usize::from(right_bucket.start_position)..]
                        [..usize::from(right_bucket.size)],
                    right_bucket.start_position,
                    &mut matches,
                    left_targets,
                );

                matches_to_results(last_table, &matches, &mut entries);
                matches.clear();
            }

            entries
        });

        let num_values = 1 << K;
        let mut t_n = Vec::with_capacity(num_values);
        entries.into_iter().flatten().collect_into(&mut t_n);
        t_n.par_sort_unstable();

        let mut ys = Vec::with_capacity(num_values);
        let mut positions = Vec::with_capacity(num_values);
        // The last table doesn't have metadata
        let mut metadatas = Vec::with_capacity(if metadata_size_bits(K, TABLE_NUMBER) > 0 {
            num_values
        } else {
            0
        });

        for (y, [left_position, right_position], metadata) in t_n.drain(..) {
            ys.push(y);
            positions.push([left_position, right_position]);
            // The last table doesn't have metadata
            if metadata_size_bits(K, TABLE_NUMBER) > 0 {
                metadatas.push(metadata);
            }
        }

        // Drop from a background thread, which typically helps with overall concurrency
        rayon::spawn(move || {
            drop(t_n);
        });

        Self::Other {
            ys,
            positions,
            metadatas,
        }
    }
}

impl<const K: u8, const TABLE_NUMBER: u8> Table<K, TABLE_NUMBER>
where
    EvaluatableUsize<{ metadata_size_bytes(K, TABLE_NUMBER) }>: Sized,
{
    /// All `y`s
    #[inline(always)]
    pub(super) fn ys(&self) -> &[Y] {
        let (Table::First { ys, .. } | Table::Other { ys, .. }) = self;
        ys
    }

    /// Returns `None` for an invalid position or first table, `Some(left_position, right_position)`
    /// in the previous table on success
    #[inline(always)]
    pub(super) fn position(&self, position: Position) -> Option<[Position; 2]> {
        match self {
            Table::First { .. } => None,
            Table::Other { positions, .. } => positions.get(usize::from(position)).copied(),
        }
    }

    /// Returns `None` for an invalid position or for table number 7
    #[inline(always)]
    fn metadata(&self, position: Position) -> Option<Metadata<K, TABLE_NUMBER>> {
        match self {
            Table::First { xs, .. } => xs.get(usize::from(position)).map(|&x| Metadata::from(x)),
            Table::Other { metadatas, .. } => metadatas.get(usize::from(position)).copied(),
        }
    }
}
