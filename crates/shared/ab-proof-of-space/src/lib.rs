//! Proof of space implementation
#![no_std]
#![expect(incomplete_features, reason = "generic_const_exprs")]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![feature(
    array_windows,
    const_convert,
    const_trait_impl,
    exact_size_is_empty,
    float_erf,
    generic_const_exprs,
    get_mut_unchecked,
    iter_array_chunks,
    maybe_uninit_fill,
    maybe_uninit_slice,
    maybe_uninit_write_slice,
    new_zeroed_alloc,
    portable_simd,
    ptr_as_ref_unchecked,
    ptr_as_uninit,
    step_trait,
    sync_unsafe_cell,
    vec_into_raw_parts
)]

pub mod chia;
pub mod chiapos;
pub mod shim;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use ab_core_primitives::pieces::Record;
use ab_core_primitives::pos::{PosProof, PosSeed};
use ab_core_primitives::sectors::SBucket;
use ab_core_primitives::solutions::SolutionPotVerifier;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use core::fmt;

/// Proof of space table type
#[derive(Debug, Clone, Copy)]
pub enum PosTableType {
    /// Chia table
    Chia,
    /// Shim table
    Shim,
}

/// Proof-of-space proofs
#[derive(Debug)]
#[cfg(feature = "alloc")]
#[repr(C)]
pub struct PosProofs {
    /// S-buckets at which proofs were found.
    ///
    /// S-buckets are grouped by 8, within each `u8` bits right to left (LSB) indicate the presence
    /// of a proof for corresponding s-bucket, so that the whole array of bytes can be thought as a
    /// large set of bits.
    ///
    /// There will be at most [`Record::NUM_CHUNKS`] proofs produced/bits set to `1`.
    pub found_proofs: [u8; Record::NUM_S_BUCKETS / u8::BITS as usize],
    /// [`Record::NUM_CHUNKS`] proofs, corresponding to set bits of `found_proofs`.
    pub proofs: [PosProof; Record::NUM_CHUNKS],
}

// TODO: A method that returns hashed proofs (with SIMD) for all s-buckets for plotting
#[cfg(feature = "alloc")]
impl PosProofs {
    // TODO: Test for this method
    /// Get proof for specified s-bucket (if exists).
    ///
    /// Note that this is not the most efficient API possible, so prefer using the `proofs` field
    /// directly if the use case allows.
    #[inline]
    pub fn for_s_bucket(&self, s_bucket: SBucket) -> Option<PosProof> {
        let bits_offset = usize::from(s_bucket);
        let found_proofs_byte_offset = bits_offset / u8::BITS as usize;
        let found_proofs_bit_offset = bits_offset as u32 % u8::BITS;
        let (found_proofs_before, found_proofs_after) =
            self.found_proofs.split_at(found_proofs_byte_offset);
        if (found_proofs_after[0] & (1 << found_proofs_bit_offset)) == 0 {
            return None;
        }
        let proof_index = found_proofs_before
            .iter()
            .map(|&bits| bits.count_ones())
            .sum::<u32>()
            + found_proofs_after[0]
                .unbounded_shl(u8::BITS - found_proofs_bit_offset)
                .count_ones();

        Some(self.proofs[proof_index as usize])
    }
}

/// Stateful table generator with better performance.
///
/// Prefer cloning it over creating multiple separate generators.
#[cfg(feature = "alloc")]
pub trait TableGenerator<T: Table>:
    fmt::Debug + Default + Clone + Send + Sync + Sized + 'static
{
    /// Generate a new table with 32 bytes seed.
    ///
    /// There is also `Self::generate_parallel()` that can achieve higher performance and lower
    /// latency at the cost of lower CPU efficiency and higher memory usage.
    fn generate(&self, seed: &PosSeed) -> T;

    /// Create proofs with 32 bytes seed.
    ///
    /// There is also `Self::create_proofs_parallel()` that can achieve higher performance and
    /// lower latency at the cost of lower CPU efficiency and higher memory usage.
    fn create_proofs(&self, seed: &PosSeed) -> Box<PosProofs>;

    /// Almost the same as [`Self::generate()`], but uses parallelism internally for better
    /// performance and lower latency at the cost of lower CPU efficiency and higher memory usage
    #[cfg(feature = "parallel")]
    fn generate_parallel(&self, seed: &PosSeed) -> T {
        self.generate(seed)
    }

    /// Almost the same as [`Self::create_proofs()`], but uses parallelism internally for better
    /// performance and lower latency at the cost of lower CPU efficiency and higher memory usage
    #[cfg(feature = "parallel")]
    fn create_proofs_parallel(&self, seed: &PosSeed) -> Box<PosProofs> {
        self.create_proofs(seed)
    }
}

/// Proof of space kind
pub trait Table: SolutionPotVerifier + Sized + Send + Sync + 'static {
    /// Proof of space table type
    const TABLE_TYPE: PosTableType;
    /// Instance that can be used to generate tables with better performance
    #[cfg(feature = "alloc")]
    type Generator: TableGenerator<Self>;

    /// Try to find proof at `challenge_index` if it exists
    #[cfg(feature = "alloc")]
    fn find_proof(&self, challenge_index: u32) -> Option<PosProof>;

    /// Check whether proof created earlier is valid
    fn is_proof_valid(seed: &PosSeed, challenge_index: u32, proof: &PosProof) -> bool;

    /// Returns a stateful table generator with better performance
    #[cfg(feature = "alloc")]
    fn generator() -> Self::Generator {
        Self::Generator::default()
    }
}
