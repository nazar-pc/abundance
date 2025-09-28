// TODO: Replace this constant with usage of `PosProof::K` after
//  https://github.com/Rust-GPU/rust-gpu/pull/249 is merged
pub(super) const K: u8 = 20;
#[cfg(not(target_arch = "spirv"))]
const _: () = {
    assert!(K == ab_core_primitives::pos::PosProof::K);
};
pub(super) const MAX_BUCKET_SIZE: usize = 512;
/// Reducing bucket size for better performance.
///
/// The number should be sufficient to produce enough proofs for sector encoding with high
/// probability.
// TODO: Statistical analysis if possible, confirming there will be enough proofs
pub(super) const REDUCED_BUCKET_SIZE: usize = 272;
/// Reducing matches count for better performance.
///
/// The number should be sufficient to produce enough proofs for sector encoding with high
/// probability.
// TODO: Statistical analysis if possible, confirming there will be enough proofs
pub(super) const REDUCED_MATCHES_COUNT: usize = 288;
/// PRNG extension parameter to avoid collisions
pub(super) const PARAM_EXT: u8 = 6;
pub(super) const PARAM_M: u16 = 1 << PARAM_EXT;
pub(super) const PARAM_B: u16 = 119;
pub(super) const PARAM_C: u16 = 127;
pub(super) const PARAM_BC: u16 = PARAM_B * PARAM_C;
/// Size of the first table and max size for other tables
pub(super) const MAX_TABLE_SIZE: u32 = 1 << K;

/// Compute the size of `y` in bits
pub(super) const fn y_size_bits(k: u8) -> usize {
    k as usize + PARAM_EXT as usize
}

/// Number of buckets for a given `k`
const fn num_buckets(k: u8) -> usize {
    2_usize
        .pow(y_size_bits(k) as u32)
        .div_ceil(PARAM_BC as usize)
}

pub(super) const NUM_BUCKETS: usize = num_buckets(K);
// Buckets are matched with a sliding window of `2`, hence one less bucket exists
pub(super) const NUM_MATCH_BUCKETS: usize = NUM_BUCKETS - 1;
