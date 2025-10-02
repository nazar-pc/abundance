//! Chia proof of space reimplementation in Rust

mod constants;
mod table;
mod tables;
mod utils;

#[cfg(feature = "alloc")]
use crate::PosProofs;
#[cfg(feature = "alloc")]
pub use crate::chiapos::table::TablesCache;
use crate::chiapos::table::{metadata_size_bytes, num_buckets};
use crate::chiapos::tables::TablesGeneric;
use crate::chiapos::utils::EvaluatableUsize;
#[cfg(feature = "alloc")]
use ab_core_primitives::pieces::Record;
#[cfg(feature = "alloc")]
use ab_core_primitives::pos::PosProof;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use core::mem::offset_of;

/// Proof-of-space proofs
#[derive(Debug)]
#[cfg(feature = "alloc")]
#[repr(C)]
pub struct Proofs<const K: u8>
where
    [(); 64 * usize::from(K) / 8]:,
{
    /// S-buckets at which proofs were found.
    ///
    /// S-buckets are grouped by 8, within each `u8` bits right to left (LSB) indicate the presence
    /// of a proof for corresponding s-bucket, so that the whole array of bytes can be thought as a
    /// large set of bits.
    ///
    /// There will be at most [`Record::NUM_CHUNKS`] proofs produced/bits set to `1`.
    pub found_proofs: [u8; Record::NUM_S_BUCKETS / u8::BITS as usize],
    /// [`Record::NUM_CHUNKS`] proofs, corresponding to set bits of `found_proofs`.
    pub proofs: [[u8; 64 * usize::from(K) / 8]; Record::NUM_CHUNKS],
}

#[cfg(feature = "alloc")]
impl From<Box<Proofs<{ PosProof::K }>>> for Box<PosProofs> {
    fn from(proofs: Box<Proofs<{ PosProof::K }>>) -> Self {
        // Statically ensure types are the same
        const {
            assert!(size_of::<Proofs<{ PosProof::K }>>() == size_of::<PosProofs>());
            assert!(align_of::<Proofs<{ PosProof::K }>>() == align_of::<PosProofs>());
            assert!(
                offset_of!(Proofs<{ PosProof::K }>, found_proofs)
                    == offset_of!(PosProofs, found_proofs)
            );
            assert!(offset_of!(Proofs<{ PosProof::K }>, proofs) == offset_of!(PosProofs, proofs));
        }
        // SAFETY: Both structs have an identical layout with `#[repr(C)]` internals
        unsafe { Box::from_raw(Box::into_raw(proofs).cast()) }
    }
}

type Seed = [u8; 32];
#[cfg(any(feature = "full-chiapos", test))]
type Challenge = [u8; 32];
#[cfg(any(feature = "full-chiapos", test))]
type Quality = [u8; 32];

/// Collection of Chia tables
#[derive(Debug)]
pub struct Tables<const K: u8>(TablesGeneric<K>)
where
    EvaluatableUsize<{ metadata_size_bytes(K, 7) }>: Sized,
    [(); 1 << K]:,
    [(); num_buckets(K)]:,
    [(); num_buckets(K) - 1]:;

macro_rules! impl_any {
    ($($k: expr$(,)? )*) => {
        $(
impl Tables<$k> {
    /// Create Chia proof of space tables.
    ///
    /// There is also `Self::create_parallel()` that can achieve higher performance and lower
    /// latency at the cost of lower CPU efficiency and higher memory usage.
    #[cfg(feature = "alloc")]
    pub fn create(seed: Seed, cache: &TablesCache) -> Self {
        Self(TablesGeneric::<$k>::create(
            seed, cache,
        ))
    }

    /// Create proofs.
    ///
    /// This is an optimized combination of `Self::create()` and `Self::find_proof()`.
    #[cfg(feature = "alloc")]
    pub fn create_proofs(seed: Seed, cache: &TablesCache) -> Box<Proofs<$k>> {
        TablesGeneric::<$k>::create_proofs(seed, cache)
    }

    /// Almost the same as [`Self::create()`], but uses parallelism internally for better
    /// performance and lower latency at the cost of lower CPU efficiency and higher memory usage
    #[cfg(feature = "parallel")]
    pub fn create_parallel(seed: Seed, cache: &TablesCache) -> Self {
        Self(TablesGeneric::<$k>::create_parallel(
            seed, cache,
        ))
    }

    /// Almost the same as [`Self::create_proofs()`], but uses parallelism internally for better
    /// performance and lower latency at the cost of lower CPU efficiency and higher memory usage
    #[cfg(feature = "alloc")]
    pub fn create_proofs_parallel(seed: Seed, cache: &TablesCache) -> Box<Proofs<$k>> {
        TablesGeneric::<$k>::create_proofs_parallel(seed, cache)
    }

    /// Find proof of space quality for a given challenge
    #[cfg(all(feature = "alloc", any(feature = "full-chiapos", test)))]
    pub fn find_quality<'a>(
        &'a self,
        challenge: &'a Challenge,
    ) -> impl Iterator<Item = Quality> + 'a {
        self.0.find_quality(challenge)
    }

    /// Similar to `Self::find_proof()`, but takes the first `k` challenge bits in the least
    /// significant bits of `u32` as a challenge instead
    #[cfg(feature = "alloc")]
    pub fn find_proof_raw<'a>(
        &'a self,
        first_k_challenge_bits: u32,
    ) -> impl Iterator<Item = [u8; 64 * $k / 8]> + 'a {
        self.0.find_proof_raw(first_k_challenge_bits)
    }

    /// Find proof of space for a given challenge
    #[cfg(all(feature = "alloc", any(feature = "full-chiapos", test)))]
    pub fn find_proof<'a>(
        &'a self,
        first_challenge_bytes: [u8; 4],
    ) -> impl Iterator<Item = [u8; 64 * $k / 8]> + 'a {
        self.0.find_proof(first_challenge_bytes)
    }

    /// Similar to `Self::verify()`, but takes the first `k` challenge bits in the least significant
    /// bits of `u32` as a challenge instead and doesn't compute quality
    pub fn verify_only_raw(
        seed: &Seed,
        first_k_challenge_bits: u32,
        proof_of_space: &[u8; 64 * $k as usize / 8],
    ) -> bool {
        TablesGeneric::<$k>::verify_only_raw(seed, first_k_challenge_bits, proof_of_space)
    }

    /// Verify proof of space for a given seed and challenge
    #[cfg(any(feature = "full-chiapos", test))]
    pub fn verify(
        seed: &Seed,
        challenge: &Challenge,
        proof_of_space: &[u8; 64 * $k as usize / 8],
    ) -> Option<Quality> {
        TablesGeneric::<$k>::verify(seed, challenge, proof_of_space)
    }
}
        )*
    }
}

// Only these k values are supported by the current implementation
#[cfg(feature = "full-chiapos")]
impl_any!(15, 16, 18, 19, 21, 22, 23, 24, 25);
#[cfg(any(feature = "full-chiapos", test))]
impl_any!(17);
impl_any!(20);
