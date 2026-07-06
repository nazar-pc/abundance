//! S-bucket binning convention for proof targets.
//!
//! The engine is convention-agnostic: f7 maps a table-7 entry's `first_k_bits` to an s-bucket via
//! the supplied [`SBucketMap`], defaulting to the native [`IdentityMap`]. Consumers with a
//! different challenge convention provide their own implementation.

// TODO: Duplicated verbatim in the CPU `ab-proof-of-space` chiapos. Consolidate into one
//  `ab-core-primitives` definition once that crate can be used from the SPIR-V shader; today it
//  is gated there as `cfg(not(target_arch = "spirv"))`.
/// Maps a table-7 entry's `first_k_bits` to its target s-bucket. A returned value `>=
/// NUM_S_BUCKETS` discards the entry (the caller bound-checks against `NUM_S_BUCKETS`).
pub trait SBucketMap {
    /// Map `first_k_bits` to an s-bucket, or a value `>= NUM_S_BUCKETS` to discard.
    fn map(first_k_bits: u32) -> u32;
}

/// Native convention: the s-bucket is the raw `first_k_bits`.
#[derive(Debug)]
pub struct IdentityMap;

impl SBucketMap for IdentityMap {
    #[inline(always)]
    fn map(first_k_bits: u32) -> u32 {
        first_k_bits
    }
}
