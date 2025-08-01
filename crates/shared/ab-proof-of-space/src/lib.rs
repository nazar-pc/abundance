//! Proof of space implementation
#![cfg_attr(not(feature = "std"), no_std)]
#![expect(incomplete_features, reason = "generic_const_exprs")]
#![warn(rust_2018_idioms, missing_debug_implementations, missing_docs)]
#![feature(array_windows, generic_const_exprs, portable_simd, step_trait)]

pub mod chia;
pub mod chiapos;
pub mod shim;

use ab_core_primitives::pos::{PosProof, PosSeed};
use ab_core_primitives::solutions::SolutionPotVerifier;
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

/// Stateful table generator with better performance
#[cfg(feature = "alloc")]
pub trait TableGenerator<T: Table>: fmt::Debug + Default + Clone + Send + Sized + 'static {
    /// Generate new table with 32 bytes seed.
    ///
    /// There is also [`Self::generate_parallel()`] that can achieve lower latency.
    fn generate(&mut self, seed: &PosSeed) -> T;

    /// Generate new table with 32 bytes seed using parallelism.
    ///
    /// This implementation will trade efficiency of CPU and memory usage for lower latency, prefer
    /// [`Self::generate()`] unless lower latency is critical.
    #[cfg(any(feature = "parallel", test))]
    fn generate_parallel(&mut self, seed: &PosSeed) -> T {
        self.generate(seed)
    }
}

/// Proof of space kind
pub trait Table: SolutionPotVerifier + Sized + Send + Sync + 'static {
    /// Proof of space table type
    const TABLE_TYPE: PosTableType;
    /// Instance that can be used to generate tables with better performance
    #[cfg(feature = "alloc")]
    type Generator: TableGenerator<Self>;

    /// Generate new table with 32 bytes seed.
    ///
    /// There is also [`Self::generate_parallel()`] that can achieve lower latency.
    #[cfg(feature = "alloc")]
    fn generate(seed: &PosSeed) -> Self;

    /// Generate new table with 32 bytes seed using parallelism.
    ///
    /// This implementation will trade efficiency of CPU and memory usage for lower latency, prefer
    /// [`Self::generate()`] unless lower latency is critical.
    #[cfg(all(feature = "alloc", any(feature = "parallel", test)))]
    fn generate_parallel(seed: &PosSeed) -> Self {
        Self::generate(seed)
    }

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
