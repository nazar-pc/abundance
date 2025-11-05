use crate::shader::compute_fn::cpu_tests::correct_compute_fn;
use crate::shader::constants::{
    MAX_BUCKET_SIZE, NUM_BUCKETS, NUM_MATCH_BUCKETS, NUM_S_BUCKETS, PARAM_BC, REDUCED_MATCHES_COUNT,
};
use crate::shader::find_matches_and_compute_f7::{
    NUM_ELEMENTS_PER_S_BUCKET, PARENT_TABLE_NUMBER, TABLE_NUMBER,
};
use crate::shader::find_matches_in_buckets::cpu_tests::find_matches_in_buckets_correct;
use crate::shader::types::{Metadata, Position, PositionExt, PositionR, Y};
use std::mem::MaybeUninit;

pub(super) fn find_matches_and_compute_f7_correct<'a>(
    parent_buckets: &[[PositionR; MAX_BUCKET_SIZE]; NUM_BUCKETS],
    parent_metadatas: &[[Metadata; REDUCED_MATCHES_COUNT]; NUM_MATCH_BUCKETS],
    table_6_proof_targets: &mut [[MaybeUninit<[Position; 2]>; NUM_ELEMENTS_PER_S_BUCKET];
             NUM_S_BUCKETS],
) -> &'a [[[Position; 2]; NUM_ELEMENTS_PER_S_BUCKET]; NUM_S_BUCKETS] {
    let parent_metadatas = parent_metadatas.as_flattened();
    let mut matches = [MaybeUninit::uninit(); _];

    let mut bucket_offsets = [0_u16; NUM_S_BUCKETS];
    for (left_bucket_index, [left_bucket, right_bucket]) in
        parent_buckets.array_windows().enumerate()
    {
        let left_bucket_base = left_bucket_index as u32 * u32::from(PARAM_BC);
        let matches = find_matches_in_buckets_correct(
            left_bucket_index as u32,
            left_bucket,
            right_bucket,
            &mut matches,
        );

        for m in matches {
            // TODO: Correct version currently doesn't compile:
            //  https://github.com/Rust-GPU/rust-gpu/issues/241#issuecomment-3005693043
            // let left_metadata = parent_metadatas[usize::from(m.left_position())];
            // let right_metadata = parent_metadatas[usize::from(m.right_position())];
            let left_metadata = parent_metadatas[m.left_position() as usize];
            let right_metadata = parent_metadatas[m.right_position() as usize];
            let (y, _) = correct_compute_fn::<TABLE_NUMBER, PARENT_TABLE_NUMBER>(
                Y::from(left_bucket_base + m.left_r()),
                left_metadata,
                right_metadata,
            );

            let s_bucket = y.first_k_bits() as usize;

            let Some(bucket_offset) = bucket_offsets.get_mut(s_bucket) else {
                continue;
            };
            // SAFETY: `s_bucket` is checked above to be correct. Bucket size upper bound is known
            // statically to be [`NUM_ELEMENTS_PER_S_BUCKET`], so `bucket_offset` is also always
            // within bounds.
            let bucket = unsafe { table_6_proof_targets.get_unchecked_mut(s_bucket) };

            bucket[*bucket_offset as usize].write([m.left_position(), m.right_position()]);
            *bucket_offset += 1;
        }
    }

    for (bucket, initialized) in table_6_proof_targets.iter_mut().zip(bucket_offsets) {
        bucket[usize::from(initialized)..].write_filled([Position::SENTINEL; 2]);
    }

    // SAFETY: All entries are initialized
    unsafe { &*table_6_proof_targets.as_ptr().cast() }
}
